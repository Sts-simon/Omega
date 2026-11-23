//! Missile design model (abstract, fictional): resizable parts grouped in stages, derived
//! mass / thrust / burn-time / thrust-to-weight figures, and the capabilities that installed
//! sensors and computers give to the programming studio.
//!
//! All coefficients are gameplay values chosen for readable trade-offs. They are not calibrated
//! to any real system and the module contains no guidance, targeting or warhead modelling.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vehicle {
    pub name: String,
    pub stages: Vec<Stage>,
}

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

impl Vehicle {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut issues = Vec::new();
        if self.stages.is_empty() {
            issues.push("the vehicle has no stages".to_string());
        }
        let mut ids = BTreeSet::new();
        for (index, stage) in self.stages.iter().enumerate() {
            let has_tank = stage.parts.iter().any(|p| matches!(p.spec, PartSpec::FuelTank { .. }));
            for part in &stage.parts {
                if !ids.insert(part.id.as_str()) {
                    issues.push(format!("duplicate part id '{}'", part.id));
                }
                if let Err(error) = part.spec.validate() {
                    issues.push(format!("{} (stage {}): {error}", part.id, index + 1));
                }
                if matches!(part.spec, PartSpec::LiquidEngine { .. } | PartSpec::AirBreathing { .. }) && !has_tank {
                    issues.push(format!(
                        "{} (stage {}): needs a fuel tank in the same stage",
                        part.id,
                        index + 1
                    ));
                }
            }
        }
        let computers: Vec<(usize, &Part)> = self
            .stages
            .iter()
            .enumerate()
            .flat_map(|(i, s)| {
                s.parts
                    .iter()
                    .filter(|p| matches!(p.spec, PartSpec::Computer { .. }))
                    .map(move |p| (i, p))
            })
            .collect();
        match computers.as_slice() {
            [] => issues.push("the vehicle needs a flight computer".to_string()),
            [(index, part)] => {
                if *index + 1 != self.stages.len() {
                    issues.push(format!(
                        "{} would be dropped with stage {}: put the computer in the last stage",
                        part.id,
                        index + 1
                    ));
                }
            }
            _ => issues.push("only one flight computer is allowed".to_string()),
        }
        if issues.is_empty() { Ok(()) } else { Err(issues) }
    }

    pub fn parts(&self) -> impl Iterator<Item = (usize, &Part)> {
        self.stages
            .iter()
            .enumerate()
            .flat_map(|(i, s)| s.parts.iter().map(move |p| (i, p)))
    }

    pub fn computer(&self) -> Option<ComputerClass> {
        self.parts().find_map(|(_, p)| match p.spec {
            PartSpec::Computer { class } => Some(class),
            _ => None,
        })
    }

    /// Best radar installed, if any.
    pub fn best_radar(&self, min_stage: usize) -> Option<RadarSpec> {
        self.parts()
            .filter(|(stage, _)| *stage >= min_stage)
            .filter_map(|(_, p)| match p.spec {
                PartSpec::Sensor {
                    kind: SensorKind::Radar(class),
                } => Some(class.spec()),
                _ => None,
            })
            .max_by(|a, b| a.base_lock_probability.total_cmp(&b.base_lock_probability))
    }

    /// Programming blocks the installed hardware allows (see `program::BlockKind`).
    pub fn available_blocks(&self) -> BTreeSet<&'static str> {
        let mut blocks: BTreeSet<&'static str> = ["wait", "ignite_stage", "set_throttle"].into_iter().collect();
        if self.stages.len() >= 2 {
            blocks.insert("separate_stage");
        }
        for (_, part) in self.parts() {
            if let PartSpec::Sensor { kind } = part.spec {
                match kind {
                    SensorKind::Pitot => blocks.insert("hold_speed"),
                    SensorKind::Barometer => blocks.insert("hold_pressure"),
                    SensorKind::Radar(_) => blocks.insert("lock_target"),
                };
            }
        }
        blocks
    }

    /// Research capabilities this design needs that are not owned yet.
    pub fn missing_research(&self, owned: &BTreeSet<String>) -> Vec<String> {
        let needed: BTreeSet<&str> = self.parts().map(|(_, p)| p.spec.required_capability()).collect();
        needed
            .into_iter()
            .filter(|c| !owned.contains(*c))
            .map(str::to_string)
            .collect()
    }

    pub fn max_diameter_m(&self) -> f64 {
        self.parts().map(|(_, p)| p.spec.stats().diameter_m).fold(0.0, f64::max)
    }

    fn stage_wet_mass(stage: &Stage) -> f64 {
        stage
            .parts
            .iter()
            .map(|p| {
                let s = p.spec.stats();
                s.dry_mass_kg + s.propellant_kg
            })
            .sum()
    }

    pub fn total_wet_mass_kg(&self) -> f64 {
        self.stages.iter().map(Self::stage_wet_mass).sum()
    }

    pub fn stage_reports(&self) -> Vec<StageReport> {
        let mut reports = Vec::new();
        for (index, stage) in self.stages.iter().enumerate() {
            let above: f64 = self.stages[index + 1..].iter().map(Self::stage_wet_mass).sum();
            let wet = Self::stage_wet_mass(stage);
            let ignition_mass = wet + above;
            let mut thrust = 0.0;
            let mut propellant = 0.0;
            let mut mass_flow_weight = 0.0;
            let mut burn_time: Option<f64> = None;
            for part in &stage.parts {
                let s = part.spec.stats();
                thrust += s.thrust_n;
                propellant += s.propellant_kg;
                if s.thrust_n > 0.0 && s.isp_s > 0.0 {
                    mass_flow_weight += s.thrust_n / (s.isp_s * G0);
                }
                if let Some(bt) = s.burn_time_s {
                    burn_time = Some(burn_time.map_or(bt, |b: f64| b.max(bt)));
                }
            }
            let isp = if mass_flow_weight > 0.0 {
                thrust / (mass_flow_weight * G0)
            } else {
                0.0
            };
            let burnout_mass = (ignition_mass - propellant).max(1e-9);
            let delta_v = if isp > 0.0 {
                isp * G0 * (ignition_mass / burnout_mass).ln()
            } else {
                0.0
            };
            reports.push(StageReport {
                index,
                stage_wet_mass_kg: wet,
                ignition_mass_kg: ignition_mass,
                thrust_n: thrust,
                burn_time_s: burn_time,
                twr_at_ignition: thrust / (ignition_mass * G0),
                delta_v_ms: delta_v,
            });
        }
        reports
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn part(id: &str, spec: PartSpec) -> Part {
        Part {
            id: id.to_string(),
            spec,
        }
    }

    pub fn sounding_rocket() -> Vehicle {
        Vehicle {
            name: "Test rocket".into(),
            stages: vec![
                Stage {
                    parts: vec![
                        part(
                            "booster",
                            PartSpec::SolidMotor {
                                length_m: 2.0,
                                diameter_m: 0.4,
                                burn_time_s: 20.0,
                            },
                        ),
                        part("dec1", PartSpec::Decoupler),
                    ],
                },
                Stage {
                    parts: vec![
                        part(
                            "sustainer",
                            PartSpec::SolidMotor {
                                length_m: 1.0,
                                diameter_m: 0.3,
                                burn_time_s: 25.0,
                            },
                        ),
                        part(
                            "pc",
                            PartSpec::Computer {
                                class: ComputerClass::Basic,
                            },
                        ),
                        part(
                            "pitot",
                            PartSpec::Sensor {
                                kind: SensorKind::Pitot,
                            },
                        ),
                        part("module", PartSpec::MissionModule { mass_kg: 40.0 }),
                    ],
                },
            ],
        }
    }

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
    fn stage_reports_compute_twr_and_delta_v_with_the_stack_above() {
        let reports = sounding_rocket().stage_reports();
        assert_eq!(reports.len(), 2);
        assert!(reports[0].ignition_mass_kg > reports[0].stage_wet_mass_kg);
        assert!(reports[0].twr_at_ignition > 1.0, "{}", reports[0].twr_at_ignition);
        assert!(reports[1].twr_at_ignition > reports[1].thrust_n / (reports[0].ignition_mass_kg * G0));
        assert!(reports[0].delta_v_ms > 0.0 && reports[1].delta_v_ms > 0.0);
        assert_eq!(reports[0].burn_time_s, Some(20.0));
    }

    #[test]
    fn valid_vehicle_passes_and_problems_are_listed() {
        assert!(sounding_rocket().validate().is_ok());
        let mut bad = sounding_rocket();
        bad.stages[0].parts.push(part(
            "pc2",
            PartSpec::Computer {
                class: ComputerClass::Basic,
            },
        ));
        bad.stages[1].parts.push(part("booster", PartSpec::Decoupler));
        let issues = bad.validate().unwrap_err().join(" | ");
        assert!(issues.contains("only one flight computer"), "{issues}");
        assert!(issues.contains("duplicate part id"), "{issues}");

        let mut dropped = sounding_rocket();
        let computer = dropped.stages[1].parts.remove(1);
        dropped.stages[0].parts.push(computer);
        assert!(dropped.validate().unwrap_err().join(" ").contains("last stage"));

        let mut no_tank = sounding_rocket();
        no_tank.stages[1]
            .parts
            .push(part("eng", PartSpec::LiquidEngine { scale: 1.0 }));
        assert!(no_tank.validate().unwrap_err().join(" ").contains("fuel tank"));

        let mut huge = sounding_rocket();
        huge.stages[0].parts[0].spec = PartSpec::SolidMotor {
            length_m: 20.0,
            diameter_m: 0.4,
            burn_time_s: 20.0,
        };
        assert!(huge.validate().is_err());
        assert!(Vehicle::default().validate().is_err());
    }

    #[test]
    fn installed_sensors_unlock_programming_blocks() {
        let mut vehicle = sounding_rocket();
        let blocks = vehicle.available_blocks();
        assert!(blocks.contains("hold_speed") && !blocks.contains("lock_target") && !blocks.contains("hold_pressure"));
        vehicle.stages[1].parts.push(part(
            "radar",
            PartSpec::Sensor {
                kind: SensorKind::Radar(RadarClass::Active),
            },
        ));
        vehicle.stages[1].parts.push(part(
            "baro",
            PartSpec::Sensor {
                kind: SensorKind::Barometer,
            },
        ));
        let blocks = vehicle.available_blocks();
        assert!(blocks.contains("lock_target") && blocks.contains("hold_pressure"));
        assert!(vehicle.best_radar(0).unwrap().class == RadarClass::Active);
        let mut single = sounding_rocket();
        single.stages.pop();
        assert!(!single.available_blocks().contains("separate_stage"));
    }

    #[test]
    fn missing_research_lists_only_what_is_not_owned() {
        let mut vehicle = sounding_rocket();
        vehicle.stages[1].parts.push(part(
            "radar",
            PartSpec::Sensor {
                kind: SensorKind::Radar(RadarClass::Active),
            },
        ));
        let owned: BTreeSet<String> = [
            "part:solid_motor",
            "part:decoupler",
            "part:mission_module",
            "computer:basic",
            "sensor:pitot",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(
            vehicle.missing_research(&owned),
            vec!["sensor:radar_active".to_string()]
        );
    }
}
