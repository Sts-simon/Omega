//! Missile workshop: a staged vehicle built from resizable parts (`omega-rules::vehicle`),
//! edited with buttons and shown in 3D. Parts are gated by research capabilities.
//! Everything here is fictional and abstract; see `docs/VISION_MISSILES.md`.

use crate::campus::{CampusState, CampusView};
use crate::save::{self, SaveFile};
use crate::scene::OrbitCameraRig;
use bevy::prelude::*;
use omega_rules::{
    AirfoilProfile, ComputerClass, ControlKind, FinLayout, Part, PartSpec, RadarClass, SensorKind, StabilizerRole,
    Stage, Vehicle, WingPlanform,
};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

/// Notches closer than this to the previous one on the same part and value form one gesture.
const WHEEL_GESTURE: Duration = Duration::from_millis(500);

pub(crate) const ANCHOR: Vec3 = Vec3::new(0.0, 0.2, 18.0);
pub(crate) const VISUAL_SCALE: f32 = 1.6;

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
    Wing,
    FinSet,
    ControlSurface,
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
            Self::Wing => "WING",
            Self::FinSet => "FIN SET",
            Self::ControlSurface => "CONTROL",
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
            Self::Wing => "wing",
            Self::FinSet => "fin",
            Self::ControlSurface => "control",
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
            Self::Wing => PartSpec::Wing {
                span_m: 1.2,
                chord_m: 0.4,
                sweep_deg: 20.0,
                planform: WingPlanform::Swept,
                profile: AirfoilProfile::Classic,
                light: false,
            },
            Self::FinSet => PartSpec::FinSet {
                span_m: 0.35,
                chord_m: 0.4,
                role: StabilizerRole::Rear,
                layout: FinLayout::Symmetric,
                light: false,
            },
            Self::ControlSurface => PartSpec::ControlSurface {
                span_m: 0.35,
                chord_m: 0.35,
                kind: ControlKind::Classic,
                light: false,
            },
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
    Span,
    Chord,
    Sweep,
}

impl Param {
    pub fn label(self) -> &'static str {
        match self {
            Self::Length => "Length (m)",
            Self::Diameter => "Diameter (m)",
            Self::BurnTime => "Burn time (s)",
            Self::Scale => "Engine scale",
            Self::Mass => "Mass (kg)",
            Self::Span => "Span (m)",
            Self::Chord => "Chord (m)",
            Self::Sweep => "Sweep (deg)",
        }
    }

    fn step(self) -> f64 {
        match self {
            Self::Length => 0.1,
            Self::Diameter => 0.05,
            Self::BurnTime => 1.0,
            Self::Scale => 0.1,
            Self::Mass => 5.0,
            Self::Span => 0.05,
            Self::Chord => 0.05,
            Self::Sweep => 5.0,
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
        PartSpec::Wing {
            span_m,
            chord_m,
            sweep_deg,
            ..
        } => vec![
            (Param::Span, span_m),
            (Param::Chord, chord_m),
            (Param::Sweep, sweep_deg),
        ],
        PartSpec::FinSet { span_m, chord_m, .. } | PartSpec::ControlSurface { span_m, chord_m, .. } => {
            vec![(Param::Span, span_m), (Param::Chord, chord_m)]
        }
        _ => Vec::new(),
    }
}

/// Discrete design choices of an aerodynamic part, cycled from the inspector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AeroChoice {
    Planform,
    Profile,
    Role,
    Layout,
    ControlKind,
    Light,
}

impl AeroChoice {
    pub fn label(self) -> &'static str {
        match self {
            Self::Planform => "PLANFORM",
            Self::Profile => "PROFILE",
            Self::Role => "STABILISER",
            Self::Layout => "LAYOUT",
            Self::ControlKind => "SURFACE",
            Self::Light => "BUILD",
        }
    }
}

pub(crate) fn choices_of(spec: &PartSpec) -> Vec<AeroChoice> {
    match *spec {
        PartSpec::Wing { .. } => vec![AeroChoice::Planform, AeroChoice::Profile, AeroChoice::Light],
        PartSpec::FinSet { .. } => vec![AeroChoice::Role, AeroChoice::Layout, AeroChoice::Light],
        PartSpec::ControlSurface { .. } => vec![AeroChoice::ControlKind, AeroChoice::Light],
        _ => Vec::new(),
    }
}

pub(crate) fn choice_value(spec: &PartSpec, choice: AeroChoice) -> String {
    match (*spec, choice) {
        (PartSpec::Wing { planform, .. }, AeroChoice::Planform) => planform.label().to_string(),
        (PartSpec::Wing { profile, .. }, AeroChoice::Profile) => profile.label().to_string(),
        (PartSpec::FinSet { role, .. }, AeroChoice::Role) => role.label().to_string(),
        (PartSpec::FinSet { layout, .. }, AeroChoice::Layout) => layout.label().to_string(),
        (PartSpec::ControlSurface { kind, .. }, AeroChoice::ControlKind) => kind.label().to_string(),
        (
            PartSpec::Wing { light, .. } | PartSpec::FinSet { light, .. } | PartSpec::ControlSurface { light, .. },
            AeroChoice::Light,
        ) => if light { "LIGHT" } else { "STANDARD" }.to_string(),
        _ => "-".to_string(),
    }
}

/// Advance one discrete design choice; `None` when the part does not offer it.
pub(crate) fn cycled(spec: PartSpec, choice: AeroChoice) -> Option<PartSpec> {
    let toggled = |light: bool| !light;
    match (spec, choice) {
        (
            PartSpec::Wing {
                span_m,
                chord_m,
                sweep_deg,
                planform,
                profile,
                light,
            },
            AeroChoice::Planform,
        ) => Some(PartSpec::Wing {
            span_m,
            chord_m,
            sweep_deg,
            planform: planform.next(),
            profile,
            light,
        }),
        (
            PartSpec::Wing {
                span_m,
                chord_m,
                sweep_deg,
                planform,
                profile,
                light,
            },
            AeroChoice::Profile,
        ) => Some(PartSpec::Wing {
            span_m,
            chord_m,
            sweep_deg,
            planform,
            profile: profile.next(),
            light,
        }),
        (
            PartSpec::Wing {
                span_m,
                chord_m,
                sweep_deg,
                planform,
                profile,
                light,
            },
            AeroChoice::Light,
        ) => Some(PartSpec::Wing {
            span_m,
            chord_m,
            sweep_deg,
            planform,
            profile,
            light: toggled(light),
        }),
        (
            PartSpec::FinSet {
                span_m,
                chord_m,
                role,
                layout,
                light,
            },
            AeroChoice::Role,
        ) => Some(PartSpec::FinSet {
            span_m,
            chord_m,
            role: role.next(),
            layout,
            light,
        }),
        (
            PartSpec::FinSet {
                span_m,
                chord_m,
                role,
                layout,
                light,
            },
            AeroChoice::Layout,
        ) => Some(PartSpec::FinSet {
            span_m,
            chord_m,
            role,
            layout: layout.next(),
            light,
        }),
        (
            PartSpec::FinSet {
                span_m,
                chord_m,
                role,
                layout,
                light,
            },
            AeroChoice::Light,
        ) => Some(PartSpec::FinSet {
            span_m,
            chord_m,
            role,
            layout,
            light: toggled(light),
        }),
        (
            PartSpec::ControlSurface {
                span_m,
                chord_m,
                kind,
                light,
            },
            AeroChoice::ControlKind,
        ) => Some(PartSpec::ControlSurface {
            span_m,
            chord_m,
            kind: kind.next(),
            light,
        }),
        (
            PartSpec::ControlSurface {
                span_m,
                chord_m,
                kind,
                light,
            },
            AeroChoice::Light,
        ) => Some(PartSpec::ControlSurface {
            span_m,
            chord_m,
            kind,
            light: toggled(light),
        }),
        _ => None,
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
        (
            PartSpec::Wing {
                span_m,
                chord_m,
                sweep_deg,
                planform,
                profile,
                light,
            },
            Param::Span,
        ) => PartSpec::Wing {
            span_m: round((span_m + delta).clamp(0.2, 4.0)),
            chord_m,
            sweep_deg,
            planform,
            profile,
            light,
        },
        (
            PartSpec::Wing {
                span_m,
                chord_m,
                sweep_deg,
                planform,
                profile,
                light,
            },
            Param::Chord,
        ) => PartSpec::Wing {
            span_m,
            chord_m: round((chord_m + delta).clamp(0.1, 2.0)),
            sweep_deg,
            planform,
            profile,
            light,
        },
        (
            PartSpec::Wing {
                span_m,
                chord_m,
                sweep_deg,
                planform,
                profile,
                light,
            },
            Param::Sweep,
        ) => PartSpec::Wing {
            span_m,
            chord_m,
            sweep_deg: round((sweep_deg + delta).clamp(0.0, 60.0)),
            planform,
            profile,
            light,
        },
        (
            PartSpec::FinSet {
                span_m,
                chord_m,
                role,
                layout,
                light,
            },
            Param::Span,
        ) => PartSpec::FinSet {
            span_m: round((span_m + delta).clamp(0.2, 4.0)),
            chord_m,
            role,
            layout,
            light,
        },
        (
            PartSpec::FinSet {
                span_m,
                chord_m,
                role,
                layout,
                light,
            },
            Param::Chord,
        ) => PartSpec::FinSet {
            span_m,
            chord_m: round((chord_m + delta).clamp(0.1, 2.0)),
            role,
            layout,
            light,
        },
        (
            PartSpec::ControlSurface {
                span_m,
                chord_m,
                kind,
                light,
            },
            Param::Span,
        ) => PartSpec::ControlSurface {
            span_m: round((span_m + delta).clamp(0.2, 4.0)),
            chord_m,
            kind,
            light,
        },
        (
            PartSpec::ControlSurface {
                span_m,
                chord_m,
                kind,
                light,
            },
            Param::Chord,
        ) => PartSpec::ControlSurface {
            span_m,
            chord_m: round((chord_m + delta).clamp(0.1, 2.0)),
            kind,
            light,
        },
        _ => return None,
    };
    Some(changed)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MissileAction {
    /// Kept for tests and saved key bindings; the palette itself drags (see `workshop`).
    #[allow(dead_code)]
    AddPart(PartTemplate),
    Select {
        id: String,
    },
    Adjust(Param, i8),
    CycleAero(AeroChoice),
    /// Keep the current aerodynamic figures as the A side of a comparison.
    CompareAero,
    ClearComparison,
    RemovePart,
    Preset(u8),
    Clear,
    CycleSymmetry,
    Undo,
    Redo,
}

#[derive(Resource)]
pub(crate) struct MissileSession {
    pub vehicle: Vehicle,
    pub selected: Option<String>,
    /// Radial copies placed by the next drop: 1 (off), 2, 3 or 4.
    pub symmetry: u8,
    next_id: u32,
    undo: Vec<Vehicle>,
    redo: Vec<Vehicle>,
    /// The continuous wheel gesture in progress: its snapshot is pushed once, on the first notch.
    wheel_gesture: Option<((String, Param), Instant)>,
    /// Aerodynamic figures kept as the A side of the workshop comparison.
    pub comparison: Option<omega_rules::AeroFigures>,
}

impl Default for MissileSession {
    fn default() -> Self {
        Self {
            vehicle: Vehicle::default(),
            selected: None,
            symmetry: 1,
            next_id: 0,
            undo: Vec::new(),
            redo: Vec::new(),
            wheel_gesture: None,
            comparison: None,
        }
    }
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
            .parts
            .iter()
            .filter_map(|p| p.id.rsplit('-').next().and_then(|n| n.parse::<u32>().ok()))
            .max()
            .unwrap_or(0);
    }

    pub fn selected_part(&self) -> Option<&omega_rules::PlacedPart> {
        self.selected.as_deref().and_then(|id| self.vehicle.part(id))
    }

    pub fn load_preset(&mut self, index: u8) {
        self.vehicle = match index {
            0 => Vehicle::from_stages(
                "Two-stage rocket",
                vec![
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
            ),
            _ => Vehicle::from_stages(
                "Cruise vehicle",
                vec![Stage {
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
            ),
        };
        self.selected = self.vehicle.topmost_id();
        self.sync_next_id();
    }

    /// Apply a workshop action. Returns the status message to show.
    pub fn apply(&mut self, action: MissileAction, owned: &BTreeSet<String>) -> String {
        // A button edit (including +/-) closes any running wheel gesture.
        self.wheel_gesture = None;
        match action {
            MissileAction::AddPart(template) => self.add_part(template, owned),
            MissileAction::Select { id } => {
                if self.vehicle.part(&id).is_some() {
                    self.selected = Some(id);
                }
                String::new()
            }
            MissileAction::Adjust(param, direction) => self.adjust_selected(param, direction, true),
            MissileAction::CycleAero(choice) => self.cycle_aero(choice, owned),
            MissileAction::CompareAero => {
                self.comparison = Some(self.vehicle.aero());
                "Aerodynamic snapshot A saved; edit the design to compare".to_string()
            }
            MissileAction::ClearComparison => {
                self.comparison = None;
                "Comparison cleared".to_string()
            }
            MissileAction::RemovePart => {
                let Some(id) = self.selected.clone() else {
                    return "Select a part first".to_string();
                };
                if self.vehicle.part(&id).is_none() {
                    return String::new();
                }
                self.push_undo();
                let removed = self.vehicle.remove_subtree(&id);
                self.selected = self.vehicle.topmost_id();
                let label = if removed.len() == 1 {
                    format!("Removed {id}")
                } else {
                    format!("Removed {id} and {} attached part(s)", removed.len() - 1)
                };
                if removed.len() > 1 {
                    // The subtree above a removed root is gone: make the loss explicit.
                    return format!("{label}; the removed subtree is no longer attached");
                }
                label
            }
            MissileAction::Preset(index) => {
                let mut candidate = Self::default();
                candidate.load_preset(index);
                let missing = candidate.vehicle.missing_research(owned);
                if !missing.is_empty() {
                    return format!("Preset locked: research {} first", missing.join(", "));
                }
                // A preset starts a new history; the old design is not one undo away.
                *self = candidate;
                format!("Loaded preset: {}", self.vehicle.name)
            }
            MissileAction::Clear => {
                self.push_undo();
                self.vehicle = Vehicle {
                    name: "New vehicle".into(),
                    parts: Vec::new(),
                };
                self.selected = None;
                self.comparison = None;
                "Started an empty vehicle: drag a part from the palette".to_string()
            }
            MissileAction::CycleSymmetry => self.cycle_symmetry(),
            MissileAction::Undo => self.undo(),
            MissileAction::Redo => self.redo(),
        }
    }

    /// Adjust the selected part. `snapshot` is false for the notches of a wheel gesture that is
    /// still running: the gesture's first notch already captured the state to return to.
    fn adjust_selected(&mut self, param: Param, direction: i8, snapshot: bool) -> String {
        let Some(id) = self.selected.clone() else {
            return "Select a part first".to_string();
        };
        let Some(part) = self.vehicle.part(&id) else {
            return String::new();
        };
        match adjusted(part.spec, param, f64::from(direction)) {
            Some(spec) => {
                if snapshot {
                    self.push_undo();
                }
                self.vehicle.replace_part(&id, spec);
                String::new()
            }
            None => "This part has no such setting".to_string(),
        }
    }

    /// One wheel notch on the selected part. Notches of the same gesture share one undo entry.
    pub fn adjust_wheel(&mut self, param: Param, direction: i8) -> String {
        self.adjust_wheel_at(param, direction, Instant::now())
    }

    fn adjust_wheel_at(&mut self, param: Param, direction: i8, now: Instant) -> String {
        let Some(id) = self.selected.clone() else {
            return "Select a part first".to_string();
        };
        if self.vehicle.part(&id).is_none() {
            return String::new();
        }
        let key = (id, param);
        let continuous = self
            .wheel_gesture
            .as_ref()
            .is_some_and(|(last, at)| *last == key && now.saturating_duration_since(*at) < WHEEL_GESTURE);
        let message = self.adjust_selected(param, direction, !continuous);
        if message.is_empty() {
            self.wheel_gesture = Some((key, now));
        } else {
            self.wheel_gesture = None;
        }
        message
    }

    fn push_undo(&mut self) {
        self.undo.push(self.vehicle.clone());
        if self.undo.len() > 50 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Undo the last workshop edit. The stack holds at most 50 vehicle snapshots.
    pub fn undo(&mut self) -> String {
        self.wheel_gesture = None;
        let Some(previous) = self.undo.pop() else {
            return "Nothing to undo".to_string();
        };
        let current = std::mem::replace(&mut self.vehicle, previous);
        self.redo.push(current);
        self.selected = self.vehicle.topmost_id();
        "Undid the last workshop edit".to_string()
    }

    pub fn redo(&mut self) -> String {
        self.wheel_gesture = None;
        let Some(next) = self.redo.pop() else {
            return "Nothing to redo".to_string();
        };
        let current = std::mem::replace(&mut self.vehicle, next);
        self.undo.push(current);
        self.selected = self.vehicle.topmost_id();
        "Redid the workshop edit".to_string()
    }

    pub fn cycle_symmetry(&mut self) -> String {
        self.symmetry = match self.symmetry {
            1 => 2,
            2 => 3,
            3 => 4,
            _ => 1,
        };
        format!("Symmetry x{}", self.symmetry)
    }

    fn cycle_aero(&mut self, choice: AeroChoice, owned: &BTreeSet<String>) -> String {
        let Some(id) = self.selected.clone() else {
            return "Select a part first".to_string();
        };
        let Some(part) = self.vehicle.part(&id) else {
            return "That part is no longer there".to_string();
        };
        match cycled(part.spec, choice) {
            Some(spec) => {
                if let Some(missing) = spec
                    .extra_capabilities()
                    .into_iter()
                    .find(|capability| !owned.contains(*capability))
                {
                    return format!(
                        "Locked: research {} first",
                        omega_rules::research::capability_label(missing)
                    );
                }
                self.push_undo();
                self.vehicle.replace_part(&id, spec);
                format!("{id}: {} {}", choice.label(), choice_value(&spec, choice))
            }
            None => "This part has no such setting".to_string(),
        }
    }

    /// A free radial mount for a surface: the selection first, then the rest of the assembly from
    /// the top down. Surfaces are leaves, so a body with a free radial slot is required.
    fn aero_mount(&self, spec: &PartSpec) -> Option<(String, omega_rules::NodeId, f64)> {
        let candidates = std::iter::once(self.selected.clone())
            .flatten()
            .chain(self.vehicle.poses().into_iter().rev().map(|pose| pose.id));
        for id in candidates {
            let Some(part) = self.vehicle.part(&id) else {
                continue;
            };
            for index in 0..omega_rules::assembly::radial_slots(&part.spec) {
                let node = omega_rules::NodeId::Radial(index);
                if self.vehicle.node_status(&id, node, spec).is_ok() {
                    return Some((id, node, f64::from(index) * 90.0));
                }
            }
        }
        None
    }

    /// Remove the selected part (and its subtree). Returns the message to show.
    pub fn delete_selected(&mut self) -> String {
        self.wheel_gesture = None;
        match self.selected.clone() {
            Some(id) if self.vehicle.part(&id).is_some() => {
                self.push_undo();
                let removed = self.vehicle.remove_subtree(&id);
                self.selected = self.vehicle.topmost_id();
                if removed.len() > 1 {
                    format!("Removed {id} and {} attached part(s)", removed.len() - 1)
                } else {
                    format!("Removed {id}")
                }
            }
            _ => "Select a part to delete".to_string(),
        }
    }

    /// Add a template on a node (or above the stack when `target` is `None`). A radial drop with
    /// symmetry creates linked copies on consecutive free radial slots.
    pub fn add_at(
        &mut self,
        template: PartTemplate,
        target: Option<(String, omega_rules::NodeId, f64)>,
        owned: &BTreeSet<String>,
    ) -> String {
        self.wheel_gesture = None;
        let spec = template.spec();
        if !owned.contains(spec.required_capability()) {
            return format!("Locked: research {} first", spec.required_capability());
        }
        if template.is_computer()
            && let Some(existing) = self
                .vehicle
                .parts
                .iter()
                .find(|p| matches!(p.spec, PartSpec::Computer { .. }))
                .map(|p| p.id.clone())
        {
            self.push_undo();
            self.vehicle.replace_part(&existing, spec);
            self.selected = Some(existing.clone());
            return format!("Flight computer {existing} set to {}", template.label());
        }
        // A surface is a leaf on a radial mount: an explicit radial drop is used as is, a palette
        // click finds the first free radial slot. Everything else mounts above the stack.
        let target = if spec.is_aero() {
            match target {
                Some((parent, node, angle)) if node.is_radial() && self.vehicle.part(&parent).is_some() => {
                    Some((parent, node, angle))
                }
                _ => self.aero_mount(&spec),
            }
        } else {
            match target {
                None if !self.vehicle.is_empty() => {
                    self.vehicle.topmost_id().map(|id| (id, omega_rules::NodeId::Top, 0.0))
                }
                other => other,
            }
        };
        if spec.is_aero() && target.is_none() {
            return "No free radial mount: select a body part with a free side".to_string();
        }
        let mut placements: Vec<(omega_rules::NodeId, f64)> = Vec::new();
        if let Some((parent, node, angle)) = &target {
            if self.vehicle.part(parent).is_none() {
                return "That part is no longer there".to_string();
            }
            if node.is_radial() && self.symmetry > 1 {
                let first = match node {
                    omega_rules::NodeId::Radial(index) => *index,
                    _ => 0,
                };
                placements = self.vehicle.symmetry_slots(parent, &spec, first, self.symmetry);
            } else {
                placements.push((*node, *angle));
            }
        }
        if let Some((parent, _, _)) = &target {
            for (node, _) in &placements {
                if let Err(error) = self.vehicle.node_status(parent, *node, &spec) {
                    return format!("Cannot attach there: {error}");
                }
            }
        }
        self.push_undo();
        let group = format!("sym-{}", self.undo.len());
        let mut created = Vec::new();
        let placements: Vec<Option<(omega_rules::NodeId, f64)>> = if placements.is_empty() {
            vec![None]
        } else {
            placements.into_iter().map(Some).collect()
        };
        for placement in placements {
            let id = self.fresh_id(template.prefix());
            self.vehicle.parts.push(omega_rules::PlacedPart {
                id: id.clone(),
                spec,
                attach: None,
                symmetry: (self.symmetry > 1).then(|| group.clone()),
            });
            if let (Some((parent, _, _)), Some((node, angle))) = (&target, placement)
                && let Err(error) = self.vehicle.attach(&id, parent, node, angle)
            {
                for created_id in created.iter().chain(std::iter::once(&id)) {
                    self.vehicle.remove_subtree(created_id);
                }
                self.undo.pop();
                return format!("Cannot attach there: {error}");
            }
            created.push(id);
        }
        self.selected = created.last().cloned();
        let stage = self
            .vehicle
            .stage_of(created.last().map(String::as_str).unwrap_or(""))
            .unwrap_or(0);
        let count = if created.len() > 1 {
            format!("{} copies", created.len())
        } else {
            created.last().cloned().unwrap_or_default()
        };
        if target.is_none() {
            format!("Added {count} as the root part")
        } else {
            format!("Added {count} to stage {}", stage + 1)
        }
    }

    /// Move an existing part (with its subtree) to another node.
    pub fn move_part(&mut self, id: &str, parent: &str, node: omega_rules::NodeId, angle: f64) -> String {
        self.wheel_gesture = None;
        let Some(part) = self.vehicle.part(id) else {
            return "That part is no longer there".to_string();
        };
        if part.attach.is_none() {
            return "The root part stays at the bottom".to_string();
        }
        let before = self.vehicle.clone();
        match self.vehicle.attach(id, parent, node, angle) {
            Ok(()) => {
                self.undo.push(before);
                self.redo.clear();
                self.selected = Some(id.to_string());
                format!("Moved {id} to {} {}", parent, node.label())
            }
            Err(error) => format!("Cannot move {id}: {error}"),
        }
    }

    fn add_part(&mut self, template: PartTemplate, owned: &BTreeSet<String>) -> String {
        let spec = template.spec();
        let capability = spec.required_capability();
        if !owned.contains(capability) {
            return format!("Locked: research {capability} first");
        }
        if template.is_computer()
            && let Some(existing) = self
                .vehicle
                .parts
                .iter()
                .find(|p| matches!(p.spec, PartSpec::Computer { .. }))
                .map(|p| p.id.clone())
        {
            self.vehicle.replace_part(&existing, spec);
            self.selected = Some(existing.clone());
            return format!("Flight computer {existing} set to {}", template.label());
        }
        let parent = self
            .selected
            .clone()
            .filter(|id| self.vehicle.part(id).is_some())
            .or_else(|| self.vehicle.topmost_id());
        let parent = parent
            .filter(|id| self.vehicle.node_status(id, omega_rules::NodeId::Top, &spec).is_ok())
            .or_else(|| self.vehicle.topmost_id());
        let id = self.fresh_id(template.prefix());
        self.vehicle.parts.push(omega_rules::PlacedPart {
            id: id.clone(),
            spec,
            attach: None,
            symmetry: None,
        });
        if let Some(parent) = parent
            && let Err(error) = self.vehicle.attach(&id, &parent, omega_rules::NodeId::Top, 0.0)
        {
            self.vehicle.remove_subtree(&id);
            return format!("Cannot attach {id}: {error}");
        }
        self.selected = Some(id.clone());
        match self.vehicle.stage_of(&id) {
            Some(stage) => format!("Added {id} to stage {}", stage + 1),
            None => format!("Added {id}"),
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(&self.vehicle).map_err(|e| e.to_string())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let vehicle: Vehicle = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        for part in &vehicle.parts {
            part.spec.validate().map_err(|e| format!("{}: {e}", part.id))?;
        }
        let mut session = Self {
            selected: vehicle.topmost_id(),
            vehicle,
            ..Self::default()
        };
        session.sync_next_id();
        Ok(session)
    }
}

// ----------------------------------------------------------------------------------------------
// Layout and 3D visuals
// ----------------------------------------------------------------------------------------------

/// Human summary of what changed between two missile designs, by part id.
pub(crate) fn vehicle_diff(before: &Vehicle, after: &Vehicle) -> String {
    use std::collections::BTreeMap;
    let index = |vehicle: &Vehicle| -> BTreeMap<String, (usize, PartSpec)> {
        vehicle
            .parts
            .iter()
            .map(|part| (part.id.clone(), (vehicle.stage_of(&part.id).unwrap_or(0), part.spec)))
            .collect()
    };
    let old = index(before);
    let new = index(after);
    let mut added = 0;
    let mut removed = 0;
    let mut changed = 0;
    let mut moved = 0;
    for (id, (stage, spec)) in &new {
        match old.get(id) {
            None => added += 1,
            Some((old_stage, old_spec)) => {
                if old_spec != spec {
                    changed += 1;
                } else if old_stage != stage {
                    moved += 1;
                }
            }
        }
    }
    removed += old.keys().filter(|id| !new.contains_key(*id)).count();
    let mass = |vehicle: &Vehicle| -> f64 {
        vehicle
            .parts
            .iter()
            .map(|part| {
                let stats = part.spec.stats();
                stats.dry_mass_kg + stats.propellant_kg
            })
            .sum()
    };
    let delta = mass(after) - mass(before);
    if added + removed + changed + moved == 0 && delta.abs() < 0.05 {
        return "no changes".to_string();
    }
    let mut parts = Vec::new();
    for (count, label) in [
        (added, "added"),
        (removed, "removed"),
        (changed, "changed"),
        (moved, "moved"),
    ] {
        if count > 0 {
            parts.push(format!("{count} {label}"));
        }
    }
    parts.push(format!("mass {delta:+.1} kg"));
    parts.join(", ")
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Shape {
    Body,
    Nozzle,
    Sensor,
    Disc,
    /// Flat plate: span outward from the mount, chord along the body.
    Surface,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PartBox {
    pub id: String,
    pub x: f32,
    pub z: f32,
    pub y_bottom: f32,
    pub height: f32,
    pub radius: f32,
    /// Full span of a surface plate, 0 for bodies.
    pub span: f32,
    /// Clockwise mount angle in degrees, used to orient the plate.
    pub angle: f32,
    pub shape: Shape,
    pub color: [f32; 3],
}

fn appearance(spec: &PartSpec) -> (Shape, [f32; 3]) {
    match *spec {
        PartSpec::SolidMotor { .. } => (Shape::Body, [0.86, 0.42, 0.22]),
        PartSpec::FuelTank { .. } => (Shape::Body, [0.82, 0.86, 0.90]),
        PartSpec::LiquidEngine { .. } => (Shape::Nozzle, [0.35, 0.38, 0.42]),
        PartSpec::AirBreathing { .. } => (Shape::Nozzle, [0.45, 0.48, 0.55]),
        PartSpec::Sensor {
            kind: SensorKind::Pitot,
        } => (Shape::Sensor, [0.35, 0.84, 0.94]),
        PartSpec::Sensor {
            kind: SensorKind::Barometer,
        } => (Shape::Disc, [0.35, 0.74, 0.80]),
        PartSpec::Sensor {
            kind: SensorKind::Radar(RadarClass::Basic),
        } => (Shape::Disc, [0.30, 0.78, 0.60]),
        PartSpec::Sensor {
            kind: SensorKind::Radar(RadarClass::Active),
        } => (Shape::Disc, [0.35, 0.90, 0.55]),
        PartSpec::Computer { .. } => (Shape::Disc, [0.40, 0.78, 0.40]),
        PartSpec::Decoupler => (Shape::Disc, [0.95, 0.80, 0.20]),
        PartSpec::MissionModule { .. } => (Shape::Body, [0.40, 0.55, 0.92]),
        PartSpec::Wing { .. } => (Shape::Surface, [0.32, 0.72, 0.86]),
        PartSpec::FinSet { .. } => (Shape::Surface, [0.55, 0.80, 0.62]),
        // Fixed surfaces are grey, moving ones amber, so the two read differently in 3D.
        PartSpec::ControlSurface {
            kind: ControlKind::Fixed,
            ..
        } => (Shape::Surface, [0.62, 0.64, 0.68]),
        PartSpec::ControlSurface { .. } => (Shape::Surface, [0.90, 0.72, 0.28]),
    }
}

/// 3D boxes for the attachment tree: axial stacks keep their old look, radial boosters sit around
/// the parent body at the angle stored on their mount.
pub(crate) fn layout(vehicle: &Vehicle) -> Vec<PartBox> {
    vehicle
        .poses()
        .into_iter()
        .filter_map(|pose| {
            let part = vehicle.part(&pose.id)?;
            let spec = part.spec;
            let (shape, color) = appearance(&spec);
            let angle = part.attach.as_ref().map_or(0.0, |attach| match attach.node {
                omega_rules::NodeId::Radial(index) => f64::from(index) * 90.0 + attach.angle_deg,
                _ => attach.angle_deg,
            });
            Some(PartBox {
                id: pose.id,
                x: pose.x_m as f32,
                z: pose.z_m as f32,
                y_bottom: (pose.y_m - pose.height_m / 2.0) as f32,
                height: pose.height_m as f32,
                radius: pose.radius_m as f32,
                span: if spec.is_aero() {
                    (pose.radius_m * 2.0) as f32
                } else {
                    0.0
                },
                angle: angle as f32,
                shape,
                color,
            })
        })
        .collect()
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
            let selected = session.selected.as_deref() == Some(b.id.as_str());
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
            let (mesh, rotation) = match b.shape {
                Shape::Nozzle => (meshes.add(Cone::new(b.radius, b.height)), Quat::IDENTITY),
                Shape::Body | Shape::Sensor | Shape::Disc => {
                    (meshes.add(Cylinder::new(b.radius, b.height)), Quat::IDENTITY)
                }
                Shape::Surface => (
                    meshes.add(Cuboid::new(b.span.max(0.05), b.height.max(0.02), 0.06)),
                    Quat::from_rotation_y(-b.angle.to_radians()),
                ),
            };
            root.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_xyz(b.x, mid, b.z).with_rotation(rotation),
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
    let height: f32 = session
        .vehicle
        .poses()
        .iter()
        .map(|pose| (pose.y_m + pose.height_m / 2.0) as f32)
        .fold(0.0, f32::max);
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

    fn select(session: &mut MissileSession, id: &str) {
        session.apply(MissileAction::Select { id: id.to_string() }, &starting());
    }

    #[test]
    fn default_preset_is_a_valid_two_stage_vehicle() {
        let session = MissileSession::new_default();
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
        assert_eq!(session.vehicle.stage_count(), 2);
        assert_eq!(session.selected.as_deref(), Some("module-6"));
    }

    #[test]
    fn adjusting_a_part_resizes_it_within_limits() {
        let mut session = MissileSession::new_default();
        let owned = starting();
        select(&mut session, "motor-1");
        let before = session.vehicle.part("motor-1").unwrap().spec.stats();
        session.apply(MissileAction::Adjust(Param::Length, 1), &owned);
        session.apply(MissileAction::Adjust(Param::BurnTime, -1), &owned);
        let after = session.vehicle.part("motor-1").unwrap().spec.stats();
        assert!(after.propellant_kg > before.propellant_kg);
        assert!(
            after.thrust_n > before.thrust_n,
            "longer motor and shorter burn = more thrust"
        );
        for _ in 0..200 {
            session.apply(MissileAction::Adjust(Param::BurnTime, -1), &owned);
        }
        assert_eq!(params_of(&session.vehicle.part("motor-1").unwrap().spec)[2].1, 1.0);
        assert!(session.vehicle.part("motor-1").unwrap().spec.validate().is_ok());
        assert!(
            session
                .apply(MissileAction::Adjust(Param::Scale, 1), &owned)
                .contains("no such setting")
        );
    }

    #[test]
    fn locked_parts_cannot_be_added_until_researched() {
        let mut session = MissileSession::new_default();
        let count = session.vehicle.parts.len();
        let message = session.apply(MissileAction::AddPart(PartTemplate::RadarActive), &starting());
        assert!(
            message.contains("Locked") && message.contains("sensor:radar_active"),
            "{message}"
        );
        assert_eq!(session.vehicle.parts.len(), count);
        let ok = session.apply(MissileAction::AddPart(PartTemplate::RadarActive), &everything_owned());
        assert!(ok.contains("Added"), "{ok}");
        assert_eq!(session.vehicle.parts.len(), count + 1);
    }

    #[test]
    fn a_new_part_is_attached_above_the_selected_part() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        select(&mut session, "module-6");
        session.apply(MissileAction::AddPart(PartTemplate::FuelTank), &owned);
        let tank = session.selected.clone().unwrap();
        assert_eq!(
            session.vehicle.part(&tank).unwrap().attach.as_ref().unwrap().parent,
            "module-6"
        );
        assert_eq!(session.vehicle.stage_of(&tank), Some(1));
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
    }

    #[test]
    fn a_second_computer_replaces_the_first() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.apply(MissileAction::AddPart(PartTemplate::ComputerAdvanced), &owned);
        let computers: Vec<_> = session
            .vehicle
            .parts
            .iter()
            .filter(|p| matches!(p.spec, PartSpec::Computer { .. }))
            .collect();
        assert_eq!(computers.len(), 1);
        assert_eq!(session.vehicle.computer(), Some(ComputerClass::Advanced));
        assert_eq!(computers[0].id, "computer-4", "the existing computer keeps its id");
    }

    #[test]
    fn removing_a_part_warns_when_a_subtree_goes_with_it() {
        let mut session = MissileSession::new_default();
        let owned = starting();
        select(&mut session, "decoupler-2");
        let before = session.vehicle.parts.len();
        let message = session.apply(MissileAction::RemovePart, &owned);
        assert!(message.contains("attached part(s)"), "{message}");
        assert_eq!(
            session.vehicle.parts.len(),
            1,
            "the decoupler and the whole upper stack are gone"
        );
        assert!(session.vehicle.validate().is_err());
        session.apply(MissileAction::Clear, &owned);
        assert!(session.vehicle.parts.is_empty());
        assert!(session.vehicle.validate().is_err());
        assert_eq!(session.vehicle.parts.len(), before - before);
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
        let ids: Vec<&str> = session.vehicle.parts.iter().map(|p| p.id.as_str()).collect();
        let unique: BTreeSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len(), "{ids:?}");
    }

    #[test]
    fn vehicle_diff_reports_edits_and_mass() {
        let session = MissileSession::new_default();
        assert_eq!(vehicle_diff(&session.vehicle, &session.vehicle), "no changes");
        let mut edited = session.vehicle.clone();
        edited.replace_part(
            "motor-1",
            PartSpec::SolidMotor {
                length_m: 3.0,
                diameter_m: 0.4,
                burn_time_s: 20.0,
            },
        );
        let summary = vehicle_diff(&session.vehicle, &edited);
        assert!(summary.contains("1 changed") && summary.contains("mass +"), "{summary}");
        let mut removed = session.vehicle.clone();
        removed.remove_subtree("module-6");
        assert!(vehicle_diff(&session.vehicle, &removed).contains("1 removed"));
    }

    #[test]
    fn layout_places_the_stack_and_radial_parts() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        let boxes = layout(&session.vehicle);
        assert_eq!(boxes.len(), 6);
        let motor = boxes.iter().find(|b| b.id == "motor-1").unwrap();
        let sustainer = boxes.iter().find(|b| b.id == "motor-3").unwrap();
        assert!(sustainer.y_bottom > motor.y_bottom);
        assert_eq!(motor.x, 0.0);
        session.apply(MissileAction::AddPart(PartTemplate::SolidMotor), &owned);
        let booster = session.selected.clone().unwrap();
        session.vehicle.detach(&booster).unwrap();
        session
            .vehicle
            .attach(&booster, "motor-1", omega_rules::NodeId::Radial(0), 0.0)
            .unwrap();
        let boxes = layout(&session.vehicle);
        let radial = boxes.iter().find(|b| b.id == booster).unwrap();
        assert!(radial.x.abs() > 0.1, "a radial part sits off the centre line");
    }

    #[test]
    fn radial_symmetry_creates_linked_copies() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.symmetry = 3;
        let message = session.add_at(
            PartTemplate::SolidMotor,
            Some(("motor-1".into(), omega_rules::NodeId::Radial(0), 0.0)),
            &owned,
        );
        assert!(message.contains("3 copies"), "{message}");
        let boosters: Vec<_> = session
            .vehicle
            .parts
            .iter()
            .filter(|p| p.symmetry.as_deref() == Some("sym-1"))
            .collect();
        assert_eq!(boosters.len(), 3);
        let slots: BTreeSet<_> = boosters
            .iter()
            .filter_map(|p| p.attach.as_ref().map(|a| a.node))
            .collect();
        assert_eq!(slots.len(), 3);
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
        session.undo();
        assert!(session.vehicle.parts.iter().all(|p| p.symmetry.is_none()));
    }

    #[test]
    fn a_subtree_can_be_moved_and_bad_moves_are_refused() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        let message = session.move_part("module-6", "motor-3", omega_rules::NodeId::Radial(0), 0.0);
        assert!(message.starts_with("Moved"), "{message}");
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
        let refused = session.move_part("motor-1", "module-6", omega_rules::NodeId::Top, 0.0);
        assert!(refused.contains("root"), "moving the root is refused: {refused}");
        let cycle = session.move_part("motor-3", "module-6", omega_rules::NodeId::Top, 0.0);
        assert!(cycle.contains("root") || cycle.contains("cycle"), "{cycle}");
        assert!(session.vehicle.part("module-6").unwrap().attach.is_some());
        let _ = owned;
    }

    #[test]
    fn clearing_then_undoing_keeps_ids_unique() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.apply(MissileAction::Clear, &owned);
        session.undo();
        session.add_at(PartTemplate::SolidMotor, None, &owned);
        session.add_at(PartTemplate::FuelTank, None, &owned);
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
        let ids: BTreeSet<_> = session.vehicle.parts.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids.len(), session.vehicle.parts.len());
    }

    #[test]
    fn a_wheel_gesture_is_one_undo_entry() {
        let mut session = MissileSession::new_default();
        select(&mut session, "motor-1");
        let start = Instant::now();
        let before = session.vehicle.part("motor-1").unwrap().spec;
        session.adjust_wheel_at(Param::Length, 1, start);
        session.adjust_wheel_at(Param::Length, 1, start + Duration::from_millis(120));
        session.adjust_wheel_at(Param::Length, 1, start + Duration::from_millis(240));
        assert_eq!(session.undo.len(), 1, "one snapshot for the whole gesture");
        session.undo();
        assert_eq!(session.vehicle.part("motor-1").unwrap().spec, before);
        // A pause ends the gesture, so the next notch is a new entry.
        select(&mut session, "motor-1");
        session.adjust_wheel_at(Param::Length, 1, start + Duration::from_secs(2));
        session.adjust_wheel_at(Param::Length, 1, start + Duration::from_secs(3));
        assert_eq!(session.undo.len(), 2);
        assert_eq!(session.wheel_gesture.as_ref().unwrap().0.1, Param::Length);
    }

    #[test]
    fn a_button_edit_closes_the_wheel_gesture() {
        let mut session = MissileSession::new_default();
        select(&mut session, "motor-1");
        let owned = everything_owned();
        let start = Instant::now();
        session.adjust_wheel_at(Param::Length, 1, start);
        session.apply(MissileAction::Adjust(Param::Length, 1), &owned);
        session.adjust_wheel_at(Param::Length, 1, start + Duration::from_millis(100));
        assert_eq!(session.undo.len(), 3, "three separate edits");
    }

    #[test]
    fn undo_and_redo_walk_the_edit_history() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        let before = session.vehicle.parts.len();
        session.add_at(PartTemplate::FuelTank, None, &owned);
        assert_eq!(session.vehicle.parts.len(), before + 1);
        assert!(session.undo().contains("Undid"));
        assert_eq!(session.vehicle.parts.len(), before);
        assert!(session.redo().contains("Redid"));
        assert_eq!(session.vehicle.parts.len(), before + 1);
        session.delete_selected();
        assert!(session.undo().contains("Undid"), "deletion is undoable too");
    }

    #[test]
    fn legacy_files_load_and_save_as_schema_four() {
        let legacy = br#"{"name":"Old","stages":[{"parts":[{"id":"m","spec":{"part":"solid_motor","length_m":2.0,"diameter_m":0.4,"burn_time_s":20.0}},{"id":"pc","spec":{"part":"computer","class":"basic"}}]}]}"#;
        let session = MissileSession::from_bytes(legacy).unwrap();
        assert_eq!(session.vehicle.stage_count(), 1);
        assert_eq!(session.vehicle.aero().surface_count, 0);
        let saved = String::from_utf8(session.to_bytes().unwrap()).unwrap();
        assert!(saved.contains("\"schema_version\": 4"), "{saved}");
        let again = MissileSession::from_bytes(saved.as_bytes()).unwrap();
        assert_eq!(again.vehicle, session.vehicle);
    }

    #[test]
    fn wings_are_added_to_a_radial_mount_and_mirror_with_symmetry() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.symmetry = 4;
        let message = session.add_at(
            PartTemplate::Wing,
            Some(("motor-1".into(), omega_rules::NodeId::Radial(0), 0.0)),
            &owned,
        );
        assert!(message.contains("4 copies"), "{message}");
        let aero = session.vehicle.aero();
        assert_eq!(aero.surface_count, 4);
        assert!(aero.wing_area_m2 > 1.0);
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
        let slots: BTreeSet<_> = session
            .vehicle
            .parts
            .iter()
            .filter(|part| part.id.starts_with("wing-"))
            .filter_map(|part| part.attach.as_ref().map(|attach| attach.node))
            .collect();
        assert_eq!(slots.len(), 4);
        // A palette click finds another free radial mount instead of stacking on the axis.
        session.symmetry = 1;
        let added = session.add_at(PartTemplate::Wing, None, &owned);
        assert!(added.contains("Added"), "{added}");
        assert_eq!(session.vehicle.aero().surface_count, 5);
        let extra = session.selected.clone().unwrap();
        assert!(
            session
                .vehicle
                .part(&extra)
                .unwrap()
                .attach
                .as_ref()
                .unwrap()
                .node
                .is_radial()
        );
    }

    #[test]
    fn clicking_an_aero_part_finds_a_free_radial_slot() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.apply(MissileAction::Clear, &owned);
        session.apply(MissileAction::AddPart(PartTemplate::SolidMotor), &owned);
        session.apply(MissileAction::AddPart(PartTemplate::ComputerBasic), &owned);
        let motor = session
            .vehicle
            .parts
            .iter()
            .find(|part| matches!(part.spec, PartSpec::SolidMotor { .. }))
            .map(|part| part.id.clone())
            .unwrap();
        session.apply(MissileAction::Select { id: motor.clone() }, &owned);
        let message = session.add_at(PartTemplate::FinSet, None, &owned);
        assert!(message.contains("Added"), "{message}");
        let fin = session.selected.clone().unwrap();
        let attach = session.vehicle.part(&fin).unwrap().attach.as_ref().unwrap();
        assert_eq!(attach.parent, motor);
        assert!(attach.node.is_radial());
        assert!(session.vehicle.validate().is_ok(), "{:?}", session.vehicle.validate());
    }

    #[test]
    fn cycling_design_choices_changes_the_figures_and_respects_research() {
        let mut session = MissileSession::new_default();
        let owned = everything_owned();
        session.add_at(
            PartTemplate::Wing,
            Some(("motor-1".into(), omega_rules::NodeId::Radial(0), 0.0)),
            &owned,
        );
        let wing = session.selected.clone().unwrap();
        let before = session.vehicle.aero();
        session.apply(MissileAction::CycleAero(AeroChoice::Planform), &owned);
        let delta = session.vehicle.aero();
        assert_ne!(delta.lift, before.lift);
        assert!(delta.lift > before.lift, "a delta wing lifts more");
        session.undo();
        assert_eq!(session.vehicle.aero().lift, before.lift);

        session.apply(MissileAction::Select { id: wing.clone() }, &owned);
        session.apply(MissileAction::CycleAero(AeroChoice::Light), &owned);
        let light = session.vehicle.aero();
        assert!(light.mass_kg < before.mass_kg);
        session.undo();

        // Experimental profiles are gated by the advanced aerodynamics research.
        let mut limited = everything_owned();
        limited.remove("aero:advanced_profiles");
        session.apply(MissileAction::Select { id: wing.clone() }, &owned);
        session.apply(MissileAction::CycleAero(AeroChoice::Profile), &limited); // LOW DRAG
        session.apply(MissileAction::CycleAero(AeroChoice::Profile), &limited); // HIGH LIFT
        let refused = session.apply(MissileAction::CycleAero(AeroChoice::Profile), &limited);
        assert!(refused.contains("Locked"), "experimental profile: {refused}");
        let accepted = session.apply(MissileAction::CycleAero(AeroChoice::Profile), &owned);
        assert!(accepted.contains("EXPERIMENTAL"), "{accepted}");
    }

    #[test]
    fn the_aero_comparison_keeps_the_previous_configuration() {
        let mut session = MissileSession::new_default();
        assert!(session.comparison.is_none());
        session.apply(MissileAction::CompareAero, &starting());
        let saved = session.comparison.clone().unwrap();
        assert_eq!(saved.surface_count, 0);
        let owned = everything_owned();
        session.add_at(
            PartTemplate::FinSet,
            Some(("motor-1".into(), omega_rules::NodeId::Radial(0), 0.0)),
            &owned,
        );
        assert!(session.vehicle.aero().stability > saved.stability);
        session.apply(MissileAction::ClearComparison, &owned);
        assert!(session.comparison.is_none());
    }
}
