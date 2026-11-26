mod campus;
mod headless;
mod inspector;
mod interface;
mod lab;
mod main_menu;
mod missile;
mod picking;
mod procurement;
mod program_editor;
mod revisions;
mod save;
mod scene;
mod subassembly;
mod trial_export;
mod trials;
mod ui_kit;
mod workshop;

use crate::ui_kit::UiInteraction;
use bevy::ecs::schedule::SystemSet;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::WindowResizeConstraints;
use campus::{CampusState, CampusView};

const DEFAULT_WINDOW: (u32, u32) = (1600, 960);
const MIN_WINDOW: (f32, f32) = (1280.0, 720.0);

/// Input handlers run first, then board state is committed and rebuilt, then the wheel-driven
/// scroll offsets run against the new UI. Named sets keep the three groups ordered without a
/// single oversized tuple.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct InputPhase;
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct BoardPhase;

fn main() {
    if let Some(code) = headless::main() {
        std::process::exit(code);
    }
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
<<<<<<< Updated upstream
                title: "OMEGA 0.39.0 | Conception de véhicules".to_string(),
                resolution: (1600, 960).into(),
=======
                title: "OMEGA 0.47.0 | Cohérence, finition et version candidate".to_string(),
                resolution: DEFAULT_WINDOW.into(),
                resize_constraints: WindowResizeConstraints {
                    min_width: MIN_WINDOW.0,
                    min_height: MIN_WINDOW.1,
                    ..default()
                },
>>>>>>> Stashed changes
                ..default()
            }),
            ..default()
        }))
        .init_resource::<CampusState>()
        .init_resource::<main_menu::MainMenuState>()
        .init_resource::<main_menu::Frontend>()
        .init_resource::<inspector::InspectorState>()
        .init_resource::<revisions::RevisionLog>()
        .insert_resource(missile::MissileSession::new_default())
        .init_resource::<program_editor::MissileProgramSession>()
        .init_resource::<trials::TrialSession>()
        .init_resource::<procurement::Procurement>()
        .init_resource::<campus::ContractScroll>()
        .init_resource::<campus::InspectorScrollOffset>()
        .init_resource::<campus::AssemblyControlScroll>()
        .init_resource::<campus::ResearchScroll>()
        .init_resource::<campus::WorkshopScroll>()
        .init_resource::<campus::WorkshopPaletteScroll>()
        .init_resource::<campus::ProgramPaletteScroll>()
        .init_resource::<workshop::DragState>()
        .init_resource::<workshop::WorkshopInput>()
        .init_resource::<workshop::WorkshopHover>()
        .init_resource::<workshop::TransformGizmo>()
        .init_resource::<scene::CameraCommand>()
        .add_systems(PreStartup, ui_kit::load_ui_font)
        .add_systems(
            Startup,
            (
                scene::setup,
                campus::setup_boards,
                missile::setup_missile,
                workshop::setup_palette_preview,
                missile::load_missile,
                program_editor::load_program,
                interface::setup,
            ),
        )
        .add_systems(
            Startup,
            main_menu::setup.after(campus::setup_boards).after(interface::setup),
        )
        .add_systems(
            Update,
            (
                interface::handle_actions,
                main_menu::handle_actions,
                inspector::handle_actions,
                campus::handle_extra_actions,
                procurement::sync_offers,
                procurement::record_trial_evidence,
                campus::handle_input,
                inspector::begin_edit,
                workshop::workshop_pointer,
                inspector::handle_keyboard,
            )
                .chain()
                .in_set(InputPhase),
        )
        .add_systems(
            Update,
            (
                campus::persist_campaign,
                procurement::persist_procurement,
                main_menu::sync_frontend,
                move_camera,
                campus::sync_boards,
            )
                .chain()
                .in_set(BoardPhase)
                .after(InputPhase),
        )
        .add_systems(Update, inspector::refresh_fields.after(BoardPhase))
        .add_systems(
            Update,
            (
                campus::scroll_contracts_board,
                campus::scroll_program_list,
                campus::scroll_research_grid,
                campus::scroll_part_list,
                campus::scroll_inspector,
                campus::scroll_assembly_control,
                campus::scroll_workshop_palette,
                campus::scroll_program_palette,
                workshop::draw_attachment_markers,
                workshop::draw_aero_markers,
                workshop::draw_selection_highlights,
                workshop::update_palette_preview,
                workshop::update_drag_ghost,
                workshop::update_palette_hint,
                missile::sync_missile_visuals,
                missile::focus_missile_camera,
                campus::update_toast,
                campus::poll_trial_batch,
                campus::update_batch_progress,
                campus::update_nav_status,
            )
                .chain()
                .after(BoardPhase),
        )
        .add_systems(Update, campus::persist_trial_history.after(BoardPhase))
        .add_systems(
            Update,
            (
                campus::drag_program_canvas,
                campus::zoom_program_canvas,
                campus::advance_program_debug,
            )
                .after(BoardPhase),
        )
        .run();
}

fn move_camera(
    frontend: Res<main_menu::Frontend>,
    workshop_input: Res<workshop::WorkshopInput>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    mut campus: ResMut<CampusState>,
    session: Res<missile::MissileSession>,
    mut camera_command: ResMut<scene::CameraCommand>,
    windows: Query<&Window>,
    ui_buttons: Query<&UiInteraction>,
    mut last_viewport_height: Local<f32>,
    mut cameras: Query<(&mut Transform, &mut scene::OrbitCameraRig), With<scene::SceneCamera>>,
) {
    if *frontend != main_menu::Frontend::Game {
        camera_command.0 = None;
        return;
    }
    if matches!(
        campus.view,
        CampusView::Contracts | CampusView::Research | CampusView::Program
    ) {
        camera_command.0 = None;
        return;
    }
    let Ok((mut transform, mut rig)) = cameras.single_mut() else {
        camera_command.0 = None;
        return;
    };

    let pointer_over_button = ui_buttons.iter().any(|interaction| *interaction != UiInteraction::None);
    let mut changed = false;
    let viewport_height = windows.iter().next().map_or(960.0, Window::height);

    // A live resize changes the panel strip above the design; keep the framed model in it.
    if *last_viewport_height > 0.0
        && (*last_viewport_height - viewport_height).abs() > 0.5
        && campus.view == CampusView::Missile
    {
        rig.target = missile::reframe_on_resize(&session.vehicle, rig.target, *last_viewport_height, viewport_height);
        changed = true;
    }
    *last_viewport_height = viewport_height;

    // Keyboard framing and camera mode are workshop commands. Text fields own these keys while
    // they are open, so a name containing "f" or "1" never moves the camera.
    let shortcuts = campus.view == CampusView::Missile && !workshop_input.text_active;
    if shortcuts {
        if keys.just_pressed(KeyCode::KeyF) {
            let (yaw, pitch) = (rig.yaw, rig.pitch);
            set_camera_goal(
                &mut rig,
                camera_goal_for(&session, true, viewport_height, yaw, pitch),
                &mut campus.message,
                "Centré sur la sélection",
            );
        }
        if keys.just_pressed(KeyCode::Home) {
            let (yaw, pitch) = (rig.yaw, rig.pitch);
            set_camera_goal(
                &mut rig,
                camera_goal_for(&session, false, viewport_height, yaw, pitch),
                &mut campus.message,
                "Véhicule cadré",
            );
        }
        for (key, view) in [
            (KeyCode::Digit1, scene::CameraView::Front),
            (KeyCode::Digit2, scene::CameraView::Side),
            (KeyCode::Digit3, scene::CameraView::Top),
            (KeyCode::Digit4, scene::CameraView::Rear),
            (KeyCode::Digit5, scene::CameraView::Iso),
        ] {
            if keys.just_pressed(key) {
                let (yaw, pitch) = view_angles(view);
                set_camera_goal(
                    &mut rig,
                    camera_goal_for(&session, false, viewport_height, yaw, pitch),
                    &mut campus.message,
                    view_label(view),
                );
            }
        }
        if keys.just_pressed(KeyCode::KeyV) {
            rig.mode = match rig.mode {
                scene::CameraMode::Orbit => scene::CameraMode::Free,
                scene::CameraMode::Free => scene::CameraMode::Orbit,
            };
            rig.goal = None;
            campus.message = format!(
                "Caméra {} : glissez au clic droit pour regarder, molette pour avancer/reculer.",
                if rig.mode == scene::CameraMode::Free {
                    "libre"
                } else {
                    "orbitale"
                }
            );
        }
        if keys.just_pressed(KeyCode::BracketLeft) {
            rig.speed = (rig.speed - 0.2).max(0.4);
            campus.message = format!("Vitesse caméra : {:.1}×", rig.speed);
        }
        if keys.just_pressed(KeyCode::BracketRight) {
            rig.speed = (rig.speed + 0.2).min(3.0);
            campus.message = format!("Vitesse caméra : {:.1}×", rig.speed);
        }
    }
    if let Some(command) = camera_command.0.take().filter(|_| campus.view == CampusView::Missile) {
        match command {
            scene::CameraAction::CenterSelection => {
                let (yaw, pitch) = (rig.yaw, rig.pitch);
                set_camera_goal(
                    &mut rig,
                    camera_goal_for(&session, true, viewport_height, yaw, pitch),
                    &mut campus.message,
                    "Centré sur la sélection",
                );
            }
            scene::CameraAction::FrameVehicle => {
                let (yaw, pitch) = (rig.yaw, rig.pitch);
                set_camera_goal(
                    &mut rig,
                    camera_goal_for(&session, false, viewport_height, yaw, pitch),
                    &mut campus.message,
                    "Véhicule cadré",
                );
            }
            scene::CameraAction::View(view) => {
                let (yaw, pitch) = view_angles(view);
                set_camera_goal(
                    &mut rig,
                    camera_goal_for(&session, false, viewport_height, yaw, pitch),
                    &mut campus.message,
                    view_label(view),
                );
            }
            scene::CameraAction::ToggleMode => {
                rig.mode = match rig.mode {
                    scene::CameraMode::Orbit => scene::CameraMode::Free,
                    scene::CameraMode::Free => scene::CameraMode::Orbit,
                };
                rig.goal = None;
                campus.message = format!(
                    "Caméra {}.",
                    if rig.mode == scene::CameraMode::Free {
                        "libre"
                    } else {
                        "orbitale"
                    }
                );
            }
            scene::CameraAction::Faster => {
                rig.speed = (rig.speed + 0.2).min(3.0);
                campus.message = format!("Vitesse caméra : {:.1}×", rig.speed);
            }
            scene::CameraAction::Slower => {
                rig.speed = (rig.speed - 0.2).max(0.4);
                campus.message = format!("Vitesse caméra : {:.1}×", rig.speed);
            }
        }
    }

    // Ease toward a framing goal; any direct manipulation cancels it below.
    if let Some(goal) = rig.goal {
        let t = 1.0 - (-8.0 * time.delta_secs()).exp();
        rig.target = rig.target.lerp(goal.target, t);
        let delta =
            (goal.yaw - rig.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        rig.yaw += delta * t;
        rig.pitch += (goal.pitch - rig.pitch) * t;
        rig.distance += (goal.distance - rig.distance) * t;
        if rig.target.distance(goal.target) < 0.03
            && delta.abs() < 0.01
            && (goal.pitch - rig.pitch).abs() < 0.01
            && (goal.distance - rig.distance).abs() < 0.05
        {
            rig.target = goal.target;
            rig.yaw = goal.yaw;
            rig.pitch = goal.pitch;
            rig.distance = goal.distance;
            rig.goal = None;
        }
        changed = true;
    } else if !pointer_over_button {
        // CAD-like mouse navigation: right drag orbits (or looks around in free mode), middle
        // drag pans, wheel zooms (or flies forward in free mode).
        let camera_position = transform.translation;
        let delta = mouse_motion.delta;
        if mouse.pressed(MouseButton::Right) && delta.length_squared() > 0.0 {
            rig.yaw -= delta.x * 0.0055;
            rig.pitch = if rig.mode == scene::CameraMode::Free {
                // Free look follows the mouse (drag up looks up), unlike an orbit which raises
                // the eye when the same drag moves the model down the screen.
                (rig.pitch + delta.y * 0.0045).clamp(-1.43, 1.43)
            } else {
                (rig.pitch - delta.y * 0.0045).clamp(0.12, 1.43)
            };
            if rig.mode == scene::CameraMode::Free {
                // Rotate in place: the look-at point follows the new view direction from the
                // current eye position, so the camera does not swing around the old target.
                let forward = view_forward(&rig);
                rig.target = camera_position + forward * rig.distance;
            }
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
            if rig.mode == scene::CameraMode::Free {
                let forward = view_forward(&rig);
                let distance = rig.distance;
                rig.target += forward * (-zoom_step * distance * 2.0);
            } else {
                rig.distance = (rig.distance * (-zoom_step).exp()).clamp(12.0, 115.0);
            }
            changed = true;
        }
    }

    // Keyboard movement remains available as a precise alternative to panning. It is skipped
    // while a framing goal is easing so a held key cannot pull against the transition.
    let mut direction = Vec3::ZERO;
    let control_held = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let forward = Vec3::new(-rig.yaw.sin(), 0.0, -rig.yaw.cos());
    let right = Vec3::new(rig.yaw.cos(), 0.0, -rig.yaw.sin());
    // In the workshop W/R belong to the transform tools, so camera keyboard panning
    // uses the arrow keys there. The rest of the campus keeps WASD navigation.
    let workshop_view = campus.view == CampusView::Missile;
    if !control_held && (keys.pressed(KeyCode::ArrowUp) || (!workshop_view && keys.pressed(KeyCode::KeyW))) {
        direction += forward;
    }
    if !control_held && (keys.pressed(KeyCode::ArrowDown) || (!workshop_view && keys.pressed(KeyCode::KeyS))) {
        direction -= forward;
    }
    if !control_held && (keys.pressed(KeyCode::ArrowLeft) || (!workshop_view && keys.pressed(KeyCode::KeyA))) {
        direction -= right;
    }
    if !control_held && (keys.pressed(KeyCode::ArrowRight) || (!workshop_view && keys.pressed(KeyCode::KeyD))) {
        direction += right;
    }
    if !workshop_view && keys.pressed(KeyCode::KeyQ) {
        direction.y -= 1.0;
    }
    if !workshop_view && keys.pressed(KeyCode::KeyE) {
        direction.y += 1.0;
    }
    if rig.goal.is_none() && direction.length_squared() > 0.0 {
        let speed = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
            24.0
        } else {
            10.0
        } * rig.speed;
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

fn view_forward(rig: &scene::OrbitCameraRig) -> Vec3 {
    Vec3::new(
        -rig.yaw.sin() * rig.pitch.cos(),
        -rig.pitch.sin(),
        -rig.yaw.cos() * rig.pitch.cos(),
    )
}

fn view_angles(view: scene::CameraView) -> (f32, f32) {
    match view {
        scene::CameraView::Front => (0.0, 0.16),
        scene::CameraView::Side => (std::f32::consts::FRAC_PI_2, 0.16),
        scene::CameraView::Top => (0.0, 1.42),
        scene::CameraView::Rear => (std::f32::consts::PI, 0.16),
        scene::CameraView::Iso => (0.72, 0.52),
    }
}

fn view_label(view: scene::CameraView) -> &'static str {
    match view {
        scene::CameraView::Front => "Vue de face",
        scene::CameraView::Side => "Vue latérale",
        scene::CameraView::Top => "Vue de dessus",
        scene::CameraView::Rear => "Vue arrière",
        scene::CameraView::Iso => "Vue isométrique",
    }
}

fn set_camera_goal(
    rig: &mut scene::OrbitCameraRig,
    goal: Option<scene::CameraGoal>,
    message: &mut String,
    label: &str,
) {
    if let Some(goal) = goal {
        rig.goal = Some(goal);
        *message = format!("{label}.");
    }
}

/// Bounds of the whole design, or of the selected parts only. `F` on a multiple selection returns
/// their common centre and a distance that fits every member, not just the primary part.
fn camera_goal_for(
    session: &missile::MissileSession,
    only_selection: bool,
    viewport_height: f32,
    yaw: f32,
    pitch: f32,
) -> Option<scene::CameraGoal> {
    let only_selection = only_selection && !session.selection.is_empty();
    let mut points = Vec::new();
    let mut extent = 0.0_f32;
    for pose in session.vehicle.poses() {
        if only_selection && !session.is_selected(&pose.id) {
            continue;
        }
        points.push(missile::world_point(&session.vehicle, pose.x_m, pose.y_m, pose.z_m));
        extent = extent.max(pose.height_m.max(pose.radius_m * 2.0) as f32 * missile::VISUAL_SCALE);
    }
    if points.is_empty() {
        return None;
    }
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for point in points {
        min = min.min(point);
        max = max.max(point);
    }
    let target = missile::framing_target(&session.vehicle, (min + max) * 0.5, viewport_height);
    Some(scene::CameraGoal {
        target,
        yaw,
        pitch,
        distance: ((max - min).length() * 1.15 + extent * 1.6 + 6.0)
            .clamp(12.0, if only_selection { 55.0 } else { 80.0 }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_opens_above_the_supported_minimum_resolution() {
        assert!(DEFAULT_WINDOW.0 as f32 >= MIN_WINDOW.0);
        assert!(DEFAULT_WINDOW.1 as f32 >= MIN_WINDOW.1);
        assert_eq!(MIN_WINDOW, (1280.0, 720.0));
    }
}
