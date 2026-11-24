//! Abstract flight test: the vehicle's speed along its path is integrated with thrust, drag and
//! (during a vertical launch) gravity, driven by the flight program.
//!
//! This is a game-level model. Altitude only changes air density; there is no steering, no
//! trajectory optimisation, no tracking and no guidance law. A target lock is a single seeded
//! probability roll that depends on the radar class and the speed at that moment.

use crate::aero::AeroSummary;
use crate::assembly::Vehicle;
use crate::program::{Block, Program, validate_program};
use crate::vehicle::{AIR_BREATHING_MIN_SPEED_MS, G0, PartSpec, RadarClass};

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
    /// Aerodynamic figures the run used: quoted by the workshop, the trial screen and the dossier.
    pub aero: AeroSummary,
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

/// The roll a seeded target-lock attempt compares with the radar probability. Exposed so the trial
/// screen can show what a seed does before a run.
pub fn lock_roll(seed: u64) -> f64 {
    unit_random(seed)
}

/// A seed's manufacturing and atmosphere spread: small, symmetric and deterministic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dispersion {
    /// Multiplier on engine thrust (grain lot and nozzle).
    pub thrust: f64,
    /// Multiplier on a solid motor's burn time (propellant mass flow).
    pub burn: f64,
    /// Multiplier on drag (skin finish and air density).
    pub drag: f64,
}

/// Amplitude of the per-seed spread. Small enough that the mean still describes the design.
pub const DISPERSION_THRUST: f64 = 0.03;
pub const DISPERSION_BURN: f64 = 0.03;
pub const DISPERSION_DRAG: f64 = 0.05;

fn spread(seed: u64, salt: u64, amplitude: f64) -> f64 {
    1.0 + amplitude * (2.0 * unit_random(seed ^ salt) - 1.0)
}

/// The spread a seed applies to the flight: ±3 % thrust and burn, ±5 % drag. The target lock stays
/// a single probability roll (`lock_roll`); this only varies the hardware, never a guidance law.
pub fn dispersion(seed: u64) -> Dispersion {
    Dispersion {
        thrust: spread(seed, 0x5EED, DISPERSION_THRUST),
        burn: spread(seed, 0x1234_5678_9ABC_DEF0, DISPERSION_BURN),
        drag: spread(seed, 0x0D15_C0FF, DISPERSION_DRAG),
    }
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
    let spread = dispersion(config.seed);
    // Aerodynamics never adds a per-step geometry pass: the figures are computed once from the
    // design and applied to the existing integration. A design without surfaces leaves every
    // term below at its previous value.
    let aero = crate::aero::figures(vehicle);
    let stability = aero.stability_ratio();
    // A fragile airframe draws at most one seeded surface fault per run; a sound one never does.
    let surface_fault = unit_random(config.seed ^ 0xF0A7_0000) < aero.surface_fault_chance();
    let authority = aero.authority_ratio() * if surface_fault { 0.5 } else { 1.0 };
    let lift = aero.lift_ratio();
    // Passive stability damps the seeded drag spread, it never removes it.
    let drag_spread = if aero.surface_count == 0 {
        spread.drag
    } else {
        1.0 + (spread.drag - 1.0) * (1.0 - 0.6 * stability)
    };
    // Wings lower the airspeed the air-breathing engine needs and reduce the gravity loss during
    // the vertical climb, with floors so a huge wing cannot make a design free.
    let air_min_speed = AIR_BREATHING_MIN_SPEED_MS * (1.0 - 0.45 * lift).max(0.55);
    let pitch_gravity = G0 * (1.0 - 0.35 * lift);
    // Authority and stability tighten the altitude controller.
    let altitude_rate = ALTITUDE_RATE_MS * (1.0 + 0.30 * authority + 0.20 * stability);
    // The stages are derived from the attachment tree; the flight model keeps consuming the same
    // flat stage views it always did.
    let stages = vehicle.stage_views();
    let stage_count = stages.len();
    let mut attached = vec![true; stage_count];
    let mut ignited = vec![false; stage_count];
    // Remaining propellant per part.
    let mut remaining: Vec<Vec<f64>> = stages
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
    if surface_fault {
        events.push((0.0, "Surface fault: authority halved".to_string()));
        anomalies.push("a control surface faulted: control authority halved for this run".to_string());
    }
    let mut first_ignition_twr = None;
    let mut lock: Option<LockResult> = None;
    let mut lifted_off = !vertical;
    let mut ever_ignited = false;
    let mut stuck_s = 0.0;
    let mut next_sample = 0.0;
    let mut max_speed = speed;

    let attached_mass = |attached: &[bool], remaining: &[Vec<f64>]| -> f64 {
        stages
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
                        let has_engine = stages[stage].parts.iter().any(|p| {
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
                                let thrust: f64 =
                                    stages[stage].parts.iter().map(|p| p.spec.stats().thrust_n).sum::<f64>()
                                        * spread.thrust;
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
                        let success = lock_roll(config.seed) < probability;
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
            let d = stages
                .iter()
                .enumerate()
                .filter(|(i, _)| attached[*i])
                .flat_map(|(_, s)| s.parts.iter())
                .map(|p| p.spec.stats().diameter_m)
                .fold(0.0, f64::max);
            d.max(0.1)
        };
        let body_drag_area = DRAG_COEFFICIENT * std::f64::consts::PI * diameter * diameter / 4.0;
        let drag = 0.5 * rho * speed * speed * (body_drag_area + aero.surface_drag_area_m2) * drag_spread;

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
            let tank_pool: f64 = stages[stage]
                .parts
                .iter()
                .enumerate()
                .filter(|(_, p)| matches!(p.spec, PartSpec::FuelTank { .. }))
                .map(|(j, _)| remaining[stage][j])
                .sum();
            let mut drawn = 0.0;
            let mut any_active = false;
            for (j, part) in stages[stage].parts.iter().enumerate() {
                let stats = part.spec.stats();
                match part.spec {
                    PartSpec::SolidMotor { burn_time_s, .. } => {
                        if remaining[stage][j] > 0.0 {
                            thrust += stats.thrust_n * spread.thrust;
                            remaining[stage][j] =
                                (remaining[stage][j] - stats.propellant_kg / (burn_time_s * spread.burn) * dt).max(0.0);
                            any_active = true;
                        }
                    }
                    PartSpec::LiquidEngine { .. } | PartSpec::AirBreathing { .. } => {
                        let can_run = !matches!(part.spec, PartSpec::AirBreathing { .. }) || speed >= air_min_speed;
                        if can_run && tank_pool - drawn > 0.0 {
                            let engine_thrust = stats.thrust_n * spread.thrust * effective_throttle;
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
                for (j, part) in stages[stage].parts.iter().enumerate() {
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
        let gravity = if vertical && !pitched_over { pitch_gravity } else { 0.0 };
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
            let step = (target - altitude).clamp(-altitude_rate * dt, altitude_rate * dt);
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
        aero: aero.summary(),
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
    use crate::assembly::tests::sounding_rocket;
    use crate::vehicle::{ComputerClass, SensorKind};

    fn two_stage_program(separate_at: f64) -> Program {
        Program::single(vec![
            Block::IgniteStage { stage: 0 },
            Block::Wait { seconds: separate_at },
            Block::SeparateStage { stage: 0 },
            Block::IgniteStage { stage: 1 },
        ])
    }

    fn cruise_vehicle() -> Vehicle {
        Vehicle::from_stages(
            "Cruiser",
            vec![crate::vehicle::Stage {
                parts: vec![
                    crate::vehicle::Part {
                        id: "tank".into(),
                        spec: PartSpec::FuelTank {
                            length_m: 1.5,
                            diameter_m: 0.3,
                        },
                    },
                    crate::vehicle::Part {
                        id: "engine".into(),
                        spec: PartSpec::AirBreathing { scale: 1.0 },
                    },
                    crate::vehicle::Part {
                        id: "pc".into(),
                        spec: PartSpec::Computer {
                            class: ComputerClass::Basic,
                        },
                    },
                    crate::vehicle::Part {
                        id: "pitot".into(),
                        spec: PartSpec::Sensor {
                            kind: SensorKind::Pitot,
                        },
                    },
                    crate::vehicle::Part {
                        id: "radar".into(),
                        spec: PartSpec::Sensor {
                            kind: SensorKind::Radar(RadarClass::Basic),
                        },
                    },
                ],
            }],
        )
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
            v.replace_part(
                "booster",
                PartSpec::SolidMotor {
                    length_m: 2.0,
                    diameter_m: 0.4,
                    burn_time_s: burn,
                },
            );
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
        vehicle.replace_part("module", PartSpec::MissionModule { mass_kg: 300.0 });
        vehicle.replace_part(
            "booster",
            PartSpec::SolidMotor {
                length_m: 0.5,
                diameter_m: 0.2,
                burn_time_s: 60.0,
            },
        );
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
        vehicle
            .add_to_top(
                "baro",
                PartSpec::Sensor {
                    kind: SensorKind::Barometer,
                },
            )
            .unwrap();
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
    fn a_seed_spreads_the_hardware_but_the_flight_stays_deterministic() {
        let vehicle = sounding_rocket();
        let program = two_stage_program(20.0);
        let paths: Vec<f64> = (0..12)
            .map(|seed| {
                simulate(
                    &vehicle,
                    &program,
                    FlightConfig {
                        seed,
                        ..FlightConfig::default()
                    },
                )
                .unwrap()
                .path_length_m
            })
            .collect();
        let min = paths.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = paths.iter().cloned().fold(0.0, f64::max);
        assert!(max - min > 1.0, "every seed flew the same path: {paths:?}");

        for seed in 0..10 {
            let spread = dispersion(seed);
            assert_eq!(spread, dispersion(seed), "the spread is deterministic");
            assert!((spread.thrust - 1.0).abs() <= DISPERSION_THRUST + 1e-12);
            assert!((spread.burn - 1.0).abs() <= DISPERSION_BURN + 1e-12);
            assert!((spread.drag - 1.0).abs() <= DISPERSION_DRAG + 1e-12);
        }
        let mean = (0..200).map(|seed| dispersion(seed).thrust).sum::<f64>() / 200.0;
        assert!((mean - 1.0).abs() < DISPERSION_THRUST / 2.0, "mean spread {mean}");
    }

    #[test]
    fn a_radar_dropped_with_its_stage_cannot_lock() {
        let mut vehicle = sounding_rocket();
        vehicle
            .add_attached(
                "radar",
                PartSpec::Sensor {
                    kind: SensorKind::Radar(RadarClass::Basic),
                },
                "booster",
                crate::assembly::NodeId::Radial(0),
            )
            .unwrap();
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
    fn the_seeded_spread_reaches_the_simulated_thrust() {
        let vehicle = sounding_rocket();
        let program = Program::single(vec![Block::IgniteStage { stage: 0 }]);
        let twr = |seed| {
            simulate(
                &vehicle,
                &program,
                FlightConfig {
                    seed,
                    ..FlightConfig::default()
                },
            )
            .unwrap()
            .first_ignition_twr
            .unwrap()
        };
        let (slow_seed, fast_seed) =
            (0..100u64)
                .map(|seed| (seed, dispersion(seed).thrust))
                .fold((0, 0), |(low, high), (seed, factor)| {
                    let low = if factor < dispersion(low).thrust { seed } else { low };
                    let high = if factor > dispersion(high).thrust { seed } else { high };
                    (low, high)
                });
        let ratio = twr(fast_seed) / twr(slow_seed);
        let expected = dispersion(fast_seed).thrust / dispersion(slow_seed).thrust;
        assert!((ratio - expected).abs() < 1e-9, "{ratio} vs {expected}");
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

    fn with_fins(vehicle: &mut Vehicle) {
        use crate::assembly::NodeId;
        for index in 0..4u8 {
            vehicle
                .add_attached(
                    format!("fin-{index}"),
                    PartSpec::FinSet {
                        span_m: 0.3,
                        chord_m: 0.35,
                        role: crate::vehicle::StabilizerRole::Rear,
                        layout: crate::vehicle::FinLayout::Symmetric,
                        light: false,
                    },
                    "booster",
                    NodeId::Radial(index),
                )
                .unwrap();
        }
    }

    fn with_surfaces(vehicle: &mut Vehicle, spec: PartSpec) {
        use crate::assembly::NodeId;
        for index in 0..4u8 {
            vehicle
                .add_attached(format!("surface-{index}"), spec, "tank", NodeId::Radial(index))
                .unwrap();
        }
    }

    #[test]
    fn a_design_without_surfaces_reports_no_aerodynamic_effect() {
        let report = simulate(&sounding_rocket(), &two_stage_program(20.0), FlightConfig::default()).unwrap();
        assert_eq!(report.aero.surface_count, 0);
        assert_eq!(report.aero.surface_drag_area_m2, 0.0);
        assert_eq!(report.aero.mass_kg, 0.0);
        assert_eq!(report.aero.stability, 0.0);
        assert_eq!(report.aero.margin_m, 0.0);
        // The integration still answers exactly the same way twice.
        let again = simulate(&sounding_rocket(), &two_stage_program(20.0), FlightConfig::default()).unwrap();
        assert_eq!(report, again);
    }

    #[test]
    fn fins_damp_the_seeded_dispersion_without_removing_it() {
        let program = two_stage_program(20.0);
        let plain = sounding_rocket();
        let mut finned = plain.clone();
        with_fins(&mut finned);
        let spread = |vehicle: &Vehicle| {
            let speeds: Vec<f64> = (0..24)
                .map(|seed| {
                    simulate(
                        vehicle,
                        &program,
                        FlightConfig {
                            seed,
                            ..FlightConfig::default()
                        },
                    )
                    .unwrap()
                    .max_speed_ms
                })
                .collect();
            let min = speeds.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = speeds.iter().cloned().fold(0.0, f64::max);
            let mean = speeds.iter().sum::<f64>() / speeds.len() as f64;
            (max - min) / mean
        };
        let loose = spread(&plain);
        let tight = spread(&finned);
        assert!(tight < loose, "fins should damp the spread: {tight} vs {loose}");
        assert!(tight > 0.0, "the seed still spreads the hardware");
        assert!(
            simulate(&finned, &program, FlightConfig::default())
                .unwrap()
                .aero
                .stability
                > 60.0
        );
    }

    #[test]
    fn wings_let_the_air_breathing_engine_run_at_a_lower_speed() {
        let program = Program::single(vec![Block::IgniteStage { stage: 0 }]);
        let config = FlightConfig {
            launch: LaunchMode::Air {
                speed_ms: 70.0,
                altitude_m: 100.0,
            },
            max_time_s: 30.0,
            ..FlightConfig::default()
        };
        let plain = cruise_vehicle();
        let mut winged = plain.clone();
        winged
            .add_attached(
                "wing-1",
                PartSpec::Wing {
                    span_m: 1.2,
                    chord_m: 0.5,
                    sweep_deg: 20.0,
                    planform: crate::vehicle::WingPlanform::Swept,
                    profile: crate::vehicle::AirfoilProfile::Classic,
                    light: false,
                },
                "tank",
                crate::assembly::NodeId::Radial(0),
            )
            .unwrap();
        assert!(winged.validate().is_ok(), "{:?}", winged.validate());
        let without = simulate(&plain, &program, config).unwrap();
        let with = simulate(&winged, &program, config).unwrap();
        assert!(with.aero.lift > 50.0, "lift {}", with.aero.lift);
        assert!(
            with.max_speed_ms > 80.0,
            "the winged vehicle accelerates: {}",
            with.max_speed_ms
        );
        assert!(
            without.max_speed_ms <= 70.0 + 1e-6,
            "the wingless turbofan cannot run at 70 m/s: {}",
            without.max_speed_ms
        );
        assert!(with.path_length_m > without.path_length_m);
    }

    #[test]
    fn a_fragile_airframe_can_fault_but_a_sound_one_never_does() {
        let program = Program::single(vec![Block::IgniteStage { stage: 0 }]);
        let mut sound = sounding_rocket();
        with_fins(&mut sound);
        let mut fragile = sounding_rocket();
        for index in 0..4u8 {
            fragile
                .add_attached(
                    format!("fin-{index}"),
                    PartSpec::FinSet {
                        span_m: 0.3,
                        chord_m: 0.35,
                        role: crate::vehicle::StabilizerRole::Rear,
                        layout: crate::vehicle::FinLayout::Asymmetric,
                        light: true,
                    },
                    "booster",
                    crate::assembly::NodeId::Radial(index),
                )
                .unwrap();
        }
        let config = |seed| FlightConfig {
            seed,
            max_time_s: 5.0,
            ..FlightConfig::default()
        };
        let faulted = |vehicle: &Vehicle, seed| {
            simulate(vehicle, &program, config(seed))
                .unwrap()
                .anomalies
                .iter()
                .any(|anomaly| anomaly.contains("surface faulted"))
        };
        assert!(
            !(0..256).any(|seed| faulted(&sound, seed)),
            "a sound airframe never faults"
        );
        let seeds: Vec<u64> = (0..256).filter(|seed| faulted(&fragile, *seed)).collect();
        assert!(!seeds.is_empty(), "the fragile airframe can draw a fault");
        assert!(
            seeds.len() < 100,
            "a fault stays an incident, not the norm: {}",
            seeds.len()
        );
        assert_eq!(
            faulted(&fragile, seeds[0]),
            faulted(&fragile, seeds[0]),
            "seeded and repeatable"
        );
    }

    #[test]
    fn control_surfaces_tighten_the_altitude_controller() {
        let program = Program::single(vec![
            Block::IgniteStage { stage: 0 },
            Block::Wait { seconds: 1.0 },
            Block::HoldPressure { hpa: 900.0 },
        ]);
        let config = FlightConfig {
            launch: LaunchMode::Air {
                speed_ms: 200.0,
                altitude_m: 0.0,
            },
            max_time_s: 60.0,
            ..FlightConfig::default()
        };
        let mut plain = cruise_vehicle();
        plain
            .add_attached(
                "baro",
                PartSpec::Sensor {
                    kind: SensorKind::Barometer,
                },
                "radar",
                crate::assembly::NodeId::Top,
            )
            .unwrap();
        let mut steered = plain.clone();
        with_surfaces(
            &mut steered,
            PartSpec::ControlSurface {
                span_m: 0.4,
                chord_m: 0.4,
                kind: crate::vehicle::ControlKind::Classic,
                light: false,
            },
        );
        assert!(steered.validate().is_ok(), "{:?}", steered.validate());
        let target = -8_000.0 * (900.0f64 / 1_013.25).ln();
        let settle = |vehicle: &Vehicle| {
            let report = simulate(vehicle, &program, config).unwrap();
            let first = report
                .samples
                .iter()
                .find(|sample| (sample.altitude_m - target).abs() < 15.0)
                .map(|sample| sample.t_s);
            (first, report.aero.control_authority)
        };
        let (slow, no_authority) = settle(&plain);
        let (fast, authority) = settle(&steered);
        assert!(authority > no_authority);
        assert!(fast.is_some(), "the steered design reaches the pressure target");
        assert!(
            fast.unwrap() < slow.unwrap(),
            "authority settles earlier: {fast:?} vs {slow:?}"
        );
    }
}
