//! Interactive planar polygon sketching for generic workshop components.
//! Sketches are persisted separately from the parametric part assembly and are
//! passed to the scientific mesher as validated polygon regions.

use crate::campus::{CampusState, CampusView};
use crate::sequence::SequenceWorkspace;
use crate::ui_kit::UiInteraction;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use omega_sim_core::{PlanarPolygonRegion, mesh_planar_polygon};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const SKETCH_SCHEMA: u32 = 1;
const SKETCH_PLANE_Y: f32 = 0.565;
const SKETCH_ANCHOR_Z: f64 = 18.0;
const SNAP_M: f64 = 0.05;
const VERTEX_PICK_RADIUS_M: f64 = 0.16;
const VERTEX_SNAP_RADIUS_M: f64 = 0.12;
const MAX_REGIONS: usize = 64;
const MAX_RING_POINTS: usize = 2_048;
const MAX_TOTAL_SKETCH_VERTICES: usize = 8_192;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PolygonSketchDocument {
    pub schema_version: u32,
    pub regions: Vec<PlanarPolygonRegion>,
}

impl Default for PolygonSketchDocument {
    fn default() -> Self {
        Self {
            schema_version: SKETCH_SCHEMA,
            regions: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VertexHandle {
    region_index: usize,
    ring_index: usize,
    vertex_index: usize,
}

#[derive(Clone, Debug)]
struct VertexDrag {
    handle: VertexHandle,
    /// Every region vertex at the same shared coordinate. Moving them as a
    /// group preserves welded material interfaces between adjacent regions.
    original_points: Vec<(VertexHandle, [f64; 2])>,
}

#[derive(Resource)]
pub struct PolygonSketchSession {
    pub document: PolygonSketchDocument,
    pub active: bool,
    pub edit_vertices: bool,
    pub snap_enabled: bool,
    pub pending_points: Vec<[f64; 2]>,
    pub drawing_hole: bool,
    pub new_region_mode: bool,
    pub selected_region: usize,
    selected_vertex: Option<VertexHandle>,
    dragging_vertex: Option<VertexDrag>,
    pub current_material_id: u16,
    pub current_thickness_m: f64,
    pub revision: u64,
    pub message: String,
}

impl Default for PolygonSketchSession {
    fn default() -> Self {
        Self {
            document: PolygonSketchDocument::default(),
            active: false,
            edit_vertices: false,
            snap_enabled: true,
            pending_points: Vec::new(),
            selected_vertex: None,
            dragging_vertex: None,
            drawing_hole: false,
            new_region_mode: true,
            selected_region: 0,
            current_material_id: 0,
            current_thickness_m: 0.01,
            revision: 1,
            message: "Press V in the Workshop to draw a polygon region".into(),
        }
    }
}

impl PolygonSketchSession {
    pub fn add_point(&mut self, point: [f64; 2]) -> Result<(), String> {
        self.add_point_with_snap(point, false)
    }

    pub fn add_point_with_snap(&mut self, point: [f64; 2], bypass_snap: bool) -> Result<(), String> {
        if !point.iter().all(|value| value.is_finite()) {
            return Err("Point coordinates must be finite".into());
        }
        if point[0].abs() > 4.5 || point[1].abs() > 3.5 {
            return Err("Point lies outside the workshop sketch pad (X +/-4.5 m, Z +/-3.5 m)".into());
        }
        if self.pending_points.len() >= MAX_RING_POINTS {
            return Err(format!("A contour is limited to {MAX_RING_POINTS} vertices"));
        }
        let snapped = self.snap_point(point, None, bypass_snap);
        if snapped[0].abs() > 4.5 || snapped[1].abs() > 3.5 {
            return Err("Point lies outside the workshop sketch pad (X +/-4.5 m, Z +/-3.5 m)".into());
        }
        if self
            .pending_points
            .last()
            .is_some_and(|previous| distance(*previous, snapped) < SNAP_M * 0.25)
        {
            return Err("Point is too close to the previous vertex".into());
        }
        self.pending_points.push(snapped);
        self.revision = self.revision.wrapping_add(1);
        self.message = format!("{} point(s) in current contour", self.pending_points.len());
        Ok(())
    }

    pub fn toggle_vertex_editing(&mut self) {
        if !self.pending_points.is_empty() {
            self.message = "Finish or remove the pending contour before editing existing vertices".into();
            return;
        }
        self.cancel_vertex_drag();
        self.edit_vertices = !self.edit_vertices;
        self.drawing_hole = false;
        self.selected_vertex = None;
        self.revision = self.revision.wrapping_add(1);
        self.message = if self.edit_vertices {
            "Vertex editing enabled: click a vertex and drag it".into()
        } else {
            "Vertex editing disabled: click to draw a contour".into()
        };
    }

    pub fn toggle_snapping(&mut self) {
        self.snap_enabled = !self.snap_enabled;
        self.revision = self.revision.wrapping_add(1);
        self.message = if self.snap_enabled {
            format!(
                "Snapping enabled: {:.0} mm grid, {:.0} mm vertex capture",
                SNAP_M * 1_000.0,
                VERTEX_SNAP_RADIUS_M * 1_000.0
            )
        } else {
            "Snapping disabled; hold no modifier while dragging for free movement".into()
        };
    }

    pub fn begin_vertex_drag(&mut self, point: [f64; 2]) -> bool {
        if !self.edit_vertices || !self.pending_points.is_empty() || self.drawing_hole {
            return false;
        }
        let Some(handle) = nearest_vertex(&self.document, point, VERTEX_PICK_RADIUS_M) else {
            self.selected_vertex = None;
            self.message = "No vertex nearby. Move closer to a visible vertex.".into();
            self.revision = self.revision.wrapping_add(1);
            return false;
        };
        let Some(original) = vertex_at(&self.document, handle) else {
            return false;
        };
        self.selected_region = handle.region_index;
        self.new_region_mode = false;
        self.selected_vertex = Some(handle);
        let linked = coincident_vertices(&self.document, original, 1.0e-8);
        self.dragging_vertex = Some(VertexDrag {
            handle,
            original_points: linked,
        });
        self.current_material_id = self.document.regions[handle.region_index].material_id;
        self.current_thickness_m = self.document.regions[handle.region_index].thickness_m;
        self.revision = self.revision.wrapping_add(1);
        self.message = format!(
            "Dragging vertex {} of {}",
            handle.vertex_index + 1,
            self.document.regions[handle.region_index].source_part_id
        );
        true
    }

    /// Updates the in-memory geometry during a drag. Persistence and polygon
    /// validation happen on mouse release, so the UI does not write a file per frame.
    pub fn update_vertex_drag(&mut self, point: [f64; 2], bypass_snap: bool) {
        let Some(drag) = self.dragging_vertex.as_ref() else {
            return;
        };
        if !point.iter().all(|value| value.is_finite()) {
            return;
        }
        let excluded: Vec<VertexHandle> = drag.original_points.iter().map(|(handle, _)| *handle).collect();
        let snapped = self.snap_point_excluding(point, &excluded, bypass_snap);
        if snapped[0].abs() > 4.5 || snapped[1].abs() > 3.5 {
            return;
        }
        for handle in excluded {
            set_vertex(&mut self.document, handle, snapped);
        }
    }

    pub fn finish_vertex_drag(&mut self) {
        let Some(drag) = self.dragging_vertex.take() else {
            return;
        };
        let Some(region) = self.document.regions.get(drag.handle.region_index).cloned() else {
            return;
        };
        let changed = drag
            .original_points
            .iter()
            .any(|(handle, original)| vertex_at(&self.document, *handle).is_some_and(|point| point != *original));
        if !changed {
            self.message = "Vertex position unchanged".into();
            return;
        }
        let affected_regions: HashSet<usize> = drag
            .original_points
            .iter()
            .map(|(handle, _)| handle.region_index)
            .collect();
        for region_index in affected_regions {
            let Some(candidate) = self.document.regions.get(region_index) else {
                continue;
            };
            if let Err(error) = mesh_planar_polygon(candidate) {
                restore_vertices(&mut self.document, &drag.original_points);
                self.revision = self.revision.wrapping_add(1);
                self.message = format!("Vertex move rejected; original geometry restored: {error}");
                return;
            }
        }
        if let Err(error) = self.persist() {
            restore_vertices(&mut self.document, &drag.original_points);
            self.revision = self.revision.wrapping_add(1);
            self.message = format!("Vertex move was not saved; original geometry restored: {error}");
            return;
        }
        self.revision = self.revision.wrapping_add(1);
        self.message = format!(
            "Moved vertex {} in {}",
            drag.handle.vertex_index + 1,
            region.source_part_id
        );
    }

    pub fn cancel_vertex_drag(&mut self) {
        let Some(drag) = self.dragging_vertex.take() else {
            return;
        };
        restore_vertices(&mut self.document, &drag.original_points);
        self.revision = self.revision.wrapping_add(1);
        self.message = "Vertex movement cancelled".into();
    }

    pub fn delete_selected_vertex(&mut self) {
        let Some(handle) = self.selected_vertex else {
            self.message = "Select a vertex before deleting it".into();
            return;
        };
        let Some(original_region) = self.document.regions.get(handle.region_index).cloned() else {
            return;
        };
        let Some(original_point) = vertex_at(&self.document, handle) else {
            return;
        };
        if coincident_vertices(&self.document, original_point, 1.0e-8).len() > 1 {
            self.message = "This vertex is shared by adjacent material regions; move it instead of deleting it".into();
            return;
        }
        let Some(region) = self.document.regions.get_mut(handle.region_index) else {
            return;
        };
        let Some(ring) = ring_mut(region, handle.ring_index) else {
            return;
        };
        if ring.len() <= 3 {
            self.message = "A polygon ring must retain at least three vertices".into();
            return;
        }
        ring.remove(handle.vertex_index);
        let candidate = self.document.regions[handle.region_index].clone();
        if let Err(error) = mesh_planar_polygon(&candidate) {
            self.document.regions[handle.region_index] = original_region;
            self.message = format!("Vertex deletion rejected: {error}");
            return;
        }
        if let Err(error) = self.persist() {
            self.document.regions[handle.region_index] = original_region;
            self.message = format!("Vertex deletion was not saved: {error}");
            return;
        }
        self.selected_vertex = None;
        self.revision = self.revision.wrapping_add(1);
        self.message = "Selected vertex deleted".into();
    }

    fn snap_point(&self, point: [f64; 2], excluded: Option<VertexHandle>, bypass_snap: bool) -> [f64; 2] {
        let excluded_handles = excluded.into_iter().collect::<Vec<_>>();
        self.snap_point_excluding(point, &excluded_handles, bypass_snap)
    }

    fn snap_point_excluding(&self, point: [f64; 2], excluded: &[VertexHandle], bypass_snap: bool) -> [f64; 2] {
        let mut result = [point[0].clamp(-4.5, 4.5), point[1].clamp(-3.5, 3.5)];
        if !self.snap_enabled || bypass_snap {
            return result;
        }
        // Divide by the reciprocal grid count instead of multiplying by SNAP_M: for a grid of
        // 0.05 m, -3 / 20 is exactly the nearest double to -0.15 while -3 * 0.05 is not.
        let cells_per_metre = (1.0 / SNAP_M).round();
        result = [
            (result[0] / SNAP_M).round() / cells_per_metre,
            (result[1] / SNAP_M).round() / cells_per_metre,
        ];
        if let Some(handle) = nearest_vertex_excluding_many(&self.document, point, VERTEX_SNAP_RADIUS_M, excluded)
            && let Some(target) = vertex_at(&self.document, handle)
        {
            result = target;
        }
        [result[0].clamp(-4.5, 4.5), result[1].clamp(-3.5, 3.5)]
    }

    pub fn undo_pending_point(&mut self) {
        if self.pending_points.pop().is_some() {
            self.revision = self.revision.wrapping_add(1);
            self.message = format!("Removed last point; {} remain", self.pending_points.len());
        }
    }

    pub fn finish_ring(&mut self) -> Result<(), String> {
        if self.pending_points.len() < 3 {
            return Err("A contour needs at least three points".into());
        }
        let added_vertices = self.pending_points.len();
        if total_document_vertices(&self.document).saturating_add(added_vertices) > MAX_TOTAL_SKETCH_VERTICES {
            return Err(format!(
                "A sketch is limited to {MAX_TOTAL_SKETCH_VERTICES} total vertices"
            ));
        }

        if self.drawing_hole {
            let (hole_count, region_id) = {
                let Some(region) = self.document.regions.get_mut(self.selected_region) else {
                    return Err("Select an existing region before drawing a hole".into());
                };
                region.holes_m.push(self.pending_points.clone());
                if let Err(error) = mesh_planar_polygon(region) {
                    region.holes_m.pop();
                    return Err(error.to_string());
                }
                (region.holes_m.len(), region.source_part_id.clone())
            };
            if let Err(error) = self.persist() {
                if let Some(region) = self.document.regions.get_mut(self.selected_region) {
                    region.holes_m.pop();
                }
                return Err(error);
            }
            self.pending_points.clear();
            self.revision = self.revision.wrapping_add(1);
            self.message = format!("Hole {hole_count} added to {region_id}");
            Ok(())
        } else {
            if self.document.regions.len() >= MAX_REGIONS {
                return Err(format!("A sketch is limited to {MAX_REGIONS} regions"));
            }
            let next_id = self
                .document
                .regions
                .iter()
                .filter_map(|region| {
                    region
                        .source_part_id
                        .strip_prefix("sketch-region-")?
                        .parse::<u64>()
                        .ok()
                })
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            let region = PlanarPolygonRegion {
                source_part_id: format!("sketch-region-{next_id:03}"),
                outer_boundary_m: self.pending_points.clone(),
                holes_m: Vec::new(),
                material_id: self.current_material_id.min(3),
                thickness_m: self.current_thickness_m,
            };
            mesh_planar_polygon(&region).map_err(|error| error.to_string())?;
            self.document.regions.push(region);
            if let Err(error) = self.persist() {
                self.document.regions.pop();
                return Err(error);
            }
            self.selected_region = self.document.regions.len() - 1;
            self.new_region_mode = false;
            self.current_material_id = self.document.regions[self.selected_region].material_id;
            self.pending_points.clear();
            self.revision = self.revision.wrapping_add(1);
            self.message = format!("Region sketch-region-{next_id:03} created");
            Ok(())
        }
    }

    pub fn select_next_region(&mut self) {
        if !self.pending_points.is_empty() {
            self.message = "Finish or remove the pending contour before changing the selected region".into();
            return;
        }
        if !self.document.regions.is_empty() {
            self.selected_region = (self.selected_region + 1) % self.document.regions.len();
            self.new_region_mode = false;
            let region = &self.document.regions[self.selected_region];
            self.current_material_id = region.material_id;
            self.current_thickness_m = region.thickness_m;
            self.revision = self.revision.wrapping_add(1);
            self.message = format!("Selected {}", region.source_part_id);
        }
    }

    pub fn cycle_material(&mut self) {
        if self.drawing_hole {
            self.message =
                "Hole contours inherit their parent region material; exit hole mode before changing it".into();
            return;
        }
        let previous = self.current_material_id;
        let next = (previous + 1) % 4;
        self.current_material_id = next;
        let selected = !self.new_region_mode && self.document.regions.get(self.selected_region).is_some();
        if selected {
            self.document.regions[self.selected_region].material_id = next;
            if let Err(error) = self.persist() {
                self.document.regions[self.selected_region].material_id = previous;
                self.current_material_id = previous;
                self.message = format!("Material change was not saved: {error}");
                return;
            }
        }
        self.revision = self.revision.wrapping_add(1);
        self.message = format!("Current material: {}", material_name(self.current_material_id));
    }

    pub fn adjust_thickness(&mut self, delta_m: f64) {
        if self.drawing_hole {
            self.message =
                "Hole contours inherit their parent region thickness; exit hole mode before changing it".into();
            return;
        }
        let previous = self.current_thickness_m;
        let next = (previous + delta_m).clamp(0.001, 0.25);
        self.current_thickness_m = next;
        let selected = !self.new_region_mode && self.document.regions.get(self.selected_region).is_some();
        if selected {
            self.document.regions[self.selected_region].thickness_m = next;
            if let Err(error) = self.persist() {
                self.document.regions[self.selected_region].thickness_m = previous;
                self.current_thickness_m = previous;
                self.message = format!("Thickness change was not saved: {error}");
                return;
            }
        }
        self.revision = self.revision.wrapping_add(1);
        self.message = format!("Current region thickness: {:.1} mm", next * 1_000.0);
    }

    pub fn delete_selected_region(&mut self) {
        if self.selected_region < self.document.regions.len() {
            let removed_index = self.selected_region;
            let removed = self.document.regions.remove(removed_index);
            let previous_selection = self.selected_region;
            self.selected_region = self.selected_region.min(self.document.regions.len().saturating_sub(1));
            if let Some(region) = self.document.regions.get(self.selected_region) {
                self.current_material_id = region.material_id;
                self.current_thickness_m = region.thickness_m;
            }
            match self.persist() {
                Ok(()) => {
                    self.pending_points.clear();
                    self.drawing_hole = false;
                    self.selected_vertex = None;
                    self.dragging_vertex = None;
                    if self.document.regions.is_empty() {
                        self.new_region_mode = true;
                    }
                    self.revision = self.revision.wrapping_add(1);
                    self.message = format!("Deleted {}", removed.source_part_id);
                }
                Err(error) => {
                    self.document.regions.insert(removed_index, removed);
                    self.selected_region = previous_selection;
                    if let Some(region) = self.document.regions.get(self.selected_region) {
                        self.current_material_id = region.material_id;
                        self.current_thickness_m = region.thickness_m;
                    }
                    self.message = format!("Could not save deletion: {error}");
                }
            }
        }
    }

    pub fn persist(&self) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(&self.document).map_err(|error| error.to_string())?;
        crate::save::write(crate::save::SaveFile::PolygonSketch, &bytes)
            .map_err(|error| format!("Could not save polygon sketch: {error}"))
    }
}

#[derive(Component)]
pub(crate) struct PolygonSketchVisual;

#[derive(Component)]
pub(crate) struct PolygonSketchHud;

#[derive(Component, Clone, Copy)]
pub(crate) struct PolygonSketchEdgeHandle {
    start: VertexHandle,
    end: VertexHandle,
}

#[derive(Component, Clone, Copy)]
pub(crate) struct PolygonSketchVertexHandle(VertexHandle);

#[derive(Resource, Default)]
pub(crate) struct PolygonSketchRenderState {
    revision: u64,
    visible: bool,
}

#[derive(Resource)]
pub(crate) struct PolygonSketchAssets {
    cube: Handle<Mesh>,
    materials: Vec<Handle<StandardMaterial>>,
}

pub(crate) fn initialize_polygon_sketch(mut session: ResMut<PolygonSketchSession>) {
    match crate::save::read(crate::save::SaveFile::PolygonSketch) {
        Ok(Some(bytes)) => match serde_json::from_slice::<PolygonSketchDocument>(&bytes) {
            Ok(document) if document.schema_version == SKETCH_SCHEMA => {
                let mut valid = document.regions.len() <= MAX_REGIONS
                    && total_document_vertices(&document) <= MAX_TOTAL_SKETCH_VERTICES;
                let mut identifiers = HashSet::new();
                for region in &document.regions {
                    if !identifiers.insert(region.source_part_id.as_str())
                        || region.material_id >= 4
                        || region.thickness_m > 0.25
                        || !region_fits_sketch_pad(region)
                    {
                        warn!(
                            "Ignoring polygon sketch with duplicate IDs or unsupported material, thickness or pad coordinates"
                        );
                        valid = false;
                    }
                    if let Err(error) = mesh_planar_polygon(region) {
                        warn!(
                            "Ignoring invalid persisted polygon region {}: {}",
                            region.source_part_id, error
                        );
                        valid = false;
                    }
                }
                if valid {
                    session.selected_region = document.regions.len().saturating_sub(1);
                    session.new_region_mode = document.regions.is_empty();
                    if let Some(region) = document.regions.last() {
                        session.current_material_id = region.material_id;
                        session.current_thickness_m = region.thickness_m;
                    }
                    session.document = document;
                    session.message = format!("Loaded {} saved polygon region(s)", session.document.regions.len());
                }
            }
            Ok(document) => warn!("Unsupported polygon sketch schema {}", document.schema_version),
            Err(error) => warn!("Could not parse saved polygon sketch: {error}"),
        },
        Ok(None) => {}
        Err(error) => warn!("Could not read saved polygon sketch: {error}"),
    }
}

pub(crate) fn setup_polygon_sketch_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("POLYGON SKETCH | Press V to start"),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::srgb(0.96, 0.96, 0.92)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(78.0),
            right: Val::Px(14.0),
            max_width: Val::Px(430.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        UiInteraction::None,
        BackgroundColor(Color::srgba(0.025, 0.04, 0.055, 0.88)),
        PolygonSketchHud,
    ));
}

pub(crate) fn initialize_polygon_sketch_visuals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(PolygonSketchRenderState::default());
    commands.insert_resource(PolygonSketchAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        materials: vec![
            materials.add(Color::srgb(0.25, 0.68, 0.83)),
            materials.add(Color::srgb(0.64, 0.67, 0.71)),
            materials.add(Color::srgb(0.28, 0.78, 0.50)),
            materials.add(Color::srgb(0.64, 0.48, 0.86)),
            materials.add(Color::srgb(1.0, 0.28, 0.24)),
            materials.add(Color::srgb(1.0, 0.78, 0.18)),
        ],
    });
}

pub(crate) fn handle_polygon_sketch_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    campus: Res<CampusState>,
    sequence: Res<SequenceWorkspace>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut session: ResMut<PolygonSketchSession>,
    ui_buttons: Query<&UiInteraction>,
) {
    if campus.view != CampusView::Construction || campus.profile_editor_open || sequence.graph_is_open() {
        session.cancel_vertex_drag();
        return;
    }
    if keys.just_pressed(KeyCode::KeyV) {
        if session.dragging_vertex.is_some() {
            session.finish_vertex_drag();
        }
        session.active = !session.active;
        session.revision = session.revision.wrapping_add(1);
        session.message = if session.active {
            "Sketch mode active: E edits vertices, otherwise click to draw".into()
        } else {
            "Sketch mode paused".into()
        };
    }
    if !session.active {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        session.cancel_vertex_drag();
        session.active = false;
        session.edit_vertices = false;
        session.pending_points.clear();
        session.drawing_hole = false;
        session.revision = session.revision.wrapping_add(1);
        session.message = "Sketch mode cancelled; completed regions remain saved".into();
        return;
    }

    if keys.just_pressed(KeyCode::KeyE) {
        session.toggle_vertex_editing();
    }
    if keys.just_pressed(KeyCode::KeyG) {
        session.toggle_snapping();
    }
    if keys.just_pressed(KeyCode::Enter) {
        if session.edit_vertices {
            session.message = "Press E to leave vertex editing before closing a contour".into();
        } else if let Err(error) = session.finish_ring() {
            session.message = error;
        }
    }
    if keys.just_pressed(KeyCode::Backspace) && !session.edit_vertices {
        session.undo_pending_point();
    }
    if keys.just_pressed(KeyCode::KeyH) {
        if session.edit_vertices {
            session.message = "Press E to leave vertex editing before drawing a hole".into();
        } else if !session.pending_points.is_empty() {
            session.message =
                "Finish the current contour or remove its pending points before changing hole mode".into();
        } else if session.document.regions.is_empty() {
            session.message = "Create an outer region before drawing a hole".into();
        } else {
            session.drawing_hole = !session.drawing_hole;
            if session.drawing_hole {
                session.new_region_mode = false;
                // Copy values out before mutating the ResMut resource. This avoids
                // holding an immutable deref-coercion borrow across assignments.
                let selected_properties = session
                    .document
                    .regions
                    .get(session.selected_region)
                    .map(|region| (region.material_id, region.thickness_m));
                if let Some((material_id, thickness_m)) = selected_properties {
                    session.current_material_id = material_id;
                    session.current_thickness_m = thickness_m;
                }
            }
            session.pending_points.clear();
            session.message = if session.drawing_hole {
                "Hole mode: draw a contour inside the selected region".into()
            } else {
                "Outer contour mode".into()
            };
            session.revision = session.revision.wrapping_add(1);
        }
    }
    if keys.just_pressed(KeyCode::KeyN) {
        session.cancel_vertex_drag();
        session.edit_vertices = false;
        session.drawing_hole = false;
        session.new_region_mode = true;
        session.pending_points.clear();
        session.selected_vertex = None;
        session.message =
            "New outer region: choose material/thickness, then click at least three vertices and press Enter".into();
        session.revision = session.revision.wrapping_add(1);
    }
    if keys.just_pressed(KeyCode::KeyF) {
        session.finish_vertex_drag();
        session.active = false;
        session.edit_vertices = false;
        session.pending_points.clear();
        session.drawing_hole = false;
        session.revision = session.revision.wrapping_add(1);
        session.message = "Sketch editing finished; regions are saved".into();
    }
    if keys.just_pressed(KeyCode::Tab) && !session.edit_vertices {
        session.select_next_region();
    }
    if keys.just_pressed(KeyCode::KeyM) && !session.edit_vertices {
        session.cycle_material();
    }
    if keys.just_pressed(KeyCode::BracketLeft) && !session.edit_vertices {
        session.adjust_thickness(-0.001);
    }
    if keys.just_pressed(KeyCode::BracketRight) && !session.edit_vertices {
        session.adjust_thickness(0.001);
    }
    if keys.just_pressed(KeyCode::Delete) {
        if session.edit_vertices {
            session.delete_selected_vertex();
        } else {
            session.delete_selected_region();
        }
    }

    let (camera_ref, camera_transform_ref) = *camera;
    let cursor_point = window.cursor_position().and_then(|cursor| {
        // The top-right sketch HUD is excluded from sketch editing.
        if (cursor.x < 470.0 && cursor.y < 520.0) || (cursor.x > window.width() - 510.0 && cursor.y < 230.0) {
            return None;
        }
        sketch_point_from_cursor(camera_ref, camera_transform_ref, cursor)
    });

    if mouse.just_pressed(MouseButton::Left) {
        if ui_buttons.iter().any(|interaction| *interaction != UiInteraction::None) {
            return;
        }
        let Some(point) = cursor_point else { return };
        if session.edit_vertices {
            session.begin_vertex_drag(point);
        } else {
            let bypass_snap = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
            let result = if bypass_snap {
                session.add_point_with_snap(point, true)
            } else {
                session.add_point(point)
            };
            if let Err(error) = result {
                session.message = error;
            }
        }
    }
    if mouse.pressed(MouseButton::Left)
        && session.dragging_vertex.is_some()
        && let Some(point) = cursor_point
    {
        let bypass_snap = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        session.update_vertex_drag(point, bypass_snap);
    }
    if mouse.just_released(MouseButton::Left) {
        session.finish_vertex_drag();
    }
}

fn sketch_point_from_cursor(camera: &Camera, camera_transform: &GlobalTransform, cursor: Vec2) -> Option<[f64; 2]> {
    let ray = camera.viewport_to_world(camera_transform, cursor).ok()?;
    let direction = ray.direction.as_vec3();
    if direction.y.abs() < 1.0e-6 {
        return None;
    }
    let distance = (SKETCH_PLANE_Y - ray.origin.y) / direction.y;
    if distance <= 0.0 {
        return None;
    }
    let hit = ray.origin + direction * distance;
    Some([f64::from(hit.x), f64::from(hit.z) - SKETCH_ANCHOR_Z])
}

pub(crate) fn sync_polygon_sketch_visuals(
    mut commands: Commands,
    session: Res<PolygonSketchSession>,
    campus: Res<CampusState>,
    mut render_state: ResMut<PolygonSketchRenderState>,
    old_visuals: Query<Entity, With<PolygonSketchVisual>>,
    assets: Res<PolygonSketchAssets>,
) {
    let visible = campus.view == CampusView::Construction;
    if render_state.revision == session.revision && render_state.visible == visible {
        return;
    }
    for entity in &old_visuals {
        commands.entity(entity).despawn();
    }
    if visible {
        for (region_index, region) in session.document.regions.iter().enumerate() {
            let material = assets.materials[usize::from(region.material_id.min(3))].clone();
            spawn_ring(
                &mut commands,
                &assets.cube,
                &material,
                &region.outer_boundary_m,
                true,
                Some((region_index, 0)),
            );
            let hole_material = assets.materials[4].clone();
            for (hole_index, hole) in region.holes_m.iter().enumerate() {
                spawn_ring(
                    &mut commands,
                    &assets.cube,
                    &hole_material,
                    hole,
                    true,
                    Some((region_index, hole_index + 1)),
                );
            }
            if session.edit_vertices {
                for (vertex_index, point) in region.outer_boundary_m.iter().copied().enumerate() {
                    spawn_document_vertex(
                        &mut commands,
                        &assets.cube,
                        &assets.materials,
                        point,
                        VertexHandle {
                            region_index,
                            ring_index: 0,
                            vertex_index,
                        },
                        session.selected_vertex,
                    );
                }
                for (hole_index, hole) in region.holes_m.iter().enumerate() {
                    for (vertex_index, point) in hole.iter().copied().enumerate() {
                        spawn_document_vertex(
                            &mut commands,
                            &assets.cube,
                            &assets.materials,
                            point,
                            VertexHandle {
                                region_index,
                                ring_index: hole_index + 1,
                                vertex_index,
                            },
                            session.selected_vertex,
                        );
                    }
                }
            }
        }
        if !session.pending_points.is_empty() {
            let material = assets.materials[5].clone();
            spawn_ring(
                &mut commands,
                &assets.cube,
                &material,
                &session.pending_points,
                false,
                None,
            );
            for point in &session.pending_points {
                spawn_marker(&mut commands, &assets.cube, &material, *point);
            }
        }
    }
    render_state.revision = session.revision;
    render_state.visible = visible;
}

/// Updates only the currently dragged node and its adjacent edges. The full
/// sketch is rebuilt only when an edit is committed or the editing mode changes.
pub(crate) fn update_polygon_sketch_drag_visuals(
    session: Res<PolygonSketchSession>,
    mut edges: Query<(&PolygonSketchEdgeHandle, &mut Transform), Without<PolygonSketchVertexHandle>>,
    mut vertices: Query<(&PolygonSketchVertexHandle, &mut Transform), Without<PolygonSketchEdgeHandle>>,
) {
    let Some(drag) = session.dragging_vertex.as_ref() else {
        return;
    };
    let dragged_handles: Vec<VertexHandle> = drag.original_points.iter().map(|(handle, _)| *handle).collect();
    for (edge, mut transform) in &mut edges {
        if dragged_handles.contains(&edge.start) || dragged_handles.contains(&edge.end) {
            let Some(start) = vertex_at(&session.document, edge.start) else {
                continue;
            };
            let Some(end) = vertex_at(&session.document, edge.end) else {
                continue;
            };
            set_edge_transform(&mut transform, start, end);
        }
    }
    for (marker, mut transform) in &mut vertices {
        if dragged_handles.contains(&marker.0)
            && let Some(point) = vertex_at(&session.document, marker.0)
        {
            transform.translation = sketch_world_position(point, SKETCH_PLANE_Y + 0.025);
        }
    }
}

fn spawn_ring(
    commands: &mut Commands,
    cube: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
    points: &[[f64; 2]],
    closed: bool,
    context: Option<(usize, usize)>,
) {
    if points.len() < 2 {
        return;
    }
    let edge_count = if closed { points.len() } else { points.len() - 1 };
    for index in 0..edge_count {
        let next = (index + 1) % points.len();
        let edge_handle = context.map(|(region_index, ring_index)| PolygonSketchEdgeHandle {
            start: VertexHandle {
                region_index,
                ring_index,
                vertex_index: index,
            },
            end: VertexHandle {
                region_index,
                ring_index,
                vertex_index: next,
            },
        });
        spawn_edge(commands, cube, material, points[index], points[next], edge_handle);
    }
}

fn spawn_edge(
    commands: &mut Commands,
    cube: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
    a: [f64; 2],
    b: [f64; 2],
    handle: Option<PolygonSketchEdgeHandle>,
) {
    let start = sketch_world_position(a, SKETCH_PLANE_Y);
    let end = sketch_world_position(b, SKETCH_PLANE_Y);
    let delta = end - start;
    let length = delta.length();
    if length <= 1.0e-4 {
        return;
    }
    let bundle = (
        Mesh3d(cube.clone()),
        MeshMaterial3d(material.clone()),
        Transform::from_translation((start + end) * 0.5)
            .with_rotation(Quat::from_rotation_arc(Vec3::X, delta / length))
            .with_scale(Vec3::new(length, 0.025, 0.025)),
        PolygonSketchVisual,
    );
    if let Some(handle) = handle {
        commands.spawn(bundle).insert(handle);
    } else {
        commands.spawn(bundle);
    }
}

fn spawn_document_vertex(
    commands: &mut Commands,
    cube: &Handle<Mesh>,
    materials: &[Handle<StandardMaterial>],
    point: [f64; 2],
    handle: VertexHandle,
    selected: Option<VertexHandle>,
) {
    let is_selected = selected == Some(handle);
    let material = if is_selected { &materials[4] } else { &materials[5] };
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(material.clone()),
        Transform::from_translation(sketch_world_position(point, SKETCH_PLANE_Y + 0.025))
            .with_scale(Vec3::splat(if is_selected { 0.14 } else { 0.085 })),
        PolygonSketchVisual,
        PolygonSketchVertexHandle(handle),
    ));
}

fn sketch_world_position(point: [f64; 2], y: f32) -> Vec3 {
    Vec3::new(point[0] as f32, y, point[1] as f32 + SKETCH_ANCHOR_Z as f32)
}

fn set_edge_transform(transform: &mut Transform, a: [f64; 2], b: [f64; 2]) {
    let start = sketch_world_position(a, SKETCH_PLANE_Y);
    let end = sketch_world_position(b, SKETCH_PLANE_Y);
    let delta = end - start;
    let length = delta.length();
    if length > 1.0e-4 {
        transform.translation = (start + end) * 0.5;
        transform.rotation = Quat::from_rotation_arc(Vec3::X, delta / length);
        transform.scale = Vec3::new(length, 0.025, 0.025);
    }
}

fn spawn_marker(commands: &mut Commands, cube: &Handle<Mesh>, material: &Handle<StandardMaterial>, point: [f64; 2]) {
    commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(material.clone()),
        Transform::from_translation(Vec3::new(
            point[0] as f32,
            SKETCH_PLANE_Y + 0.025,
            point[1] as f32 + SKETCH_ANCHOR_Z as f32,
        ))
        .with_scale(Vec3::splat(0.09)),
        PolygonSketchVisual,
    ));
}

pub(crate) fn update_polygon_sketch_hud(
    session: Res<PolygonSketchSession>,
    campus: Res<CampusState>,
    mut hud: Query<(&mut Text, &mut Node), With<PolygonSketchHud>>,
) {
    let Ok((mut text, mut node)) = hud.single_mut() else {
        return;
    };
    node.display = if campus.view == CampusView::Construction && session.active {
        Display::Flex
    } else {
        Display::None
    };
    let selected = session.document.regions.get(session.selected_region);
    let selected_label = selected
        .map(|region| {
            format!(
                "{} | material {} | {:.1} mm",
                region.source_part_id,
                material_name(region.material_id),
                region.thickness_m * 1_000.0
            )
        })
        .unwrap_or_else(|| "No region selected".into());
    let vertex_label = session
        .selected_vertex
        .map(|handle| format!("vertex {} / ring {}", handle.vertex_index + 1, handle.ring_index))
        .unwrap_or_else(|| "no vertex selected".into());
    text.0 = format!(
        "POLYGON SKETCH | {}\n{} regions | {}\nMode: {} | snapping: {}\nSelected: {}\nMaterial: {} | thickness: {:.1} mm\n{}\n\nV: toggle sketch | E: edit existing vertices | click + drag: move vertex\nDrawing: click to add | Enter: close contour | Shift: bypass snap\nG: toggle snapping (grid {:.0} mm / vertex {:.0} mm) | Backspace: undo pending point\nN: new region | H: hole mode | Tab: next region\nM: material | [ / ]: thickness | Delete: vertex/region | F: finish\n\n{}",
        if session.active { "EDITING" } else { "INACTIVE" },
        session.document.regions.len(),
        selected_label,
        if session.edit_vertices {
            "VERTEX EDIT"
        } else if session.new_region_mode {
            "NEW REGION"
        } else if session.drawing_hole {
            "HOLE"
        } else {
            "DRAW CONTOUR"
        },
        if session.snap_enabled { "ON" } else { "OFF" },
        vertex_label,
        material_name(session.current_material_id),
        session.current_thickness_m * 1_000.0,
        if session.drawing_hole {
            "Drawing a hole"
        } else {
            "Planar polygon editor"
        },
        SNAP_M * 1_000.0,
        VERTEX_SNAP_RADIUS_M * 1_000.0,
        session.message
    );
}

fn nearest_vertex(document: &PolygonSketchDocument, point: [f64; 2], radius: f64) -> Option<VertexHandle> {
    nearest_vertex_excluding_many(document, point, radius, &[])
}

fn coincident_vertices(
    document: &PolygonSketchDocument,
    point: [f64; 2],
    tolerance: f64,
) -> Vec<(VertexHandle, [f64; 2])> {
    let mut result = Vec::new();
    for (region_index, region) in document.regions.iter().enumerate() {
        for (ring_index, points) in std::iter::once(&region.outer_boundary_m)
            .chain(region.holes_m.iter())
            .enumerate()
        {
            for (vertex_index, candidate) in points.iter().copied().enumerate() {
                if distance(point, candidate) <= tolerance {
                    result.push((
                        VertexHandle {
                            region_index,
                            ring_index,
                            vertex_index,
                        },
                        candidate,
                    ));
                }
            }
        }
    }
    result
}

fn nearest_vertex_excluding_many(
    document: &PolygonSketchDocument,
    point: [f64; 2],
    radius: f64,
    excluded: &[VertexHandle],
) -> Option<VertexHandle> {
    let mut best: Option<(VertexHandle, f64)> = None;
    for (region_index, region) in document.regions.iter().enumerate() {
        for (ring_index, points) in std::iter::once(&region.outer_boundary_m)
            .chain(region.holes_m.iter())
            .enumerate()
        {
            for (vertex_index, candidate) in points.iter().copied().enumerate() {
                let handle = VertexHandle {
                    region_index,
                    ring_index,
                    vertex_index,
                };
                if excluded.contains(&handle) {
                    continue;
                }
                let candidate_distance = distance(point, candidate);
                if candidate_distance <= radius && best.is_none_or(|(_, current)| candidate_distance < current) {
                    best = Some((handle, candidate_distance));
                }
            }
        }
    }
    best.map(|(handle, _)| handle)
}

fn ring(region: &PlanarPolygonRegion, ring_index: usize) -> Option<&Vec<[f64; 2]>> {
    if ring_index == 0 {
        Some(&region.outer_boundary_m)
    } else {
        region.holes_m.get(ring_index - 1)
    }
}

fn ring_mut(region: &mut PlanarPolygonRegion, ring_index: usize) -> Option<&mut Vec<[f64; 2]>> {
    if ring_index == 0 {
        Some(&mut region.outer_boundary_m)
    } else {
        region.holes_m.get_mut(ring_index - 1)
    }
}

fn vertex_at(document: &PolygonSketchDocument, handle: VertexHandle) -> Option<[f64; 2]> {
    let region = document.regions.get(handle.region_index)?;
    ring(region, handle.ring_index)?.get(handle.vertex_index).copied()
}

fn vertex_at_mut(document: &mut PolygonSketchDocument, handle: VertexHandle) -> Option<&mut [f64; 2]> {
    let region = document.regions.get_mut(handle.region_index)?;
    ring_mut(region, handle.ring_index)?.get_mut(handle.vertex_index)
}

fn set_vertex(document: &mut PolygonSketchDocument, handle: VertexHandle, point: [f64; 2]) {
    if let Some(target) = vertex_at_mut(document, handle) {
        *target = point;
    }
}

fn restore_vertices(document: &mut PolygonSketchDocument, originals: &[(VertexHandle, [f64; 2])]) {
    for (handle, point) in originals.iter().copied() {
        set_vertex(document, handle, point);
    }
}

fn region_fits_sketch_pad(region: &PlanarPolygonRegion) -> bool {
    region
        .outer_boundary_m
        .iter()
        .chain(region.holes_m.iter().flatten())
        .all(|point| point[0].is_finite() && point[1].is_finite() && point[0].abs() <= 4.5 && point[1].abs() <= 3.5)
}

fn total_document_vertices(document: &PolygonSketchDocument) -> usize {
    document.regions.iter().fold(0_usize, |total, region| {
        let hole_vertices = region
            .holes_m
            .iter()
            .fold(0_usize, |count, hole| count.saturating_add(hole.len()));
        total
            .saturating_add(region.outer_boundary_m.len())
            .saturating_add(hole_vertices)
    })
}

fn material_name(material_id: u16) -> &'static str {
    match material_id {
        0 => "aluminium",
        1 => "steel",
        2 => "polymer",
        _ => "composite",
    }
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sketch_document_round_trips_json() {
        let document = PolygonSketchDocument {
            schema_version: SKETCH_SCHEMA,
            regions: vec![PlanarPolygonRegion {
                source_part_id: "region-a".into(),
                outer_boundary_m: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                holes_m: vec![],
                material_id: 1,
                thickness_m: 0.02,
            }],
        };
        let bytes = serde_json::to_vec(&document).unwrap();
        let restored: PolygonSketchDocument = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored, document);
    }

    #[test]
    fn sketch_snaps_points_to_five_centimetres() {
        let mut session = PolygonSketchSession::default();
        session.add_point([0.123, -0.127]).unwrap();
        assert_eq!(session.pending_points[0], [0.1, -0.15]);
    }

    #[test]
    fn sketch_rejects_points_outside_the_pad() {
        let mut session = PolygonSketchSession::default();
        assert!(session.add_point([5.0, 0.0]).is_err());
    }

    #[test]
    fn new_region_mode_keeps_material_choice_as_a_draft_default() {
        let mut session = PolygonSketchSession::default();
        session.cycle_material();
        assert_eq!(session.current_material_id, 1);
        assert!(session.new_region_mode);
        assert!(session.document.regions.is_empty());
    }

    #[test]
    fn vertex_snap_prefers_near_existing_vertex() {
        let mut session = PolygonSketchSession::default();
        session.document.regions.push(PlanarPolygonRegion {
            source_part_id: "region-a".into(),
            outer_boundary_m: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            holes_m: vec![],
            material_id: 0,
            thickness_m: 0.01,
        });
        let snapped = session.snap_point([0.94, 0.04], None, false);
        assert_eq!(snapped, [1.0, 0.0]);
    }

    #[test]
    fn shift_style_bypass_keeps_free_coordinate() {
        let session = PolygonSketchSession::default();
        let point = session.snap_point([0.123, -0.127], None, true);
        assert_eq!(point, [0.123, -0.127]);
    }

    #[test]
    fn nearest_vertex_can_address_hole_vertices() {
        let document = PolygonSketchDocument {
            schema_version: SKETCH_SCHEMA,
            regions: vec![PlanarPolygonRegion {
                source_part_id: "region-a".into(),
                outer_boundary_m: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
                holes_m: vec![vec![[0.5, 0.5], [1.0, 0.5], [1.0, 1.0], [0.5, 1.0]]],
                material_id: 0,
                thickness_m: 0.01,
            }],
        };
        let handle = nearest_vertex(&document, [0.51, 0.49], 0.1).unwrap();
        assert_eq!(
            handle,
            VertexHandle {
                region_index: 0,
                ring_index: 1,
                vertex_index: 0
            }
        );
    }

    #[test]
    fn vertex_drag_cancel_restores_original_coordinate() {
        let mut session = PolygonSketchSession::default();
        session.document.regions.push(PlanarPolygonRegion {
            source_part_id: "region-a".into(),
            outer_boundary_m: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            holes_m: vec![],
            material_id: 0,
            thickness_m: 0.01,
        });
        session.edit_vertices = true;
        assert!(session.begin_vertex_drag([0.01, 0.02]));
        session.update_vertex_drag([0.45, 0.2], true);
        assert_eq!(session.document.regions[0].outer_boundary_m[0], [0.45, 0.2]);
        session.cancel_vertex_drag();
        assert_eq!(session.document.regions[0].outer_boundary_m[0], [0.0, 0.0]);
    }

    #[test]
    fn nearest_vertex_excludes_the_vertex_being_dragged() {
        let document = PolygonSketchDocument {
            schema_version: SKETCH_SCHEMA,
            regions: vec![PlanarPolygonRegion {
                source_part_id: "region-a".into(),
                outer_boundary_m: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                holes_m: vec![],
                material_id: 0,
                thickness_m: 0.01,
            }],
        };
        let current = VertexHandle {
            region_index: 0,
            ring_index: 0,
            vertex_index: 0,
        };
        assert_eq!(
            nearest_vertex_excluding_many(&document, [0.02, 0.02], 0.1, &[current]),
            None
        );
        assert_eq!(
            nearest_vertex_excluding_many(&document, [0.96, 0.02], 0.1, &[current]),
            Some(VertexHandle {
                region_index: 0,
                ring_index: 0,
                vertex_index: 1
            })
        );
    }

    #[test]
    fn shared_vertex_cannot_be_deleted_independently() {
        let mut session = PolygonSketchSession::default();
        session.document.regions = vec![
            PlanarPolygonRegion {
                source_part_id: "left".into(),
                outer_boundary_m: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                holes_m: vec![],
                material_id: 0,
                thickness_m: 0.01,
            },
            PlanarPolygonRegion {
                source_part_id: "right".into(),
                outer_boundary_m: vec![[1.0, 0.0], [2.0, 0.0], [2.0, 1.0], [1.0, 1.0]],
                holes_m: vec![],
                material_id: 1,
                thickness_m: 0.02,
            },
        ];
        session.selected_vertex = Some(VertexHandle {
            region_index: 0,
            ring_index: 0,
            vertex_index: 1,
        });
        session.delete_selected_vertex();
        assert_eq!(session.document.regions[0].outer_boundary_m[1], [1.0, 0.0]);
        assert!(session.message.contains("shared"));
    }

    #[test]
    fn invalid_vertex_move_is_rolled_back() {
        let mut session = PolygonSketchSession::default();
        session.document.regions.push(PlanarPolygonRegion {
            source_part_id: "region-a".into(),
            outer_boundary_m: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            holes_m: vec![],
            material_id: 0,
            thickness_m: 0.01,
        });
        session.edit_vertices = true;
        assert!(session.begin_vertex_drag([0.0, 0.0]));
        session.update_vertex_drag([1.0, 1.0], true);
        session.finish_vertex_drag();
        assert_eq!(session.document.regions[0].outer_boundary_m[0], [0.0, 0.0]);
        assert!(session.message.contains("rejected"));
    }

    #[test]
    fn moving_a_shared_vertex_moves_both_region_copies() {
        let mut session = PolygonSketchSession::default();
        session.document.regions = vec![
            PlanarPolygonRegion {
                source_part_id: "left".into(),
                outer_boundary_m: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                holes_m: vec![],
                material_id: 0,
                thickness_m: 0.01,
            },
            PlanarPolygonRegion {
                source_part_id: "right".into(),
                outer_boundary_m: vec![[1.0, 0.0], [2.0, 0.0], [2.0, 1.0], [1.0, 1.0]],
                holes_m: vec![],
                material_id: 1,
                thickness_m: 0.02,
            },
        ];
        session.edit_vertices = true;
        assert!(session.begin_vertex_drag([1.0, 0.0]));
        session.update_vertex_drag([1.0, -0.2], true);
        assert_eq!(session.document.regions[0].outer_boundary_m[1], [1.0, -0.2]);
        assert_eq!(session.document.regions[1].outer_boundary_m[0], [1.0, -0.2]);
        session.cancel_vertex_drag();
        assert_eq!(session.document.regions[0].outer_boundary_m[1], [1.0, 0.0]);
        assert_eq!(session.document.regions[1].outer_boundary_m[0], [1.0, 0.0]);
    }

    #[test]
    fn total_vertex_counter_includes_holes() {
        let document = PolygonSketchDocument {
            schema_version: SKETCH_SCHEMA,
            regions: vec![PlanarPolygonRegion {
                source_part_id: "region-a".into(),
                outer_boundary_m: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
                holes_m: vec![vec![[0.2, 0.2], [0.4, 0.2], [0.3, 0.4]]],
                material_id: 0,
                thickness_m: 0.01,
            }],
        };
        assert_eq!(total_document_vertices(&document), 6);
    }
}
