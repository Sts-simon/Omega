//! Trial screen: run the headless flight model (`omega-rules::flight`) on the missile and show what
//! it produced. Everything here is fictional and abstract; see `docs/VISION_MISSILES.md`.
//!
//! The model is deterministic: the same vehicle, program and `FlightConfig` always give the same
//! report. The screen shows that input hash so a run can be replayed and compared.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::lab::{
    self, AnomalyInsight, BatchEntryInput, EntryInput, PhaseSummary, TrialContext, TrialHistory, TrialHistoryEntry,
    TrialStats,
};
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TrialAction {
    SeedStep(i64),
    ToggleLaunch,
    Run,
    Batch(usize),
    Snapshot,
    Compare,
    SetTab(TrialTab),
    SelectHistory(u64),
    Relaunch(u64),
    ExportHtml,
    ExportCsv,
    ExportPdf,
    InspectPart(String),
    InspectBlock { thread: usize, block: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TrialTab {
    Dashboard,
    Compare,
    History,
}

/// Validation errors that stopped a run before it could produce a report.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TrialFailure {
    pub message: String,
    pub anomalies: Vec<AnomalyInsight>,
    pub vehicle_name: String,
    pub seed: u64,
    pub air_launch: bool,
}

/// One completed trial and the inputs that produced it.
pub(crate) struct TrialRun {
    pub report: FlightReport,
    pub seed: u64,
    pub air_launch: bool,
    pub input_hash: String,
    /// Canonical hashes of the exact vehicle and program that flew, for contract evidence.
    pub vehicle_hash: String,
    pub program_hash: String,
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
    /// Per-phase means over every report in the batch, in phase order.
    pub phases: Vec<PhaseSummary>,
    /// The same outcome counters and extremes as [`TrialStats`], ready for history and exports.
    pub stats: TrialStats,
    /// Distinct structured anomalies seen at least once in the batch.
    pub anomaly_insights: Vec<AnomalyInsight>,
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
            phases: Vec::new(),
            stats: TrialStats::default(),
            anomaly_insights: Vec::new(),
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
        TrialStats::accumulate(report, &mut self.stats);
        for phase in lab::phases(report) {
            if let Some(existing) = self.phases.iter_mut().find(|entry| entry.name == phase.name) {
                existing.merge(&phase);
            } else {
                self.phases.push(phase);
            }
        }
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
    /// Inputs the batch was started with, kept so its result can be marked stale later.
    input_hash: String,
    vehicle: Vehicle,
    program: Program,
    seed: u64,
    air_launch: bool,
    context: TrialContext,
    relaunched_from: Option<u64>,
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
    /// Vehicle, program, profile and first seed the stored batch was flown with.
    pub batch_hash: Option<String>,
    pub baseline: Option<Baseline>,
    pub comparison: Option<Comparison>,
    /// Seed batch currently flying on a worker thread.
    pub batch_job: Option<BatchJob>,
    /// Bumped on every change so the board knows it has to be rebuilt.
    pub revision: u64,
    /// Completed single runs, so an identical rerun still counts as new evidence.
    pub runs: u64,
    /// Current laboratory page.
    pub tab: TrialTab,
    /// Last failed validation, kept so the dashboard explains why no report exists.
    pub failure: Option<TrialFailure>,
    /// Persisted trial campaigns for the active slot.
    pub history: TrialHistory,
    pub history_dirty: bool,
    /// Campaign selected in the history tab.
    pub selected_history: Option<u64>,
    /// Revision, program version and change summary captured before a run.
    pub context: TrialContext,
}

impl Default for TrialSession {
    fn default() -> Self {
        Self {
            seed: 1,
            air_launch: false,
            report: None,
            batch: None,
            batch_hash: None,
            baseline: None,
            comparison: None,
            batch_job: None,
            revision: 0,
            runs: 0,
            tab: TrialTab::Dashboard,
            failure: None,
            history: TrialHistory::default(),
            history_dirty: false,
            selected_history: None,
            context: TrialContext::default(),
        }
    }
}

impl TrialSession {
    pub(crate) fn set_context(&mut self, context: TrialContext) {
        if self.context != context {
            self.context = context;
            self.revision += 1;
        }
    }

    pub(crate) fn reset_new(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn load_active(&mut self) {
        *self = Self::default();
        match crate::save::read(crate::save::SaveFile::TrialHistory) {
            Ok(Some(bytes)) => match TrialHistory::from_bytes(&bytes) {
                Ok(history) => self.history = history,
                Err(error) => warn!("Could not read the trial history: {error}"),
            },
            Ok(None) => {}
            Err(error) => warn!("Could not read the trial history: {error}"),
        }
        self.selected_history = self.history.entries().last().map(|entry| entry.id);
    }

    pub(crate) fn persist_history(&mut self) -> Result<(), String> {
        if !self.history_dirty || crate::save::active_slot().is_none() {
            return Ok(());
        }
        self.history
            .to_bytes()
            .and_then(|bytes| crate::save::write(crate::save::SaveFile::TrialHistory, &bytes))?;
        self.history_dirty = false;
        Ok(())
    }

    fn record(&mut self, entry: TrialHistoryEntry, relaunched_from: Option<u64>) -> u64 {
        let mut entry = entry;
        entry.relaunched_from = relaunched_from;
        let id = self.history.push(entry);
        self.history_dirty = true;
        self.selected_history = Some(id);
        id
    }

    /// Takes the finished batch, if the worker has delivered one, and remembers its inputs.
    pub(crate) fn take_batch_outcome(&mut self) -> Option<Result<BatchSummary, String>> {
        let outcome = self.batch_job.as_ref()?.outcome.lock().ok()?.take()?;
        let job = self.batch_job.take();
        if outcome.is_ok()
            && let Some(job) = job
        {
            self.batch_hash = Some(job.input_hash.clone());
            if let Ok(summary) = &outcome {
                let entry = lab::entry_from_batch(
                    BatchEntryInput {
                        context: &job.context,
                        vehicle: &job.vehicle,
                        program: &job.program,
                        seed: job.seed,
                        air_launch: job.air_launch,
                        runs: job.total,
                        input_hash: job.input_hash.clone(),
                        stats: &summary.stats,
                        phases: &summary.phases,
                        anomalies: &summary.anomaly_insights,
                    },
                    job.relaunched_from,
                );
                self.record(entry, job.relaunched_from);
            }
            self.failure = None;
        } else if let Err(message) = &outcome
            && let Some(job) = job
        {
            self.failure = Some(TrialFailure {
                message: message.clone(),
                anomalies: lab::analyze_failure(&job.vehicle, &job.program, std::slice::from_ref(message)),
                vehicle_name: job.vehicle.name.clone(),
                seed: job.seed,
                air_launch: job.air_launch,
            });
        }
        self.revision += 1;
        Some(outcome)
    }

    /// Whether the stored batch was flown with a different vehicle, program, profile or seed.
    pub(crate) fn batch_stale(&self, vehicle: &Vehicle, program: &Program) -> bool {
        if self.batch.is_none() {
            return false;
        }
        let current = input_hash(vehicle, program, &config(self));
        self.batch_hash.as_deref() != Some(current.as_str())
    }

    /// Whether the displayed single run no longer matches the current design, program, profile
    /// or seed. A history entry stays valid for its own inputs; this check is for the live result.
    pub(crate) fn report_stale(&self, vehicle: &Vehicle, program: &Program) -> bool {
        self.report
            .as_ref()
            .is_some_and(|run| run.input_hash != input_hash(vehicle, program, &config(self)))
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
    let semantic_program = program.semantic_copy();
    let seed = config.seed;
    let launch = match config.launch {
        LaunchMode::Vertical => format!("vertical|{seed}"),
        LaunchMode::Air { speed_ms, altitude_m } => format!("air|{speed_ms}|{altitude_m}|{seed}"),
    };
    hash_serializable(&(
        vehicle,
        &semantic_program,
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
        for anomaly in lab::analyze_report_anomalies(&report, vehicle, program) {
            if !summary
                .anomaly_insights
                .iter()
                .any(|known| known.message == anomaly.message)
            {
                summary.anomaly_insights.push(anomaly);
            }
        }
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
    let report = match simulate(vehicle, program, config) {
        Ok(report) => report,
        Err(issues) => {
            let message = issues.join("; ");
            session.report = None;
            session.failure = Some(TrialFailure {
                message: message.clone(),
                anomalies: lab::analyze_failure(vehicle, program, &issues),
                vehicle_name: vehicle.name.clone(),
                seed: config.seed,
                air_launch: session.air_launch,
            });
            session.revision += 1;
            return Err(message);
        }
    };
    let hash = input_hash(vehicle, program, &config);
    let vehicle_hash = hash_serializable(vehicle)
        .map(|hash| hash.as_str().to_string())
        .unwrap_or_default();
    let program_hash = hash_serializable(&program.semantic_copy())
        .map(|hash| hash.as_str().to_string())
        .unwrap_or_default();
    let entry = lab::entry_from_report(
        lab::EntryInput {
            context: &session.context,
            vehicle,
            program,
            seed: config.seed,
            air_launch: session.air_launch,
            report: &report,
            input_hash: hash.clone(),
        },
        None,
    );
    session.record(entry, None);
    session.failure = None;
    session.report = Some(TrialRun {
        report,
        seed: config.seed,
        air_launch: session.air_launch,
        input_hash: hash,
        vehicle_hash,
        program_hash,
    });
    session.runs += 1;
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
    let context = session.context.clone();
    start_batch_with(
        session,
        vehicle,
        program,
        config(session),
        runs,
        monte_carlo,
        context,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn start_batch_with(
    session: &mut TrialSession,
    vehicle: &Vehicle,
    program: &Program,
    base: FlightConfig,
    runs: usize,
    monte_carlo: bool,
    context: TrialContext,
    relaunched_from: Option<u64>,
) -> Result<String, String> {
    gate(monte_carlo)?;
    if session.batch_job.is_some() {
        return Err("Batch already running.".to_string());
    }
    let vehicle = vehicle.clone();
    let program = program.clone();
    let worker_vehicle = vehicle.clone();
    let worker_program = program.clone();
    let input_hash = input_hash(&vehicle, &program, &base);
    let done = Arc::new(AtomicUsize::new(0));
    let outcome = Arc::new(Mutex::new(None));
    let worker_done = Arc::clone(&done);
    let worker_outcome = Arc::clone(&outcome);
    thread::spawn(move || {
        let result = run_batch(&worker_vehicle, &worker_program, base, runs, &worker_done);
        if let Ok(mut slot) = worker_outcome.lock() {
            *slot = Some(result);
        }
    });
    session.batch_job = Some(BatchJob {
        total: runs,
        done,
        outcome,
        input_hash,
        vehicle,
        program,
        seed: base.seed,
        air_launch: matches!(base.launch, LaunchMode::Air { .. }),
        context,
        relaunched_from,
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
    let left = match run_batch(&baseline.vehicle, &baseline.program, base, COMPARE_RUNS, &progress) {
        Ok(summary) => summary,
        Err(error) => {
            session.failure = Some(TrialFailure {
                message: error.clone(),
                anomalies: lab::analyze_failure(&baseline.vehicle, &baseline.program, std::slice::from_ref(&error)),
                vehicle_name: baseline.vehicle.name.clone(),
                seed: base.seed,
                air_launch: session.air_launch,
            });
            session.revision += 1;
            return Err(error);
        }
    };
    let right = match run_batch(vehicle, program, base, COMPARE_RUNS, &progress) {
        Ok(summary) => summary,
        Err(error) => {
            session.failure = Some(TrialFailure {
                message: error.clone(),
                anomalies: lab::analyze_failure(vehicle, program, std::slice::from_ref(&error)),
                vehicle_name: vehicle.name.clone(),
                seed: base.seed,
                air_launch: session.air_launch,
            });
            session.revision += 1;
            return Err(error);
        }
    };
    session.failure = None;
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

/// Replay a history campaign exactly: same vehicle, program, seed range and flight profile.
pub(crate) fn relaunch(session: &mut TrialSession, id: u64, monte_carlo: bool) -> Result<String, String> {
    let entry = session
        .history
        .get(id)
        .cloned()
        .ok_or_else(|| format!("Campaign #{id} is not in the history"))?;
    session.seed = entry.seed;
    session.air_launch = entry.air_launch;
    if entry.runs > 1 {
        let config = lab::entry_config(&entry);
        return start_batch_with(
            session,
            &entry.vehicle,
            &entry.program,
            config,
            entry.runs,
            monte_carlo,
            entry.context(),
            Some(entry.id),
        )
        .map(|_| format!("Campaign #{} replayed ({} seeds)", entry.id, entry.runs));
    }
    let config = lab::entry_config(&entry);
    let report = match simulate(&entry.vehicle, &entry.program, config) {
        Ok(report) => report,
        Err(issues) => {
            let message = issues.join("; ");
            session.report = None;
            session.failure = Some(TrialFailure {
                message: message.clone(),
                anomalies: lab::analyze_failure(&entry.vehicle, &entry.program, &issues),
                vehicle_name: entry.vehicle.name.clone(),
                seed: entry.seed,
                air_launch: entry.air_launch,
            });
            session.revision += 1;
            return Err(message);
        }
    };
    let hash = input_hash(&entry.vehicle, &entry.program, &config);
    let vehicle_hash = hash_serializable(&entry.vehicle)
        .map(|hash| hash.as_str().to_string())
        .unwrap_or_default();
    let program_hash = hash_serializable(&entry.program.semantic_copy())
        .map(|hash| hash.as_str().to_string())
        .unwrap_or_default();
    let context = entry.context();
    let new_entry = lab::entry_from_report(
        EntryInput {
            context: &context,
            vehicle: &entry.vehicle,
            program: &entry.program,
            seed: entry.seed,
            air_launch: entry.air_launch,
            report: &report,
            input_hash: hash.clone(),
        },
        Some(entry.id),
    );
    session.record(new_entry, Some(entry.id));
    session.failure = None;
    session.report = Some(TrialRun {
        report,
        seed: entry.seed,
        air_launch: entry.air_launch,
        input_hash: hash,
        vehicle_hash,
        program_hash,
    });
    session.runs += 1;
    session.revision += 1;
    Ok(format!("Campaign #{} replayed", entry.id))
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
        TrialAction::SetTab(tab) => {
            session.tab = tab;
            session.revision += 1;
            Ok(String::new())
        }
        TrialAction::SelectHistory(id) => {
            if session.history.get(id).is_none() {
                return Err(format!("Campaign #{id} is not in the history"));
            }
            session.selected_history = Some(id);
            session.revision += 1;
            Ok(String::new())
        }
        TrialAction::Relaunch(id) => relaunch(session, id, monte_carlo),
        // Export and inspector actions need the campaign, revisions and workshop resources; the
        // board dispatches them after this match.
        TrialAction::ExportHtml
        | TrialAction::ExportCsv
        | TrialAction::ExportPdf
        | TrialAction::InspectPart(_)
        | TrialAction::InspectBlock { .. } => Ok(String::new()),
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
        Vehicle::from_stages(
            "Test rocket",
            vec![
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
        )
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
        assert_eq!(session.runs, 2, "an identical rerun is still a new run");
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
    fn a_live_report_goes_stale_when_its_inputs_change() {
        let vehicle = rocket();
        let program = two_stage(20.0);
        let mut session = TrialSession::default();
        run(&mut session, &vehicle, &program).unwrap();
        assert!(!session.report_stale(&vehicle, &program));
        session.seed = 2;
        assert!(session.report_stale(&vehicle, &program), "seed changed");
        session.seed = 1;
        assert!(!session.report_stale(&vehicle, &program));
        session.air_launch = true;
        assert!(session.report_stale(&vehicle, &program), "profile changed");
        session.air_launch = false;
        assert!(session.report_stale(&vehicle, &two_stage(8.0)), "program changed");
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
        assert!(session.batch_hash.is_some());
        assert_eq!(session.batch_progress(), None);
        start_batch(&mut session, &rocket(), &two_stage(20.0), 10, true).unwrap();
        assert!(session.batch_progress().is_some());
        wait_for_batch(&mut session);
    }

    #[test]
    fn a_finished_batch_goes_stale_when_its_inputs_change() {
        let vehicle = rocket();
        let program = two_stage(20.0);
        let mut session = TrialSession::default();
        assert!(!session.batch_stale(&vehicle, &program));

        start_batch(&mut session, &vehicle, &program, 10, true).unwrap();
        session.batch = Some(wait_for_batch(&mut session));
        assert!(session.batch_hash.is_some());
        assert!(
            !session.batch_stale(&vehicle, &program),
            "the inputs it flew are current"
        );

        session.seed = 2;
        assert!(session.batch_stale(&vehicle, &program));
        session.seed = 1;
        assert!(!session.batch_stale(&vehicle, &program), "back to the flown seed");

        assert!(session.batch_stale(&vehicle, &two_stage(8.0)), "program changed");

        let mut edited = rocket();
        edited.replace_part(
            "booster",
            PartSpec::SolidMotor {
                length_m: 3.0,
                diameter_m: 0.4,
                burn_time_s: 20.0,
            },
        );
        assert!(session.batch_stale(&edited, &program), "vehicle changed");

        session.air_launch = true;
        assert!(session.batch_stale(&vehicle, &program), "profile changed");
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
        vehicle
            .add_to_top(
                "radar",
                PartSpec::Sensor {
                    kind: omega_rules::SensorKind::Radar(omega_rules::RadarClass::Basic),
                },
            )
            .unwrap();
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

    #[test]
    fn a_run_and_its_history_keep_phases_and_anomaly_anchors() {
        let mut session = TrialSession::default();
        run(&mut session, &rocket(), &two_stage(20.0)).unwrap();
        let report_anomalies = session.report.as_ref().unwrap().report.anomalies.len();
        let entry = session.history.entries().last().unwrap();
        assert_eq!(entry.kind, "SINGLE");
        assert!(!entry.phases.is_empty());
        assert!(entry.phases.iter().any(|phase| phase.duration_s > 0.0));
        assert_eq!(entry.anomalies.len(), report_anomalies);

        let error = run(&mut session, &rocket(), &Program::single(vec![Block::LockTarget])).unwrap_err();
        assert!(error.to_lowercase().contains("lock target"), "{error}");
        let failure = session.failure.as_ref().unwrap();
        let insight = failure
            .anomalies
            .iter()
            .find(|insight| insight.kind == lab::AnomalyKind::ProgramIncompatible)
            .expect("the incompatible program is not classified");
        assert!(insight.block.is_some(), "the failing block is not linked");
        assert!(insight.component.is_none(), "this design has no radar to link");
    }

    #[test]
    fn relaunching_a_campaign_replays_its_exact_snapshot() {
        let vehicle = rocket();
        let program = two_stage(20.0);
        let mut session = TrialSession::default();
        run(&mut session, &vehicle, &program).unwrap();
        let id = session.history.entries().last().unwrap().id;
        let original_report = session.report.as_ref().unwrap().report.clone();
        let original_hash = session.report.as_ref().unwrap().input_hash.clone();

        apply_action(&mut session, TrialAction::Relaunch(id), &vehicle, &program, true).unwrap();
        let replay = session.report.as_ref().unwrap();
        assert_eq!(original_report, replay.report);
        assert_eq!(original_hash, replay.input_hash);
        let entry = session.history.entries().last().unwrap();
        assert_eq!(entry.relaunched_from, Some(id));
        assert_eq!(entry.input_hash, original_hash);
    }

    #[test]
    fn a_batch_is_recorded_with_its_exact_seed_range() {
        let mut session = TrialSession::default();
        start_batch(&mut session, &rocket(), &two_stage(20.0), 10, true).unwrap();
        session.batch = Some(wait_for_batch(&mut session));
        let entry = session.history.entries().last().unwrap();
        assert_eq!(entry.kind, "BATCH");
        assert_eq!(entry.runs, 10);
        assert_eq!(entry.seeds_label(), "Graines 1–10");
        assert_eq!(entry.stats.runs, 10);
        assert_eq!(entry.phases.len(), session.batch.as_ref().unwrap().phases.len());
    }

    #[test]
    fn a_history_entry_goes_stale_only_after_its_inputs_change() {
        let vehicle = rocket();
        let program = two_stage(20.0);
        let mut session = TrialSession::default();
        run(&mut session, &vehicle, &program).unwrap();
        let entry = session.history.entries().last().unwrap().clone();
        assert!(!entry.stale(&vehicle, &program));
        assert!(entry.stale_reason(&vehicle, &program).is_none());

        assert!(entry.stale(&vehicle, &two_stage(8.0)));
        assert!(
            entry
                .stale_reason(&vehicle, &two_stage(8.0))
                .unwrap()
                .contains("programme")
        );

        let mut edited = rocket();
        edited.replace_part(
            "booster",
            PartSpec::SolidMotor {
                length_m: 3.0,
                diameter_m: 0.4,
                burn_time_s: 20.0,
            },
        );
        assert!(entry.stale(&edited, &program));
        assert_eq!(entry.stale_reason(&edited, &program), Some("conception modifiée"));
    }

    #[test]
    fn the_history_round_trips_through_its_json_file() {
        let mut session = TrialSession::default();
        run(&mut session, &rocket(), &two_stage(20.0)).unwrap();
        let expected = session.history.entries().first().unwrap().input_hash.clone();
        let bytes = session.history.to_bytes().unwrap();
        let restored = TrialHistory::from_bytes(&bytes).unwrap();
        assert_eq!(restored, session.history);
        assert_eq!(restored.entries().len(), 1);
        assert_eq!(lab::entry_hash(restored.entries().first().unwrap()), expected);
    }
}
