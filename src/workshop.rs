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
use crate::ui_kit::UiInteraction;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use omega_rules::assembly::radial_slots;
use omega_rules::{NodeId, Vehicle};

/// A palette entry that can be dragged instead of clicked.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct PalettePart(pub(crate) PartTemplate);

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DragSource {
    Palette(PartTemplate),
    Part(String),
}

#[derive(Resource, Default)]
pub(crate) struct DragState {
    pub source: Option<DragSource>,
    pub hover: Option<(String, NodeId, f64)>,
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
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TransformMode {
    #[default]
    Translate,
    Rotate,
}

/// KSP-style transform gizmo state. A handle drag is committed as one undoable edit on release.
#[derive(Resource, Default)]
pub(crate) struct TransformGizmo {
    pub mode: TransformMode,
    pub drag: Option<(usize, Vec2, Vec2)>,
    pub before: Option<Vehicle>,
}

#[derive(Clone, Debug)]
struct NodePoint {
    parent: String,
    node: NodeId,
    angle: f64,
    world: Vec3,
}

fn gizmo_amount(cursor: Vec2, start: Vec2, screen_axis: Vec2, mode: TransformMode, fine: bool) -> f64 {
    if mode == TransformMode::Translate {
        let metres = (cursor - start).dot(screen_axis.normalize_or_zero()) as f64 * 0.02;
        if fine { metres } else { (metres / 0.1).round() * 0.1 }
    } else {
        let start_angle = (start.y - screen_axis.y).atan2(start.x - screen_axis.x);
        let current_angle = (cursor.y - screen_axis.y).atan2(cursor.x - screen_axis.x);
        let mut radians = current_angle - start_angle;
        if radians > std::f32::consts::PI {
            radians -= std::f32::consts::TAU;
        } else if radians < -std::f32::consts::PI {
            radians += std::f32::consts::TAU;
        }
        let degrees = radians.to_degrees() as f64;
        if fine { degrees } else { (degrees / 5.0).round() * 5.0 }
    }
}

fn world_of(anchor: Vec3, x: f64, y: f64, z: f64) -> Vec3 {
    anchor + Vec3::new(x as f32, y as f32, z as f32) * missile::VISUAL_SCALE
}

/// Every node of the design that has no child yet, in world space.
fn free_nodes(vehicle: &Vehicle, anchor: Vec3) -> Vec<NodePoint> {
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
        if part.spec.is_aero() {
            continue; // a surface is a leaf: it offers no attachment node
        }
        let center = world_of(anchor, pose.x_m, pose.y_m, pose.z_m);
        let orientation = Quat::from_euler(
            EulerRot::XYZ,
            (pose.rotation_x_deg as f32).to_radians(),
            (pose.rotation_y_deg as f32).to_radians(),
            (pose.rotation_z_deg as f32).to_radians(),
        );
        let half = pose.height_m as f32 * missile::VISUAL_SCALE * 0.5;
        if is_free(&pose.id, NodeId::Top) {
            nodes.push(NodePoint {
                parent: pose.id.clone(),
                node: NodeId::Top,
                angle: 0.0,
                world: center + orientation * Vec3::Y * half,
            });
        }
        if is_free(&pose.id, NodeId::Bottom) {
            nodes.push(NodePoint {
                parent: pose.id.clone(),
                node: NodeId::Bottom,
                angle: 0.0,
                world: center - orientation * Vec3::Y * half,
            });
        }
        for index in 0..radial_slots(&part.spec) {
            let node = NodeId::Radial(index);
            if is_free(&pose.id, node) {
                let angle = f64::from(index) * 90.0;
                let local_direction = Vec3::new(
                    angle.to_radians().cos() as f32,
                    0.0,
                    angle.to_radians().sin() as f32,
                );
                nodes.push(NodePoint {
                    parent: pose.id.clone(),
                    node,
                    angle,
                    world: center
                        + orientation * local_direction * pose.radius_m as f32 * missile::VISUAL_SCALE,
                });
            }
        }
    }
    nodes
}

const SURFACE_HALF_THICKNESS_M: f32 = 0.05;

fn picked_part(vehicle: &Vehicle, origin: Vec3, direction: Vec3) -> Option<String> {
    let anchor = missile::ANCHOR;
    let poses = vehicle.poses();
    let shapes: Vec<PickShape> = poses
        .iter()
        .map(|pose| {
            let center = world_of(anchor, pose.x_m, pose.y_m, pose.z_m);
            let scale = missile::VISUAL_SCALE;
            let part = vehicle.part(&pose.id);
            if part.is_some_and(|part| part.spec.is_aero()) {
                let angle = part
                    .and_then(|part| part.attach.as_ref())
                    .map_or(0.0, |attach| match attach.node {
                        NodeId::Radial(index) => f64::from(index) * 90.0 + attach.angle_deg,
                        _ => attach.angle_deg,
                    });
                PickShape::Box {
                    center,
                    half: Vec3::new(
                        (pose.radius_m as f32) * scale,
                        (pose.height_m as f32) * scale * 0.5,
                        SURFACE_HALF_THICKNESS_M * scale,
                    ),
                    yaw: (angle as f32).to_radians(),
                }
            } else {
                PickShape::Cylinder {
                    center,
                    radius: (pose.radius_m as f32).max(0.12) * scale,
                    half_height: (pose.height_m as f32) * scale * 0.5,
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
    mut session: ResMut<MissileSession>,
    mut campus: ResMut<CampusState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    mut typed: MessageReader<KeyboardInput>,
    palettes: Query<(&UiInteraction, &PalettePart)>,
    ui: Query<&UiInteraction>,
    mut drag: ResMut<DragState>,
    mut input: ResMut<WorkshopInput>,
    mut transform_gizmo: ResMut<TransformGizmo>,
    mut gizmos: Gizmos,
) {
    input.wheel_used = false;
    input.drag_active = false;
    if campus.view != CampusView::Missile {
        drag.source = None;
        drag.hover = None;
        drag.pending = None;
        return;
    }
    let owned = campus.owned_capabilities();
    let pointer_over_ui = ui.iter().any(|interaction| *interaction != UiInteraction::None);

    let Some(window) = windows.iter().next() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    let ray = camera
        .viewport_to_world(camera_transform, cursor)
        .ok()
        .map(|ray| (ray.origin, *ray.direction));
    let picked = ray.and_then(|(origin, direction)| picked_part(&session.vehicle, origin, direction));

    // Read logical keys so W/R work on AZERTY as printed, not only at QWERTY scan positions.
    let mut translate_tool_pressed = false;
    let mut rotate_tool_pressed = false;
    let mut undo_pressed = false;
    let mut redo_pressed = false;
    for event in typed.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        if let Key::Character(character) = &event.logical_key {
            match (control, character.to_lowercase().as_str()) {
                (false, "w") => translate_tool_pressed = true,
                (false, "r") => rotate_tool_pressed = true,
                (true, "z") => undo_pressed = true,
                (true, "y") => redo_pressed = true,
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
            TransformMode::Rotate
        } else {
            TransformMode::Translate
        };
    }

    // --- start a drag ---
    if mouse.just_pressed(MouseButton::Left) {
        if !pointer_over_ui
            && let Some(id) = session.selected.as_deref()
            && let Some(pose) = session.vehicle.poses().into_iter().find(|pose| pose.id == id)
        {
            let center = world_of(missile::ANCHOR, pose.x_m, pose.y_m, pose.z_m);
            let axes = [Vec3::X, Vec3::Y, Vec3::Z];
            let center_screen = camera.world_to_viewport(camera_transform, center).ok();
            let mut nearest: Option<(usize, Vec2, f32)> = None;
            for (axis, direction) in axes.into_iter().enumerate() {
                if transform_gizmo.mode == TransformMode::Translate {
                    if let (Some(origin), Ok(point)) = (
                        center_screen,
                        camera.world_to_viewport(camera_transform, center + direction * 1.5),
                    ) {
                        let distance = cursor.distance(point);
                        if distance <= 20.0 && nearest.is_none_or(|(_, _, best)| distance < best) {
                            nearest = Some((axis, point - origin, distance));
                        }
                    }
                } else {
                    for step in 0..48 {
                        let angle = std::f32::consts::TAU * step as f32 / 48.0;
                        let ring_point = match axis {
                            0 => center + Vec3::new(0.0, angle.cos(), angle.sin()) * 1.05,
                            1 => center + Vec3::new(angle.cos(), 0.0, angle.sin()) * 1.05,
                            _ => center + Vec3::new(angle.cos(), angle.sin(), 0.0) * 1.05,
                        };
                        if let Ok(point) = camera.world_to_viewport(camera_transform, ring_point) {
                            let distance = cursor.distance(point);
                            if distance <= 20.0 && nearest.is_none_or(|(_, _, best)| distance < best) {
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
            if let Some((_, palette)) = palettes.iter().find(|(i, _)| **i == UiInteraction::Pressed) {
                drag.source = Some(DragSource::Palette(palette.0));
                drag.hover = None;
            } else if !pointer_over_ui && let Some(id) = picked.clone() {
                session.apply(missile::MissileAction::Select { id: id.clone() }, &owned);
                drag.pending = session
                    .vehicle
                    .part(&id)
                    .is_some_and(|part| part.attach.is_some())
                    .then_some((cursor, DragSource::Part(id)));
                drag.hover = None;
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
    let nodes = free_nodes(&session.vehicle, missile::ANCHOR);
    let screen: Vec<Vec2> = nodes
        .iter()
        .filter_map(|node| camera.world_to_viewport(camera_transform, node.world).ok())
        .collect();
    let snapped = (screen.len() == nodes.len())
        .then(|| nearest_snap(cursor, &screen, SNAP_PIXELS))
        .flatten();

    if let Some(source) = drag.source.clone() {
        drag.hover = snapped.map(|index| {
            let node = &nodes[index];
            (node.parent.clone(), node.node, node.angle)
        });
        // Node markers while dragging; the hovered one is brighter.
        for (index, node) in nodes.iter().enumerate() {
            let hovered = snapped == Some(index);
            let color = if hovered {
                Color::srgb(0.35, 0.95, 0.55)
            } else {
                Color::srgba(0.36, 0.84, 0.94, 0.55)
            };
            let radius = if hovered { 0.34 } else { 0.18 };
            gizmos.sphere(Isometry3d::from_translation(node.world), radius, color);
        }
        if let Some(index) = snapped {
            gizmos.cube(
                Transform::from_translation(nodes[index].world).with_scale(Vec3::splat(0.45)),
                Color::srgb(0.35, 0.95, 0.55),
            );
        } else if let Some((origin, direction)) = ray {
            // A small ghost along the ray so the drag is visible even between nodes.
            gizmos.line(
                origin + direction * 4.0,
                origin + direction * 4.2,
                Color::srgba(0.36, 0.84, 0.94, 0.8),
            );
        }
        let _ = source;
    }

    // Live preview while dragging, then one history entry on release.
    if mouse.pressed(MouseButton::Left)
        && let Some((axis, start, screen_axis)) = transform_gizmo.drag
        && let Some(before) = transform_gizmo.before.as_ref()
    {
        let fine = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let amount = gizmo_amount(cursor, start, screen_axis, transform_gizmo.mode, fine);
        session.preview_transform(before, axis, amount, transform_gizmo.mode == TransformMode::Rotate);
    }
    if mouse.just_released(MouseButton::Left)
        && let Some((axis, start, screen_axis)) = transform_gizmo.drag.take()
    {
        let fine = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        let amount = gizmo_amount(cursor, start, screen_axis, transform_gizmo.mode, fine);
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
                let over_palette = palettes
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
        && let Some(pose) = session.vehicle.poses().into_iter().find(|pose| pose.id == id)
        && session.vehicle.part(id).is_some_and(|part| part.attach.is_some())
    {
        let center = world_of(missile::ANCHOR, pose.x_m, pose.y_m, pose.z_m);
        let axes = [
            (Vec3::X, Color::srgb(0.95, 0.25, 0.25)),
            (Vec3::Y, Color::srgb(0.30, 0.90, 0.40)),
            (Vec3::Z, Color::srgb(0.25, 0.55, 1.0)),
        ];
        for (axis_index, (axis, color)) in axes.into_iter().enumerate() {
            if transform_gizmo.mode == TransformMode::Translate {
                let end = center + axis * 1.5;
                gizmos.line(center, end, color);
                gizmos.sphere(Isometry3d::from_translation(end), 0.11, color);
            } else {
                let mut previous = None;
                for step in 0..=48 {
                    let angle = std::f32::consts::TAU * step as f32 / 48.0;
                    let point = match axis_index {
                        0 => center + Vec3::new(0.0, angle.cos(), angle.sin()) * 1.05,
                        1 => center + Vec3::new(angle.cos(), 0.0, angle.sin()) * 1.05,
                        _ => center + Vec3::new(angle.cos(), angle.sin(), 0.0) * 1.05,
                    };
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
        campus.message = "Drag cancelled".to_string();
    }
    if drag.source.is_none() && transform_gizmo.drag.is_none() {
        if keys.just_pressed(KeyCode::KeyQ) {
            campus.message = session.rotate_selected(-1);
        }
        if keys.just_pressed(KeyCode::KeyE) {
            campus.message = session.rotate_selected(1);
        }
        if keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace) {
            campus.message = session.delete_selected();
        }
        if keys.just_pressed(KeyCode::KeyX) {
            campus.message = session.cycle_symmetry();
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
        .and_then(|part| missile::params_of(&part.spec).first().map(|(param, _)| *param));
    if lines != 0.0
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
    if session.is_changed()
        && let Err(error) = missile::persist_missile(&session)
    {
        warn!("Could not save the vehicle: {error}");
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
    if part.spec.is_aero() {
        return;
    }
    for node in free_nodes(&session.vehicle, missile::ANCHOR)
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
    let scale = missile::VISUAL_SCALE;
    let anchor = missile::ANCHOR;
    let at = |y: f64| anchor + Vec3::Y * (y as f32) * scale;
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
