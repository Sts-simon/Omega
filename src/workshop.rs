//! Pointer interaction for the missile workshop: pick a part with a camera ray, drag a template
//! from the palette (or an existing part) onto a free attachment node, move a whole subtree, delete
//! it, cycle radial symmetry, undo/redo and adjust the selected part with the wheel.
//!
//! Geometry lives in `picking.rs` and the rules live in `omega-rules::assembly`; this module only
//! turns pointer input into those calls and draws the node markers/ghost with gizmos.

use crate::campus::{CampusState, CampusView};
use crate::missile::{self, MissileSession, PartTemplate};
use crate::picking::{PickShape, SNAP_PIXELS, nearest_hit, nearest_snap};

/// Pixels the pointer must travel before a press on a part becomes a move.
const DRAG_PIXELS: f32 = 5.0;
/// A drag snaps to the nearest free mount inside this screen radius. It is deliberately larger
/// than `SNAP_PIXELS`: the preview should follow the nearest available point, not require pixel
/// precision, while the exact-node highlight still uses the tighter radius.
const AUTO_SNAP_PIXELS: f32 = 140.0;
/// World-space size of the transform handles. 1.5 m dwarfed a 2 m missile.
const GIZMO_LENGTH: f32 = 0.7;
const GIZMO_RING: f32 = 0.5;
use crate::ui_kit::UiInteraction;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::ecs::system::SystemParam;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::window::PrimaryWindow;
use omega_rules::assembly::{radial_angle, radial_slots};
use omega_rules::{NodeId, PartCategory, PartSpec, Vehicle};
use std::collections::BTreeSet;

/// A palette entry that can be dragged instead of clicked.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct PalettePart(pub(crate) PartTemplate);

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DragSource {
    Palette(PartTemplate),
    Part(String),
}

#[derive(Clone, Debug)]
pub(crate) struct GhostPreview {
    pub template: PartTemplate,
    pub world: Vec3,
    pub orientation: Quat,
    pub valid: bool,
}

#[derive(Resource, Default)]
pub(crate) struct DragState {
    pub source: Option<DragSource>,
    pub hover: Option<(String, NodeId, f64)>,
    /// The transparent part the placement system draws under the pointer.
    pub ghost: Option<GhostPreview>,
    /// Where a press on a part began; the part only starts moving past `DRAG_PIXELS`, so a plain
    /// click selects without relocating the part.
    pending: Option<(Vec2, DragSource)>,
}

/// Tells the camera system that the wheel was consumed by a part adjustment this frame.
#[derive(Resource, Default)]
pub(crate) struct WorkshopInput {
    pub wheel_used: bool,
    /// True while a press has become (or is about to become) a drag; Escape then cancels the drag
    /// instead of leaving the workshop.
    pub drag_active: bool,
    /// True while the palette search or a group rename owns the keyboard.
    pub text_active: bool,
    pub search_active: bool,
}

/// What the pointer is over, updated by `workshop_pointer`. Kept outside the session so hovering
/// never marks the design dirty or rebuilds a board.
#[derive(Resource, Default)]
pub(crate) struct WorkshopHover {
    pub part: Option<String>,
    pub palette: Option<PartTemplate>,
}

/// The small render-to-texture camera and image shown above the palette.
#[derive(Resource)]
pub(crate) struct PalettePreview {
    pub image: Handle<Image>,
    current: Option<PartTemplate>,
    root: Entity,
}

#[derive(Component)]
pub(crate) struct PaletteHintText;

#[derive(Component)]
pub(crate) struct PalettePreviewRoot;

#[derive(Component)]
pub(crate) struct DragGhostRoot;

/// Handles and entities of the transparent placement preview.
#[derive(Resource)]
pub(crate) struct DragGhost {
    root: Entity,
    child: Option<Entity>,
    current: Option<PartTemplate>,
    valid_material: Handle<StandardMaterial>,
    invalid_material: Handle<StandardMaterial>,
}

/// The four queries `workshop_pointer` needs. Grouping them keeps the system below Bevy's
/// system-parameter limit while leaving the resources readable at their call sites.
#[derive(SystemParam)]
pub(crate) struct WorkshopQueries<'w, 's> {
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    cameras: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<crate::scene::SceneCamera>>,
    palettes: Query<'w, 's, (&'static UiInteraction, &'static PalettePart)>,
    ui: Query<'w, 's, &'static UiInteraction>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TransformMode {
    #[default]
    Translate,
    Rotate,
}

/// KSP-style transform gizmo state. A handle drag is committed as one undoable edit on release.
#[derive(Resource)]
pub(crate) struct TransformGizmo {
    pub mode: TransformMode,
    pub drag: Option<(usize, Vec2, Vec2)>,
    pub before: Option<Vehicle>,
    /// Grid snapping is enabled by default; Shift temporarily bypasses it.
    pub snap_enabled: bool,
    /// 0 = fine, 1 = normal, 2 = coarse.
    pub snap_preset: usize,
}

impl Default for TransformGizmo {
    fn default() -> Self {
        Self {
            mode: TransformMode::Translate,
            drag: None,
            before: None,
            snap_enabled: true,
            snap_preset: 1,
        }
    }
}

#[derive(Clone, Debug)]
struct NodePoint {
    parent: String,
    node: NodeId,
    angle: f64,
    world: Vec3,
    /// Orientation of the part that will be mounted here. Axial mounts follow the parent; a
    /// surface turns with its radial mount so it stays tangent to the body.
    orientation: Quat,
}

fn snap_step(mode: TransformMode, preset: usize) -> f64 {
    const TRANSLATION: [f64; 3] = [0.01, 0.10, 0.50];
    const ROTATION: [f64; 3] = [1.0, 5.0, 15.0];
    let index = preset.min(2);
    if mode == TransformMode::Translate {
        TRANSLATION[index]
    } else {
        ROTATION[index]
    }
}

fn snap_amount(amount: f64, step: f64) -> f64 {
    (amount / step).round() * step
}

fn gizmo_amount(
    cursor: Vec2,
    start: Vec2,
    screen_axis: Vec2,
    mode: TransformMode,
    fine: bool,
    snap_enabled: bool,
    snap_preset: usize,
) -> f64 {
    let raw = if mode == TransformMode::Translate {
        (cursor - start).dot(screen_axis.normalize_or_zero()) as f64 * 0.02
    } else {
        let start_angle = (start.y - screen_axis.y).atan2(start.x - screen_axis.x);
        let current_angle = (cursor.y - screen_axis.y).atan2(cursor.x - screen_axis.x);
        let mut radians = current_angle - start_angle;
        if radians > std::f32::consts::PI {
            radians -= std::f32::consts::TAU;
        } else if radians < -std::f32::consts::PI {
            radians += std::f32::consts::TAU;
        }
        radians.to_degrees() as f64
    };
    if fine || !snap_enabled {
        raw
    } else {
        snap_amount(raw, snap_step(mode, snap_preset))
    }
}

fn distance_to_segment(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let segment = end - start;
    let length_squared = segment.length_squared();
    if length_squared <= f32::EPSILON {
        return point.distance(start);
    }
    let t = ((point - start).dot(segment) / length_squared).clamp(0.0, 1.0);
    point.distance(start + segment * t)
}

/// One point of a rotation ring, in the selected part's local space. The draw pass and the hit
/// test share it so a ring cannot drift away from the handle the pointer grabs.
fn gizmo_ring_point(center: Vec3, orientation: Quat, axis: usize, angle: f32) -> Vec3 {
    let local = match axis {
        0 => Vec3::new(0.0, angle.cos(), angle.sin()),
        1 => Vec3::new(angle.cos(), 0.0, angle.sin()),
        _ => Vec3::new(angle.cos(), angle.sin(), 0.0),
    };
    center + orientation * local * GIZMO_RING
}

/// Every node of the design that can still take a part, in world space. Hidden subtrees offer no
/// target even though their rules still count them.
fn free_nodes(vehicle: &Vehicle, hidden: &BTreeSet<String>) -> Vec<NodePoint> {
    let children: Vec<(&str, NodeId)> = vehicle
        .parts
        .iter()
        .filter_map(|part| {
            let attach = part.attach.as_ref()?;
            Some((attach.parent.as_str(), attach.node))
        })
        .collect();
    let is_free = |parent: &str, node: NodeId| !children.iter().any(|(p, n)| *p == parent && *n == node);
    let mut nodes = Vec::new();
    for pose in vehicle.poses() {
        let Some(part) = vehicle.part(&pose.id) else {
            continue;
        };
        if part.spec.is_aero() || part.spec.is_nose() || hidden.contains(&pose.id) {
            continue; // a leaf or a hidden subtree offers no attachment node
        }
        let center = missile::world_point(vehicle, pose.x_m, pose.y_m, pose.z_m);
        let orientation = missile::part_rotation(
            vehicle,
            pose.rotation_x_deg as f32,
            pose.rotation_y_deg as f32,
            pose.rotation_z_deg as f32,
        );
        let half = pose.height_m as f32 * missile::VISUAL_SCALE * 0.5;
        if is_free(&pose.id, NodeId::Top) {
            nodes.push(NodePoint {
                parent: pose.id.clone(),
                node: NodeId::Top,
                angle: 0.0,
                world: center + orientation * Vec3::Y * half,
                orientation,
            });
        }
        if is_free(&pose.id, NodeId::Bottom) {
            nodes.push(NodePoint {
                parent: pose.id.clone(),
                node: NodeId::Bottom,
                angle: 0.0,
                world: center - orientation * Vec3::Y * half,
                orientation,
            });
        }
        for index in 0..radial_slots(&part.spec) {
            let node = NodeId::Radial(index);
            if is_free(&pose.id, node) {
                let angle = radial_angle(&part.spec, index);
                let local_direction = Vec3::new(angle.to_radians().cos() as f32, 0.0, angle.to_radians().sin() as f32);
                nodes.push(NodePoint {
                    parent: pose.id.clone(),
                    node,
                    // The node itself sets the direction; the stored attachment angle stays zero,
                    // otherwise the pose would add the same angle twice.
                    angle: 0.0,
                    world: center + orientation * local_direction * pose.radius_m as f32 * missile::VISUAL_SCALE,
                    orientation: orientation * Quat::from_rotation_y(-(angle as f32).to_radians()),
                });
            }
        }
    }
    nodes
}

const SURFACE_HALF_THICKNESS_M: f32 = 0.05;

#[cfg(test)]
fn picked_part(vehicle: &Vehicle, origin: Vec3, direction: Vec3) -> Option<String> {
    picked_visible_part(vehicle, origin, direction, &BTreeSet::new())
}

fn picked_visible_part(vehicle: &Vehicle, origin: Vec3, direction: Vec3, hidden: &BTreeSet<String>) -> Option<String> {
    let poses: Vec<_> = vehicle
        .poses()
        .into_iter()
        .filter(|pose| !hidden.contains(&pose.id))
        .collect();
    let shapes: Vec<PickShape> = poses
        .iter()
        .map(|pose| {
            let center = missile::world_point(vehicle, pose.x_m, pose.y_m, pose.z_m);
            let scale = missile::VISUAL_SCALE;
            let part = vehicle.part(&pose.id);
            let orientation = missile::part_rotation(
                vehicle,
                pose.rotation_x_deg as f32,
                pose.rotation_y_deg as f32,
                pose.rotation_z_deg as f32,
            );
            if part.is_some_and(|part| part.spec.is_aero()) {
                let angle = part
                    .and_then(|part| part.attach.as_ref())
                    .map_or(0.0, |attach| match attach.node {
                        NodeId::Radial(index) => {
                            let mount = vehicle
                                .part(&attach.parent)
                                .map_or(0.0, |parent| omega_rules::assembly::radial_angle(&parent.spec, index));
                            mount + attach.angle_deg
                        }
                        _ => attach.angle_deg,
                    });
                PickShape::Box {
                    center,
                    half: Vec3::new(
                        (pose.radius_m as f32) * scale,
                        (pose.height_m as f32) * scale * 0.5,
                        SURFACE_HALF_THICKNESS_M * scale,
                    ),
                    rotation: orientation * Quat::from_rotation_y(-(angle as f32).to_radians()),
                }
            } else {
                PickShape::Cylinder {
                    center,
                    radius: (pose.radius_m as f32).max(0.12) * scale,
                    half_height: (pose.height_m as f32) * scale * 0.5,
                    rotation: orientation,
                }
            }
        })
        .collect();
    let ids: Vec<String> = poses.into_iter().map(|pose| pose.id).collect();
    let (index, _) = nearest_hit(origin, direction, &shapes)?;
    ids.get(index).cloned()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn workshop_pointer(
    frontend: Res<crate::main_menu::Frontend>,
    inspector: Res<crate::inspector::InspectorState>,
    mut hover: ResMut<WorkshopHover>,
    mut session: ResMut<MissileSession>,
    mut campus: ResMut<CampusState>,
    queries: WorkshopQueries,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    mut typed: MessageReader<KeyboardInput>,
    mut drag: ResMut<DragState>,
    mut input: ResMut<WorkshopInput>,
    mut revisions: ResMut<crate::revisions::RevisionLog>,
    mut transform_gizmo: ResMut<TransformGizmo>,
    mut saved_vehicle: Local<Option<Vehicle>>,
    mut gizmos: Gizmos,
) {
    if *frontend != crate::main_menu::Frontend::Game {
        input.wheel_used = false;
        input.drag_active = false;
        input.text_active = false;
        hover.part = None;
        hover.palette = None;
        drag.source = None;
        drag.hover = None;
        drag.ghost = None;
        return;
    }
    input.wheel_used = false;
    input.drag_active = false;
    drag.ghost = None;
    if campus.view != CampusView::Missile {
        input.text_active = false;
        hover.part = None;
        hover.palette = None;
        drag.source = None;
        drag.hover = None;
        drag.ghost = None;
        drag.pending = None;
        return;
    }
    if inspector.is_editing() {
        // The numeric field owns the keyboard while it is being edited, including Escape. Keep
        // `drag_active` true so the campus Escape handler lets the inspector cancel instead of
        // leaving the workshop.
        input.wheel_used = false;
        input.drag_active = true;
        input.text_active = true;
        hover.part = None;
        hover.palette = None;
        drag.source = None;
        drag.hover = None;
        drag.ghost = None;
        drag.pending = None;
        // The inspector has its own keyboard reader; discard these events so W/R/G/T typed into a
        // numeric field cannot activate a gizmo tool once editing ends.
        for _ in typed.read() {}
        return;
    }
    let owned = campus.owned_capabilities();
    let pointer_over_ui = queries.ui.iter().any(|interaction| *interaction != UiInteraction::None);

    let Some(window) = queries.windows.iter().next() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, camera_transform)) = queries.cameras.single() else {
        return;
    };
    let ray = camera
        .viewport_to_world(camera_transform, cursor)
        .ok()
        .map(|ray| (ray.origin, *ray.direction));
    let hidden = session.hidden_parts();
    let picked = ray.and_then(|(origin, direction)| picked_visible_part(&session.vehicle, origin, direction, &hidden));
    hover.part = (!pointer_over_ui).then_some(picked.clone()).flatten();
    hover.palette = queries
        .palettes
        .iter()
        .find(|(interaction, _)| **interaction == UiInteraction::Hovered)
        .map(|(_, palette)| palette.0);

    // Read logical keys so W/R work on AZERTY as printed, not only at QWERTY scan positions. The
    // search field and the group rename own the same reader while they are open.
    let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let text_mode = input.search_active || session.rename_target.is_some() || revisions.naming_target.is_some();
    input.text_active = text_mode;
    let mut translate_tool_pressed = false;
    let mut rotate_tool_pressed = false;
    let mut undo_pressed = false;
    let mut redo_pressed = false;
    let mut toggle_snap_pressed = false;
    let mut cycle_snap_pressed = false;
    for event in typed.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        if text_mode {
            match &event.logical_key {
                Key::Character(character) if !control => {
                    if revisions.naming_target.is_some() {
                        if revisions.naming_draft.chars().count() < 40 {
                            revisions.naming_draft.push_str(character);
                        }
                    } else if session.rename_target.is_some() {
                        if session.rename_draft.chars().count() < 32 {
                            session.rename_draft.push_str(character);
                        }
                    } else if session.palette_search.chars().count() < 32 {
                        session.palette_search.push_str(character);
                    }
                }
                Key::Backspace => {
                    if revisions.naming_target.is_some() {
                        revisions.naming_draft.pop();
                    } else if session.rename_target.is_some() {
                        session.rename_draft.pop();
                    } else {
                        session.palette_search.pop();
                    }
                }
                Key::Enter => {
                    if let Some(id) = revisions.naming_target {
                        let draft = revisions.naming_draft.clone();
                        campus.message = match revisions.rename(id, &draft) {
                            Ok(()) => match revisions
                                .to_bytes()
                                .and_then(|bytes| crate::save::write(crate::save::SaveFile::Revisions, &bytes))
                            {
                                Ok(()) => format!("Revision {id} renamed."),
                                Err(error) => format!("Revision renamed but not saved: {error}"),
                            },
                            Err(error) => format!("Revision not renamed: {error}"),
                        };
                        revisions.cancel_naming();
                    } else if session.rename_target.is_some() {
                        campus.message = session.apply(missile::MissileAction::CommitRename, &owned);
                    } else {
                        input.search_active = false;
                        campus.message = "Search applied; press / to edit it".to_string();
                    }
                }
                _ => {}
            }
            continue;
        }
        if let Key::Character(character) = &event.logical_key {
            match (control, character.to_lowercase().as_str()) {
                (false, "w") => translate_tool_pressed = true,
                (false, "r") => rotate_tool_pressed = true,
                (true, "z") => undo_pressed = true,
                (true, "y") => redo_pressed = true,
                (false, "g") => toggle_snap_pressed = true,
                (false, "t") => cycle_snap_pressed = true,
                (false, "/") => {
                    input.search_active = true;
                    campus.message = "Search: type a name, Enter to apply, Escape to clear".to_string();
                }
                _ => {}
            }
        }
    }

    // KSP-style tool selection. W = translate, R = rotate; the coloured handles select X/Y/Z directly.
    if translate_tool_pressed || rotate_tool_pressed {
        if let Some(before) = transform_gizmo.before.take() {
            session.vehicle = before;
        }
        transform_gizmo.drag = None;
        transform_gizmo.mode = if rotate_tool_pressed {
            campus.message = "Rotation : glissez un anneau. G : accrochage, T : pas, Maj : libre.".to_string();
            TransformMode::Rotate
        } else {
            campus.message = "Déplacement : glissez une flèche. G : accrochage, T : pas, Maj : libre.".to_string();
            TransformMode::Translate
        };
    }

    if toggle_snap_pressed {
        transform_gizmo.snap_enabled = !transform_gizmo.snap_enabled;
        campus.message = format!(
            "Accrochage {}. G : activer/désactiver, T : précision, Maj : libre.",
            if transform_gizmo.snap_enabled {
                "activé"
            } else {
                "désactivé"
            }
        );
    }
    if cycle_snap_pressed {
        transform_gizmo.snap_preset = (transform_gizmo.snap_preset + 1) % 3;
        let step = snap_step(transform_gizmo.mode, transform_gizmo.snap_preset);
        let unit = if transform_gizmo.mode == TransformMode::Translate {
            "m"
        } else {
            "°"
        };
        campus.message = format!("Pas d'accrochage : {step}{unit}");
    }

    // --- start a drag ---
    if mouse.just_pressed(MouseButton::Left) {
        if !pointer_over_ui
            && !text_mode
            && let Some(id) = session.selected.as_deref()
            && !session.edit_locked(id)
            && !hidden.contains(id)
            && let Some(pose) = session.vehicle.poses().into_iter().find(|pose| pose.id == id)
        {
            let center = missile::world_point(&session.vehicle, pose.x_m, pose.y_m, pose.z_m);
            let orientation = missile::part_rotation(
                &session.vehicle,
                pose.rotation_x_deg as f32,
                pose.rotation_y_deg as f32,
                pose.rotation_z_deg as f32,
            );
            // KSP-style local space: the handles follow the selected part's orientation.
            let axes = [orientation * Vec3::X, orientation * Vec3::Y, orientation * Vec3::Z];
            let center_screen = camera.world_to_viewport(camera_transform, center).ok();
            let mut nearest: Option<(usize, Vec2, f32)> = None;
            for (axis, direction) in axes.into_iter().enumerate() {
                if transform_gizmo.mode == TransformMode::Translate {
                    if let (Some(origin), Ok(point)) = (
                        center_screen,
                        camera.world_to_viewport(camera_transform, center + direction * GIZMO_LENGTH),
                    ) {
                        let distance = distance_to_segment(cursor, origin, point);
                        if distance <= 14.0 && nearest.is_none_or(|(_, _, best)| distance < best) {
                            nearest = Some((axis, point - origin, distance));
                        }
                    }
                } else {
                    for step in 0..48 {
                        let angle = std::f32::consts::TAU * step as f32 / 48.0;
                        let ring_point = gizmo_ring_point(center, orientation, axis, angle);
                        if let Ok(point) = camera.world_to_viewport(camera_transform, ring_point) {
                            let distance = cursor.distance(point);
                            if distance <= 26.0 && nearest.is_none_or(|(_, _, best)| distance < best) {
                                nearest = Some((axis, center_screen.unwrap_or(cursor), distance));
                            }
                        }
                    }
                }
            }
            if let Some((axis, screen_axis, _)) = nearest {
                let drag_reference = if transform_gizmo.mode == TransformMode::Translate {
                    screen_axis.normalize_or_zero()
                } else {
                    screen_axis
                };
                transform_gizmo.drag = Some((axis, cursor, drag_reference));
                transform_gizmo.before = Some(session.vehicle.clone());
            }
        }
        if transform_gizmo.drag.is_none() {
            if let Some((_, palette)) = queries.palettes.iter().find(|(i, _)| **i == UiInteraction::Pressed) {
                drag.source = Some(DragSource::Palette(palette.0));
                drag.hover = None;
            } else if !pointer_over_ui && !text_mode {
                match picked.clone() {
                    Some(id) => {
                        if control {
                            campus.message =
                                session.apply(missile::MissileAction::ToggleSelect { id: id.clone() }, &owned);
                            drag.pending = None;
                        } else {
                            // Clicking a member of a multi-selection keeps the group and only
                            // moves the primary part, so the gizmo can move the whole selection.
                            if !session.is_selected(&id) || session.selection.len() <= 1 {
                                session.apply(missile::MissileAction::Select { id: id.clone() }, &owned);
                            } else {
                                session.selected = Some(id.clone());
                            }
                            drag.pending = session
                                .vehicle
                                .part(&id)
                                .is_some_and(|part| part.attach.is_some() && !session.edit_locked(&id))
                                .then_some((cursor, DragSource::Part(id)));
                        }
                        drag.hover = None;
                    }
                    None if !control => {
                        campus.message = session.clear_selection();
                        drag.pending = None;
                    }
                    None => {}
                }
            }
        }
    }
    if mouse.pressed(MouseButton::Left) {
        if let Some((start, source)) = drag.pending.clone()
            && cursor.distance(start) > DRAG_PIXELS
        {
            drag.source = Some(source);
            drag.pending = None;
        }
    } else {
        drag.pending = None;
    }

    // --- free nodes and their on-screen positions ---
    let nodes = free_nodes(&session.vehicle, &hidden);
    let screen: Vec<Vec2> = nodes
        .iter()
        .filter_map(|node| camera.world_to_viewport(camera_transform, node.world).ok())
        .collect();
    let dragged_spec = match drag.source.as_ref() {
        Some(DragSource::Palette(template)) => Some(template.spec()),
        Some(DragSource::Part(id)) => session.vehicle.part(id).map(|part| part.spec),
        None => None,
    };
    let dragged_part = match drag.source.clone() {
        Some(DragSource::Part(id)) => Some(id),
        _ => None,
    };
    let node_valid = |index: usize, moving: Option<&str>| -> bool {
        let node = &nodes[index];
        let Some(spec) = dragged_spec else {
            return false;
        };
        if let Some(id) = moving
            && (id == node.parent || session.vehicle.is_descendant(id, &node.parent))
        {
            return false;
        }
        !session.is_locked(&node.parent) && session.vehicle.node_status(&node.parent, node.node, &spec).is_ok()
    };
    // Automatic snapping: the nearest *valid* mount wins inside a generous radius. If none is
    // valid, the nearest mount is still shown in red so the player sees why the drop is refused.
    let snapped = (!pointer_over_ui && screen.len() == nodes.len())
        .then(|| {
            let nearest_valid = (0..nodes.len())
                .filter(|index| node_valid(*index, dragged_part.as_deref()))
                .filter_map(|index| {
                    let distance = screen[index].distance(cursor);
                    (distance <= AUTO_SNAP_PIXELS).then_some((index, distance))
                })
                .min_by(|(_, a), (_, b)| a.total_cmp(b))
                .map(|(index, _)| index);
            nearest_valid.or_else(|| nearest_snap(cursor, &screen, AUTO_SNAP_PIXELS))
        })
        .flatten();

    if let Some(source) = drag.source.clone() {
        drag.hover = None;
        drag.ghost = None;
        if let Some(index) = snapped {
            let node = &nodes[index];
            let valid = node_valid(index, dragged_part.as_deref());
            if valid {
                drag.hover = Some((node.parent.clone(), node.node, node.angle));
            }
            if let DragSource::Palette(template) = source {
                drag.ghost = Some(GhostPreview {
                    template,
                    world: node.world,
                    orientation: node.orientation,
                    valid,
                });
            }
        } else if let (DragSource::Palette(template), Some((origin, direction))) = (&source, ray) {
            drag.ghost = Some(GhostPreview {
                template: *template,
                world: origin + direction * 4.0,
                orientation: Quat::IDENTITY,
                valid: false,
            });
        }
        // Node markers while dragging: green for a legal mount, red for an incompatible one, and
        // a brighter sphere on the automatically selected point.
        for (index, node) in nodes.iter().enumerate() {
            let hovered = snapped == Some(index);
            let valid = node_valid(index, dragged_part.as_deref());
            let color = if hovered {
                if valid {
                    Color::srgb(0.35, 0.95, 0.55)
                } else {
                    Color::srgb(0.95, 0.32, 0.28)
                }
            } else if valid {
                Color::srgba(0.36, 0.84, 0.94, 0.55)
            } else {
                Color::srgba(0.88, 0.32, 0.28, 0.45)
            };
            let precise = nearest_snap(cursor, &screen, SNAP_PIXELS) == Some(index);
            let radius = if precise { 0.34 } else { 0.18 };
            gizmos.sphere(Isometry3d::from_translation(node.world), radius, color);
        }
        if let Some(index) = snapped {
            let color = if node_valid(index, dragged_part.as_deref()) {
                Color::srgb(0.35, 0.95, 0.55)
            } else {
                Color::srgb(0.95, 0.32, 0.28)
            };
            gizmos.cube(
                Transform::from_translation(nodes[index].world).with_scale(Vec3::splat(0.45)),
                color,
            );
        } else if let Some((origin, direction)) = ray {
            // A small marker along the ray so the drag is visible even between nodes.
            gizmos.line(
                origin + direction * 4.0,
                origin + direction * 4.2,
                Color::srgba(0.36, 0.84, 0.94, 0.8),
            );
        }
    }

    // Live preview while dragging, then one history entry on release.
    if mouse.pressed(MouseButton::Left)
        && let Some((axis, start, screen_axis)) = transform_gizmo.drag
        && let Some(before) = transform_gizmo.before.as_ref()
    {
        let fine = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let amount = gizmo_amount(
            cursor,
            start,
            screen_axis,
            transform_gizmo.mode,
            fine,
            transform_gizmo.snap_enabled,
            transform_gizmo.snap_preset,
        );
        let rotating = transform_gizmo.mode == TransformMode::Rotate;
        let unit = if rotating { "°" } else { " m" };
        campus.message = format!(
            "{} {} : {amount:+.2}{unit}{}",
            if rotating { "Rotation" } else { "Déplacement" },
            ["X", "Y", "Z"][axis],
            if fine {
                " | précision libre".to_string()
            } else if transform_gizmo.snap_enabled {
                if rotating {
                    format!(
                        " | pas {}°",
                        snap_step(transform_gizmo.mode, transform_gizmo.snap_preset)
                    )
                } else {
                    format!(
                        " | pas {} m",
                        snap_step(transform_gizmo.mode, transform_gizmo.snap_preset)
                    )
                }
            } else {
                " | sans accrochage".to_string()
            }
        );
        session.preview_transform(before, axis, amount, rotating);
    }
    if mouse.just_released(MouseButton::Left)
        && let Some((axis, start, screen_axis)) = transform_gizmo.drag.take()
    {
        let fine = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let amount = gizmo_amount(
            cursor,
            start,
            screen_axis,
            transform_gizmo.mode,
            fine,
            transform_gizmo.snap_enabled,
            transform_gizmo.snap_preset,
        );
        if let Some(before) = transform_gizmo.before.take() {
            session.preview_transform(&before, axis, amount, transform_gizmo.mode == TransformMode::Rotate);
            session.commit_transform(before);
        }
        if amount.abs() > 0.001 {
            campus.message = format!(
                "{} {} {amount:+.2}",
                if transform_gizmo.mode == TransformMode::Rotate {
                    "Rotation"
                } else {
                    "Déplacement"
                },
                ["X", "Y", "Z"][axis]
            );
        }
    }

    if mouse.just_released(MouseButton::Left)
        && let Some(source) = drag.source.clone()
    {
        let mut message = None;
        match (&source, drag.hover.clone()) {
            (DragSource::Palette(template), Some((parent, node, angle))) => {
                message = Some(session.add_at(*template, Some((parent, node, angle)), &owned));
            }
            (DragSource::Part(id), Some((parent, node, angle))) => {
                message = Some(session.move_part(id, &parent, node, angle));
            }
            (DragSource::Palette(template), None) => {
                // A plain click on the palette still adds above the current selection.
                let over_palette = queries
                    .palettes
                    .iter()
                    .any(|(interaction, _)| *interaction != UiInteraction::None);
                if over_palette {
                    message = Some(session.add_at(*template, None, &owned));
                }
            }
            (DragSource::Part(_), None) => {}
        }
        if let Some(message) = message {
            campus.message = message;
        }
        drag.source = None;
        drag.hover = None;
        drag.pending = None;
    }

    // Three coloured translation handles or rotation rings, centred on the selected part.
    if drag.source.is_none()
        && let Some(id) = session.selected.as_deref()
        && !session.edit_locked(id)
        && !session.hidden_parts().contains(id)
        && let Some(pose) = session.vehicle.poses().into_iter().find(|pose| pose.id == id)
        && session.vehicle.part(id).is_some_and(|part| part.attach.is_some())
    {
        let center = missile::world_point(&session.vehicle, pose.x_m, pose.y_m, pose.z_m);
        let orientation = missile::part_rotation(
            &session.vehicle,
            pose.rotation_x_deg as f32,
            pose.rotation_y_deg as f32,
            pose.rotation_z_deg as f32,
        );
        let axes = [
            (orientation * Vec3::X, Color::srgb(0.95, 0.25, 0.25)),
            (orientation * Vec3::Y, Color::srgb(0.30, 0.90, 0.40)),
            (orientation * Vec3::Z, Color::srgb(0.25, 0.55, 1.0)),
        ];
        for (axis_index, (axis, color)) in axes.into_iter().enumerate() {
            if transform_gizmo.mode == TransformMode::Translate {
                let end = center + axis * GIZMO_LENGTH;
                gizmos.line(center, end, color);
                gizmos.sphere(Isometry3d::from_translation(end), 0.11, color);
            } else {
                let mut previous = None;
                for step in 0..=48 {
                    let angle = std::f32::consts::TAU * step as f32 / 48.0;
                    let point = gizmo_ring_point(center, orientation, axis_index, angle);
                    if let Some(from) = previous {
                        gizmos.line(from, point, color);
                    }
                    previous = Some(point);
                }
            }
        }
    }

    // --- keyboard ---
    if keys.just_pressed(KeyCode::Escape)
        && (drag.source.is_some() || drag.pending.is_some() || transform_gizmo.drag.is_some())
    {
        drag.source = None;
        drag.hover = None;
        drag.pending = None;
        transform_gizmo.drag = None;
        if let Some(before) = transform_gizmo.before.take() {
            session.vehicle = before;
        }
        campus.message = "Manipulation annulée.".to_string();
    } else if keys.just_pressed(KeyCode::Escape) && revisions.naming_target.is_some() {
        input.text_active = false;
        revisions.cancel_naming();
        campus.message = "Revision rename cancelled.".to_string();
    } else if keys.just_pressed(KeyCode::Escape) && session.rename_target.is_some() {
        input.text_active = false;
        campus.message = session.apply(missile::MissileAction::CancelRename, &owned);
    } else if keys.just_pressed(KeyCode::Escape) && input.search_active {
        input.search_active = false;
        input.text_active = false;
        session.palette_search.clear();
        campus.message = "Search cleared".to_string();
    } else if keys.just_pressed(KeyCode::Escape) && !session.selection.is_empty() {
        campus.message = session.clear_selection();
    }
    if !text_mode && drag.source.is_none() && transform_gizmo.drag.is_none() {
        if keys.just_pressed(KeyCode::KeyQ) {
            campus.message = session.rotate_selected(-1);
        }
        if keys.just_pressed(KeyCode::KeyE) {
            campus.message = session.rotate_selected(1);
        }
        if keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace) {
            campus.message = session.delete_selected();
        }
        let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        if keys.just_pressed(KeyCode::KeyX) {
            campus.message = if shift {
                session.cycle_symmetry_kind()
            } else {
                session.cycle_symmetry()
            };
        }
        if shift && keys.just_pressed(KeyCode::KeyK) {
            campus.message = session.break_symmetry();
        }
        if shift && keys.just_pressed(KeyCode::KeyJ) {
            campus.message = session.link_symmetry();
        }
        if shift && keys.just_pressed(KeyCode::KeyD) {
            campus.message = session.duplicate_selected();
        }
        if undo_pressed {
            campus.message = session.undo();
        }
        if redo_pressed {
            campus.message = session.redo();
        }
    }

    // --- wheel adjusts the selected part's main parameter ---
    let lines = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 24.0,
    };
    let primary = session
        .selected
        .as_deref()
        .and_then(|id| session.vehicle.part(id))
        .and_then(|part| missile::params_of(part).first().map(|(param, _)| *param));
    if lines != 0.0
        && !text_mode
        && !pointer_over_ui
        && picked.as_deref() == session.selected.as_deref()
        && let Some(param) = primary
    {
        let message = session.adjust_wheel(param, if lines > 0.0 { 1 } else { -1 });
        if !message.is_empty() {
            campus.message = message;
        }
        input.wheel_used = true;
    }
    input.drag_active = drag.source.is_some() || drag.pending.is_some() || transform_gizmo.drag.is_some();

    // The palette, drag, wheel and keyboard paths edit the design directly while the inspector's
    // buttons go through `UiAction`, which saves on its own. Saving here too means a palette-only
    // edit (adding a wing, moving a fin) survives the next launch instead of only living in memory.
    // UI-only state (hover, search, collapse) must not rewrite the vehicle every frame.
    if saved_vehicle.as_ref() != Some(&session.vehicle) {
        *saved_vehicle = Some(session.vehicle.clone());
        if let Err(error) = missile::persist_missile(&session) {
            warn!("Could not save the vehicle: {error}");
        }
    }
}

/// Show free attachment points on the selected body even when no drag is active.
/// This makes valid mounting locations discoverable instead of hiding them until a drag starts.
pub(crate) fn draw_attachment_markers(session: Res<MissileSession>, campus: Res<CampusState>, mut gizmos: Gizmos) {
    if campus.view != CampusView::Missile {
        return;
    }
    let Some(selected) = session.selected.as_deref() else {
        return;
    };
    let Some(part) = session.vehicle.part(selected) else {
        return;
    };
    let hidden = session.hidden_parts();
    if part.spec.is_aero() || hidden.contains(selected) || session.is_locked(selected) {
        return;
    }
    for node in free_nodes(&session.vehicle, &hidden)
        .into_iter()
        .filter(|node| node.parent == selected)
    {
        let color = match node.node {
            NodeId::Top => Color::srgb(0.35, 0.90, 0.62),
            NodeId::Bottom => Color::srgb(0.95, 0.70, 0.30),
            NodeId::Radial(_) => Color::srgb(0.36, 0.84, 0.94),
        };
        gizmos.sphere(Isometry3d::from_translation(node.world), 0.13, color);
    }
}

/// Wire boxes around the hovered and selected parts. Gizmos keep the highlight independent of
/// the 3D material rebuild, so moving the pointer never respawns the vehicle meshes.
pub(crate) fn draw_selection_highlights(
    session: Res<MissileSession>,
    hover: Res<WorkshopHover>,
    campus: Res<CampusState>,
    mut gizmos: Gizmos,
) {
    if campus.view != CampusView::Missile {
        return;
    }
    let hidden = session.hidden_parts();
    for b in missile::layout(&session.vehicle) {
        if hidden.contains(&b.id) {
            continue;
        }
        let selected = session.is_selected(&b.id);
        let hovered = hover.part.as_deref() == Some(b.id.as_str());
        if !selected && !hovered {
            continue;
        }
        let center = missile::world_point(
            &session.vehicle,
            b.x as f64,
            (b.y_bottom + b.height * 0.5) as f64,
            b.z as f64,
        );
        let rotation = Quat::from_euler(
            EulerRot::XYZ,
            b.rotation_x_deg.to_radians(),
            (b.rotation_y_deg - b.angle).to_radians(),
            b.rotation_z_deg.to_radians(),
        );
        let scale = Vec3::new(b.radius * 2.0, b.height, b.radius * 2.0) * missile::VISUAL_SCALE * 1.04;
        let color = if hovered {
            Color::srgb(0.35, 0.95, 1.0)
        } else if session.edit_locked(&b.id) {
            Color::srgb(0.95, 0.35, 0.30)
        } else {
            Color::srgb(0.98, 0.68, 0.24)
        };
        gizmos.cube(
            Transform::from_translation(center)
                .with_rotation(rotation)
                .with_scale(scale),
            color,
        );
    }
}

/// Centre-of-gravity and centre-of-aerodynamics markers plus the acceptable middle band. Drawn
/// only in the workshop, and only when the design has at least one aerodynamic surface.
pub(crate) fn draw_aero_markers(session: Res<MissileSession>, campus: Res<CampusState>, mut gizmos: Gizmos) {
    if campus.view != CampusView::Missile {
        return;
    }
    let figures = session.vehicle.aero();
    if figures.surface_count == 0 {
        return;
    }
    let at = |y: f64| missile::world_point(&session.vehicle, 0.0, y, 0.0);
    let cg = at(figures.cg_y_m);
    let ca = at(figures.ca_y_m);
    let (low, high) = figures.stable_band_m();
    // Acceptable CA positions sit behind the CG: between cg - high and cg - low.
    gizmos.line(
        at(figures.cg_y_m - high),
        at(figures.cg_y_m - low),
        Color::srgba(0.45, 0.85, 0.55, 0.7),
    );
    gizmos.line(cg, ca, Color::srgba(0.85, 0.88, 0.92, 0.45));
    gizmos.sphere(Isometry3d::from_translation(cg), 0.22, Color::srgb(0.95, 0.82, 0.25));
    gizmos.cube(
        Transform::from_translation(ca).with_scale(Vec3::splat(0.30)),
        Color::srgb(0.30, 0.85, 0.95),
    );
}

/// A small isolated 3D scene renders the palette preview to a texture. Its camera and light only
/// see render layer 2, so the workshop's own model is never affected.
pub(crate) fn setup_palette_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let image = Image::new_target_texture(320, 200, TextureFormat::Rgba8Unorm, Some(TextureFormat::Rgba8UnormSrgb));
    let image = images.add(image);
    commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            clear_color: Color::srgba(0.025, 0.045, 0.060, 1.0).into(),
            ..default()
        },
        RenderTarget::Image(image.clone().into()),
        Transform::from_xyz(1.9, 1.3, 2.7).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
        RenderLayers::layer(2),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 5_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(2.0, 4.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(2),
    ));
    let root = commands
        .spawn((
            Transform::default(),
            Visibility::Hidden,
            PalettePreviewRoot,
            RenderLayers::layer(2),
        ))
        .id();
    commands.insert_resource(PalettePreview {
        image,
        current: None,
        root,
    });

    let valid_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.20, 0.92, 0.42, 0.34),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..default()
    });
    let invalid_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.95, 0.22, 0.18, 0.34),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        cull_mode: None,
        ..default()
    });
    let root = commands
        .spawn((Transform::default(), Visibility::Hidden, DragGhostRoot))
        .id();
    commands.insert_resource(DragGhost {
        root,
        child: None,
        current: None,
        valid_material,
        invalid_material,
    });
}

/// Transparent mesh of the dragged palette entry. It uses the real part dimensions so the preview
/// matches the silhouette the player is about to place.
fn ghost_mesh(spec: PartSpec) -> Mesh {
    let (height, radius) = omega_rules::assembly::dimensions(&spec);
    let (height, radius) = (height as f32, radius as f32);
    match spec {
        PartSpec::NoseCone { .. } => Cone::new(radius, height).into(),
        PartSpec::Wing { .. } | PartSpec::FinSet { .. } | PartSpec::ControlSurface { .. } => {
            Cuboid::new(radius * 2.0, 0.05, height).into()
        }
        _ => Cylinder::new(radius, height).into(),
    }
}

/// Move and recolour the placement ghost. The mesh is rebuilt only when the palette entry changes;
/// moving between a legal and an illegal mount only swaps the material handle.
pub(crate) fn update_drag_ghost(
    mut commands: Commands,
    mut ghost: ResMut<DragGhost>,
    drag: Res<DragState>,
    campus: Res<CampusState>,
    mut roots: Query<(&mut Transform, &mut Visibility), With<DragGhostRoot>>,
    mut materials: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Ok((mut transform, mut visibility)) = roots.single_mut() else {
        return;
    };
    let preview = (campus.view == CampusView::Missile)
        .then_some(drag.ghost.as_ref())
        .flatten();
    let Some(preview) = preview else {
        *visibility = Visibility::Hidden;
        return;
    };
    *visibility = Visibility::Inherited;
    transform.translation = preview.world;
    transform.rotation = preview.orientation;
    transform.scale = Vec3::splat(missile::VISUAL_SCALE);
    if ghost.current != Some(preview.template) {
        ghost.current = Some(preview.template);
        commands.entity(ghost.root).despawn_related::<Children>();
        let material = if preview.valid {
            ghost.valid_material.clone()
        } else {
            ghost.invalid_material.clone()
        };
        let child = commands
            .spawn((
                Mesh3d(meshes.add(ghost_mesh(preview.template.spec()))),
                MeshMaterial3d(material),
                Transform::default(),
            ))
            .id();
        commands.entity(ghost.root).add_child(child);
        ghost.child = Some(child);
    } else if let Some(child) = ghost.child
        && let Ok(mut material) = materials.get_mut(child)
    {
        let wanted = if preview.valid {
            ghost.valid_material.clone()
        } else {
            ghost.invalid_material.clone()
        };
        if material.0 != wanted {
            material.0 = wanted;
        }
    }
}

fn preview_part(template: PartTemplate) -> (Mesh, Color, f32) {
    let spec = template.spec();
    let (height, radius) = omega_rules::assembly::dimensions(&spec);
    let color = match spec.category() {
        PartCategory::Propulsion => Color::srgb(0.44, 0.46, 0.43),
        PartCategory::Airframe => Color::srgb(0.56, 0.51, 0.38),
        PartCategory::Aerodynamics => Color::srgb(0.38, 0.52, 0.66),
        PartCategory::Sensors => Color::srgb(0.34, 0.62, 0.64),
        PartCategory::Avionics => Color::srgb(0.42, 0.36, 0.58),
    };
    let (height, radius) = (height as f32, radius as f32);
    let mesh = match spec {
        PartSpec::NoseCone { .. } => Cone::new(radius, height).into(),
        PartSpec::Wing { .. } | PartSpec::FinSet { .. } | PartSpec::ControlSurface { .. } => {
            Cuboid::new(radius * 2.0, 0.05, height).into()
        }
        PartSpec::Adapter { .. } | PartSpec::LiquidEngine { .. } | PartSpec::AirBreathing { .. } => {
            Cylinder::new(radius, height).into()
        }
        _ => Cylinder::new(radius, height).into(),
    };
    let size = height.max(radius * 2.0).max(0.2);
    (mesh, color, 1.6 / size)
}

pub(crate) fn update_palette_preview(
    mut commands: Commands,
    time: Res<Time>,
    campus: Res<CampusState>,
    hover: Res<WorkshopHover>,
    mut preview: ResMut<PalettePreview>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut roots: Query<(&mut Transform, &mut Visibility), With<PalettePreviewRoot>>,
) {
    let Ok((mut transform, mut visibility)) = roots.single_mut() else {
        return;
    };
    let wanted = (campus.view == CampusView::Missile).then_some(hover.palette).flatten();
    let Some(template) = wanted else {
        *visibility = Visibility::Hidden;
        return;
    };
    *visibility = Visibility::Inherited;
    if preview.current != Some(template) {
        preview.current = Some(template);
        commands.entity(preview.root).despawn_related::<Children>();
        let (mesh, color, scale) = preview_part(template);
        let mesh = meshes.add(mesh);
        let material = materials.add(StandardMaterial {
            base_color: color,
            metallic: 0.25,
            perceptual_roughness: 0.48,
            cull_mode: None,
            ..default()
        });
        commands.entity(preview.root).with_children(|root| {
            root.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_scale(Vec3::splat(scale)),
                RenderLayers::layer(2),
            ));
        });
    }
    transform.rotate_y(time.delta_secs() * 0.8);
}

/// Hover help under the palette: what the part does, and the technology that still locks it.
pub(crate) fn update_palette_hint(
    hover: Res<WorkshopHover>,
    campus: Res<CampusState>,
    session: Res<MissileSession>,
    mut text: Query<&mut Text, With<PaletteHintText>>,
) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let wanted = match hover.palette {
        Some(template) => {
            let capability = template.spec().required_capability();
            let mut line = format!(
                "{} — {}",
                crate::ui_kit::fr(template.label()),
                missile::template_description(template)
            );
            if !campus.owned_capabilities().contains(capability) {
                line.push_str(&format!(
                    " | Recherche : {} (Centre de recherche)",
                    omega_rules::research::capability_label(capability)
                ));
            }
            if session.favorites.contains(&template) {
                line.push_str(" | Favori");
            }
            line
        }
        None => "Survolez une piece pour son apercu 3D et son role. / : rechercher.".to_string(),
    };
    if text.0 != wanted {
        text.0 = wanted;
    }
}

/// The palette builds its own buttons so they can carry `PalettePart` and be dragged; a plain
/// click still adds above the selection.
pub(crate) fn spawn_palette_button(parent: &mut ChildSpawnerCommands<'_>, template: PartTemplate, unlocked: bool) {
    let tone = if unlocked {
        crate::ui_kit::ButtonTone::Secondary
    } else {
        crate::ui_kit::ButtonTone::Disabled
    };
    parent
        .spawn((
            UiInteraction::default(),
            bevy::ui::FocusPolicy::Block,
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                min_height: Val::Px(28.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect {
                    left: Val::Px(3.0),
                    right: Val::Px(3.0),
                    top: Val::Px(5.0),
                    bottom: Val::Px(5.0),
                },
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(tone.background(UiInteraction::None)),
            tone,
            PalettePart(template),
        ))
        .with_children(|b| {
            crate::ui_kit::label(b, template.label(), 10.0, crate::ui_kit::TEXT);
        });
}

#[cfg(test)]
mod transform_gizmo_tests {
    use super::*;

    #[test]
    fn snapping_uses_three_translation_and_rotation_presets() {
        assert_eq!(snap_step(TransformMode::Translate, 0), 0.01);
        assert_eq!(snap_step(TransformMode::Translate, 1), 0.10);
        assert_eq!(snap_step(TransformMode::Translate, 2), 0.50);
        assert_eq!(snap_step(TransformMode::Rotate, 0), 1.0);
        assert_eq!(snap_step(TransformMode::Rotate, 1), 5.0);
        assert_eq!(snap_step(TransformMode::Rotate, 2), 15.0);
    }

    #[test]
    fn snapping_rounds_to_nearest_increment() {
        assert!((snap_amount(0.14, 0.10) - 0.10).abs() < 1e-9);
        assert!((snap_amount(-12.0, 5.0) - -10.0).abs() < 1e-9);
    }

    #[test]
    fn handle_hit_test_measures_distance_to_the_whole_arrow() {
        assert_eq!(
            distance_to_segment(Vec2::new(50.0, 8.0), Vec2::ZERO, Vec2::new(100.0, 0.0)),
            8.0
        );
        assert_eq!(
            distance_to_segment(Vec2::new(120.0, 0.0), Vec2::ZERO, Vec2::new(100.0, 0.0)),
            20.0
        );
    }

    #[test]
    fn an_adapter_is_picked_through_its_widest_section() {
        let mut vehicle = Vehicle::default();
        vehicle.parts.push(omega_rules::PlacedPart {
            id: "adapter".to_string(),
            spec: omega_rules::PartSpec::Adapter {
                length_m: 0.5,
                diameter_m: 0.6,
                exit_diameter_m: 0.3,
                shape: omega_rules::AdapterShape::Stepped,
            },
            attach: None,
            symmetry: None,
            size_scale: 1.0,
        });
        let pose = vehicle.poses().into_iter().next().unwrap();
        let center = missile::world_point(&vehicle, pose.x_m, pose.y_m, pose.z_m);
        assert_eq!(
            picked_part(&vehicle, center + Vec3::X * 5.0, Vec3::NEG_X).as_deref(),
            Some("adapter")
        );
        let just_outside = center + Vec3::X * 5.0 + Vec3::Z * (0.3 * missile::VISUAL_SCALE + 0.05);
        assert!(picked_part(&vehicle, just_outside, Vec3::NEG_X).is_none());
    }
}
