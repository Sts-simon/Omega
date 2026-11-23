//! Abstract flight test: the vehicle's speed along its path is integrated with thrust, drag and
//! (during a vertical launch) gravity, driven by the flight program.
//!
//! This is a game-level model. Altitude only changes air density; there is no steering, no
//! trajectory optimisation, no tracking and no guidance law. A target lock is a single seeded
//! probability roll that depends on the radar class and the speed at that moment.

use crate::program::{Block, Program, validate_program};
use crate::vehicle::{AIR_BREATHING_MIN_SPEED_MS, G0, PartSpec, RadarClass, Vehicle};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LaunchMode {
    /// Gravity applies until the vehicle reaches the pitch-over speed.
    Vertical,
    Air {
        speed_ms: f64,
        altitude_m: f64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightConfig {
    pub launch: LaunchMode,
    pub max_time_s: f64,
    pub dt_s: f64,
    pub seed: u64,
}

impl Default for FlightConfig {
    fn default() -> Self {
        Self {
            launch: LaunchMode::Vertical,
            max_time_s: 600.0,
            dt_s: 0.1,
            seed: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub t_s: f64,
    pub speed_ms: f64,
    pub path_m: f64,
    pub altitude_m: f64,
    pub mass_kg: f64,
    pub thrust_n: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LockResult {
    /// `None` when no radar was left on the vehicle.
    pub sensor: Option<RadarClass>,
    pub probability: f64,
    pub success: bool,
    pub speed_ms: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlightReport {
    pub samples: Vec<Sample>,
    pub events: Vec<(f64, String)>,
    pub anomalies: Vec<String>,
    /// Thrust-to-weight ratio when the first stage was ignited.
    pub first_ignition_twr: Option<f64>,
    pub lifted_off: bool,
    pub max_speed_ms: f64,
    pub path_length_m: f64,
    pub final_speed_ms: f64,
    pub flight_time_s: f64,
    pub lock: Option<LockResult>,
}

const SEA_LEVEL_HPA: f64 = 1_013.25;
const SCALE_HEIGHT_M: f64 = 8_000.0;
const PITCH_OVER_SPEED_MS: f64 = 100.0;
const ALTITUDE_RATE_MS: f64 = 60.0;
const DRAG_COEFFICIENT: f64 = 0.35;
const SAMPLE_PERIOD_S: f64 = 0.5;

fn splitmix64(mut state: u64) -> u64 {
    state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn unit_random(seed: u64) -> f64 {
    (splitmix64(seed) >> 11) as f64 / (1u64 << 53) as f64
}

fn air_density(altitude_m: f64) -> f64 {
    1.225 * (-altitude_m / SCALE_HEIGHT_M).exp()
}

struct ThreadState {
    pc: usize,
    wait_until: f64,
}

pub fn simulate(vehicle: &Vehicle, program: &Program, config: FlightConfig) -> Result<FlightReport, Vec<String>> {
    let mut problems = Vec::new();
    if let Err(issues) = vehicle.validate() {
        problems.extend(issues);
    } else {
        problems.extend(validate_program(program, vehicle));
    }
    if !(0.01..=1.0).contains(&config.dt_s) || !(1.0..=3_600.0).contains(&config.max_time_s) {
        problems.push("test configuration: dt must be 0.01-1 s and duration 1-3600 s".to_string());
    }
    if !problems.is_empty() {
        return Err(problems);
    }

    let dt = config.dt_s;
    let stage_count = vehicle.stages.len();
    let mut attached = vec![true; stage_count];
    let mut ignited = vec![false; stage_count];
    // Remaining propellant per part.
    let mut remaining: Vec<Vec<f64>> = vehicle
        .stages
        .iter()
        .map(|s| s.parts.iter().map(|p| p.spec.stats().propellant_kg).collect())
        .collect();
    let mut burnout_reported = vec![false; stage_count];

    let (mut speed, mut altitude, vertical) = match config.launch {
        LaunchMode::Vertical => (0.0, 0.0, true),
        LaunchMode::Air { speed_ms, altitude_m } => (speed_ms.max(0.0), altitude_m.max(0.0), false),
    };
    let mut pitched_over = !vertical;
    let mut altitude_target: Option<f64> = if vertical { None } else { Some(altitude) };
    let mut speed_target: Option<f64> = None;
    let mut throttle = 1.0;
    let mut throttle_integral = 0.5;
    let mut threads: Vec<ThreadState> = program
        .threads
        .iter()
        .map(|_| ThreadState { pc: 0, wait_until: 0.0 })
        .collect();

    let mut t = 0.0;
    let mut path = 0.0;
    let mut samples = Vec::new();
    let mut events: Vec<(f64, String)> = Vec::new();
    let mut anomalies: Vec<String> = Vec::new();
    let mut first_ignition_twr = None;
    let mut lock: Option<LockResult> = None;
    let mut lifted_off = !vertical;
    let mut ever_ignited = false;
    let mut stuck_s = 0.0;
    let mut next_sample = 0.0;
    let mut max_speed = speed;

    let attached_mass = |attached: &[bool], remaining: &[Vec<f64>]| -> f64 {
        vehicle
            .stages
            .iter()
            .enumerate()
            .filter(|(i, _)| attached[*i])
            .map(|(i, s)| {
                s.parts
                    .iter()
                    .enumerate()
                    .map(|(j, p)| p.spec.stats().dry_mass_kg + remaining[i][j])
                    .sum::<f64>()
            })
            .sum()
    };

    loop {
        // --- program ---
        for thread_index in 0..threads.len() {
            loop {
                let thread = &threads[thread_index];
                if t < thread.wait_until || thread.pc >= program.threads[thread_index].len() {
                    break;
                }
                let block = program.threads[thread_index][thread.pc];
                threads[thread_index].pc += 1;
                match block {
                    Block::Wait { seconds } => {
                        threads[thread_index].wait_until = t + seconds;
                        break;
                    }
                    Block::IgniteStage { stage } => {
                        let has_engine = vehicle.stages[stage].parts.iter().any(|p| {
                            matches!(
                                p.spec,
                                PartSpec::SolidMotor { .. }
                                    | PartSpec::LiquidEngine { .. }
                                    | PartSpec::AirBreathing { .. }
                            )
                        });
                        if !attached[stage] {
                            anomalies.push(format!(
                                "t={t:.1}s: stage {} was already dropped, ignition ignored",
                                stage + 1
                            ));
                        } else if !has_engine {
                            anomalies.push(format!("t={t:.1}s: stage {} has no engine to ignite", stage + 1));
                        } else if !ignited[stage] {
                            ignited[stage] = true;
                            ever_ignited = true;
                            events.push((t, format!("Stage {} ignited", stage + 1)));
                            if first_ignition_twr.is_none() {
                                let thrust: f64 = vehicle.stages[stage]
                                    .parts
                                    .iter()
                                    .map(|p| p.spec.stats().thrust_n)
                                    .sum();
                                first_ignition_twr = Some(thrust / (attached_mass(&attached, &remaining) * G0));
                            }
                        }
                    }
                    Block::SeparateStage { stage } => {
                        if !attached[stage] {
                            anomalies.push(format!("t={t:.1}s: stage {} was already dropped", stage + 1));
                        } else if (0..stage).any(|lower| attached[lower]) {
                            anomalies.push(format!(
                                "t={t:.1}s: stage {} cannot separate while a stage below it is attached",
                                stage + 1
                            ));
                        } else {
                            let wasted: f64 = remaining[stage].iter().sum();
                            if wasted > 1.0 {
                                anomalies.push(format!(
                                    "t={t:.1}s: stage {} dropped with {wasted:.0} kg of propellant unburnt",
                                    stage + 1
                                ));
                            }
                            attached[stage] = false;
                            events.push((t, format!("Stage {} separated", stage + 1)));
                        }
                    }
                    Block::SetThrottle { fraction } => {
                        throttle = fraction;
                        speed_target = None;
                    }
                    Block::HoldSpeed { speed_ms } => speed_target = Some(speed_ms),
                    Block::HoldPressure { hpa } => {
                        altitude_target = Some(-SCALE_HEIGHT_M * (hpa / SEA_LEVEL_HPA).ln());
                        events.push((t, format!("Hold {hpa:.0} hPa")));
                    }
                    Block::LockTarget => {
                        let lowest = attached.iter().position(|a| *a).unwrap_or(stage_count);
                        let radar = vehicle.best_radar(lowest);
                        let (probability, sensor) = match radar {
                            Some(spec) => {
                                if speed > spec.max_lock_speed_ms {
                                    anomalies.push(format!(
                                        "t={t:.1}s: lock attempted at {speed:.0} m/s, above this radar's limit of {:.0} m/s",
                                        spec.max_lock_speed_ms
                                    ));
                                    (0.0, Some(spec.class))
                                } else {
                                    (spec.base_lock_probability, Some(spec.class))
                                }
                            }
                            None => {
                                anomalies
                                    .push(format!("t={t:.1}s: lock attempted but no radar is left on the vehicle"));
                                (0.0, None)
                            }
                        };
                        let success = unit_random(config.seed) < probability;
                        events.push((
                            t,
                            format!("Lock target: {}", if success { "success" } else { "failed" }),
                        ));
                        lock = Some(LockResult {
                            sensor,
                            probability,
                            success,
                            speed_ms: speed,
                        });
                    }
                }
            }
        }

        // --- environment and controller ---
        let rho = air_density(altitude);
        let diameter = {
            let d = vehicle
                .stages
                .iter()
                .enumerate()
                .filter(|(i, _)| attached[*i])
                .flat_map(|(_, s)| s.parts.iter())
                .map(|p| p.spec.stats().diameter_m)
                .fold(0.0, f64::max);
            d.max(0.1)
        };
        let drag = 0.5 * rho * speed * speed * DRAG_COEFFICIENT * std::f64::consts::PI * diameter * diameter / 4.0;

        let effective_throttle = match speed_target {
            Some(target) => {
                throttle_integral = (throttle_integral + 0.002 * (target - speed) * dt).clamp(0.0, 1.0);
                (throttle_integral + 0.01 * (target - speed)).clamp(0.0, 1.0)
            }
            None => throttle,
        };

        // --- thrust and propellant ---
        let mut thrust = 0.0;
        for stage in 0..stage_count {
            if !attached[stage] || !ignited[stage] {
                continue;
            }
            let tank_pool: f64 = vehicle.stages[stage]
                .parts
                .iter()
                .enumerate()
                .filter(|(_, p)| matches!(p.spec, PartSpec::FuelTank { .. }))
                .map(|(j, _)| remaining[stage][j])
                .sum();
            let mut drawn = 0.0;
            let mut any_active = false;
            for (j, part) in vehicle.stages[stage].parts.iter().enumerate() {
                let stats = part.spec.stats();
                match part.spec {
                    PartSpec::SolidMotor { burn_time_s, .. } => {
                        if remaining[stage][j] > 0.0 {
                            thrust += stats.thrust_n;
                            remaining[stage][j] =
                                (remaining[stage][j] - stats.propellant_kg / burn_time_s * dt).max(0.0);
                            any_active = true;
                        }
                    }
                    PartSpec::LiquidEngine { .. } | PartSpec::AirBreathing { .. } => {
                        let can_run =
                            !matches!(part.spec, PartSpec::AirBreathing { .. }) || speed >= AIR_BREATHING_MIN_SPEED_MS;
                        if can_run && tank_pool - drawn > 0.0 {
                            let engine_thrust = stats.thrust_n * effective_throttle;
                            thrust += engine_thrust;
                            drawn += engine_thrust / (stats.isp_s * G0) * dt;
                            any_active = true;
                        }
                    }
                    _ => {}
                }
            }
            if drawn > 0.0 {
                let mut to_remove = drawn;
                for (j, part) in vehicle.stages[stage].parts.iter().enumerate() {
                    if matches!(part.spec, PartSpec::FuelTank { .. }) && to_remove > 0.0 {
                        let take = remaining[stage][j].min(to_remove);
                        remaining[stage][j] -= take;
                        to_remove -= take;
                    }
                }
            }
            if !any_active && !burnout_reported[stage] {
                burnout_reported[stage] = true;
                events.push((t, format!("Stage {} burnout", stage + 1)));
            }
        }

        // --- motion ---
        let mass = attached_mass(&attached, &remaining);
        let gravity = if vertical && !pitched_over { G0 } else { 0.0 };
        let accel = (thrust - drag) / mass - gravity;
        if vertical && !pitched_over && ever_ignited && !lifted_off {
            if accel > 0.0 {
                lifted_off = true;
                events.push((t, "Liftoff".to_string()));
            } else {
                stuck_s += dt;
            }
        }
        speed = (speed + accel * dt).max(0.0);
        path += speed * dt;
        if vertical && !pitched_over {
            altitude += speed * dt;
            if speed >= PITCH_OVER_SPEED_MS {
                pitched_over = true;
                events.push((t, "Pitch-over".to_string()));
                altitude_target.get_or_insert(altitude);
            }
        } else if let Some(target) = altitude_target {
            let step = (target - altitude).clamp(-ALTITUDE_RATE_MS * dt, ALTITUDE_RATE_MS * dt);
            altitude = (altitude + step).max(0.0);
        }
        max_speed = max_speed.max(speed);
        t += dt;

        if t >= next_sample {
            samples.push(Sample {
                t_s: t,
                speed_ms: speed,
                path_m: path,
                altitude_m: altitude,
                mass_kg: mass,
                thrust_n: thrust,
            });
            next_sample += SAMPLE_PERIOD_S;
        }

        // --- end conditions ---
        let finished_programs = threads.iter().zip(&program.threads).all(|(s, p)| s.pc >= p.len());
        if vertical && !lifted_off && ever_ignited && stuck_s > 2.0 {
            anomalies.push("the vehicle cannot lift off: thrust-to-weight ratio is below 1".to_string());
            break;
        }
        if !ever_ignited && finished_programs {
            anomalies.push("no stage was ever ignited".to_string());
            break;
        }
        if ever_ignited && thrust <= 0.0 && speed < 20.0 && finished_programs && lifted_off {
            events.push((t, "Stalled".to_string()));
            break;
        }
        if t >= config.max_time_s {
            break;
        }
    }

    if samples.last().is_none_or(|s| s.t_s < t - 1e-9) {
        samples.push(Sample {
            t_s: t,
            speed_ms: speed,
            path_m: path,
            altitude_m: altitude,
            mass_kg: attached_mass(&attached, &remaining),
            thrust_n: 0.0,
        });
    }
    Ok(FlightReport {
        samples,
        events,
        anomalies,
        first_ignition_twr,
        lifted_off,
        max_speed_ms: max_speed,
        path_length_m: path,
        final_speed_ms: speed,
        flight_time_s: t,
        lock,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vehicle::tests::{part, sounding_rocket};
    use crate::vehicle::{ComputerClass, SensorKind, Stage};

    fn two_stage_program(separate_at: f64) -> Program {
        Program::single(vec![
            Block::IgniteStage { stage: 0 },
            Block::Wait { seconds: separate_at },
            Block::SeparateStage { stage: 0 },
            Block::IgniteStage { stage: 1 },
        ])
    }

    fn cruise_vehicle() -> Vehicle {
        Vehicle {
            name: "Cruiser".into(),
            stages: vec![Stage {
                parts: vec![
                    part(
                        "tank",
                        PartSpec::FuelTank {
                            length_m: 1.5,
                            diameter_m: 0.3,
                        },
                    ),
                    part("engine", PartSpec::AirBreathing { scale: 1.0 }),
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
                    part(
                        "radar",
                        PartSpec::Sensor {
                            kind: SensorKind::Radar(RadarClass::Basic),
                        },
                    ),
                ],
            }],
        }
    }

    fn air(speed: f64) -> FlightConfig {
        FlightConfig {
            launch: LaunchMode::Air {
                speed_ms: speed,
                altitude_m: 0.0,
            },
            max_time_s: 300.0,
            ..FlightConfig::default()
        }
    }

    #[test]
    fn a_two_stage_sequence_lifts_off_stages_in_order_and_is_deterministic() {
        let vehicle = sounding_rocket();
        let program = two_stage_program(20.0);
        let a = simulate(&vehicle, &program, FlightConfig::default()).unwrap();
        let b = simulate(&vehicle, &program, FlightConfig::default()).unwrap();
        assert_eq!(a, b);
        assert!(
            a.lifted_off && a.path_length_m > 1_000.0 && a.max_speed_ms > 150.0,
            "{:?}",
            a.events
        );
        let labels: Vec<&str> = a.events.iter().map(|(_, e)| e.as_str()).collect();
        let pos = |name: &str| {
            labels
                .iter()
                .position(|l| *l == name)
                .unwrap_or_else(|| panic!("{name} missing in {labels:?}"))
        };
        assert!(pos("Stage 1 ignited") < pos("Liftoff"));
        assert!(pos("Stage 1 separated") < pos("Stage 2 ignited"));
        assert!(a.first_ignition_twr.unwrap() > 1.0);
    }

    #[test]
    fn separating_before_burnout_wastes_propellant_and_flies_shorter() {
        let vehicle = sounding_rocket();
        let early = simulate(&vehicle, &two_stage_program(8.0), FlightConfig::default()).unwrap();
        let late = simulate(&vehicle, &two_stage_program(20.5), FlightConfig::default()).unwrap();
        assert!(
            early.anomalies.iter().any(|a| a.contains("unburnt")),
            "{:?}",
            early.anomalies
        );
        assert!(
            late.anomalies.iter().all(|a| !a.contains("unburnt")),
            "{:?}",
            late.anomalies
        );
        assert!(late.path_length_m > early.path_length_m);
    }

    #[test]
    fn a_faster_burning_motor_gives_a_higher_liftoff_twr() {
        let build = |burn| {
            let mut v = sounding_rocket();
            v.stages[0].parts[0].spec = PartSpec::SolidMotor {
                length_m: 2.0,
                diameter_m: 0.4,
                burn_time_s: burn,
            };
            v
        };
        let program = Program::single(vec![Block::IgniteStage { stage: 0 }]);
        let slow = simulate(&build(40.0), &program, FlightConfig::default()).unwrap();
        let fast = simulate(&build(10.0), &program, FlightConfig::default()).unwrap();
        assert!(fast.first_ignition_twr.unwrap() > 3.0 * slow.first_ignition_twr.unwrap());
    }

    #[test]
    fn a_vehicle_too_heavy_for_its_motor_cannot_lift_off() {
        let mut vehicle = sounding_rocket();
        vehicle.stages[1].parts[3].spec = PartSpec::MissionModule { mass_kg: 300.0 };
        vehicle.stages[0].parts[0].spec = PartSpec::SolidMotor {
            length_m: 0.5,
            diameter_m: 0.2,
            burn_time_s: 60.0,
        };
        let report = simulate(
            &vehicle,
            &Program::single(vec![Block::IgniteStage { stage: 0 }]),
            FlightConfig::default(),
        )
        .unwrap();
        assert!(!report.lifted_off);
        assert!(report.first_ignition_twr.unwrap() < 1.0);
        assert!(
            report.anomalies.iter().any(|a| a.contains("cannot lift off")),
            "{:?}",
            report.anomalies
        );
    }

    #[test]
    fn hold_speed_with_a_pitot_converges_on_the_target() {
        let program = Program::single(vec![
            Block::IgniteStage { stage: 0 },
            Block::HoldSpeed { speed_ms: 220.0 },
            Block::Wait { seconds: 250.0 },
        ]);
        let report = simulate(&cruise_vehicle(), &program, air(200.0)).unwrap();
        assert!(
            (report.final_speed_ms - 220.0).abs() < 22.0,
            "final {}",
            report.final_speed_ms
        );
        assert!(report.path_length_m > 40_000.0);
    }

    #[test]
    fn air_breathing_engines_need_airspeed() {
        let program = Program::single(vec![Block::IgniteStage { stage: 0 }, Block::Wait { seconds: 20.0 }]);
        let slow = simulate(&cruise_vehicle(), &program, air(40.0)).unwrap();
        assert!(slow.max_speed_ms < 41.0, "{}", slow.max_speed_ms);
    }

    #[test]
    fn hold_pressure_sets_the_altitude_from_the_pressure_level() {
        let mut vehicle = cruise_vehicle();
        vehicle.stages[0].parts.push(part(
            "baro",
            PartSpec::Sensor {
                kind: SensorKind::Barometer,
            },
        ));
        let program = Program::single(vec![
            Block::IgniteStage { stage: 0 },
            Block::HoldPressure { hpa: 900.0 },
            Block::Wait { seconds: 60.0 },
        ]);
        let report = simulate(&vehicle, &program, air(200.0)).unwrap();
        let expected = -8_000.0 * (900.0_f64 / 1013.25).ln();
        assert!(
            (report.samples.last().unwrap().altitude_m - expected).abs() < 5.0,
            "{} vs {expected}",
            report.samples.last().unwrap().altitude_m
        );
    }

    #[test]
    fn lock_target_is_a_seeded_roll_limited_by_radar_speed() {
        let program = Program::single(vec![
            Block::IgniteStage { stage: 0 },
            Block::HoldSpeed { speed_ms: 220.0 },
            Block::Wait { seconds: 30.0 },
            Block::LockTarget,
        ]);
        let report = simulate(&cruise_vehicle(), &program, air(200.0)).unwrap();
        let lock = report.lock.unwrap();
        assert_eq!(lock.sensor, Some(RadarClass::Basic));
        assert!((lock.probability - 0.55).abs() < 1e-9);
        let outcomes: Vec<bool> = (0..40)
            .map(|seed| {
                simulate(&cruise_vehicle(), &program, FlightConfig { seed, ..air(200.0) })
                    .unwrap()
                    .lock
                    .unwrap()
                    .success
            })
            .collect();
        assert!(outcomes.iter().any(|s| *s) && outcomes.iter().any(|s| !*s));
        let hits = outcomes.iter().filter(|s| **s).count() as f64 / 40.0;
        assert!((hits - 0.55).abs() < 0.25, "{hits}");

        let fast = Program::single(vec![
            Block::IgniteStage { stage: 0 },
            Block::Wait { seconds: 1.0 },
            Block::LockTarget,
        ]);
        let too_fast = simulate(&cruise_vehicle(), &fast, air(600.0)).unwrap();
        assert_eq!(too_fast.lock.unwrap().probability, 0.0);
        assert!(
            too_fast.anomalies.iter().any(|a| a.contains("above this radar")),
            "{:?}",
            too_fast.anomalies
        );
    }

    #[test]
    fn a_radar_dropped_with_its_stage_cannot_lock() {
        let mut vehicle = sounding_rocket();
        vehicle.stages[0].parts.push(part(
            "radar",
            PartSpec::Sensor {
                kind: SensorKind::Radar(RadarClass::Basic),
            },
        ));
        let program = Program::single(vec![
            Block::IgniteStage { stage: 0 },
            Block::Wait { seconds: 21.0 },
            Block::SeparateStage { stage: 0 },
            Block::LockTarget,
        ]);
        // The radar sits in stage 1 and is gone after separation.
        let report = simulate(&vehicle, &program, FlightConfig::default()).unwrap();
        assert_eq!(report.lock.unwrap().sensor, None);
        assert!(
            report.anomalies.iter().any(|a| a.contains("no radar is left")),
            "{:?}",
            report.anomalies
        );
    }

    #[test]
    fn out_of_order_separation_and_missing_ignition_are_anomalies() {
        let vehicle = sounding_rocket();
        let out_of_order = simulate(
            &vehicle,
            &Program::single(vec![Block::IgniteStage { stage: 0 }, Block::SeparateStage { stage: 1 }]),
            FlightConfig::default(),
        )
        .unwrap();
        assert!(
            out_of_order.anomalies.iter().any(|a| a.contains("cannot separate")),
            "{:?}",
            out_of_order.anomalies
        );
        let idle = simulate(
            &vehicle,
            &Program::single(vec![Block::Wait { seconds: 1.0 }]),
            FlightConfig::default(),
        )
        .unwrap();
        assert!(idle.anomalies.iter().any(|a| a.contains("ever ignited")));
    }

    #[test]
    fn invalid_vehicle_program_or_config_is_rejected() {
        let vehicle = sounding_rocket();
        assert!(
            simulate(
                &vehicle,
                &Program::single(vec![Block::HoldPressure { hpa: 900.0 }]),
                FlightConfig::default()
            )
            .is_err()
        );
        assert!(simulate(&Vehicle::default(), &Program::default(), FlightConfig::default()).is_err());
        assert!(
            simulate(
                &vehicle,
                &Program::default(),
                FlightConfig {
                    dt_s: 5.0,
                    ..FlightConfig::default()
                }
            )
            .is_err()
        );
    }
}
