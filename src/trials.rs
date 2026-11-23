//! Trial screen: run the headless flight model (`omega-rules::flight`) on the missile and show what
//! it produced. Everything here is fictional and abstract; see `docs/VISION_MISSILES.md`.
//!
//! The model is deterministic: the same vehicle, program and `FlightConfig` always give the same
//! report. The screen shows that input hash so a run can be replayed and compared.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use bevy::prelude::*;
use omega_data::hash_serializable;
use omega_rules::flight::{FlightConfig, FlightReport, LaunchMode, lock_roll, simulate};
use omega_rules::program::Block;
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
    fn new() -> Self {
        Self {
            runs: 0,
            no_liftoff: 0,
            no_separation: 0,
            anomalies: 0,
            lock_attempts: 0,
            lock_success: 0,
            mean_max_speed_ms: 0.0,
            mean_path_m: 0.0,
            mean_flight_time_s: 0.0,
        }
    }

    /// Folds one report into the running totals so a batch never keeps the reports alive.
    fn accumulate(&mut self, report: &FlightReport) {
        self.runs += 1;
        let runs = self.runs as f64;
        if !report.lifted_off {
            self.no_liftoff += 1;
        }
        if !report.events.iter().any(|(_, label)| label.contains("separated")) {
            self.no_separation += 1;
        }
        if !report.anomalies.is_empty() {
            self.anomalies += 1;
        }
        if let Some(lock) = report.lock {
            self.lock_attempts += 1;
            if lock.success {
                self.lock_success += 1;
            }
        }
        self.mean_max_speed_ms += (report.max_speed_ms - self.mean_max_speed_ms) / runs;
        self.mean_path_m += (report.path_length_m - self.mean_path_m) / runs;
        self.mean_flight_time_s += (report.flight_time_s - self.mean_flight_time_s) / runs;
    }

    pub fn lock_rate(&self) -> Option<f64> {
        (self.lock_attempts > 0).then(|| self.lock_success as f64 / self.lock_attempts as f64)
    }
}

/// A seeded batch running on a worker thread; the frame only polls it.
pub(crate) struct BatchJob {
    pub total: usize,
    done: Arc<AtomicUsize>,
    outcome: Arc<Mutex<Option<Result<BatchSummary, String>>>>,
}

/// What the seeded target-lock roll gives for the current vehicle and seed.
pub(crate) struct LockProbe {
    pub roll: f64,
    pub probability: f64,
    pub max_speed_ms: f64,
    pub success: bool,
}

/// `None` when there is no lock block to run or no radar to run it with.
pub(crate) fn lock_probe(vehicle: &Vehicle, program: &Program, seed: u64) -> Option<LockProbe> {
    let wants_lock = program
        .threads
        .iter()
        .flatten()
        .any(|block| matches!(block, Block::LockTarget));
    if !wants_lock {
        return None;
    }
    let radar = vehicle.best_radar(0)?;
    let roll = lock_roll(seed);
    Some(LockProbe {
        roll,
        probability: radar.base_lock_probability,
        max_speed_ms: radar.max_lock_speed_ms,
        success: roll < radar.base_lock_probability,
    })
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
    /// Seed batch currently flying on a worker thread.
    pub batch_job: Option<BatchJob>,
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
            batch_job: None,
            revision: 0,
        }
    }
}

impl TrialSession {
    /// Takes the finished batch, if the worker has delivered one.
    pub(crate) fn take_batch_outcome(&mut self) -> Option<Result<BatchSummary, String>> {
        let outcome = self.batch_job.as_ref()?.outcome.lock().ok()?.take()?;
        self.batch_job = None;
        self.revision += 1;
        Some(outcome)
    }

    /// Seeds flown and seeds requested by the running batch.
    pub(crate) fn batch_progress(&self) -> Option<(usize, usize)> {
        self.batch_job
            .as_ref()
            .map(|job| (job.done.load(Ordering::Relaxed), job.total))
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

/// Flies `runs` seeds one after the other and keeps only the running totals.
fn run_batch(
    vehicle: &Vehicle,
    program: &Program,
    base: FlightConfig,
    runs: usize,
    progress: &AtomicUsize,
) -> Result<BatchSummary, String> {
    let mut summary = BatchSummary::new();
    for step in 0..runs {
        let config = FlightConfig {
            seed: base.seed.saturating_add(step as u64),
            ..base
        };
        let report = simulate(vehicle, program, config).map_err(|issues| issues.join("; "))?;
        summary.accumulate(&report);
        progress.store(step + 1, Ordering::Relaxed);
    }
    Ok(summary)
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

/// Hands the batch to a worker thread: a thousand seeds never block a frame.
pub(crate) fn start_batch(
    session: &mut TrialSession,
    vehicle: &Vehicle,
    program: &Program,
    runs: usize,
    monte_carlo: bool,
) -> Result<String, String> {
    gate(monte_carlo)?;
    if session.batch_job.is_some() {
        return Err("Batch already running.".to_string());
    }
    let vehicle = vehicle.clone();
    let program = program.clone();
    let base = config(session);
    let done = Arc::new(AtomicUsize::new(0));
    let outcome = Arc::new(Mutex::new(None));
    let worker_done = Arc::clone(&done);
    let worker_outcome = Arc::clone(&outcome);
    thread::spawn(move || {
        let result = run_batch(&vehicle, &program, base, runs, &worker_done);
        if let Ok(mut slot) = worker_outcome.lock() {
            *slot = Some(result);
        }
    });
    session.batch_job = Some(BatchJob {
        total: runs,
        done,
        outcome,
    });
    session.revision += 1;
    Ok("Batch started".to_string())
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
    let base = config(session);
    let progress = AtomicUsize::new(0);
    let left = run_batch(&baseline.vehicle, &baseline.program, base, COMPARE_RUNS, &progress)?;
    let right = run_batch(vehicle, program, base, COMPARE_RUNS, &progress)?;
    let config = config(session);
    session.comparison = Some(Comparison {
        baseline: left,
        current: right,
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
        TrialAction::Batch(runs) => start_batch(session, vehicle, program, runs, monte_carlo),
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

    fn wait_for_batch(session: &mut TrialSession) -> BatchSummary {
        for _ in 0..1_000 {
            if let Some(outcome) = session.take_batch_outcome() {
                return outcome.unwrap();
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("the batch never reported back");
    }

    #[test]
    fn batches_cover_every_seed_and_count_outcomes() {
        let summary = run_batch(
            &rocket(),
            &two_stage(20.0),
            config(&TrialSession::default()),
            10,
            &AtomicUsize::new(0),
        )
        .unwrap();
        assert_eq!(summary.runs, 10);
        assert_eq!(summary.no_liftoff, 0);
        assert_eq!(summary.no_separation, 0);
        assert!(summary.mean_path_m > 1_000.0, "{:?}", summary);
    }

    #[test]
    fn a_batch_runs_off_the_frame_and_reports_back() {
        let mut session = TrialSession::default();
        let message = start_batch(&mut session, &rocket(), &two_stage(20.0), 60, true).unwrap();
        assert!(message.contains("Batch"), "{message}");
        assert!(session.batch_job.is_some());
        let error = start_batch(&mut session, &rocket(), &two_stage(20.0), 10, true).unwrap_err();
        assert!(error.contains("Batch"), "{error}");
        let summary = wait_for_batch(&mut session);
        assert_eq!(summary.runs, 60);
        assert!(session.batch_job.is_none());
        assert_eq!(session.batch_progress(), None);
        start_batch(&mut session, &rocket(), &two_stage(20.0), 10, true).unwrap();
        assert!(session.batch_progress().is_some());
        wait_for_batch(&mut session);
    }

    #[test]
    fn batches_are_refused_without_the_grant() {
        let mut session = TrialSession::default();
        let error = start_batch(&mut session, &rocket(), &two_stage(20.0), 10, false).unwrap_err();
        assert!(error.contains("grant"), "{error}");
        assert!(session.batch_job.is_none());
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
    fn the_seed_roll_is_visible_and_flips_the_lock() {
        let mut vehicle = rocket();
        vehicle.stages[1].parts.push(part(
            "radar",
            PartSpec::Sensor {
                kind: omega_rules::SensorKind::Radar(omega_rules::RadarClass::Basic),
            },
        ));
        let program = Program::single(vec![Block::IgniteStage { stage: 0 }, Block::LockTarget]);
        let first = lock_probe(&vehicle, &program, 1).unwrap();
        assert_eq!(first.roll, lock_roll(1));
        assert_eq!(first.success, first.roll < first.probability);
        let flipped = (2..200u64)
            .map(|seed| lock_probe(&vehicle, &program, seed).unwrap())
            .find(|probe| probe.success != first.success)
            .expect("no seed in the first 200 flips the lock");
        assert_ne!(flipped.success, first.success);
    }

    #[test]
    fn the_seed_probe_needs_a_lock_block_and_a_radar() {
        assert!(lock_probe(&rocket(), &two_stage(20.0), 1).is_none());
        let lock_program = Program::single(vec![Block::LockTarget]);
        assert!(lock_probe(&rocket(), &lock_program, 1).is_none());
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
