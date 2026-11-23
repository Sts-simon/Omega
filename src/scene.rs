use crate::campus::{BuildingVisual, FacilityKind};
use bevy::prelude::*;

#[derive(Component, Clone, Copy, Debug)]
pub struct OrbitCameraRig {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // A deliberately simple research island: readability matters more than asset detail.
    let ocean = materials.add(Color::srgb(0.035, 0.16, 0.22));
    let terrain = materials.add(Color::srgb(0.22, 0.34, 0.25));
    let concrete = materials.add(Color::srgb(0.38, 0.41, 0.43));
    let hangar = materials.add(Color::srgb(0.62, 0.66, 0.68));
    let lab = materials.add(Color::srgb(0.34, 0.55, 0.64));
    let accent = materials.add(Color::srgb(0.86, 0.48, 0.20));
    let dark = materials.add(Color::srgb(0.16, 0.19, 0.21));

    spawn_box(
        &mut commands,
        &mut meshes,
        ocean.clone(),
        Vec3::new(0.0, -2.0, 0.0),
        Vec3::new(180.0, 2.0, 180.0),
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        terrain,
        Vec3::new(0.0, -0.45, 0.0),
        Vec3::new(66.0, 1.2, 48.0),
    );

    // Dedicated assembly pad in front of the workshop.
    spawn_box(
        &mut commands,
        &mut meshes,
        concrete.clone(),
        Vec3::new(0.0, 0.34, 18.0),
        Vec3::new(10.0, 0.30, 8.0),
    );

    // Main access road and central workshop.
    spawn_box(
        &mut commands,
        &mut meshes,
        concrete.clone(),
        Vec3::new(0.0, 0.18, 0.0),
        Vec3::new(5.0, 0.12, 39.0),
    );
    spawn_facility_box(
        &mut commands,
        &mut meshes,
        hangar.clone(),
        Vec3::new(-10.0, 3.2, -7.0),
        Vec3::new(18.0, 6.4, 13.0),
        FacilityKind::Construction,
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        dark.clone(),
        Vec3::new(-10.0, 6.5, -7.0),
        Vec3::new(18.5, 0.35, 13.5),
    );

    // Research centre: dedicated to technology development.
    spawn_facility_box(
        &mut commands,
        &mut meshes,
        lab.clone(),
        Vec3::new(12.0, 2.5, -10.0),
        Vec3::new(12.0, 5.0, 10.0),
        FacilityKind::Research,
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        dark.clone(),
        Vec3::new(12.0, 5.1, -10.0),
        Vec3::new(12.5, 0.3, 10.5),
    );

    // Programming studio: separate building for the visual sequence editor.
    spawn_facility_box(
        &mut commands,
        &mut meshes,
        hangar.clone(),
        Vec3::new(-11.0, 2.2, 10.0),
        Vec3::new(12.0, 4.4, 9.0),
        FacilityKind::Programming,
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        dark.clone(),
        Vec3::new(-11.0, 4.5, 10.0),
        Vec3::new(12.5, 0.3, 9.5),
    );

    spawn_facility_box(
        &mut commands,
        &mut meshes,
        accent.clone(),
        Vec3::new(21.0, 2.0, 8.0),
        Vec3::new(8.0, 4.0, 8.0),
        FacilityKind::Contracts,
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        dark.clone(),
        Vec3::new(21.0, 4.1, 8.0),
        Vec3::new(8.5, 0.3, 8.5),
    );

    // Small coloured markers make the four interactive facilities easy to identify.
    spawn_box(
        &mut commands,
        &mut meshes,
        accent.clone(),
        Vec3::new(26.0, 1.0, 8.0),
        Vec3::new(0.7, 2.0, 0.7),
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        lab.clone(),
        Vec3::new(12.0, 0.9, -16.0),
        Vec3::new(0.7, 1.8, 0.7),
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        hangar.clone(),
        Vec3::new(-10.0, 0.9, 1.5),
        Vec3::new(0.7, 1.8, 0.7),
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        hangar.clone(),
        Vec3::new(-11.0, 0.9, 5.5),
        Vec3::new(0.7, 1.8, 0.7),
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        lab.clone(),
        Vec3::new(5.0, 0.9, -24.0),
        Vec3::new(0.7, 1.8, 0.7),
    );

    // Generic test building and storage blocks. No flight or weapon systems are modelled.
    spawn_box(
        &mut commands,
        &mut meshes,
        concrete.clone(),
        Vec3::new(-25.0, 1.0, 15.0),
        Vec3::new(8.0, 2.0, 6.0),
    );
    spawn_box(
        &mut commands,
        &mut meshes,
        hangar.clone(),
        Vec3::new(7.0, 1.7, 13.0),
        Vec3::new(9.0, 3.4, 7.0),
    );

    let camera_target = Vec3::new(0.0, 2.0, 13.0);
    let camera_position = Vec3::new(42.0, 29.0, 42.0);
    let camera_offset = camera_position - camera_target;
    let camera_rig = OrbitCameraRig {
        target: camera_target,
        yaw: camera_offset.x.atan2(camera_offset.z),
        pitch: (camera_offset.y / camera_offset.length()).asin(),
        distance: camera_offset.length(),
    };
    commands.spawn((
        Camera3d::default(),
        camera_rig,
        Transform::from_translation(camera_position).looking_at(camera_target, Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 13_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, -0.55, 0.0)),
    ));
}

fn spawn_facility_box(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    material: Handle<StandardMaterial>,
    position: Vec3,
    size: Vec3,
    facility: FacilityKind,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
        MeshMaterial3d(material),
        Transform::from_translation(position),
        BuildingVisual {
            facility,
            half_extents: size * 0.5,
        },
    ));
}

fn spawn_box(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    material: Handle<StandardMaterial>,
    position: Vec3,
    size: Vec3,
) {
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
        MeshMaterial3d(material),
        Transform::from_translation(position),
    ));
}
