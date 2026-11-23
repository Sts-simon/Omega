//! Trial screen: run the headless flight model (`omega-rules::flight`) on the missile and show what
//! it produced. Everything here is fictional and abstract; see `docs/VISION_MISSILES.md`.
//!
//! The model is deterministic: the same vehicle, program and `FlightConfig` always give the same
//! report. The screen shows that input hash so a run can be replayed and compared.

use bevy::prelude::*;
use omega_data::hash_serializable;
use omega_rules::flight::{FlightConfig, FlightReport, LaunchMode, simulate};
use omega_rules::{Program, Vehicle};

/// Capability that unlocks seeded batches and A/B comparisons.
pub(crate) const MONTE_CARLO: &str = "analysis:monte_carlo";
pub(crate) const BATCH_SIZES: [usize; 3] = [10, 100, 1_000];
const COMPARE_RUNS: usize = 10;
const AIR_SPEED_MS: f64 = 200.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TrialAction {
    SeedStep(i64),
    ToggleLaunch,
    Run,
    Batch(usize),
    Snapshot,
    Compare,
}

/// One completed trial and the inputs that produced it.
pub(crate) struct TrialRun {
    pub report: FlightReport,
    pub seed: u64,
    pub air_launch: bool,
    pub input_hash: String,
}

/// Outcome counts and means over a set of seeds.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BatchSummary {
    pub runs: usize,
    pub no_liftoff: usize,
    pub no_separation: usize,
    pub anomalies: usize,
    pub lock_attempts: usize,
    pub lock_success: usize,
    pub mean_max_speed_ms: f64,
    pub mean_path_m: f64,
    pub mean_flight_time_s: f64,
}

impl BatchSummary {
    fn from_reports(reports: &[FlightReport]) -> Self {
        let runs = reports.len().max(1) as f64;
        let count = |predicate: &dyn Fn(&FlightReport) -> bool| reports.iter().filter(|r| predicate(r)).count();
        Self {
            runs: reports.len(),
            no_liftoff: count(&|r| !r.lifted_off),
            no_separation: count(&|r| !r.events.iter().any(|(_, label)| label.contains("separated"))),
            anomalies: count(&|r| !r.anomalies.is_empty()),
            lock_attempts: count(&|r| r.lock.is_some()),
            lock_success: count(&|r| r.lock.is_some_and(|l| l.success)),
            mean_max_speed_ms: reports.iter().map(|r| r.max_speed_ms).sum::<f64>() / runs,
            mean_path_m: reports.iter().map(|r| r.path_length_m).sum::<f64>() / runs,
            mean_flight_time_s: reports.iter().map(|r| r.flight_time_s).sum::<f64>() / runs,
        }
    }

    pub fn lock_rate(&self) -> Option<f64> {
        (self.lock_attempts > 0).then(|| self.lock_success as f64 / self.lock_attempts as f64)
    }
}

pub(crate) struct Baseline {
    pub vehicle: Vehicle,
    pub program: Program,
    pub hash: String,
}

/// Same seed set flown by the snapshot design and by the current one.
pub(crate) struct Comparison {
    pub baseline: BatchSummary,
    pub current: BatchSummary,
    pub baseline_hash: String,
    pub current_hash: String,
}

#[derive(Resource)]
pub(crate) struct TrialSession {
    pub seed: u64,
    pub air_launch: bool,
    pub report: Option<TrialRun>,
    pub batch: Option<BatchSummary>,
    pub baseline: Option<Baseline>,
    pub comparison: Option<Comparison>,
    /// Bumped on every change so the board knows it has to be rebuilt.
    pub revision: u64,
}

impl Default for TrialSession {
    fn default() -> Self {
        Self {
            seed: 1,
            air_launch: false,
            report: None,
            batch: None,
            baseline: None,
            comparison: None,
            revision: 0,
        }
    }
}

pub(crate) fn config(session: &TrialSession) -> FlightConfig {
    FlightConfig {
        launch: if session.air_launch {
            LaunchMode::Air {
                speed_ms: AIR_SPEED_MS,
                altitude_m: 0.0,
            }
        } else {
            LaunchMode::Vertical
        },
        seed: session.seed,
        ..FlightConfig::default()
    }
}

/// Hash of everything a run depends on, so a report can be replayed.
pub(crate) fn input_hash(vehicle: &Vehicle, program: &Program, config: &FlightConfig) -> String {
    let seed = config.seed;
    let launch = match config.launch {
        LaunchMode::Vertical => format!("vertical|{seed}"),
        LaunchMode::Air { speed_ms, altitude_m } => format!("air|{speed_ms}|{altitude_m}|{seed}"),
    };
    hash_serializable(&(
        vehicle,
        program,
        config.dt_s.to_bits(),
        config.max_time_s.to_bits(),
        launch,
    ))
    .map(|hash| hash.as_str().to_string())
    .unwrap_or_default()
}

fn simulate_batch(
    vehicle: &Vehicle,
    program: &Program,
    session: &TrialSession,
    runs: usize,
) -> Result<Vec<FlightReport>, String> {
    let mut reports = Vec::with_capacity(runs);
    for step in 0..runs {
        let config = FlightConfig {
            seed: session.seed.saturating_add(step as u64),
            ..config(session)
        };
        reports.push(simulate(vehicle, program, config).map_err(|issues| issues.join("; "))?);
    }
    Ok(reports)
}

fn gate(monte_carlo: bool) -> Result<(), String> {
    if monte_carlo {
        Ok(())
    } else {
        Err("Research grant required.".to_string())
    }
}

pub(crate) fn run(session: &mut TrialSession, vehicle: &Vehicle, program: &Program) -> Result<String, String> {
    let config = config(session);
    let report = simulate(vehicle, program, config).map_err(|issues| issues.join("; "))?;
    let hash = input_hash(vehicle, program, &config);
    session.report = Some(TrialRun {
        report,
        seed: config.seed,
        air_launch: session.air_launch,
        input_hash: hash,
    });
    session.revision += 1;
    Ok("Trial done".to_string())
}

pub(crate) fn batch(
    session: &mut TrialSession,
    vehicle: &Vehicle,
    program: &Program,
    runs: usize,
    monte_carlo: bool,
) -> Result<String, String> {
    gate(monte_carlo)?;
    let reports = simulate_batch(vehicle, program, session, runs)?;
    let summary = BatchSummary::from_reports(&reports);
    session.batch = Some(summary);
    session.revision += 1;
    Ok("Batch done".to_string())
}

pub(crate) fn snapshot(session: &mut TrialSession, vehicle: &Vehicle, program: &Program) -> Result<String, String> {
    let config = config(session);
    let hash = input_hash(vehicle, program, &config);
    session.baseline = Some(Baseline {
        vehicle: vehicle.clone(),
        program: program.clone(),
        hash: hash[..8.min(hash.len())].to_string(),
    });
    session.revision += 1;
    Ok("A stored".to_string())
}

pub(crate) fn compare(
    session: &mut TrialSession,
    vehicle: &Vehicle,
    program: &Program,
    monte_carlo: bool,
) -> Result<String, String> {
    gate(monte_carlo)?;
    let baseline = session
        .baseline
        .as_ref()
        .ok_or_else(|| "Snapshot A first".to_string())?;
    let left = simulate_batch(&baseline.vehicle, &baseline.program, session, COMPARE_RUNS)?;
    let right = simulate_batch(vehicle, program, session, COMPARE_RUNS)?;
    let config = config(session);
    session.comparison = Some(Comparison {
        baseline: BatchSummary::from_reports(&left),
        current: BatchSummary::from_reports(&right),
        baseline_hash: baseline.hash.clone(),
        current_hash: input_hash(vehicle, program, &config),
    });
    session.revision += 1;
    Ok("A/B compared".to_string())
}

/// Route one UI action; the returned message is what the message lane shows.
pub(crate) fn apply_action(
    session: &mut TrialSession,
    action: TrialAction,
    vehicle: &Vehicle,
    program: &Program,
    monte_carlo: bool,
) -> Result<String, String> {
    match action {
        TrialAction::SeedStep(delta) => {
            let seed = i128::from(session.seed) + i128::from(delta);
            session.seed = seed.clamp(0, i128::from(u64::MAX)) as u64;
            session.revision += 1;
            Ok(format!("Seed {}", session.seed))
        }
        TrialAction::ToggleLaunch => {
            session.air_launch = !session.air_launch;
            session.revision += 1;
            Ok(if session.air_launch {
                "Air launch".to_string()
            } else {
                "Vertical launch".to_string()
            })
        }
        TrialAction::Run => run(session, vehicle, program),
        TrialAction::Batch(runs) => batch(session, vehicle, program, runs, monte_carlo),
        TrialAction::Snapshot => snapshot(session, vehicle, program),
        TrialAction::Compare => compare(session, vehicle, program, monte_carlo),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_rules::{Block, ComputerClass, Part, PartSpec, Stage};

    fn part(id: &str, spec: PartSpec) -> Part {
        Part { id: id.into(), spec }
    }

    fn rocket() -> Vehicle {
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
                    ],
                },
            ],
        }
    }

    fn two_stage(separate_at: f64) -> Program {
        Program::single(vec![
            Block::IgniteStage { stage: 0 },
            Block::Wait { seconds: separate_at },
            Block::SeparateStage { stage: 0 },
            Block::IgniteStage { stage: 1 },
        ])
    }

    #[test]
    fn a_trial_is_reproducible_and_carries_the_same_hash() {
        let mut session = TrialSession::default();
        run(&mut session, &rocket(), &two_stage(20.0)).unwrap();
        let first = session.report.as_ref().unwrap();
        let (report, hash) = (first.report.clone(), first.input_hash.clone());
        run(&mut session, &rocket(), &two_stage(20.0)).unwrap();
        let second = session.report.as_ref().unwrap();
        assert_eq!(report, second.report);
        assert_eq!(hash, second.input_hash);
        assert_eq!(hash.len(), 16);
    }

    #[test]
    fn changing_the_seed_changes_the_hash() {
        let vehicle = rocket();
        let program = two_stage(20.0);
        let mut session = TrialSession::default();
        run(&mut session, &vehicle, &program).unwrap();
        let first = session.report.as_ref().unwrap().input_hash.clone();
        session.seed = 2;
        run(&mut session, &vehicle, &program).unwrap();
        assert_ne!(first, session.report.as_ref().unwrap().input_hash);
    }

    #[test]
    fn batches_cover_every_seed_and_count_outcomes() {
        let mut session = TrialSession::default();
        let message = batch(&mut session, &rocket(), &two_stage(20.0), 10, true).unwrap();
        let summary = session.batch.as_ref().unwrap();
        assert_eq!(summary.runs, 10);
        assert_eq!(summary.no_liftoff, 0);
        assert_eq!(summary.no_separation, 0);
        assert!(summary.mean_path_m > 1_000.0, "{:?}", summary);
        assert!(message.contains("Batch"), "{message}");
    }

    #[test]
    fn batches_are_refused_without_the_grant() {
        let mut session = TrialSession::default();
        let error = batch(&mut session, &rocket(), &two_stage(20.0), 10, false).unwrap_err();
        assert!(error.contains("grant"), "{error}");
        assert!(session.batch.is_none());
        assert_eq!(session.revision, 0);
    }

    #[test]
    fn comparison_uses_the_same_seeds_and_reports_a_shorter_early_separation() {
        let vehicle = rocket();
        let mut session = TrialSession::default();
        snapshot(&mut session, &vehicle, &two_stage(20.0)).unwrap();
        let message = compare(&mut session, &vehicle, &two_stage(8.0), true).unwrap();
        assert!(message.contains("A/B"), "{message}");
        let comparison = session.comparison.as_ref().unwrap();
        assert_eq!(comparison.baseline.runs, comparison.current.runs);
        assert!(comparison.current.mean_path_m < comparison.baseline.mean_path_m);
    }

    #[test]
    fn comparison_needs_a_snapshot_and_the_grant() {
        let mut session = TrialSession::default();
        let error = compare(&mut session, &rocket(), &two_stage(20.0), true).unwrap_err();
        assert!(error.contains("Snapshot"), "{error}");
        snapshot(&mut session, &rocket(), &two_stage(20.0)).unwrap();
        let error = compare(&mut session, &rocket(), &two_stage(20.0), false).unwrap_err();
        assert!(error.contains("grant"), "{error}");
    }

    #[test]
    fn an_invalid_program_is_reported_instead_of_run() {
        let mut session = TrialSession::default();
        let error = run(&mut session, &rocket(), &Program::single(vec![Block::LockTarget])).unwrap_err();
        assert!(error.to_lowercase().contains("lock target"), "{error}");
        assert!(session.report.is_none());
    }

    #[test]
    fn seed_steps_never_go_negative_and_air_launch_toggles() {
        let mut session = TrialSession::default();
        apply_action(
            &mut session,
            TrialAction::SeedStep(-5),
            &rocket(),
            &two_stage(20.0),
            true,
        )
        .unwrap();
        assert_eq!(session.seed, 0);
        apply_action(
            &mut session,
            TrialAction::ToggleLaunch,
            &rocket(),
            &two_stage(20.0),
            true,
        )
        .unwrap();
        assert!(session.air_launch);
        assert!(matches!(config(&session).launch, LaunchMode::Air { .. }));
    }
}
