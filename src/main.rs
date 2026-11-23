mod campus;
mod design;
mod interface;
mod jobs;
mod missile;
mod save;
mod scene;
mod sequence;
mod ui_kit;

use crate::ui_kit::UiInteraction;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use campus::{CampusState, CampusView};
use design::DesignSession;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "OMEGA 0.26.0 | Research Island".to_string(),
                resolution: (1600, 960).into(),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<CampusState>()
        .init_resource::<DesignSession>()
        .init_resource::<design::PolygonSketchSession>()
        .init_resource::<design::RevisionLog>()
        .init_resource::<ui_kit::UiState>()
        .insert_resource(missile::MissileSession::new_default())
        .init_resource::<design::InspectorState>()
        .init_resource::<jobs::JobManager>()
        .init_resource::<sequence::SequenceWorkspace>()
        .add_systems(
            Startup,
            (
                scene::setup,
                campus::setup_ui,
                campus::setup_boards,
                missile::setup_missile,
                missile::load_missile,
                design::setup_ui,
                design::setup_inspector_ui,
                design::initialize_editor,
                design::initialize_polygon_sketch,
                design::initialize_revision_log,
                design::initialize_polygon_sketch_visuals,
                design::setup_polygon_sketch_ui,
                jobs::setup_ui,
                sequence::setup_ui,
                sequence::setup_graph_ui,
                interface::setup,
            ),
        )
        .add_systems(
            Update,
            (
                (
                    interface::handle_actions,
                    campus::handle_extra_actions,
                    design::handle_inspector_input,
                    design::handle_inspector_text_input,
                    campus::handle_input,
                    campus::handle_computation_input,
                    design::handle_gizmo_input,
                    design::pick_part_with_mouse,
                    design::handle_keyboard,
                    design::handle_revision_input,
                    design::handle_polygon_sketch_input,
                    poll_jobs,
                    campus::sync_jobs,
                )
                    .chain(),
                (
                    campus::persist_campaign,
                    jobs::update_hud,
                    move_camera,
                    design::sync_part_visuals,
                    design::sync_gizmo_visuals,
                    design::update_gizmo_visuals,
                    design::sync_polygon_sketch_visuals,
                    design::update_polygon_sketch_drag_visuals,
                    design::update_hud,
                    design::update_inspector_ui,
                    campus::sync_boards,
                    missile::sync_missile_visuals,
                    missile::focus_missile_camera,
                    campus::update_toast,
                    campus::update_nav_status,
                )
                    .chain(),
                (
                    design::update_polygon_sketch_hud,
                    sequence::handle_keyboard,
                    sequence::graph_input,
                    sequence::sync_graph_ui,
                    sequence::tick,
                    sequence::update_hud,
                    campus::update_hud,
                    interface::sync_layout,
                )
                    .chain(),
            )
                .chain(),
        )
        .run();
}

fn poll_jobs(mut jobs: ResMut<jobs::JobManager>) {
    for message in jobs.collect_updates() {
        info!("{message}");
    }
}

fn move_camera(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    campus: Res<CampusState>,
    sequence: Res<sequence::SequenceWorkspace>,
    sketch: Res<design::PolygonSketchSession>,
    gizmo: Res<design::GizmoState>,
    inspector: Res<design::InspectorState>,
    ui_buttons: Query<&UiInteraction>,
    mut cameras: Query<(&mut Transform, &mut scene::OrbitCameraRig), With<Camera3d>>,
) {
    if gizmo.pointer_captured
        || inspector.is_editing()
        || matches!(
            campus.view,
            CampusView::Contracts | CampusView::Research | CampusView::Programming | CampusView::Computation
        )
        || campus.profile_editor_open
        || sequence.graph_is_open()
        || sketch.active
    {
        return;
    }
    let Ok((mut transform, mut rig)) = cameras.single_mut() else {
        return;
    };

    let pointer_over_button = ui_buttons.iter().any(|interaction| *interaction != UiInteraction::None);
    let mut changed = false;

    // CAD-like mouse navigation: right drag orbits, middle drag pans, wheel zooms.
    if !pointer_over_button {
        let delta = mouse_motion.delta;
        if mouse.pressed(MouseButton::Right) && delta.length_squared() > 0.0 {
            rig.yaw -= delta.x * 0.0055;
            rig.pitch = (rig.pitch - delta.y * 0.0045).clamp(0.12, 1.43);
            changed = true;
        }
        if mouse.pressed(MouseButton::Middle) && delta.length_squared() > 0.0 {
            let right = Vec3::new(rig.yaw.cos(), 0.0, -rig.yaw.sin());
            let forward = Vec3::new(-rig.yaw.sin(), 0.0, -rig.yaw.cos());
            let pan_scale = rig.distance * 0.0015;
            rig.target += right * (-delta.x * pan_scale) + forward * (delta.y * pan_scale);
            changed = true;
        }
        if mouse_scroll.delta.y != 0.0 {
            // Mouse wheels usually report lines; touchpads often report pixels.
            // Normalize both so a small trackpad gesture does not jump several metres.
            let scroll_lines = match mouse_scroll.unit {
                MouseScrollUnit::Line => mouse_scroll.delta.y,
                MouseScrollUnit::Pixel => mouse_scroll.delta.y / 100.0,
            };
            let zoom_step = (scroll_lines * 0.12).clamp(-0.6, 0.6);
            rig.distance = (rig.distance * (-zoom_step).exp()).clamp(12.0, 115.0);
            changed = true;
        }
    }

    // Keyboard movement remains available as a precise alternative to panning.
    let mut direction = Vec3::ZERO;
    let control_held = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let forward = Vec3::new(-rig.yaw.sin(), 0.0, -rig.yaw.cos());
    let right = Vec3::new(rig.yaw.cos(), 0.0, -rig.yaw.sin());
    if !control_held && (keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp)) {
        direction += forward;
    }
    if !control_held && (keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown)) {
        direction -= forward;
    }
    if !control_held && (keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft)) {
        direction -= right;
    }
    if !control_held && (keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight)) {
        direction += right;
    }
    if keys.pressed(KeyCode::KeyQ) {
        direction.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyE) {
        direction.y += 1.0;
    }
    if direction.length_squared() > 0.0 {
        let speed = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            24.0
        } else {
            10.0
        };
        rig.target += direction.normalize() * speed * time.delta_secs();
        changed = true;
    }

    if changed {
        let horizontal = rig.pitch.cos() * rig.distance;
        transform.translation = rig.target
            + Vec3::new(
                rig.yaw.sin() * horizontal,
                rig.pitch.sin() * rig.distance,
                rig.yaw.cos() * horizontal,
            );
        transform.look_at(rig.target, Vec3::Y);
    }
}
