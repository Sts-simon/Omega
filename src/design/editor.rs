use super::{Assembly, DesignSession, MaterialKind, Part, PartKind, PolygonSketchSession};
use crate::campus::{CampusState, CampusView};
use crate::sequence::SequenceWorkspace;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

const ASSEMBLY_ANCHOR: Vec3 = Vec3::new(0.0, 0.55, 18.0);

#[derive(Component)]
pub struct PartVisual {
    id: String,
}

#[derive(Component)]
pub struct ConnectionVisual;

#[derive(Component)]
pub struct EditorHud;

#[derive(Resource, Default)]
pub struct RenderedRevision {
    revision: u64,
    visible: bool,
}

#[derive(Resource)]
pub struct EditorAssets {
    cube: Handle<Mesh>,
    cylinder: Handle<Mesh>,
    gizmo_cone: Handle<Mesh>,
    selected: Handle<StandardMaterial>,
    aluminum: Handle<StandardMaterial>,
    steel: Handle<StandardMaterial>,
    polymer: Handle<StandardMaterial>,
    composite: Handle<StandardMaterial>,
    connection: Handle<StandardMaterial>,
    gizmo_x: Handle<StandardMaterial>,
    gizmo_y: Handle<StandardMaterial>,
    gizmo_z: Handle<StandardMaterial>,
    gizmo_hover: Handle<StandardMaterial>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GizmoMode {
    #[default]
    Translate,
    Rotate,
}

#[derive(Resource)]
pub struct GizmoState {
    pub mode: GizmoMode,
    pub pointer_captured: bool,
    drag: Option<GizmoDrag>,
    hovered_axis: Option<usize>,
    rendered_selection: Option<String>,
    rendered_mode: GizmoMode,
    rendered_visible: bool,
    rendered_dimensions: [f64; 3],
}

impl Default for GizmoState {
    fn default() -> Self {
        Self {
            mode: GizmoMode::Translate,
            pointer_captured: false,
            drag: None,
            hovered_axis: None,
            rendered_selection: None,
            rendered_mode: GizmoMode::Translate,
            rendered_visible: false,
            rendered_dimensions: [0.0; 3],
        }
    }
}

impl GizmoState {
    pub fn set_mode(&mut self, mode: GizmoMode) {
        if self.mode != mode {
            self.mode = mode;
            self.drag = None;
            self.pointer_captured = false;
            self.hovered_axis = None;
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum GizmoDrag {
    Translate {
        axis: usize,
        axis_world: Vec3,
        origin: Vec3,
        view_forward: Vec3,
        start_parameter: f32,
        start_position_m: [f64; 3],
    },
    Rotate {
        axis: usize,
        axis_world: Vec3,
        origin: Vec3,
        last_vector: Vec3,
        accumulated_angle: f32,
        base_rotation_rad: [f64; 3],
    },
}

#[derive(Component)]
pub(crate) struct GizmoVisual;

#[derive(Component)]
pub(crate) struct GizmoAxisVisual(usize);

#[derive(Clone, Copy, Debug)]
enum GizmoPrimitive {
    AxisShaft { axis: usize, length: f32, width: f32 },
    AxisTip { axis: usize, length: f32, size: f32 },
    RingSegment { axis: usize, angle: f32, radius: f32, width: f32 },
}

#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct GizmoPrimitiveVisual(GizmoPrimitive);

pub fn setup_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("AEROLAB | Assembly editor\nLoading editor state..."),
        TextFont {
            font_size: FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::srgb(0.93, 0.96, 0.98)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(78.0),
            left: Val::Px(254.0),
            max_width: Val::Px(360.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        Interaction::None,
        BackgroundColor(Color::srgba(0.025, 0.04, 0.055, 0.88)),
        EditorHud,
    ));
}

pub fn initialize_editor(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(RenderedRevision::default());
    commands.insert_resource(EditorAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        cylinder: meshes.add(Cylinder::new(0.5, 1.0)),
        gizmo_cone: meshes.add(Cone::new(0.65, 1.5)),
        selected: materials.add(Color::srgb(1.0, 0.72, 0.20)),
        aluminum: materials.add(Color::srgb(0.58, 0.67, 0.72)),
        steel: materials.add(Color::srgb(0.34, 0.39, 0.44)),
        polymer: materials.add(Color::srgb(0.22, 0.55, 0.62)),
        composite: materials.add(Color::srgb(0.18, 0.22, 0.25)),
        connection: materials.add(Color::srgb(0.12, 0.88, 0.48)),
        gizmo_x: materials.add(Color::srgb(0.96, 0.24, 0.22)),
        gizmo_y: materials.add(Color::srgb(0.30, 0.88, 0.42)),
        gizmo_z: materials.add(Color::srgb(0.24, 0.52, 0.98)),
        gizmo_hover: materials.add(Color::srgb(1.0, 0.82, 0.20)),
    });
    commands.insert_resource(GizmoState::default());
}

pub fn handle_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    campus: Res<CampusState>,
    sequence: Res<SequenceWorkspace>,
    sketch: Res<PolygonSketchSession>,
    mut session: ResMut<DesignSession>,
    mut gizmo: ResMut<GizmoState>,
    inspector: Res<super::InspectorState>,
) {
    if campus.view != CampusView::Construction || campus.profile_editor_open || sequence.graph_is_open() || sketch.active || gizmo.pointer_captured || inspector.is_editing() {
        return;
    }
    let control = control_down(&keys);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);

    if !control && keys.just_pressed(KeyCode::KeyT) {
        session.finish_gizmo_edit();
        gizmo.set_mode(GizmoMode::Translate);
    }
    if !control && keys.just_pressed(KeyCode::KeyR) {
        session.finish_gizmo_edit();
        gizmo.set_mode(GizmoMode::Rotate);
    }

    if control && keys.just_pressed(KeyCode::KeyZ) {
        if session.undo() { info!("Undo applied."); } else { info!("Nothing to undo."); }
    }
    if control && keys.just_pressed(KeyCode::KeyY) {
        if session.redo() { info!("Redo applied."); } else { info!("Nothing to redo."); }
    }

    if !control && keys.just_pressed(KeyCode::Tab) {
        let reverse = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        session.select_next(reverse);
    }

    if !control && keys.just_pressed(KeyCode::Digit1) {
        session.add_part(PartKind::Panel);
    }
    if !control && keys.just_pressed(KeyCode::Digit2) {
        session.add_part(PartKind::Beam);
    }
    if !control && keys.just_pressed(KeyCode::Digit3) {
        session.add_part(PartKind::Enclosure);
    }
    if !control && keys.just_pressed(KeyCode::Digit4) {
        session.add_part(PartKind::Platform);
    }
    if !control && keys.just_pressed(KeyCode::Digit5) {
        session.add_part(PartKind::Cylinder);
    }
    if !control && (keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace)) {
        session.remove_selected();
    }

    if !control && keys.just_pressed(KeyCode::KeyP) {
        if shift {
            let removed = session.disconnect_selected();
            info!("Removed {removed} connection(s) from selected component.");
        } else {
            match session.connect_selected_to_nearest() {
                Ok(id) => info!("Created assembly relationship {id}."),
                Err(message) => info!("Connection not created: {message}"),
            }
        }
    }

    let move_step = if alt { 0.01 } else { 0.1 };
    let mut movement = [0.0_f64; 3];
    if !control && keys.just_pressed(KeyCode::KeyJ) { movement[0] -= move_step; }
    if !control && keys.just_pressed(KeyCode::KeyL) { movement[0] += move_step; }
    if !control && keys.just_pressed(KeyCode::KeyI) { movement[2] -= move_step; }
    if !control && keys.just_pressed(KeyCode::KeyK) { movement[2] += move_step; }
    if !control && keys.just_pressed(KeyCode::KeyU) { movement[1] += move_step; }
    if !control && keys.just_pressed(KeyCode::KeyO) { movement[1] -= move_step; }
    if movement.iter().any(|value| *value != 0.0) {
        session.move_selected(movement);
    }

    if !control && keys.just_pressed(KeyCode::BracketLeft) {
        session.resize_selected(0.95);
    }
    if !control && keys.just_pressed(KeyCode::BracketRight) {
        session.resize_selected(1.05);
    }
    if !control && keys.just_pressed(KeyCode::KeyM) {
        session.cycle_selected_material();
    }

    let rotation_magnitude = (if alt { 1.0_f64 } else { 15.0_f64 }).to_radians();
    let rotation_step = if shift { -rotation_magnitude } else { rotation_magnitude };
    if !control && keys.just_pressed(KeyCode::KeyX) {
        session.rotate_selected([rotation_step, 0.0, 0.0]);
    }
    if !control && keys.just_pressed(KeyCode::KeyY) {
        session.rotate_selected([0.0, rotation_step, 0.0]);
    }
    if !control && keys.just_pressed(KeyCode::KeyZ) {
        session.rotate_selected([0.0, 0.0, rotation_step]);
    }

    if !control && keys.just_pressed(KeyCode::KeyG) {
        session.toggle_snap_grid();
    }

    // Per-axis dimension edits. F4/F5 = X (F6 opens the sequencer graph),
    // F7/F8 = Y, F9/F10 = Z.
    for (axis, smaller, larger) in [
        (0, KeyCode::F5, KeyCode::F4),
        (1, KeyCode::F7, KeyCode::F8),
        (2, KeyCode::F9, KeyCode::F10),
    ] {
        if !control && keys.just_pressed(smaller) {
            if alt { session.adjust_selected_dimension(axis, -0.01); }
            else { session.resize_selected_axis(axis, 0.95); }
        }
        if !control && keys.just_pressed(larger) {
            if alt { session.adjust_selected_dimension(axis, 0.01); }
            else { session.resize_selected_axis(axis, 1.05); }
        }
    }

    if keys.just_pressed(KeyCode::KeyS) && control_down(&keys) {
        match session.assembly.to_bytes().and_then(|bytes| crate::save::write(crate::save::SaveFile::Assembly, &bytes)) {
            Ok(()) => info!("Assembly saved to the save slot"),
            Err(error) => error!("Could not save assembly: {error}"),
        }
    }

    if keys.just_pressed(KeyCode::KeyL) && control_down(&keys) {
        let loaded = crate::save::read(crate::save::SaveFile::Assembly)
            .and_then(|bytes| bytes.ok_or_else(|| "no saved assembly found".to_string()))
            .and_then(|bytes| Assembly::from_bytes(&bytes));
        match loaded {
            Ok(assembly) => {
                session.replace_assembly(assembly);
                info!("Assembly loaded from the save slot");
            }
            Err(error) => warn!("Could not load assembly: {error}"),
        }
    }
}

fn control_down(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight)
}

pub fn pick_part_with_mouse(
    mouse: Res<ButtonInput<MouseButton>>,
    campus: Res<CampusState>,
    sequence: Res<SequenceWorkspace>,
    sketch: Res<PolygonSketchSession>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    visuals: Query<(&GlobalTransform, &PartVisual)>,
    mut session: ResMut<DesignSession>,
    ui_buttons: Query<&Interaction>,
    gizmo: Res<GizmoState>,
    inspector: Res<super::InspectorState>,
) {
    if gizmo.pointer_captured || inspector.is_editing() || campus.is_changed()
        || campus.view != CampusView::Construction
        || campus.profile_editor_open
        || sequence.graph_is_open()
        || sketch.active
        || !mouse.just_pressed(MouseButton::Left)
        || ui_buttons.iter().any(|interaction| *interaction != Interaction::None)
    {
        return;
    }
    let Some(cursor) = window.cursor_position() else { return };
    let (camera, camera_transform) = *camera;
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor) else { return };
    let origin = ray.origin;
    let direction = ray.direction.as_vec3();

    let mut closest: Option<(f32, String)> = None;
    for (transform, visual) in &visuals {
        // Transform the ray into the component's local coordinates. This keeps picking
        // accurate after rotation and non-uniform scaling.
        let inverse = transform.to_matrix().inverse();
        let local_origin = inverse.transform_point3(origin);
        let local_direction = inverse.transform_vector3(direction);
        if let Some(distance) = ray_aabb(
            local_origin,
            local_direction,
            Vec3::splat(-0.5),
            Vec3::splat(0.5),
        ) {
            if closest.as_ref().is_none_or(|(best, _)| distance < *best) {
                closest = Some((distance, visual.id.clone()));
            }
        }
    }

    if let Some((_, id)) = closest {
        if session.selected_part_id.as_deref() != Some(id.as_str()) {
            session.selected_part_id = Some(id);
            session.touch();
        }
    }
}

/// Handles mouse hover, picking and dragging of the selected component's 3D transform gizmo.
pub fn handle_gizmo_input(
    mouse: Res<ButtonInput<MouseButton>>,
    campus: Res<CampusState>,
    sequence: Res<SequenceWorkspace>,
    sketch: Res<PolygonSketchSession>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    ui_interactions: Query<&Interaction>,
    mut gizmo: ResMut<GizmoState>,
    mut session: ResMut<DesignSession>,
    inspector: Res<super::InspectorState>,
) {
    if inspector.is_editing() {
        gizmo.hovered_axis = None;
        return;
    }
    let active_view = campus.view == CampusView::Construction
        && !campus.profile_editor_open
        && !sequence.graph_is_open()
        && !sketch.active;

    if !active_view {
        gizmo.drag = None;
        gizmo.pointer_captured = false;
        gizmo.hovered_axis = None;
        session.finish_gizmo_edit();
        return;
    }

    if let Some(active_drag) = gizmo.drag {
        let mouse_held = mouse.pressed(MouseButton::Left);
        let (camera_ref, camera_transform_ref) = *camera;
        if let Some((ray_origin, ray_direction)) = current_cursor_ray(*window, camera_ref, camera_transform_ref) {
            match active_drag {
                GizmoDrag::Translate {
                    axis_world, origin, view_forward, start_parameter, start_position_m, ..
                } => {
                    if let Some(parameter) = translation_parameter(ray_origin, ray_direction, origin, axis_world, view_forward) {
                        let delta = parameter - start_parameter;
                        let target = [
                            start_position_m[0] + axis_world.x as f64 * delta as f64,
                            start_position_m[1] + axis_world.y as f64 * delta as f64,
                            start_position_m[2] + axis_world.z as f64 * delta as f64,
                        ];
                        session.set_selected_gizmo_position(target);
                    }
                }
                GizmoDrag::Rotate {
                    axis, axis_world, origin, last_vector, accumulated_angle, base_rotation_rad,
                } => {
                    if let Some(hit) = ray_plane_hit(ray_origin, ray_direction, origin, axis_world) {
                        let current_vector = (hit - origin).normalize_or_zero();
                        if current_vector.length_squared() > 0.5 {
                            let signed_sine = axis_world.dot(last_vector.cross(current_vector));
                            let cosine = last_vector.dot(current_vector).clamp(-1.0, 1.0);
                            let incremental_angle = signed_sine.atan2(cosine);
                            let total_angle = accumulated_angle + incremental_angle;
                            if incremental_angle.abs() > 1.0e-6 {
                                session.set_selected_gizmo_rotation(axis, base_rotation_rad, total_angle as f64);
                                gizmo.drag = Some(GizmoDrag::Rotate {
                                    axis, axis_world, origin, last_vector: current_vector,
                                    accumulated_angle: total_angle, base_rotation_rad,
                                });
                            }
                        }
                    }
                }
            }
        }
        // Apply the final cursor position before closing the transaction. This avoids
        // dropping the last few pixels of a fast drag on the mouse-release frame.
        if !mouse_held {
            gizmo.drag = None;
            gizmo.pointer_captured = false;
            gizmo.hovered_axis = None;
            session.finish_gizmo_edit();
        }
        return;
    }

    gizmo.pointer_captured = false;
    if ui_interactions.iter().any(|interaction| *interaction != Interaction::None) {
        gizmo.hovered_axis = None;
        return;
    }

    let Some(index) = session.selected_index() else {
        gizmo.hovered_axis = None;
        return;
    };
    let part = &session.assembly.parts[index];
    let origin = Vec3::from_array(part.position_m.map(|value| value as f32)) + ASSEMBLY_ANCHOR;
    let rotation = part_rotation(part);
    let (camera_ref, camera_transform_ref) = *camera;
    let Some((ray_origin, ray_direction)) = current_cursor_ray(*window, camera_ref, camera_transform_ref) else {
        gizmo.hovered_axis = None;
        return;
    };
    let camera_forward = camera_transform_ref.forward().as_vec3();
    let view_distance = camera_transform_ref.translation().distance(origin);
    let scale = gizmo_scale_factor(view_distance);
    let (axis_length, ring_radius) = gizmo_dimensions(part);

    let hit = hit_test_gizmo(
        ray_origin,
        ray_direction,
        camera_forward,
        origin,
        rotation,
        axis_length * scale,
        ring_radius * scale,
        part.position_m,
        part.rotation_rad,
        gizmo.mode,
    );
    gizmo.hovered_axis = hit.map(|(axis, _)| axis);

    if mouse.just_pressed(MouseButton::Left) {
        if let Some((_axis, drag)) = hit {
            gizmo.drag = Some(drag);
            gizmo.pointer_captured = true;
        }
    }
}

/// Creates/removes gizmo meshes when the selected component, mode or dimensions change.
pub fn sync_gizmo_visuals(
    mut commands: Commands,
    session: Res<DesignSession>,
    campus: Res<CampusState>,
    sequence: Res<SequenceWorkspace>,
    sketch: Res<PolygonSketchSession>,
    mut gizmo: ResMut<GizmoState>,
    old_visuals: Query<Entity, With<GizmoVisual>>,
    assets: Res<EditorAssets>,
) {
    let visible = campus.view == CampusView::Construction
        && !campus.profile_editor_open
        && !sequence.graph_is_open()
        && !sketch.active
        && session.selected_index().is_some();
    let selected = session.selected_part_id.clone();
    let dimensions = session.selected_index()
        .map(|index| session.assembly.parts[index].dimensions_m)
        .unwrap_or([0.0; 3]);
    let changed = gizmo.rendered_visible != visible
        || gizmo.rendered_selection != selected
        || gizmo.rendered_mode != gizmo.mode
        || gizmo.rendered_dimensions != dimensions;
    if !changed { return; }

    for entity in &old_visuals {
        commands.entity(entity).despawn();
    }
    gizmo.rendered_visible = visible;
    gizmo.rendered_selection = selected;
    gizmo.rendered_mode = gizmo.mode;
    gizmo.rendered_dimensions = dimensions;

    if !visible { return; }
    let Some(index) = session.selected_index() else { return };
    let part = &session.assembly.parts[index];
    let (axis_length, ring_radius) = gizmo_dimensions(part);

    match gizmo.mode {
        GizmoMode::Translate => {
            for axis in 0..3 {
                let width = (axis_length * 0.035).clamp(0.025, 0.065);
                spawn_gizmo_primitive(
                    &mut commands, &assets, axis,
                    GizmoPrimitive::AxisShaft { axis, length: axis_length, width },
                );
                spawn_gizmo_primitive(
                    &mut commands, &assets, axis,
                    GizmoPrimitive::AxisTip { axis, length: axis_length, size: width * 3.2 },
                );
            }
        }
        GizmoMode::Rotate => {
            const SEGMENTS: usize = 48;
            for axis in 0..3 {
                for segment in 0..SEGMENTS {
                    let angle = std::f32::consts::TAU * segment as f32 / SEGMENTS as f32;
                    spawn_gizmo_primitive(
                        &mut commands, &assets, axis,
                        GizmoPrimitive::RingSegment {
                            axis, angle, radius: ring_radius,
                            width: (ring_radius * 0.018).clamp(0.018, 0.04),
                        },
                    );
                }
            }
        }
    }
}

/// Updates the already-created gizmo meshes as the selected component moves or rotates.
pub fn update_gizmo_visuals(
    session: Res<DesignSession>,
    campus: Res<CampusState>,
    gizmo: Res<GizmoState>,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    assets: Res<EditorAssets>,
    mut visuals: Query<(
        &GizmoPrimitiveVisual,
        &GizmoAxisVisual,
        &mut Transform,
        &mut MeshMaterial3d<StandardMaterial>,
    ), With<GizmoVisual>>,
) {
    if campus.view != CampusView::Construction { return; }
    let Some(index) = session.selected_index() else { return };
    let part = &session.assembly.parts[index];
    let origin = Vec3::from_array(part.position_m.map(|value| value as f32)) + ASSEMBLY_ANCHOR;
    let rotation = part_rotation(part);
    let scale = gizmo_scale_factor(camera.translation().distance(origin));

    for (primitive, axis, mut transform, mut material) in &mut visuals {
        *transform = gizmo_primitive_transform(origin, rotation, primitive.0, scale);
        material.0 = if gizmo.hovered_axis == Some(axis.0) {
            assets.gizmo_hover.clone()
        } else {
            match axis.0 {
                0 => assets.gizmo_x.clone(),
                1 => assets.gizmo_y.clone(),
                _ => assets.gizmo_z.clone(),
            }
        };
    }
}

fn spawn_gizmo_primitive(
    commands: &mut Commands,
    assets: &EditorAssets,
    axis: usize,
    primitive: GizmoPrimitive,
) {
    let mesh = match primitive {
        GizmoPrimitive::AxisTip { .. } => assets.gizmo_cone.clone(),
        _ => assets.cylinder.clone(),
    };
    let transform = Transform::default();
    let material = match axis {
        0 => assets.gizmo_x.clone(),
        1 => assets.gizmo_y.clone(),
        _ => assets.gizmo_z.clone(),
    };
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        transform,
        GizmoVisual,
        GizmoAxisVisual(axis),
        GizmoPrimitiveVisual(primitive),
    ));
}

fn gizmo_primitive_transform(
    origin: Vec3,
    part_rotation: Quat,
    primitive: GizmoPrimitive,
    view_scale: f32,
) -> Transform {
    match primitive {
        GizmoPrimitive::AxisShaft { axis, length, width } => {
            let length = length * view_scale;
            let width = width * view_scale;
            let local_axis = axis_vector(axis);
            let direction = part_rotation * local_axis;
            let rotation = part_rotation * Quat::from_rotation_arc(Vec3::Y, local_axis);
            Transform::from_translation(origin + direction * (length * 0.5))
                .with_rotation(rotation)
                .with_scale(Vec3::new(width * 2.0, length, width * 2.0))
        }
        GizmoPrimitive::AxisTip { axis, length, size } => {
            let direction = part_rotation * axis_vector(axis);
            let length = length * view_scale;
            let size = size * view_scale;
            let rotation = part_rotation * Quat::from_rotation_arc(Vec3::Y, axis_vector(axis));
            // The cone's apex points along +Y. Place it just beyond the shaft end
            // while its base overlaps the shaft slightly.
            Transform::from_translation(origin + direction * (length - size * 0.40))
                .with_rotation(rotation)
                .with_scale(Vec3::splat(size))
        }
        GizmoPrimitive::RingSegment { axis, angle, radius, width } => {
            let radius = radius * view_scale;
            let width = width * view_scale;
            let a = ring_point(axis, angle, radius);
            let b = ring_point(axis, angle + std::f32::consts::TAU / 48.0, radius);
            let delta = b - a;
            let local_rotation = Quat::from_rotation_arc(Vec3::Y, delta.normalize_or_zero());
            Transform::from_translation(origin + part_rotation * ((a + b) * 0.5))
                .with_rotation(part_rotation * local_rotation)
                .with_scale(Vec3::new(width * 2.0, delta.length(), width * 2.0))
        }
    }
}

fn gizmo_dimensions(part: &Part) -> (f32, f32) {
    let maximum = part.dimensions_m.iter().copied().fold(0.0_f64, f64::max) as f32;
    ((maximum * 0.85 + 0.7).clamp(0.9, 2.8), (maximum * 0.65 + 0.45).clamp(0.75, 2.2))
}

/// Approximate screen-constant gizmo sizing for the fixed-FOV workshop camera.
fn gizmo_scale_factor(view_distance: f32) -> f32 {
    if !view_distance.is_finite() { return 1.0; }
    (view_distance / 24.0).clamp(0.65, 4.8)
}

fn part_rotation(part: &Part) -> Quat {
    Quat::from_euler(
        EulerRot::XYZ,
        part.rotation_rad[0] as f32,
        part.rotation_rad[1] as f32,
        part.rotation_rad[2] as f32,
    )
}

fn axis_vector(axis: usize) -> Vec3 {
    match axis { 0 => Vec3::X, 1 => Vec3::Y, _ => Vec3::Z }
}

fn ring_point(axis: usize, angle: f32, radius: f32) -> Vec3 {
    let c = angle.cos() * radius;
    let s = angle.sin() * radius;
    match axis {
        0 => Vec3::new(0.0, c, s),
        1 => Vec3::new(c, 0.0, s),
        _ => Vec3::new(c, s, 0.0),
    }
}

fn current_cursor_ray(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
) -> Option<(Vec3, Vec3)> {
    let cursor = window.cursor_position()?;
    let ray = camera.viewport_to_world(camera_transform, cursor).ok()?;
    Some((ray.origin, ray.direction.as_vec3()))
}

fn ray_plane_hit(ray_origin: Vec3, ray_direction: Vec3, plane_origin: Vec3, plane_normal: Vec3) -> Option<Vec3> {
    let denominator = ray_direction.dot(plane_normal);
    if denominator.abs() < 1.0e-5 { return None; }
    let distance = (plane_origin - ray_origin).dot(plane_normal) / denominator;
    if distance < 0.0 { return None; }
    Some(ray_origin + ray_direction * distance)
}

fn translation_parameter(
    ray_origin: Vec3,
    ray_direction: Vec3,
    axis_origin: Vec3,
    axis: Vec3,
    view_forward: Vec3,
) -> Option<f32> {
    // Intersect the cursor ray with a camera-facing plane through the gizmo origin,
    // then project onto the visible part of the axis. An axis aimed directly at the
    // camera has no stable screen-space drag and is intentionally not pickable.
    let view_normal = view_forward.normalize_or_zero();
    if view_normal.length_squared() < 0.5 { return None; }
    let hit = ray_plane_hit(ray_origin, ray_direction, axis_origin, view_normal)?;
    let projected_axis = axis - view_normal * axis.dot(view_normal);
    let projected_length_squared = projected_axis.length_squared();
    if projected_length_squared < 1.0e-4 { return None; }
    Some((hit - axis_origin).dot(projected_axis) / projected_length_squared)
}

fn hit_test_gizmo(
    ray_origin: Vec3,
    ray_direction: Vec3,
    view_forward: Vec3,
    origin: Vec3,
    part_rotation: Quat,
    axis_length: f32,
    ring_radius: f32,
    base_position_m: [f64; 3],
    base_rotation_rad: [f64; 3],
    mode: GizmoMode,
) -> Option<(usize, GizmoDrag)> {
    match mode {
        GizmoMode::Translate => {
            let mut best: Option<(f32, usize, f32, Vec3)> = None;
            for axis in 0..3 {
                let axis_world = (part_rotation * axis_vector(axis)).normalize_or_zero();
                let Some(parameter) = translation_parameter(
                    ray_origin, ray_direction, origin, axis_world, view_forward,
                ) else { continue };
                if !(0.0..=axis_length).contains(&parameter) { continue; }
                let view_normal = view_forward.normalize_or_zero();
                let Some(hit) = ray_plane_hit(ray_origin, ray_direction, origin, view_normal) else { continue };
                let projected_axis = axis_world - view_normal * axis_world.dot(view_normal);
                let closest = origin + projected_axis * parameter;
                let distance = (hit - closest).length();
                if distance <= (axis_length * 0.085).max(0.11)
                    && best.as_ref().is_none_or(|(current, ..)| distance < *current)
                {
                    best = Some((distance, axis, parameter, axis_world));
                }
            }
            best.map(|(_, axis, parameter, axis_world)| (
                axis,
                GizmoDrag::Translate {
                    axis, axis_world, origin, view_forward, start_parameter: parameter,
                    start_position_m: base_position_m,
                },
            ))
        }
        GizmoMode::Rotate => {
            let tolerance = (ring_radius * 0.14).max(0.10);
            let mut best: Option<(f32, usize, Vec3)> = None;
            for axis in 0..3 {
                let axis_world = (part_rotation * axis_vector(axis)).normalize_or_zero();
                let Some(hit) = ray_plane_hit(ray_origin, ray_direction, origin, axis_world) else { continue };
                let radial = hit - origin;
                let radial_length = radial.length();
                let error = (radial_length - ring_radius).abs();
                if radial_length > 1.0e-5 && error <= tolerance
                    && best.as_ref().is_none_or(|(current, ..)| error < *current)
                {
                    best = Some((error, axis, radial / radial_length));
                }
            }
            best.map(|(_, axis, last_vector)| {
                let axis_world = (part_rotation * axis_vector(axis)).normalize_or_zero();
                (axis, GizmoDrag::Rotate {
                    axis, axis_world, origin, last_vector, accumulated_angle: 0.0, base_rotation_rad,
                })
            })
        }
    }
}

fn ray_aabb(origin: Vec3, direction: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let mut near = f32::NEG_INFINITY;
    let mut far = f32::INFINITY;
    for axis in 0..3 {
        let o = origin[axis];
        let d = direction[axis];
        if d.abs() < 1e-6 {
            if o < min[axis] || o > max[axis] {
                return None;
            }
            continue;
        }
        let mut t1 = (min[axis] - o) / d;
        let mut t2 = (max[axis] - o) / d;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        near = near.max(t1);
        far = far.min(t2);
        if near > far {
            return None;
        }
    }
    if far < 0.0 { None } else { Some(near.max(0.0)) }
}

pub fn sync_part_visuals(
    mut commands: Commands,
    session: Res<DesignSession>,
    campus: Res<CampusState>,
    mut rendered: ResMut<RenderedRevision>,
    old_visuals: Query<Entity, Or<(With<PartVisual>, With<ConnectionVisual>)>>,
    assets: Res<EditorAssets>,
) {
    let visible = campus.view == CampusView::Construction;
    if rendered.revision == session.revision && rendered.visible == visible {
        return;
    }

    for entity in &old_visuals {
        commands.entity(entity).despawn();
    }

    if visible {
        for part in &session.assembly.parts {
            spawn_part(
                &mut commands,
                &assets,
                part,
                session.selected_part_id.as_deref() == Some(part.id.as_str()),
            );
        }
        for connection in &session.assembly.connections {
            let parent = session.assembly.parts.iter()
                .find(|part| part.id == connection.parent_part_id);
            let child = session.assembly.parts.iter()
                .find(|part| part.id == connection.child_part_id);
            if let (Some(parent), Some(child)) = (parent, child) {
                spawn_connection(&mut commands, &assets, parent.position_m, child.position_m);
            }
        }
    }
    rendered.revision = session.revision;
    rendered.visible = visible;
}

fn spawn_connection(
    commands: &mut Commands,
    assets: &EditorAssets,
    parent_position: [f64; 3],
    child_position: [f64; 3],
) {
    let start = Vec3::from_array(parent_position.map(|value| value as f32)) + ASSEMBLY_ANCHOR;
    let end = Vec3::from_array(child_position.map(|value| value as f32)) + ASSEMBLY_ANCHOR;
    let delta = end - start;
    let length = delta.length();
    if length <= 1e-4 { return; }

    let rotation = Quat::from_rotation_arc(Vec3::Y, delta / length);
    commands.spawn((
        Mesh3d(assets.cube.clone()),
        MeshMaterial3d(assets.connection.clone()),
        Transform::from_translation((start + end) * 0.5)
            .with_rotation(rotation)
            .with_scale(Vec3::new(0.025, length, 0.025)),
        ConnectionVisual,
    ));
}

fn spawn_part(
    commands: &mut Commands,
    assets: &EditorAssets,
    part: &Part,
    selected: bool,
) {
    let mesh = match &part.kind {
        PartKind::Cylinder => assets.cylinder.clone(),
        _ => assets.cube.clone(),
    };
    let material = if selected {
        assets.selected.clone()
    } else {
        match &part.material {
            MaterialKind::Aluminum => assets.aluminum.clone(),
            MaterialKind::Steel => assets.steel.clone(),
            MaterialKind::Polymer => assets.polymer.clone(),
            MaterialKind::Composite => assets.composite.clone(),
        }
    };
    let position = Vec3::from_array(part.position_m.map(|value| value as f32)) + ASSEMBLY_ANCHOR;
    let scale = Vec3::from_array(part.dimensions_m.map(|value| value as f32));
    let rotation = Quat::from_euler(
        EulerRot::XYZ,
        part.rotation_rad[0] as f32,
        part.rotation_rad[1] as f32,
        part.rotation_rad[2] as f32,
    );

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(position).with_rotation(rotation).with_scale(scale),
        PartVisual { id: part.id.clone() },
    ));
}

pub fn update_hud(
    session: Res<DesignSession>,
    campus: Res<CampusState>,
    sketch: Res<PolygonSketchSession>,
    gizmo: Res<GizmoState>,
    revisions: Res<super::RevisionLog>,
    ui: Res<crate::ui_kit::UiState>,
    mut hud: Query<(&mut Text, &mut Node), With<EditorHud>>,
) {
    let Ok((mut text, mut node)) = hud.single_mut() else { return };
    node.display = if campus.view == CampusView::Construction && !campus.profile_editor_open && !sketch.active { Display::Flex } else { Display::None };
    if campus.view != CampusView::Construction || campus.profile_editor_open || sketch.active {
        return;
    }
    let selected = session.selected_index().map(|index| &session.assembly.parts[index]);
    let selected_summary = if let Some(part) = selected {
        let parent = session.assembly.connections.iter()
            .find(|connection| connection.child_part_id == part.id)
            .map(|connection| connection.parent_part_id.as_str())
            .unwrap_or("none");
        let child_count = session.assembly.connections.iter()
            .filter(|connection| connection.parent_part_id == part.id)
            .count();
        format!(
            "Selected: {} [{}]\nType: {:?} | Material: {:?}\nSize: {:.2} x {:.2} x {:.2} m\nPosition: {:.2}, {:.2}, {:.2} m\nRotation: {:.1} deg, {:.1} deg, {:.1} deg\nEstimated mass: {:.2} kg\nParent: {} | Child links: {}",
            part.label,
            part.id,
            part.kind,
            part.material,
            part.dimensions_m[0], part.dimensions_m[1], part.dimensions_m[2],
            part.position_m[0], part.position_m[1], part.position_m[2],
            part.rotation_rad[0].to_degrees(), part.rotation_rad[1].to_degrees(), part.rotation_rad[2].to_degrees(),
            part.estimated_mass_kg(),
            parent,
            child_count,
        )
    } else {
        "No component selected".to_string()
    };

    text.0 = format!(
        "WORKSHOP INSPECTOR\n\n{}\n\nAssembly: {}\n{} components | {} links | {:.2} kg total\nValidation: {} | Grid snap: {}\nGizmo: {}\nHistory: {} undo step(s) | Undo {} | Redo {}\n\n{}\n\n{}",
        selected_summary,
        session.assembly.name,
        session.assembly.parts.len(),
        session.assembly.connections.len(),
        session.assembly.estimated_mass_kg(),
        if session.assembly.validate().is_ok() { "VALID" } else { "CHECK REQUIRED" },
        if session.snap_grid { "ON" } else { "OFF" },
        match gizmo.mode { GizmoMode::Translate => "TRANSLATE", GizmoMode::Rotate => "ROTATE" },
        session.history_depth(),
        if session.can_undo() { "available" } else { "none" },
        if session.can_redo() { "available" } else { "none" },
        if ui.help_open {
            "MOUSE\nLeft click: select component / drag gizmo\nRight drag: orbit camera\nMiddle drag: pan camera\nWheel: zoom\n\nKEYBOARD (H hides this)\nT / R: translate / rotate gizmo | G: grid snap\nTab: next component | Delete: remove\nI/J/K/L + U/O: move | X/Y/Z: rotate\nF4/F5, F7/F8, F9/F10: dimensions\nCtrl+Z / Ctrl+Y: undo / redo\nCtrl+S / Ctrl+L: save / load assembly\nCtrl+B / Ctrl+Backspace: record / restore revision\nCtrl+Enter: submit design | F11: instrument profile\nV: draw polygon surfaces\nAlt: fine adjustment | Shift: reverse rotation"
        } else {
            "H: show mouse and keyboard help"
        },
        revisions.hud_text(&session.assembly),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_plane_intersection_returns_forward_hit() {
        let hit = ray_plane_hit(
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::NEG_Z,
            Vec3::ZERO,
            Vec3::Z,
        ).unwrap();
        assert!((hit - Vec3::ZERO).length() < 1.0e-6);
    }

    #[test]
    fn translation_parameter_tracks_distance_along_axis() {
        let parameter = translation_parameter(
            Vec3::new(0.4, 0.05, 5.0),
            Vec3::NEG_Z,
            Vec3::ZERO,
            Vec3::X,
            Vec3::NEG_Z,
        ).unwrap();
        assert!((parameter - 0.4).abs() < 1.0e-6);
    }

    #[test]
    fn translation_gizmo_detects_visible_axis_pick() {
        let hit = hit_test_gizmo(
            Vec3::new(0.6, 0.02, 5.0),
            Vec3::NEG_Z,
            Vec3::NEG_Z,
            Vec3::ZERO,
            Quat::IDENTITY,
            1.5,
            1.0,
            [0.0; 3],
            [0.0; 3],
            GizmoMode::Translate,
        );
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().0, 0);
    }

    #[test]
    fn rotation_gizmo_detects_ring_pick() {
        let hit = hit_test_gizmo(
            Vec3::new(1.0, 0.0, 5.0),
            Vec3::NEG_Z,
            Vec3::NEG_Z,
            Vec3::ZERO,
            Quat::IDENTITY,
            1.5,
            1.0,
            [0.0; 3],
            [0.0; 3],
            GizmoMode::Rotate,
        );
        assert!(hit.is_some());
        assert!(matches!(hit.unwrap().1, GizmoDrag::Rotate { .. }));
    }

    #[test]
    fn gizmo_scale_tracks_camera_distance_with_safe_limits() {
        assert!(gizmo_scale_factor(60.0) > gizmo_scale_factor(15.0));
        assert_eq!(gizmo_scale_factor(1.0), 0.65);
        assert_eq!(gizmo_scale_factor(1000.0), 4.8);
    }

    #[test]
    fn conical_translation_tip_points_along_the_selected_local_axis() {
        let part_rotation = Quat::from_rotation_y(0.4);
        for axis in 0..3 {
            let transform = gizmo_primitive_transform(
                Vec3::ZERO,
                part_rotation,
                GizmoPrimitive::AxisTip { axis, length: 1.5, size: 0.2 },
                1.0,
            );
            let expected = part_rotation * axis_vector(axis);
            let actual = transform.rotation * Vec3::Y;
            assert!(actual.dot(expected) > 0.9999);
        }
    }

    #[test]
    fn ray_aabb_detects_a_hit() {
        let hit = ray_aabb(
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::NEG_Z,
            Vec3::splat(-1.0),
            Vec3::splat(1.0),
        );
        assert_eq!(hit, Some(4.0));
    }

    #[test]
    fn ray_aabb_rejects_a_miss() {
        let hit = ray_aabb(
            Vec3::new(3.0, 0.0, 5.0),
            Vec3::NEG_Z,
            Vec3::splat(-1.0),
            Vec3::splat(1.0),
        );
        assert_eq!(hit, None);
    }
}
