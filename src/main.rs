mod campus;
mod headless;
mod interface;
mod missile;
mod picking;
mod procurement;
mod program_editor;
mod revisions;
mod save;
mod scene;
mod trials;
mod ui_kit;
mod workshop;

use crate::ui_kit::UiInteraction;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use campus::{CampusState, CampusView};

fn main() {
    if let Some(code) = headless::main() {
        std::process::exit(code);
    }
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "OMEGA 0.37.0 | Conception de véhicules".to_string(),
                resolution: (1600, 960).into(),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<CampusState>()
        .init_resource::<revisions::RevisionLog>()
        .insert_resource(missile::MissileSession::new_default())
        .init_resource::<program_editor::MissileProgramSession>()
        .init_resource::<trials::TrialSession>()
        .init_resource::<procurement::Procurement>()
        .init_resource::<campus::ResearchScroll>()
        .init_resource::<campus::WorkshopScroll>()
        .init_resource::<workshop::DragState>()
        .init_resource::<workshop::WorkshopInput>()
        .init_resource::<workshop::TransformGizmo>()
        .add_systems(
            Startup,
            (
                scene::setup,
                campus::setup_boards,
                missile::setup_missile,
                missile::load_missile,
                program_editor::load_program,
                interface::setup,
            ),
        )
        .add_systems(
            Update,
            (
                (
                    interface::handle_actions,
                    campus::handle_extra_actions,
                    procurement::sync_offers,
                    procurement::record_trial_evidence,
                    campus::handle_input,
                    workshop::workshop_pointer,
                )
                    .chain(),
                (
                    campus::persist_campaign,
                    procurement::persist_procurement,
                    move_camera,
                    campus::sync_boards,
                    campus::scroll_program_list,
                    campus::scroll_research_grid,
                    campus::scroll_part_list,
                    workshop::draw_attachment_markers,
                    workshop::draw_aero_markers,
                    missile::sync_missile_visuals,
                    missile::focus_missile_camera,
                    campus::update_toast,
                    campus::poll_trial_batch,
                    campus::update_batch_progress,
                    campus::update_nav_status,
                    interface::sync_layout,
                )
                    .chain(),
            )
                .chain(),
        )
        .run();
}

fn move_camera(
    workshop_input: Res<workshop::WorkshopInput>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    campus: Res<CampusState>,
    session: Res<missile::MissileSession>,
    ui_buttons: Query<&UiInteraction>,
    mut cameras: Query<(&mut Transform, &mut scene::OrbitCameraRig), With<Camera3d>>,
) {
    if matches!(
        campus.view,
        CampusView::Contracts | CampusView::Research | CampusView::Program
    ) {
        return;
    }
    let Ok((mut transform, mut rig)) = cameras.single_mut() else {
        return;
    };

    let pointer_over_button = ui_buttons.iter().any(|interaction| *interaction != UiInteraction::None);
    let mut changed = false;

    // KSP-style framing shortcuts: F focuses the selected part, Home frames the whole design.
    if campus.view == CampusView::Missile {
        if keys.just_pressed(KeyCode::KeyF)
            && let Some(pose) = session
                .selected
                .as_deref()
                .and_then(|id| session.vehicle.poses().into_iter().find(|pose| pose.id == id))
        {
            rig.target =
                missile::ANCHOR + Vec3::new(pose.x_m as f32, pose.y_m as f32, pose.z_m as f32) * missile::VISUAL_SCALE;
            rig.distance = ((pose.radius_m as f32) * missile::VISUAL_SCALE * 4.0 + 8.0).clamp(12.0, 45.0);
            changed = true;
        }
        if keys.just_pressed(KeyCode::Home) {
            let poses = session.vehicle.poses();
            if poses.is_empty() {
                rig.target = missile::ANCHOR;
                rig.distance = 22.0;
            } else {
                let mut min = Vec3::splat(f32::INFINITY);
                let mut max = Vec3::splat(f32::NEG_INFINITY);
                let mut extent = 0.0_f32;
                for pose in &poses {
                    let point = missile::ANCHOR
                        + Vec3::new(pose.x_m as f32, pose.y_m as f32, pose.z_m as f32) * missile::VISUAL_SCALE;
                    min = min.min(point);
                    max = max.max(point);
                    extent = extent.max(pose.height_m.max(pose.radius_m * 2.0) as f32 * missile::VISUAL_SCALE);
                }
                rig.target = (min + max) * 0.5;
                rig.distance = ((max - min).length() * 1.25 + extent * 2.0 + 8.0).clamp(12.0, 115.0);
            }
            changed = true;
        }
    }

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
        if mouse_scroll.delta.y != 0.0 && !workshop_input.wheel_used {
            // Mouse wheels usually report lines; touchpads often report pixels.
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
