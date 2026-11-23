//! Missile design model (abstract, fictional): resizable parts grouped in stages, derived
//! mass / thrust / burn-time / thrust-to-weight figures, and the capabilities that installed
//! sensors and computers give to the programming studio.
//!
//! All coefficients are gameplay values chosen for readable trade-offs. They are not calibrated
//! to any real system and the module contains no guidance, targeting or warhead modelling.

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
            Self::MissionModule { mass_kg } => in_range("module mass (kg)", mass_kg, 1.0, 300.0),
            Self::Sensor { .. } | Self::Computer { .. } | Self::Decoupler => Ok(()),
        }
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
    }
}
