//! Pointer interaction for the missile workshop: pick a part with a camera ray, drag a template
//! from the palette (or an existing part) onto a free attachment node, move a whole subtree, delete
//! it, cycle radial symmetry, undo/redo and adjust the selected part with the wheel.
//!
//! Geometry lives in `picking.rs` and the rules live in `omega-rules::assembly`; this module only
//! turns pointer input into those calls and draws the node markers/ghost with gizmos.

use crate::campus::{CampusState, CampusView};
use crate::missile::{self, MissileSession, PartTemplate};
use crate::picking::{SNAP_PIXELS, nearest_part, nearest_snap};

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
}

#[derive(Clone, Debug)]
struct NodePoint {
    parent: String,
    node: NodeId,
    angle: f64,
    world: Vec3,
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
        let center = world_of(anchor, pose.x_m, pose.y_m, pose.z_m);
        let half = pose.height_m as f32 * missile::VISUAL_SCALE * 0.5;
        if is_free(&pose.id, NodeId::Top) {
            nodes.push(NodePoint {
                parent: pose.id.clone(),
                node: NodeId::Top,
                angle: 0.0,
                world: center + Vec3::Y * half,
            });
        }
        if is_free(&pose.id, NodeId::Bottom) {
            nodes.push(NodePoint {
                parent: pose.id.clone(),
                node: NodeId::Bottom,
                angle: 0.0,
                world: center - Vec3::Y * half,
            });
        }
        for index in 0..radial_slots(&part.spec) {
            let node = NodeId::Radial(index);
            if is_free(&pose.id, node) {
                let angle = f64::from(index) * 90.0;
                let direction = Vec3::new(angle.to_radians().cos() as f32, 0.0, angle.to_radians().sin() as f32);
                nodes.push(NodePoint {
                    parent: pose.id.clone(),
                    node,
                    angle,
                    world: center + direction * pose.radius_m as f32 * missile::VISUAL_SCALE,
                });
            }
        }
    }
    nodes
}

fn picked_part(vehicle: &Vehicle, origin: Vec3, direction: Vec3) -> Option<String> {
    let anchor = missile::ANCHOR;
    let parts: Vec<(Vec3, f32, f32)> = vehicle
        .poses()
        .into_iter()
        .map(|pose| {
            (
                world_of(anchor, pose.x_m, pose.y_m, pose.z_m),
                (pose.radius_m as f32).max(0.12) * missile::VISUAL_SCALE,
                pose.height_m as f32 * missile::VISUAL_SCALE,
            )
        })
        .collect();
    let ids: Vec<String> = vehicle.poses().into_iter().map(|pose| pose.id).collect();
    let (index, _) = nearest_part(origin, direction, &parts)?;
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
    mut gizmos: Gizmos,
) {
    input.wheel_used = false;
    if campus.view != CampusView::Missile {
        drag.source = None;
        drag.hover = None;
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

    // --- start a drag ---
    if mouse.just_pressed(MouseButton::Left) {
        if let Some((interaction, palette)) = palettes.iter().find(|(i, _)| **i == UiInteraction::Pressed) {
            let _ = interaction;
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

    // --- finish a drag ---
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

    // --- keyboard ---
    if keys.just_pressed(KeyCode::Escape) && drag.source.is_some() {
        drag.source = None;
        drag.hover = None;
        campus.message = "Drag cancelled".to_string();
    }
    if drag.source.is_none() {
        if keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace) {
            campus.message = session.delete_selected();
        }
        if keys.just_pressed(KeyCode::KeyX) {
            campus.message = session.cycle_symmetry();
        }
        // Logical keys, not physical ones: on an AZERTY keyboard Ctrl+Z is the KeyW scan code.
        for event in typed.read() {
            if event.state != ButtonState::Pressed {
                continue;
            }
            let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
            if !control {
                continue;
            }
            if let Key::Character(text) = &event.logical_key {
                match text.to_lowercase().as_str() {
                    "z" => campus.message = session.undo(),
                    "y" => campus.message = session.redo(),
                    _ => {}
                }
            }
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
        session.apply(
            missile::MissileAction::Adjust(param, if lines > 0.0 { 1 } else { -1 }),
            &owned,
        );
        input.wheel_used = true;
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
