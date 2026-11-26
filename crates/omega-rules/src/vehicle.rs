//! Missile design model (abstract, fictional): resizable parts grouped in stages, derived
//! mass / thrust / burn-time / thrust-to-weight figures, and the capabilities that installed
//! sensors and computers give to the programming studio.
//!
//! All coefficients are gameplay values chosen for readable trade-offs. They are not calibrated
//! to any real system and the module contains no guidance, targeting or warhead modelling.

use crate::avionics::AvionicsModule;
use serde::{Deserialize, Serialize};

pub const G0: f64 = 9.80665;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RadarClass {
    Basic,
    Active,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorKind {
    Pitot,
    Barometer,
    Radar(RadarClass),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerClass {
    Basic,
    Standard,
    Advanced,
}

impl ComputerClass {
    /// Ordering used by compatibility rules (a control surface needs a class at least this good).
    pub fn rank(self) -> u8 {
        match self {
            Self::Basic => 0,
            Self::Standard => 1,
            Self::Advanced => 2,
        }
    }

    /// Parallel program threads the computer can run.
    pub fn threads(self) -> usize {
        match self {
            Self::Basic => 1,
            Self::Standard => 2,
            Self::Advanced => 4,
        }
    }

    /// Number of program blocks that fit in memory.
    pub fn ram_blocks(self) -> usize {
        match self {
            Self::Basic => 8,
            Self::Standard => 16,
            Self::Advanced => 32,
        }
    }
}

/// Wing planform, an abstract gameplay shape. Each one trades lift, drag, stability, authority,
/// complexity, reliability and mass; the numbers live in `crate::aero`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WingPlanform {
    Straight,
    Swept,
    Delta,
    Trapezoidal,
    VariableGeometry,
}

impl WingPlanform {
    pub const ALL: [Self; 5] = [
        Self::Straight,
        Self::Swept,
        Self::Delta,
        Self::Trapezoidal,
        Self::VariableGeometry,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Straight => "STRAIGHT",
            Self::Swept => "SWEPT",
            Self::Delta => "DELTA",
            Self::Trapezoidal => "TRAPEZOIDAL",
            Self::VariableGeometry => "VARIABLE GEOMETRY",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Straight => Self::Swept,
            Self::Swept => Self::Delta,
            Self::Delta => Self::Trapezoidal,
            Self::Trapezoidal => Self::VariableGeometry,
            Self::VariableGeometry => Self::Straight,
        }
    }
}

/// Fictional wing section. No real lift/drag tables: each profile is a fixed set of game effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AirfoilProfile {
    Symmetric,
    Classic,
    LowDrag,
    HighLift,
    Experimental,
}

impl AirfoilProfile {
    pub const ALL: [Self; 5] = [
        Self::Symmetric,
        Self::Classic,
        Self::LowDrag,
        Self::HighLift,
        Self::Experimental,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Symmetric => "SYMMETRIC",
            Self::Classic => "CLASSIC",
            Self::LowDrag => "LOW DRAG",
            Self::HighLift => "HIGH LIFT",
            Self::Experimental => "EXPERIMENTAL",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Symmetric => Self::Classic,
            Self::Classic => Self::LowDrag,
            Self::LowDrag => Self::HighLift,
            Self::HighLift => Self::Experimental,
            Self::Experimental => Self::Symmetric,
        }
    }
}

/// Where a fin set sits on the airframe. The attachment node still decides the physical mount;
/// this is the design intent the rules read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StabilizerRole {
    Rear,
    Front,
    Blended,
    Combined,
}

impl StabilizerRole {
    pub const ALL: [Self; 4] = [Self::Rear, Self::Front, Self::Blended, Self::Combined];

    pub fn label(self) -> &'static str {
        match self {
            Self::Rear => "REAR",
            Self::Front => "FRONT",
            Self::Blended => "BLENDED",
            Self::Combined => "COMBINED",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Rear => Self::Front,
            Self::Front => Self::Blended,
            Self::Blended => Self::Combined,
            Self::Combined => Self::Rear,
        }
    }
}

/// Symmetric is the normal fin set; an asymmetric set is an experimental trade of stability for
/// authority and needs the advanced aerodynamics research.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinLayout {
    Symmetric,
    Asymmetric,
}

impl FinLayout {
    pub fn label(self) -> &'static str {
        match self {
            Self::Symmetric => "SYMMETRIC",
            Self::Asymmetric => "ASYMMETRIC",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Symmetric => Self::Asymmetric,
            Self::Asymmetric => Self::Symmetric,
        }
    }
}

/// Abstract control-surface family. `Fixed` adds passive stability only; the moving kinds need a
/// compatible flight computer and give control authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    Classic,
    Integrated,
    Combined,
    Fixed,
}

impl ControlKind {
    pub const ALL: [Self; 4] = [Self::Classic, Self::Integrated, Self::Combined, Self::Fixed];

    pub fn label(self) -> &'static str {
        match self {
            Self::Classic => "CLASSIC",
            Self::Integrated => "INTEGRATED",
            Self::Combined => "COMBINED",
            Self::Fixed => "FIXED",
        }
    }

    pub fn is_moving(self) -> bool {
        !matches!(self, Self::Fixed)
    }

    pub fn next(self) -> Self {
        match self {
            Self::Classic => Self::Integrated,
            Self::Integrated => Self::Combined,
            Self::Combined => Self::Fixed,
            Self::Fixed => Self::Classic,
        }
    }
}

/// Axial transition between two body diameters. The shape only changes the silhouette and the
/// casing mass; the inlet and outlet still meet the parts they join.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterShape {
    Conical,
    Ogive,
    Stepped,
}

impl AdapterShape {
    pub const ALL: [Self; 3] = [Self::Conical, Self::Ogive, Self::Stepped];

    pub fn label(self) -> &'static str {
        match self {
            Self::Conical => "CONICAL",
            Self::Ogive => "OGIVE",
            Self::Stepped => "STEPPED",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Conical => Self::Ogive,
            Self::Ogive => Self::Stepped,
            Self::Stepped => Self::Conical,
        }
    }
}

/// Player-chosen nose profile. The shape changes the silhouette and the shell mass only, not the
/// flight model; the base diameter and length are set by the designer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoseShape {
    Conical,
    Ogive,
    Parabolic,
}

impl NoseShape {
    pub fn label(self) -> &'static str {
        match self {
            Self::Conical => "CONICAL",
            Self::Ogive => "OGIVE",
            Self::Parabolic => "PARABOLIC",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Conical => Self::Ogive,
            Self::Ogive => Self::Parabolic,
            Self::Parabolic => Self::Conical,
        }
    }

    /// Casing mass multiplier relative to a conical shell of the same envelope.
    pub fn mass_factor(self) -> f64 {
        match self {
            Self::Conical => 1.0,
            Self::Ogive => 1.12,
            Self::Parabolic => 1.06,
        }
    }
}

/// Abstract explosive payload grade. The grade does not model chemistry or blast effects: it is a
/// game multiplier on the payload yield and a research gate for the workshop palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExplosiveGrade {
    Standard,
    Improved,
    Advanced,
}

impl ExplosiveGrade {
    pub const ALL: [Self; 3] = [Self::Standard, Self::Improved, Self::Advanced];

    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "STANDARD",
            Self::Improved => "IMPROVED",
            Self::Advanced => "ADVANCED",
        }
    }

    /// Game yield per kilogram of charge, not a real explosive performance figure.
    pub fn yield_multiplier(self) -> f64 {
        match self {
            Self::Standard => 1.0,
            Self::Improved => 1.25,
            Self::Advanced => 1.6,
        }
    }

    pub fn capability(self) -> &'static str {
        match self {
            Self::Standard => "part:explosive",
            Self::Improved => "part:explosive_improved",
            Self::Advanced => "part:explosive_advanced",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "part", rename_all = "snake_case")]
pub enum PartSpec {
    /// Solid motor: a shorter burn time gives more thrust and a heavier casing.
    SolidMotor {
        length_m: f64,
        diameter_m: f64,
        burn_time_s: f64,
    },
    FuelTank {
        length_m: f64,
        diameter_m: f64,
    },
    /// Liquid engine drawing from tanks in the same stage; `scale` resizes the engine.
    LiquidEngine {
        scale: f64,
    },
    /// Cruise engine: needs a minimum airspeed to run, very economical.
    AirBreathing {
        scale: f64,
    },
    Sensor {
        kind: SensorKind,
    },
    Computer {
        class: ComputerClass,
    },
    Decoupler,
    /// Abstract mission module: only mass matters here.
    MissionModule {
        mass_kg: f64,
    },
    /// Explosive payload package. `mass_kg` is the charge itself; its casing is added to the dry
    /// mass and its grade sets the abstract game yield.
    Explosive {
        mass_kg: f64,
        grade: ExplosiveGrade,
    },
    /// Airframe adapter: a body transition from `diameter_m` (mount side) to `exit_diameter_m`
    /// (next part). Its two axial nodes stay on the body axis, so a stack can change size without
    /// a visual or validation step.
    Adapter {
        length_m: f64,
        diameter_m: f64,
        exit_diameter_m: f64,
        shape: AdapterShape,
    },
    /// Player-placed nose cone capping the top of the stack. It is never generated implicitly:
    /// the designer chooses its length, base diameter and profile.
    NoseCone {
        length_m: f64,
        diameter_m: f64,
        shape: NoseShape,
    },
    /// Lifting surface for cruise flight. `light` selects the light-structure build once its
    /// research is done: less mass, more complexity and less reliability.
    Wing {
        span_m: f64,
        chord_m: f64,
        sweep_deg: f64,
        planform: WingPlanform,
        profile: AirfoilProfile,
        light: bool,
    },
    /// Passive stabiliser. `role` is the designer's declared configuration; the mount node still
    /// decides where the set physically sits.
    FinSet {
        span_m: f64,
        chord_m: f64,
        role: StabilizerRole,
        layout: FinLayout,
        light: bool,
    },
    /// Control surface: authority when moving, stability when fixed.
    ControlSurface {
        span_m: f64,
        chord_m: f64,
        kind: ControlKind,
        light: bool,
    },
    /// Abstract mission electronics. The module carries its category, its cost, its power draw
    /// and the gameplay effect it contributes; the mass comes from the same module table.
    Avionics {
        module: AvionicsModule,
    },
}

/// Palette grouping. Presentation metadata, owned here so the workshop cannot invent a category
/// the rules do not know: the palette renders one section per entry of [`PartCategory::ALL`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PartCategory {
    Propulsion,
    Airframe,
    Aerodynamics,
    Sensors,
    Avionics,
}

impl PartCategory {
    pub const ALL: [Self; 5] = [
        Self::Propulsion,
        Self::Airframe,
        Self::Aerodynamics,
        Self::Sensors,
        Self::Avionics,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Propulsion => "PROPULSION",
            Self::Airframe => "AIRFRAME & PAYLOAD",
            Self::Aerodynamics => "AERODYNAMIC SURFACES",
            Self::Sensors => "SENSORS",
            Self::Avionics => "AVIONICS",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PartStats {
    pub dry_mass_kg: f64,
    pub propellant_kg: f64,
    pub thrust_n: f64,
    pub isp_s: f64,
    /// Fixed burn time (solid motors only).
    pub burn_time_s: Option<f64>,
    pub diameter_m: f64,
    /// Explosive charge mass carried by this part, in kilograms.
    pub payload_kg: f64,
    /// Abstract game yield of the charge, already multiplied by its grade.
    pub payload_yield_kg: f64,
}

const SOLID_DENSITY: f64 = 1_700.0;
const LIQUID_DENSITY: f64 = 1_000.0;
const SOLID_ISP_S: f64 = 235.0;
const LIQUID_ISP_S: f64 = 280.0;
const AIR_ISP_S: f64 = 2_500.0;
pub const AIR_BREATHING_MIN_SPEED_MS: f64 = 90.0;

fn cylinder_surface(length: f64, diameter: f64) -> f64 {
    std::f64::consts::PI * diameter * length + 2.0 * std::f64::consts::PI * diameter * diameter / 4.0
}

fn cylinder_volume(length: f64, diameter: f64) -> f64 {
    std::f64::consts::PI * diameter * diameter / 4.0 * length
}

impl PartSpec {
    pub fn validate(&self) -> Result<(), String> {
        let in_range = |name: &str, value: f64, lo: f64, hi: f64| {
            if value.is_finite() && (lo..=hi).contains(&value) {
                Ok(())
            } else {
                Err(format!("{name} must be between {lo} and {hi} (got {value})"))
            }
        };
        match *self {
            Self::SolidMotor {
                length_m,
                diameter_m,
                burn_time_s,
            } => {
                in_range("motor length (m)", length_m, 0.3, 8.0)?;
                in_range("motor diameter (m)", diameter_m, 0.1, 1.2)?;
                in_range("burn time (s)", burn_time_s, 1.0, 60.0)
            }
            Self::FuelTank { length_m, diameter_m } => {
                in_range("tank length (m)", length_m, 0.3, 8.0)?;
                in_range("tank diameter (m)", diameter_m, 0.1, 1.2)
            }
            Self::LiquidEngine { scale } | Self::AirBreathing { scale } => in_range("engine scale", scale, 0.3, 3.0),
            Self::Adapter {
                length_m,
                diameter_m,
                exit_diameter_m,
                ..
            } => {
                in_range("adapter length (m)", length_m, 0.1, 2.0)?;
                in_range("adapter inlet diameter (m)", diameter_m, 0.05, 1.2)?;
                in_range("adapter outlet diameter (m)", exit_diameter_m, 0.05, 1.2)
            }
            Self::NoseCone {
                length_m, diameter_m, ..
            } => {
                in_range("nose length (m)", length_m, 0.1, 3.0)?;
                in_range("nose diameter (m)", diameter_m, 0.05, 1.2)
            }
            Self::MissionModule { mass_kg } => in_range("module mass (kg)", mass_kg, 1.0, 300.0),
            Self::Explosive { mass_kg, .. } => in_range("charge mass (kg)", mass_kg, 5.0, 300.0),
            Self::Wing {
                span_m,
                chord_m,
                sweep_deg,
                ..
            } => {
                in_range("surface span (m)", span_m, 0.2, 4.0)?;
                in_range("surface chord (m)", chord_m, 0.1, 2.0)?;
                in_range("wing sweep (deg)", sweep_deg, 0.0, 60.0)
            }
            Self::FinSet { span_m, chord_m, .. } | Self::ControlSurface { span_m, chord_m, .. } => {
                in_range("surface span (m)", span_m, 0.2, 4.0)?;
                in_range("surface chord (m)", chord_m, 0.1, 2.0)
            }
            Self::Sensor { .. } | Self::Computer { .. } | Self::Decoupler | Self::Avionics { .. } => Ok(()),
        }
    }

    /// True for the nose cone, the only part that closes the top of the stack visually.
    pub fn is_nose(&self) -> bool {
        matches!(self, Self::NoseCone { .. })
    }

    /// True for the aerodynamic surfaces, which are always leaf parts on a radial mount.
    pub fn is_aero(&self) -> bool {
        matches!(
            self,
            Self::Wing { .. } | Self::FinSet { .. } | Self::ControlSurface { .. }
        )
    }

    /// Palette section this part belongs to.
    pub fn category(&self) -> PartCategory {
        match self {
            Self::SolidMotor { .. } | Self::FuelTank { .. } | Self::LiquidEngine { .. } | Self::AirBreathing { .. } => {
                PartCategory::Propulsion
            }
            Self::Decoupler
            | Self::MissionModule { .. }
            | Self::Explosive { .. }
            | Self::Adapter { .. }
            | Self::NoseCone { .. } => PartCategory::Airframe,
            Self::Wing { .. } | Self::FinSet { .. } | Self::ControlSurface { .. } => PartCategory::Aerodynamics,
            Self::Sensor { .. } => PartCategory::Sensors,
            // Every mission module shares the electronics section; the inspector and the rules
            // still name the narrower category (control, navigation, power, diagnostics).
            Self::Computer { .. } | Self::Avionics { .. } => PartCategory::Avionics,
        }
    }

    /// Research beyond the part itself: advanced shapes and the light-structure build.
    pub fn extra_capabilities(&self) -> Vec<&'static str> {
        let mut needed = Vec::new();
        match *self {
            Self::Wing {
                planform,
                profile,
                light,
                ..
            } => {
                if matches!(planform, WingPlanform::VariableGeometry) || matches!(profile, AirfoilProfile::Experimental)
                {
                    needed.push("aero:advanced_profiles");
                }
                if light {
                    needed.push("aero:light_structures");
                }
            }
            Self::FinSet { layout, light, .. } => {
                if matches!(layout, FinLayout::Asymmetric) {
                    needed.push("aero:advanced_profiles");
                }
                if light {
                    needed.push("aero:light_structures");
                }
            }
            Self::ControlSurface { light: true, .. } => needed.push("aero:light_structures"),
            _ => {}
        }
        needed
    }

    pub fn stats(&self) -> PartStats {
        match *self {
            Self::SolidMotor {
                length_m,
                diameter_m,
                burn_time_s,
            } => {
                let propellant = 0.88 * cylinder_volume(length_m, diameter_m) * SOLID_DENSITY;
                // Faster burns mean higher chamber pressure and a heavier casing.
                let pressure_factor = (20.0 / burn_time_s).clamp(1.0, 2.5).sqrt();
                let casing = 12.0 * cylinder_surface(length_m, diameter_m) * pressure_factor;
                PartStats {
                    dry_mass_kg: casing + 8.0,
                    propellant_kg: propellant,
                    thrust_n: propellant * SOLID_ISP_S * G0 / burn_time_s,
                    isp_s: SOLID_ISP_S,
                    burn_time_s: Some(burn_time_s),
                    diameter_m,
                    ..PartStats::default()
                }
            }
            Self::FuelTank { length_m, diameter_m } => PartStats {
                dry_mass_kg: 9.0 * cylinder_surface(length_m, diameter_m) + 4.0,
                propellant_kg: 0.95 * cylinder_volume(length_m, diameter_m) * LIQUID_DENSITY,
                diameter_m,
                ..PartStats::default()
            },
            Self::LiquidEngine { scale } => PartStats {
                dry_mass_kg: 35.0 * scale.powi(3) + 5.0,
                thrust_n: 12_000.0 * scale * scale,
                isp_s: LIQUID_ISP_S,
                diameter_m: 0.25 * scale,
                ..PartStats::default()
            },
            Self::AirBreathing { scale } => PartStats {
                dry_mass_kg: 60.0 * scale.powi(3) + 10.0,
                thrust_n: 2_500.0 * scale * scale,
                isp_s: AIR_ISP_S,
                diameter_m: 0.3 * scale,
                ..PartStats::default()
            },
            Self::Sensor { kind } => PartStats {
                dry_mass_kg: match kind {
                    SensorKind::Pitot => 1.5,
                    SensorKind::Barometer => 1.0,
                    SensorKind::Radar(RadarClass::Basic) => 12.0,
                    SensorKind::Radar(RadarClass::Active) => 28.0,
                },
                ..PartStats::default()
            },
            Self::Computer { class } => PartStats {
                dry_mass_kg: match class {
                    ComputerClass::Basic => 3.0,
                    ComputerClass::Standard => 6.0,
                    ComputerClass::Advanced => 11.0,
                },
                ..PartStats::default()
            },
            Self::Decoupler => PartStats {
                dry_mass_kg: 6.0,
                ..PartStats::default()
            },
            Self::MissionModule { mass_kg } => PartStats {
                dry_mass_kg: mass_kg,
                ..PartStats::default()
            },
            Self::Explosive { mass_kg, grade } => PartStats {
                // The charge and a light casing both count as launch mass.
                dry_mass_kg: mass_kg * 1.15 + 2.0,
                diameter_m: 0.20,
                payload_kg: mass_kg,
                payload_yield_kg: mass_kg * grade.yield_multiplier(),
                ..PartStats::default()
            },
            Self::Adapter {
                length_m,
                diameter_m,
                exit_diameter_m,
                shape,
            } => {
                let mean_diameter = (diameter_m + exit_diameter_m) * 0.5;
                let shape_factor = match shape {
                    AdapterShape::Conical => 1.0,
                    AdapterShape::Ogive => 1.12,
                    AdapterShape::Stepped => 0.92,
                };
                PartStats {
                    dry_mass_kg: (2.8 * cylinder_surface(length_m, mean_diameter) + 1.0) * shape_factor,
                    diameter_m: diameter_m.max(exit_diameter_m),
                    ..PartStats::default()
                }
            }
            Self::NoseCone {
                length_m,
                diameter_m,
                shape,
            } => PartStats {
                // A thin shell over the envelope, not a solid cone.
                dry_mass_kg: (1.6 * cylinder_surface(length_m, diameter_m * 0.7) + 0.6) * shape.mass_factor(),
                diameter_m,
                ..PartStats::default()
            },
            Self::Wing { .. } | Self::FinSet { .. } | Self::ControlSurface { .. } => PartStats {
                dry_mass_kg: crate::aero::surface_mass_kg(self).unwrap_or(0.0),
                ..PartStats::default()
            },
            Self::Avionics { module } => PartStats {
                dry_mass_kg: module.spec().mass_kg,
                ..PartStats::default()
            },
        }
    }

    /// Research capability needed to place this part in the workshop.
    pub fn required_capability(&self) -> &'static str {
        match *self {
            Self::SolidMotor { .. } => "part:solid_motor",
            Self::FuelTank { .. } => "part:fuel_tank",
            Self::LiquidEngine { .. } => "part:liquid_engine",
            Self::AirBreathing { .. } => "part:air_breathing",
            Self::Sensor {
                kind: SensorKind::Pitot,
            } => "sensor:pitot",
            Self::Sensor {
                kind: SensorKind::Barometer,
            } => "sensor:barometer",
            Self::Sensor {
                kind: SensorKind::Radar(RadarClass::Basic),
            } => "sensor:radar_basic",
            Self::Sensor {
                kind: SensorKind::Radar(RadarClass::Active),
            } => "sensor:radar_active",
            Self::Computer {
                class: ComputerClass::Basic,
            } => "computer:basic",
            Self::Computer {
                class: ComputerClass::Standard,
            } => "computer:standard",
            Self::Computer {
                class: ComputerClass::Advanced,
            } => "computer:advanced",
            Self::Decoupler => "part:decoupler",
            Self::MissionModule { .. } => "part:mission_module",
            Self::Explosive { grade, .. } => grade.capability(),
            Self::Adapter { .. } => "part:adapter",
            Self::NoseCone { .. } => "part:nose",
            Self::Wing { .. } => "part:wing",
            Self::FinSet { .. } => "part:fin",
            Self::ControlSurface { .. } => "part:control_surface",
            Self::Avionics { module } => module.capability(),
        }
    }

    /// Display label for a part family, shared by the palette and the inspector.
    pub fn short_label(&self) -> &'static str {
        match self {
            Self::Wing { .. } => "WING",
            Self::FinSet { .. } => "FIN SET",
            Self::ControlSurface { .. } => "CONTROL",
            Self::Adapter { .. } => "ADAPTER",
            Self::NoseCone { .. } => "NOSE",
            Self::Explosive { .. } => "CHARGE",
            _ => "PART",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub id: String,
    pub spec: PartSpec,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stage {
    pub parts: Vec<Part>,
}

/// Stages in firing order: `stages[0]` is the first to burn and the first to be dropped.
#[derive(Clone, Debug, PartialEq)]
pub struct StageReport {
    pub index: usize,
    /// Mass of this stage with propellant, in kilograms.
    pub stage_wet_mass_kg: f64,
    /// Mass at ignition of this stage: itself plus every stage above it.
    pub ignition_mass_kg: f64,
    pub thrust_n: f64,
    pub burn_time_s: Option<f64>,
    pub twr_at_ignition: f64,
    pub delta_v_ms: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadarSpec {
    pub class: RadarClass,
    pub max_lock_speed_ms: f64,
    pub base_lock_probability: f64,
}

impl RadarClass {
    pub fn spec(self) -> RadarSpec {
        match self {
            Self::Basic => RadarSpec {
                class: self,
                max_lock_speed_ms: 450.0,
                base_lock_probability: 0.55,
            },
            Self::Active => RadarSpec {
                class: self,
                max_lock_speed_ms: 900.0,
                base_lock_probability: 0.80,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solid_motor_burn_time_trades_thrust_for_duration_and_casing_mass() {
        let slow = PartSpec::SolidMotor {
            length_m: 2.0,
            diameter_m: 0.4,
            burn_time_s: 40.0,
        }
        .stats();
        let fast = PartSpec::SolidMotor {
            length_m: 2.0,
            diameter_m: 0.4,
            burn_time_s: 10.0,
        }
        .stats();
        assert!(
            (slow.propellant_kg - fast.propellant_kg).abs() < 1e-9,
            "same propellant"
        );
        assert!(
            (fast.thrust_n / slow.thrust_n - 4.0).abs() < 1e-9,
            "4x faster burn = 4x thrust"
        );
        assert!(
            fast.dry_mass_kg > slow.dry_mass_kg,
            "faster burn needs a heavier casing"
        );
    }

    #[test]
    fn lengthening_a_tank_and_scaling_an_engine_changes_mass_and_thrust() {
        let short = PartSpec::FuelTank {
            length_m: 1.0,
            diameter_m: 0.4,
        }
        .stats();
        let long = PartSpec::FuelTank {
            length_m: 2.0,
            diameter_m: 0.4,
        }
        .stats();
        assert!((long.propellant_kg / short.propellant_kg - 2.0).abs() < 1e-9);
        let small = PartSpec::LiquidEngine { scale: 1.0 }.stats();
        let big = PartSpec::LiquidEngine { scale: 2.0 }.stats();
        assert!((big.thrust_n / small.thrust_n - 4.0).abs() < 1e-9);
        assert!(big.dry_mass_kg > 4.0 * small.dry_mass_kg * 0.9);
    }

    #[test]
    fn every_part_family_has_one_palette_category() {
        let parts = [
            PartSpec::SolidMotor {
                length_m: 1.0,
                diameter_m: 0.4,
                burn_time_s: 10.0,
            },
            PartSpec::FuelTank {
                length_m: 1.0,
                diameter_m: 0.4,
            },
            PartSpec::LiquidEngine { scale: 1.0 },
            PartSpec::AirBreathing { scale: 1.0 },
            PartSpec::Decoupler,
            PartSpec::MissionModule { mass_kg: 10.0 },
            PartSpec::Explosive {
                mass_kg: 40.0,
                grade: ExplosiveGrade::Standard,
            },
            PartSpec::Sensor {
                kind: SensorKind::Pitot,
            },
            PartSpec::Computer {
                class: ComputerClass::Basic,
            },
            PartSpec::Wing {
                span_m: 1.0,
                chord_m: 0.4,
                sweep_deg: 0.0,
                planform: WingPlanform::Straight,
                profile: AirfoilProfile::Classic,
                light: false,
            },
        ];
        let used: std::collections::BTreeSet<PartCategory> = parts.iter().map(PartSpec::category).collect();
        assert_eq!(used.len(), PartCategory::ALL.len(), "every category is reachable");
        for category in PartCategory::ALL {
            assert!(!category.label().is_empty());
            assert!(used.contains(&category), "{category:?} has no part");
        }
    }

    #[test]
    fn part_ranges_are_enforced() {
        assert!(
            PartSpec::SolidMotor {
                length_m: 20.0,
                diameter_m: 0.4,
                burn_time_s: 20.0
            }
            .validate()
            .is_err()
        );
        assert!(PartSpec::LiquidEngine { scale: 0.1 }.validate().is_err());
        assert!(PartSpec::Decoupler.validate().is_ok());
        assert!(
            PartSpec::Explosive {
                mass_kg: 1.0,
                grade: ExplosiveGrade::Standard,
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn explosive_grades_trade_research_for_yield_and_never_model_real_blast() {
        let charge = |grade| PartSpec::Explosive { mass_kg: 40.0, grade };
        let standard = charge(ExplosiveGrade::Standard);
        let improved = charge(ExplosiveGrade::Improved);
        let advanced = charge(ExplosiveGrade::Advanced);
        assert_eq!(standard.stats().payload_kg, 40.0);
        assert!(improved.stats().payload_yield_kg > standard.stats().payload_yield_kg);
        assert!(advanced.stats().payload_yield_kg > improved.stats().payload_yield_kg);
        assert!(advanced.stats().dry_mass_kg > 40.0, "the casing is launch mass too");
        assert_eq!(standard.required_capability(), "part:explosive");
        assert_eq!(improved.required_capability(), "part:explosive_improved");
        assert_eq!(advanced.required_capability(), "part:explosive_advanced");
        assert_eq!(advanced.category(), PartCategory::Airframe);
    }

    #[test]
    fn adapter_shape_trades_casing_mass_and_envelope() {
        let adapter = |shape| PartSpec::Adapter {
            length_m: 0.4,
            diameter_m: 0.6,
            exit_diameter_m: 0.3,
            shape,
        };
        let conical = adapter(AdapterShape::Conical);
        let ogive = adapter(AdapterShape::Ogive);
        let stepped = adapter(AdapterShape::Stepped);
        assert!(conical.validate().is_ok());
        assert!(ogive.stats().dry_mass_kg > conical.stats().dry_mass_kg);
        assert!(stepped.stats().dry_mass_kg < conical.stats().dry_mass_kg);
        assert_eq!(conical.stats().diameter_m, 0.6);
        assert_eq!(conical.category(), PartCategory::Airframe);
        assert_eq!(conical.required_capability(), "part:adapter");
    }
}
