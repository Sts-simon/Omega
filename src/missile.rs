//! Missile workshop: a staged vehicle built from resizable parts (`omega-rules::vehicle`),
//! edited with buttons and shown in 3D. Parts are gated by research capabilities.
//! Everything here is fictional and abstract; see `docs/VISION_MISSILES.md`.

use crate::campus::{CampusState, CampusView};
use crate::save::{self, SaveFile};
use crate::scene::OrbitCameraRig;
use bevy::prelude::*;
use omega_rules::{ComputerClass, Part, PartSpec, RadarClass, SensorKind, Stage, Vehicle};
use std::collections::BTreeSet;

const ANCHOR: Vec3 = Vec3::new(0.0, 0.2, 18.0);
const VISUAL_SCALE: f32 = 1.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PartTemplate {
    SolidMotor,
    FuelTank,
    LiquidEngine,
    AirBreathing,
    Pitot,
    Barometer,
    RadarBasic,
    RadarActive,
    ComputerBasic,
    ComputerStandard,
    ComputerAdvanced,
    Decoupler,
    MissionModule,
}

impl PartTemplate {
    pub fn label(self) -> &'static str {
        match self {
            Self::SolidMotor => "SOLID MOTOR",
            Self::FuelTank => "FUEL TANK",
            Self::LiquidEngine => "LIQUID ENGINE",
            Self::AirBreathing => "TURBOFAN",
            Self::Pitot => "PITOT TUBE",
            Self::Barometer => "BAROMETER",
            Self::RadarBasic => "RADAR BASIC",
            Self::RadarActive => "RADAR ACTIVE",
            Self::ComputerBasic => "COMPUTER 1",
            Self::ComputerStandard => "COMPUTER 2",
            Self::ComputerAdvanced => "COMPUTER 3",
            Self::Decoupler => "DECOUPLER",
            Self::MissionModule => "MISSION MODULE",
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::SolidMotor => "motor",
            Self::FuelTank => "tank",
            Self::LiquidEngine => "engine",
            Self::AirBreathing => "turbofan",
            Self::Pitot => "pitot",
            Self::Barometer => "baro",
            Self::RadarBasic | Self::RadarActive => "radar",
            Self::ComputerBasic | Self::ComputerStandard | Self::ComputerAdvanced => "computer",
            Self::Decoupler => "decoupler",
            Self::MissionModule => "module",
        }
    }

    pub fn spec(self) -> PartSpec {
        match self {
            Self::SolidMotor => PartSpec::SolidMotor {
                length_m: 1.5,
                diameter_m: 0.4,
                burn_time_s: 15.0,
            },
            Self::FuelTank => PartSpec::FuelTank {
                length_m: 1.5,
                diameter_m: 0.4,
            },
            Self::LiquidEngine => PartSpec::LiquidEngine { scale: 1.0 },
            Self::AirBreathing => PartSpec::AirBreathing { scale: 1.0 },
            Self::Pitot => PartSpec::Sensor {
                kind: SensorKind::Pitot,
            },
            Self::Barometer => PartSpec::Sensor {
                kind: SensorKind::Barometer,
            },
            Self::RadarBasic => PartSpec::Sensor {
                kind: SensorKind::Radar(RadarClass::Basic),
            },
            Self::RadarActive => PartSpec::Sensor {
                kind: SensorKind::Radar(RadarClass::Active),
            },
            Self::ComputerBasic => PartSpec::Computer {
                class: ComputerClass::Basic,
            },
            Self::ComputerStandard => PartSpec::Computer {
                class: ComputerClass::Standard,
            },
            Self::ComputerAdvanced => PartSpec::Computer {
                class: ComputerClass::Advanced,
            },
            Self::Decoupler => PartSpec::Decoupler,
            Self::MissionModule => PartSpec::MissionModule { mass_kg: 40.0 },
        }
    }

    pub fn is_computer(self) -> bool {
        matches!(
            self,
            Self::ComputerBasic | Self::ComputerStandard | Self::ComputerAdvanced
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Param {
    Length,
    Diameter,
    BurnTime,
    Scale,
    Mass,
}

impl Param {
    pub fn label(self) -> &'static str {
        match self {
            Self::Length => "Length (m)",
            Self::Diameter => "Diameter (m)",
            Self::BurnTime => "Burn time (s)",
            Self::Scale => "Engine scale",
            Self::Mass => "Mass (kg)",
        }
    }

    fn step(self) -> f64 {
        match self {
            Self::Length => 0.1,
            Self::Diameter => 0.05,
            Self::BurnTime => 1.0,
            Self::Scale => 0.1,
            Self::Mass => 5.0,
        }
    }
}

/// Adjustable parameters of a part, with their current values.
pub(crate) fn params_of(spec: &PartSpec) -> Vec<(Param, f64)> {
    match *spec {
        PartSpec::SolidMotor {
            length_m,
            diameter_m,
            burn_time_s,
        } => {
            vec![
                (Param::Length, length_m),
                (Param::Diameter, diameter_m),
                (Param::BurnTime, burn_time_s),
            ]
        }
        PartSpec::FuelTank { length_m, diameter_m } => vec![(Param::Length, length_m), (Param::Diameter, diameter_m)],
        PartSpec::LiquidEngine { scale } | PartSpec::AirBreathing { scale } => {
            vec![(Param::Scale, scale)]
        }
        PartSpec::MissionModule { mass_kg } => vec![(Param::Mass, mass_kg)],
        _ => Vec::new(),
    }
}

/// `direction` is +1 or -1; the result is clamped to the part's valid range.
pub(crate) fn adjusted(spec: PartSpec, param: Param, direction: f64) -> Option<PartSpec> {
    let round = |v: f64| (v * 1000.0).round() / 1000.0;
    let delta = param.step() * direction;
    let changed = match (spec, param) {
        (
            PartSpec::SolidMotor {
                length_m,
                diameter_m,
                burn_time_s,
            },
            Param::Length,
        ) => PartSpec::SolidMotor {
            length_m: round((length_m + delta).clamp(0.3, 8.0)),
            diameter_m,
            burn_time_s,
        },
        (
            PartSpec::SolidMotor {
                length_m,
                diameter_m,
                burn_time_s,
            },
            Param::Diameter,
        ) => PartSpec::SolidMotor {
            length_m,
            diameter_m: round((diameter_m + delta).clamp(0.1, 1.2)),
            burn_time_s,
        },
        (
            PartSpec::SolidMotor {
                length_m,
                diameter_m,
                burn_time_s,
            },
            Param::BurnTime,
        ) => PartSpec::SolidMotor {
            length_m,
            diameter_m,
            burn_time_s: round((burn_time_s + delta).clamp(1.0, 60.0)),
        },
        (PartSpec::FuelTank { length_m, diameter_m }, Param::Length) => PartSpec::FuelTank {
            length_m: round((length_m + delta).clamp(0.3, 8.0)),
            diameter_m,
        },
        (PartSpec::FuelTank { length_m, diameter_m }, Param::Diameter) => PartSpec::FuelTank {
            length_m,
            diameter_m: round((diameter_m + delta).clamp(0.1, 1.2)),
        },
        (PartSpec::LiquidEngine { scale }, Param::Scale) => PartSpec::LiquidEngine {
            scale: round((scale + delta).clamp(0.3, 3.0)),
        },
        (PartSpec::AirBreathing { scale }, Param::Scale) => PartSpec::AirBreathing {
            scale: round((scale + delta).clamp(0.3, 3.0)),
        },
        (PartSpec::MissionModule { mass_kg }, Param::Mass) => PartSpec::MissionModule {
            mass_kg: round((mass_kg + delta).clamp(1.0, 300.0)),
        },
        _ => return None,
    };
    Some(changed)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MissileAction {
    AddPart(PartTemplate),
    Select { stage: usize, part: usize },
    ActiveStage(usize),
    Adjust(Param, i8),
    RemovePart,
    NewStage,
    RemoveStage,
    Preset(u8),
    Clear,
}

#[derive(Resource, Default)]
pub(crate) struct MissileSession {
    pub vehicle: Vehicle,
    pub selected: Option<(usize, usize)>,
    pub active_stage: usize,
    next_id: u32,
}

fn part(id: &str, spec: PartSpec) -> Part {
    Part {
        id: id.to_string(),
        spec,
    }
}

impl MissileSession {
    pub fn new_default() -> Self {
        let mut session = Self::default();
        session.load_preset(0);
        session
    }

    fn fresh_id(&mut self, prefix: &str) -> String {
        self.next_id += 1;
        format!("{prefix}-{}", self.next_id)
    }

    fn sync_next_id(&mut self) {
        self.next_id = self
            .vehicle
            .parts()
            .filter_map(|(_, p)| p.id.rsplit('-').next().and_then(|n| n.parse::<u32>().ok()))
            .max()
            .unwrap_or(0);
    }

    fn move_computer_to_last_stage(&mut self) {
        let last = self.vehicle.stages.len().saturating_sub(1);
        let mut found = None;
        for (s, stage) in self.vehicle.stages.iter().enumerate() {
            if let Some(p) = stage
                .parts
                .iter()
                .position(|p| matches!(p.spec, PartSpec::Computer { .. }))
            {
                found = Some((s, p));
            }
        }
        if let Some((s, p)) = found
            && s != last
        {
            let computer = self.vehicle.stages[s].parts.remove(p);
            self.vehicle.stages[last].parts.push(computer);
        }
        self.selected = None;
    }

    pub fn selected_part(&self) -> Option<&Part> {
        let (s, p) = self.selected?;
        self.vehicle.stages.get(s)?.parts.get(p)
    }

    pub fn load_preset(&mut self, index: u8) {
        self.vehicle = match index {
            0 => Vehicle {
                name: "Two-stage rocket".into(),
                stages: vec![
                    Stage {
                        parts: vec![
                            part(
                                "motor-1",
                                PartSpec::SolidMotor {
                                    length_m: 2.0,
                                    diameter_m: 0.4,
                                    burn_time_s: 20.0,
                                },
                            ),
                            part("decoupler-2", PartSpec::Decoupler),
                        ],
                    },
                    Stage {
                        parts: vec![
                            part(
                                "motor-3",
                                PartSpec::SolidMotor {
                                    length_m: 1.0,
                                    diameter_m: 0.3,
                                    burn_time_s: 25.0,
                                },
                            ),
                            part(
                                "computer-4",
                                PartSpec::Computer {
                                    class: ComputerClass::Basic,
                                },
                            ),
                            part(
                                "pitot-5",
                                PartSpec::Sensor {
                                    kind: SensorKind::Pitot,
                                },
                            ),
                            part("module-6", PartSpec::MissionModule { mass_kg: 40.0 }),
                        ],
                    },
                ],
            },
            _ => Vehicle {
                name: "Cruise vehicle".into(),
                stages: vec![Stage {
                    parts: vec![
                        part(
                            "tank-1",
                            PartSpec::FuelTank {
                                length_m: 1.5,
                                diameter_m: 0.3,
                            },
                        ),
                        part("turbofan-2", PartSpec::AirBreathing { scale: 1.0 }),
                        part(
                            "computer-3",
                            PartSpec::Computer {
                                class: ComputerClass::Basic,
                            },
                        ),
                        part(
                            "pitot-4",
                            PartSpec::Sensor {
                                kind: SensorKind::Pitot,
                            },
                        ),
                        part("module-5", PartSpec::MissionModule { mass_kg: 30.0 }),
                    ],
                }],
            },
        };
        self.active_stage = self.vehicle.stages.len().saturating_sub(1);
        self.selected = None;
        self.sync_next_id();
    }

    /// Apply a button action. Returns the status message to show.
    pub fn apply(&mut self, action: MissileAction, owned: &BTreeSet<String>) -> String {
        match action {
            MissileAction::AddPart(template) => self.add_part(template, owned),
            MissileAction::Select { stage, part } => {
                if self.vehicle.stages.get(stage).and_then(|s| s.parts.get(part)).is_some() {
                    self.selected = Some((stage, part));
                    self.active_stage = stage;
                }
                String::new()
            }
            MissileAction::ActiveStage(stage) => {
                if stage < self.vehicle.stages.len() {
                    self.active_stage = stage;
                }
                format!("New parts go to stage {}", self.active_stage + 1)
            }
            MissileAction::Adjust(param, direction) => {
                let Some((s, p)) = self.selected else {
                    return "Select a part first".to_string();
                };
                let Some(current) = self.vehicle.stages.get(s).and_then(|st| st.parts.get(p)) else {
                    return String::new();
                };
                match adjusted(current.spec, param, f64::from(direction)) {
                    Some(spec) => {
                        self.vehicle.stages[s].parts[p].spec = spec;
                        String::new()
                    }
                    None => "This part has no such setting".to_string(),
                }
            }
            MissileAction::RemovePart => {
                let Some((s, p)) = self.selected else {
                    return "Select a part first".to_string();
                };
                if s < self.vehicle.stages.len() && p < self.vehicle.stages[s].parts.len() {
                    let removed = self.vehicle.stages[s].parts.remove(p);
                    self.selected = None;
                    return format!("Removed {}", removed.id);
                }
                String::new()
            }
            MissileAction::NewStage => {
                if self.vehicle.stages.len() >= 4 {
                    return "A vehicle can have at most 4 stages".to_string();
                }
                self.vehicle.stages.push(Stage::default());
                self.active_stage = self.vehicle.stages.len() - 1;
                self.move_computer_to_last_stage();
                format!(
                    "Stage {} added; the flight computer moved into it",
                    self.vehicle.stages.len()
                )
            }
            MissileAction::RemoveStage => {
                if self.vehicle.stages.len() <= 1 {
                    return "A vehicle needs at least one stage".to_string();
                }
                let index = self.active_stage.min(self.vehicle.stages.len() - 1);
                self.vehicle.stages.remove(index);
                self.active_stage = self.vehicle.stages.len() - 1;
                self.move_computer_to_last_stage();
                format!("Stage {} removed", index + 1)
            }
            MissileAction::Preset(index) => {
                let preset_vehicle = Self {
                    next_id: 0,
                    ..Self::default()
                };
                let mut candidate = preset_vehicle;
                candidate.load_preset(index);
                let missing = candidate.vehicle.missing_research(owned);
                if !missing.is_empty() {
                    return format!("Preset locked: research {} first", missing.join(", "));
                }
                *self = candidate;
                format!("Loaded preset: {}", self.vehicle.name)
            }
            MissileAction::Clear => {
                self.vehicle = Vehicle {
                    name: "New vehicle".into(),
                    stages: vec![Stage::default()],
                };
                self.active_stage = 0;
                self.selected = None;
                self.next_id = 0;
                "Started an empty vehicle".to_string()
            }
        }
    }

    fn add_part(&mut self, template: PartTemplate, owned: &BTreeSet<String>) -> String {
        let spec = template.spec();
        let capability = spec.required_capability();
        if !owned.contains(capability) {
            return format!("Locked: research {capability} first");
        }
        if self.vehicle.stages.is_empty() {
            self.vehicle.stages.push(Stage::default());
        }
        if template.is_computer() {
            // Only one computer: replace the existing one and keep it in the last stage.
            for stage in &mut self.vehicle.stages {
                stage.parts.retain(|p| !matches!(p.spec, PartSpec::Computer { .. }));
            }
            let id = self.fresh_id(template.prefix());
            let last = self.vehicle.stages.len() - 1;
            self.vehicle.stages[last].parts.push(Part { id, spec });
            self.selected = None;
            return format!("Flight computer set to {}", template.label());
        }
        let stage = self.active_stage.min(self.vehicle.stages.len() - 1);
        let id = self.fresh_id(template.prefix());
        self.vehicle.stages[stage].parts.push(Part { id: id.clone(), spec });
        self.selected = Some((stage, self.vehicle.stages[stage].parts.len() - 1));
        format!("Added {id} to stage {}", stage + 1)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(&self.vehicle).map_err(|e| e.to_string())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let vehicle: Vehicle = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        for stage in &vehicle.stages {
            for p in &stage.parts {
                p.spec.validate().map_err(|e| format!("{}: {e}", p.id))?;
            }
        }
        let mut session = Self {
            vehicle,
            ..Self::default()
        };
        session.active_stage = session.vehicle.stages.len().saturating_sub(1);
        session.sync_next_id();
        Ok(session)
    }
}

// ----------------------------------------------------------------------------------------------
// Layout and 3D visuals
// ----------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Shape {
    Body,
    Nozzle,
    Sensor,
    Disc,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PartBox {
    pub stage: usize,
    pub index: usize,
    pub y_bottom: f32,
    pub height: f32,
    pub radius: f32,
    pub shape: Shape,
    pub color: [f32; 3],
}

fn order_key(spec: &PartSpec) -> u8 {
    match spec {
        PartSpec::LiquidEngine { .. } | PartSpec::AirBreathing { .. } => 0,
        PartSpec::SolidMotor { .. } | PartSpec::FuelTank { .. } => 1,
        PartSpec::Decoupler => 3,
        _ => 2,
    }
}

/// Stack the parts from the bottom (stage 1) to the top, engines first inside each stage.
pub(crate) fn layout(vehicle: &Vehicle) -> Vec<PartBox> {
    let body_radius = (vehicle.max_diameter_m() / 2.0).max(0.1) as f32;
    let mut boxes = Vec::new();
    let mut y = 0.0_f32;
    for (s, stage) in vehicle.stages.iter().enumerate() {
        let mut order: Vec<usize> = (0..stage.parts.len()).collect();
        order.sort_by_key(|i| order_key(&stage.parts[*i].spec));
        for i in order {
            let (height, radius, shape, color) = match stage.parts[i].spec {
                PartSpec::SolidMotor {
                    length_m, diameter_m, ..
                } => (
                    length_m as f32,
                    diameter_m as f32 / 2.0,
                    Shape::Body,
                    [0.86, 0.42, 0.22],
                ),
                PartSpec::FuelTank { length_m, diameter_m } => (
                    length_m as f32,
                    diameter_m as f32 / 2.0,
                    Shape::Body,
                    [0.82, 0.86, 0.90],
                ),
                PartSpec::LiquidEngine { scale } => (
                    0.35 * scale as f32,
                    0.15 * scale as f32,
                    Shape::Nozzle,
                    [0.35, 0.38, 0.42],
                ),
                PartSpec::AirBreathing { scale } => (
                    0.5 * scale as f32,
                    0.17 * scale as f32,
                    Shape::Nozzle,
                    [0.45, 0.48, 0.55],
                ),
                PartSpec::Sensor {
                    kind: SensorKind::Pitot,
                } => (0.25, 0.03, Shape::Sensor, [0.35, 0.84, 0.94]),
                PartSpec::Sensor {
                    kind: SensorKind::Barometer,
                } => (0.12, body_radius * 0.9, Shape::Disc, [0.35, 0.74, 0.80]),
                PartSpec::Sensor {
                    kind: SensorKind::Radar(RadarClass::Basic),
                } => (0.2, body_radius * 0.95, Shape::Disc, [0.30, 0.78, 0.60]),
                PartSpec::Sensor {
                    kind: SensorKind::Radar(RadarClass::Active),
                } => (0.28, body_radius, Shape::Disc, [0.35, 0.90, 0.55]),
                PartSpec::Computer { .. } => (0.15, body_radius * 0.9, Shape::Disc, [0.40, 0.78, 0.40]),
                PartSpec::Decoupler => (0.06, body_radius * 1.05, Shape::Disc, [0.95, 0.80, 0.20]),
                PartSpec::MissionModule { mass_kg } => (
                    (0.2 + mass_kg as f32 / 150.0).min(2.2),
                    body_radius,
                    Shape::Body,
                    [0.40, 0.55, 0.92],
                ),
            };
            boxes.push(PartBox {
                stage: s,
                index: i,
                y_bottom: y,
                height,
                radius,
                shape,
                color,
            });
            y += height;
        }
    }
    boxes
}

#[derive(Component)]
pub(crate) struct MissileRoot;

pub(crate) fn setup_missile(mut commands: Commands) {
    commands.spawn((
        Transform::from_translation(ANCHOR).with_scale(Vec3::splat(VISUAL_SCALE)),
        Visibility::Hidden,
        MissileRoot,
    ));
}

pub(crate) fn load_missile(mut session: ResMut<MissileSession>) {
    match save::read(SaveFile::Vehicle) {
        Ok(Some(bytes)) => match MissileSession::from_bytes(&bytes) {
            Ok(loaded) => *session = loaded,
            Err(error) => warn!("Could not read the saved vehicle: {error}"),
        },
        Ok(None) => {}
        Err(error) => warn!("Could not read the saved vehicle: {error}"),
    }
}

pub(crate) fn persist_missile(session: &MissileSession) -> Result<(), String> {
    session
        .to_bytes()
        .and_then(|bytes| save::write(SaveFile::Vehicle, &bytes))
}

/// Rebuild the 3D stack whenever the vehicle or the selection changes.
pub(crate) fn sync_missile_visuals(
    mut commands: Commands,
    campus: Res<CampusState>,
    session: Res<MissileSession>,
    mut root: Query<(Entity, &mut Visibility), With<MissileRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut last: Local<String>,
) {
    let Ok((entity, mut visibility)) = root.single_mut() else {
        return;
    };
    let show = campus.view == CampusView::Missile;
    let wanted = if show {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *visibility != wanted {
        *visibility = wanted;
    }
    if !show {
        return;
    }
    let signature = format!("{:?}|{:?}", session.vehicle, session.selected);
    if *last == signature {
        return;
    }
    *last = signature;
    commands.entity(entity).despawn_related::<Children>();
    let boxes = layout(&session.vehicle);
    let top = boxes.iter().map(|b| b.y_bottom + b.height).fold(0.0, f32::max);
    let nose_radius = boxes.last().map_or(0.15, |b| b.radius.max(0.08));
    commands.entity(entity).with_children(|root| {
        for b in &boxes {
            let selected = session.selected == Some((b.stage, b.index));
            let [r, g, bl] = b.color;
            let material = materials.add(StandardMaterial {
                base_color: Color::srgb(r, g, bl),
                emissive: if selected {
                    LinearRgba::rgb(0.25, 0.55, 0.65)
                } else {
                    LinearRgba::BLACK
                },
                perceptual_roughness: 0.6,
                ..default()
            });
            let mid = b.y_bottom + b.height / 2.0;
            let mesh = match b.shape {
                Shape::Nozzle => meshes.add(Cone::new(b.radius, b.height)),
                Shape::Body | Shape::Sensor | Shape::Disc => meshes.add(Cylinder::new(b.radius, b.height)),
            };
            root.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_xyz(0.0, mid, 0.0),
            ));
        }
        if !boxes.is_empty() {
            let nose = materials.add(StandardMaterial {
                base_color: Color::srgb(0.88, 0.90, 0.94),
                perceptual_roughness: 0.5,
                ..default()
            });
            let height = (nose_radius * 3.0).max(0.3);
            root.spawn((
                Mesh3d(meshes.add(Cone::new(nose_radius, height))),
                MeshMaterial3d(nose),
                Transform::from_xyz(0.0, top + height / 2.0, 0.0),
            ));
        }
    });
}

/// Centre the camera on the vehicle when the workshop is opened.
pub(crate) fn focus_missile_camera(
    campus: Res<CampusState>,
    session: Res<MissileSession>,
    mut cameras: Query<(&mut Transform, &mut OrbitCameraRig), With<Camera3d>>,
    mut previous: Local<Option<CampusView>>,
) {
    let current = campus.view;
    let entered = *previous != Some(CampusView::Missile) && current == CampusView::Missile;
    *previous = Some(current);
    if !entered {
        return;
    }
    let Ok((mut transform, mut rig)) = cameras.single_mut() else {
        return;
    };
    let height: f32 = layout(&session.vehicle).iter().map(|b| b.height).sum();
    rig.target = ANCHOR + Vec3::new(0.0, height * VISUAL_SCALE * 0.5, 0.0);
    rig.distance = (14.0 + height * 2.5).clamp(14.0, 40.0);
    rig.yaw = 0.7;
    rig.pitch = 0.30;
    let horizontal = rig.pitch.cos() * rig.distance;
    transform.translation = rig.target
        + Vec3::new(
            rig.yaw.sin() * horizontal,
            rig.pitch.sin() * rig.distance,
            rig.yaw.cos() * horizontal,
        );
    transform.look_at(rig.target, Vec3::Y);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn everything_owned() -> BTreeSet<String> {
        let tree = omega_rules::default_tree();
        let state = omega_rules::ResearchState {
            completed: tree.iter().map(|n| n.id.clone()).collect(),
            active: None,
        };
        state.owned_capabilities(&tree)
    }

    fn starting() -> BTreeSet<String> {
        omega_rules::ResearchState::default().owned_capabilities(&omega_rules::default_tree())
    }

    #[test]
    fn default_preset_is_a_valid_two_stage_vehicle() {
        let session = MissileSession::new_default();
        assert!(session.vehicle.validate().is_ok());
        assert_eq!(session.vehicle.stages.len(), 2);
        assert_eq!(session.active_stage, 1);
    }

    #[test]
    fn adjusting_a_part_resizes_it_within_limits() {
        let mut session = MissileSession::new_default();
        let owned = starting();
        session.apply(MissileAction::Select { stage: 0, part: 0 }, &owned);
        let before = session.vehicle.stages[0].parts[0].spec.stats();
        session.apply(MissileAction::Adjust(Param::Length, 1), &owned);
        session.apply(MissileAction::Adjust(Param::BurnTime, -1), &owned);
        let after = session.vehicle.stages[0].parts[0].spec.stats();
        assert!(after.propellant_kg > before.propellant_kg);
        assert!(
            after.thrust_n > before.thrust_n,
            "longer motor and shorter burn = more thrust"
        );
        for _ in 0..200 {
            session.apply(MissileAction::Adjust(Param::BurnTime, -1), &owned);
        }
        assert_eq!(
            params_of(&session.vehicle.stages[0].parts[0].spec)[2].1,
            1.0,
            "clamped at the minimum burn time"
        );
        assert!(session.vehicle.stages[0].parts[0].spec.validate().is_ok());
        assert!(
            session
                .apply(MissileAction::Adjust(Param::Scale, 1), &owned)
                .contains("no such setting")
        );
    }

    #[test]
    fn locked_parts_cannot_be_added_until_researched() {
        let mut session = MissileSession::new_default();
        let message = session.apply(MissileAction::AddPart(PartTemplate::RadarActive), &starting());
        assert!(
            message.contains("Locked") && message.contains("sensor:radar_active"),
            "{message}"
        );
        let count = session.vehicle.parts().count();
        assert_eq!(session.vehicle.parts().count(), count);
        let ok = session.apply(MissileAction::AddPart(PartTemplate::RadarActive), &everything_owned());
        assert!(ok.contains("Added"), "{ok}");
        assert_eq!(session.vehicle.parts().count(), count + 1);
    }

    #[test]
    fn a_second_computer_replaces_the_first_and_stays_in_the_last_stage() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.apply(MissileAction::AddPart(PartTemplate::ComputerAdvanced), &owned);
        let computers: Vec<_> = session
            .vehicle
            .parts()
            .filter(|(_, p)| matches!(p.spec, PartSpec::Computer { .. }))
            .collect();
        assert_eq!(computers.len(), 1);
        assert_eq!(session.vehicle.computer(), Some(ComputerClass::Advanced));
        session.apply(MissileAction::NewStage, &owned);
        assert_eq!(session.vehicle.stages.len(), 3);
        assert!(
            session.vehicle.stages[2]
                .parts
                .iter()
                .any(|p| matches!(p.spec, PartSpec::Computer { .. })),
            "computer follows the new last stage"
        );
    }

    #[test]
    fn stages_can_be_added_and_removed_with_limits() {
        let mut session = MissileSession::new_default();
        let owned = starting();
        session.apply(MissileAction::NewStage, &owned);
        session.apply(MissileAction::NewStage, &owned);
        assert!(session.apply(MissileAction::NewStage, &owned).contains("at most 4"));
        for _ in 0..5 {
            session.apply(MissileAction::RemoveStage, &owned);
        }
        assert_eq!(session.vehicle.stages.len(), 1);
        assert!(
            session
                .apply(MissileAction::RemoveStage, &owned)
                .contains("at least one")
        );
    }

    #[test]
    fn presets_respect_research_and_ids_stay_unique() {
        let mut session = MissileSession::new_default();
        assert!(session.apply(MissileAction::Preset(1), &starting()).contains("locked"));
        assert_eq!(
            session.vehicle.name, "Two-stage rocket",
            "a locked preset does not replace the design"
        );
        assert!(
            session
                .apply(MissileAction::Preset(1), &everything_owned())
                .contains("Cruise vehicle")
        );
        let owned = everything_owned();
        for _ in 0..3 {
            session.apply(MissileAction::AddPart(PartTemplate::FuelTank), &owned);
        }
        let ids: Vec<&str> = session.vehicle.parts().map(|(_, p)| p.id.as_str()).collect();
        let unique: BTreeSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "{ids:?}");
    }

    #[test]
    fn remove_part_and_clear_work() {
        let mut session = MissileSession::new_default();
        let owned = starting();
        let before = session.vehicle.parts().count();
        session.apply(MissileAction::Select { stage: 1, part: 3 }, &owned);
        assert!(session.apply(MissileAction::RemovePart, &owned).starts_with("Removed"));
        assert_eq!(session.vehicle.parts().count(), before - 1);
        session.apply(MissileAction::Clear, &owned);
        assert_eq!(session.vehicle.parts().count(), 0);
        assert!(session.vehicle.validate().is_err());
    }

    #[test]
    fn layout_stacks_bottom_up_with_engines_first_in_each_stage() {
        let mut session = MissileSession::new_default();
        session.apply(MissileAction::Preset(1), &everything_owned());
        let boxes = layout(&session.vehicle);
        assert_eq!(boxes.len(), 5);
        assert!(
            boxes
                .windows(2)
                .all(|w| (w[0].y_bottom + w[0].height - w[1].y_bottom).abs() < 1e-6),
            "contiguous stack"
        );
        assert_eq!(boxes[0].shape, Shape::Nozzle, "the turbofan sits at the bottom");
        let two_stage = layout(&MissileSession::new_default().vehicle);
        assert!(
            two_stage
                .iter()
                .filter(|b| b.stage == 0)
                .all(|b| b.y_bottom < two_stage.iter().find(|x| x.stage == 1).unwrap().y_bottom + 1e-6)
        );
    }

    #[test]
    fn saving_and_loading_round_trips_and_rejects_bad_parts() {
        let session = MissileSession::new_default();
        let bytes = session.to_bytes().unwrap();
        let loaded = MissileSession::from_bytes(&bytes).unwrap();
        assert_eq!(loaded.vehicle, session.vehicle);
        let mut bad = session.vehicle.clone();
        bad.stages[0].parts[0].spec = PartSpec::SolidMotor {
            length_m: 99.0,
            diameter_m: 0.4,
            burn_time_s: 10.0,
        };
        assert!(MissileSession::from_bytes(&serde_json::to_vec(&bad).unwrap()).is_err());
        assert!(MissileSession::from_bytes(b"not json").is_err());
    }

    #[test]
    fn locked_messages_name_the_technology() {
        let mut session = MissileSession::new_default();
        let raw = session.apply(MissileAction::AddPart(PartTemplate::RadarActive), &starting());
        let friendly = crate::campus::friendly_capabilities(&raw);
        assert!(
            friendly.contains("Active radar") && !friendly.contains("sensor:radar_active"),
            "{friendly}"
        );
    }
}
