//! Trial laboratory model (0.44): design metrics for comparisons, phase summaries, structured
//! anomaly analysis, compliance snapshots and the persisted history of trial campaigns.
//!
//! A history entry stores the exact vehicle, program, seed and flight profile it flew. That makes
//! a campaign replayable byte-for-byte and lets the UI mark results obsolete as soon as the
//! current design or program hash no longer matches the evidence.
//!
//! Everything here is fictional and abstract; see `docs/VISION_MISSILES.md`.

use omega_data::hash_serializable;
use omega_rules::assembly::{IssueSeverity, VehicleIssue};
use omega_rules::compliance::{CheckStatus, contract_preview};
use omega_rules::contracts::{ContractOffer, DesignGoal, DesignGoalMetric, MissileTrial};
use omega_rules::dossier::{Dossier, DossierDecision};
use omega_rules::flight::{FlightConfig, FlightReport, LaunchMode, Sample};
use omega_rules::program::{Block, Program};
use omega_rules::requirement::RequirementStatus;
use omega_rules::{ContractCheck, PartSpec, Vehicle};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const HISTORY_SCHEMA: u32 = 1;
const MAX_ENTRIES: usize = 80;

/// The six figures the comparison table quotes for a design. The cost is a fictional planning
/// number derived from the airframe and electronics, not a real procurement estimate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct DesignMetrics {
    pub mass_kg: f64,
    pub cost_cr: f64,
    pub stability: f64,
    pub authority: f64,
    pub complexity: f64,
    pub reliability: f64,
}

impl DesignMetrics {
    pub(crate) fn of(vehicle: &Vehicle) -> Self {
        let aero = vehicle.aero();
        let avionics = vehicle.avionics();
        let mass = vehicle.total_wet_mass_kg();
        Self {
            mass_kg: mass,
            // A deterministic fictional budget: structure and propellant dominate, electronics
            // and surface complexity add their own share.
            cost_cr: avionics.cost_cr as f64
                + mass * 18.0
                + aero.surface_count as f64 * 850.0
                + aero.complexity * 120.0,
            stability: aero.stability,
            authority: (aero.control_authority * avionics.authority_multiplier()).clamp(0.0, 100.0),
            complexity: (aero.complexity + avionics.complexity * 10.0).max(0.0),
            reliability: (aero.reliability + avionics.reliability * 10.0).clamp(0.0, 100.0),
        }
    }

    pub(crate) fn rows(&self) -> [(&'static str, f64); 6] {
        [
            ("Masse", self.mass_kg),
            ("Coût fictif", self.cost_cr),
            ("Stabilité", self.stability),
            ("Autorité", self.authority),
            ("Complexité", self.complexity),
            ("Fiabilité", self.reliability),
        ]
    }
}

/// One contract goal evaluated against the current abstract design metrics.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GoalStatus {
    pub id: String,
    pub label: String,
    pub value: f64,
    pub limit: u32,
    pub at_most: bool,
    pub met: bool,
}

impl GoalStatus {
    pub(crate) fn text(&self) -> String {
        let comparator = if self.at_most { "≤" } else { "≥" };
        format!("{} {comparator} {} · actuel {:.0}", self.label, self.limit, self.value)
    }
}

/// Evaluate the integer design goals stored on a contract. Old offers have no goals and therefore
/// produce an empty list, preserving their previous delivery rubric.
pub(crate) fn goal_statuses(goals: &[DesignGoal], metrics: &DesignMetrics) -> Vec<GoalStatus> {
    goals
        .iter()
        .map(|goal| {
            let value = match goal.metric {
                DesignGoalMetric::MassKg => metrics.mass_kg,
                DesignGoalMetric::Reliability => metrics.reliability,
                DesignGoalMetric::Complexity => metrics.complexity,
            };
            let met = if goal.at_most {
                value <= f64::from(goal.limit)
            } else {
                value >= f64::from(goal.limit)
            };
            GoalStatus {
                id: goal.id.clone(),
                label: goal.label.clone(),
                value,
                limit: goal.limit,
                at_most: goal.at_most,
                met,
            }
        })
        .collect()
}

pub(crate) fn budget_ok(offer: &ContractOffer, metrics: &DesignMetrics) -> bool {
    offer.design_budget_cr == 0 || metrics.cost_cr <= offer.design_budget_cr as f64
}

/// Percentage variation from A to B. `None` when A is zero, since a ratio would be meaningless.
pub(crate) fn percent_delta(a: f64, b: f64) -> Option<f64> {
    (a.abs() > 1e-9).then_some((b - a) / a * 100.0)
}

/// One coherent slice of a flight, used by the dashboard's per-phase table.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct PhaseSummary {
    pub name: String,
    pub runs: usize,
    pub samples: usize,
    pub duration_s: f64,
    pub mean_speed_ms: f64,
    pub max_speed_ms: f64,
    pub mean_altitude_m: f64,
    pub max_altitude_m: f64,
    pub mean_mass_kg: f64,
}

impl PhaseSummary {
    fn from_samples(name: &str, samples: &[Sample]) -> Option<Self> {
        if samples.is_empty() {
            return None;
        }
        let first = samples.first()?.t_s;
        let last = samples.last()?.t_s;
        let speed = samples.iter().map(|s| s.speed_ms);
        let altitude = samples.iter().map(|s| s.altitude_m);
        let mass = samples.iter().map(|s| s.mass_kg);
        Some(Self {
            name: name.to_string(),
            runs: 1,
            samples: samples.len(),
            duration_s: (last - first).max(0.0),
            mean_speed_ms: mean(speed.clone()),
            max_speed_ms: max(speed),
            mean_altitude_m: mean(altitude.clone()),
            max_altitude_m: max(altitude),
            mean_mass_kg: mean(mass),
        })
    }

    pub(crate) fn merge(&mut self, other: &Self) {
        let self_weight = self.runs.max(1) as f64;
        let other_weight = other.runs.max(1) as f64;
        let weight = self_weight + other_weight;
        self.mean_speed_ms = (self.mean_speed_ms * self_weight + other.mean_speed_ms * other_weight) / weight;
        self.mean_altitude_m = (self.mean_altitude_m * self_weight + other.mean_altitude_m * other_weight) / weight;
        self.mean_mass_kg = (self.mean_mass_kg * self_weight + other.mean_mass_kg * other_weight) / weight;
        self.duration_s = (self.duration_s * self_weight + other.duration_s * other_weight) / weight;
        self.max_speed_ms = self.max_speed_ms.max(other.max_speed_ms);
        self.max_altitude_m = self.max_altitude_m.max(other.max_altitude_m);
        self.samples += other.samples;
        self.runs += other.runs;
    }
}

fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let mut sum = 0.0;
    let mut count = 0usize;
    for value in values {
        sum += value;
        count += 1;
    }
    if count == 0 { 0.0 } else { sum / count as f64 }
}

fn max(values: impl Iterator<Item = f64>) -> f64 {
    values.fold(0.0, f64::max)
}

/// Split a flight into launch, climb and terminal slices. The cut points are time based, so a
/// report with very few samples still produces a stable, deterministic set of rows.
pub(crate) fn phases(report: &FlightReport) -> Vec<PhaseSummary> {
    let samples = &report.samples;
    if samples.is_empty() {
        return Vec::new();
    }
    let start = samples[0].t_s;
    let end = samples.last().map_or(start, |s| s.t_s).max(start);
    let span = (end - start).max(0.001);
    let first_cut = start + span * 0.10;
    let burnout = samples
        .iter()
        .filter(|s| s.thrust_n > 1.0)
        .map(|s| s.t_s)
        .fold(None, |last, t| Some(last.map_or(t, |value: f64| value.max(t))))
        .unwrap_or(first_cut);
    let second_cut = burnout.clamp(first_cut, end);

    let mut groups: BTreeMap<&str, Vec<Sample>> = BTreeMap::new();
    for sample in samples {
        let name = if sample.t_s <= first_cut {
            "DÉCOLLAGE"
        } else if sample.t_s <= second_cut {
            "MONTÉE"
        } else {
            "TERMINAL"
        };
        groups.entry(name).or_default().push(*sample);
    }
    groups
        .into_iter()
        .filter_map(|(name, group)| PhaseSummary::from_samples(name, &group))
        .collect()
}

/// Summary figures over a single run or a whole seeded campaign.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct TrialStats {
    pub runs: usize,
    pub successes: usize,
    pub no_liftoff: usize,
    pub no_separation: usize,
    pub anomaly_runs: usize,
    pub lock_attempts: usize,
    pub lock_success: usize,
    pub mean_max_speed_ms: f64,
    pub mean_path_m: f64,
    pub mean_flight_time_s: f64,
    pub mean_altitude_m: f64,
    pub max_speed_ms: f64,
    pub max_path_m: f64,
    pub max_altitude_m: f64,
}

impl TrialStats {
    pub(crate) fn from_report(report: &FlightReport) -> Self {
        let success = report.lifted_off && report.anomalies.is_empty();
        Self {
            runs: 1,
            successes: usize::from(success),
            no_liftoff: usize::from(!report.lifted_off),
            no_separation: usize::from(!report.events.iter().any(|(_, label)| label.contains("separated"))),
            anomaly_runs: usize::from(!report.anomalies.is_empty()),
            lock_attempts: usize::from(report.lock.is_some()),
            lock_success: usize::from(report.lock.is_some_and(|lock| lock.success)),
            mean_max_speed_ms: report.max_speed_ms,
            mean_path_m: report.path_length_m,
            mean_flight_time_s: report.flight_time_s,
            mean_altitude_m: report.max_altitude_m,
            max_speed_ms: report.max_speed_ms,
            max_path_m: report.path_length_m,
            max_altitude_m: report.max_altitude_m,
        }
    }

    pub(crate) fn accumulate(report: &FlightReport, stats: &mut Self) {
        stats.runs += 1;
        let runs = stats.runs as f64;
        stats.successes += usize::from(report.lifted_off && report.anomalies.is_empty());
        stats.no_liftoff += usize::from(!report.lifted_off);
        stats.no_separation += usize::from(!report.events.iter().any(|(_, label)| label.contains("separated")));
        stats.anomaly_runs += usize::from(!report.anomalies.is_empty());
        stats.lock_attempts += usize::from(report.lock.is_some());
        stats.lock_success += usize::from(report.lock.is_some_and(|lock| lock.success));
        stats.mean_max_speed_ms += (report.max_speed_ms - stats.mean_max_speed_ms) / runs;
        stats.mean_path_m += (report.path_length_m - stats.mean_path_m) / runs;
        stats.mean_flight_time_s += (report.flight_time_s - stats.mean_flight_time_s) / runs;
        stats.mean_altitude_m += (report.max_altitude_m - stats.mean_altitude_m) / runs;
        stats.max_speed_ms = stats.max_speed_ms.max(report.max_speed_ms);
        stats.max_path_m = stats.max_path_m.max(report.path_length_m);
        stats.max_altitude_m = stats.max_altitude_m.max(report.max_altitude_m);
    }

    pub(crate) fn lock_rate(&self) -> Option<f64> {
        (self.lock_attempts > 0).then(|| self.lock_success as f64 / self.lock_attempts as f64)
    }

    pub(crate) fn success_rate(&self) -> Option<f64> {
        (self.runs > 0).then(|| self.successes as f64 / self.runs as f64)
    }
}

/// A block an anomaly can point at in the program editor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BlockFocus {
    pub thread: usize,
    pub block: usize,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnomalyKind {
    Instability,
    ComponentFailure,
    ResourceShortage,
    ProgramIncompatible,
    ContractRequirement,
    Timeout,
    Other,
}

impl AnomalyKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Instability => "INSTABILITÉ",
            Self::ComponentFailure => "COMPOSANT",
            Self::ResourceShortage => "RESSOURCE",
            Self::ProgramIncompatible => "PROGRAMME",
            Self::ContractRequirement => "CONTRAT",
            Self::Timeout => "DÉLAI",
            Self::Other => "ANOMALIE",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct AnomalyInsight {
    pub kind: AnomalyKind,
    pub message: String,
    pub detail: String,
    pub component: Option<String>,
    pub block: Option<BlockFocus>,
    pub warning: bool,
}

impl AnomalyInsight {
    pub(crate) fn new(kind: AnomalyKind, message: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            detail: detail.into(),
            component: None,
            block: None,
            warning: false,
        }
    }

    fn with_component(mut self, component: Option<String>) -> Self {
        self.component = component;
        self
    }

    fn with_block(mut self, block: Option<BlockFocus>) -> Self {
        self.block = block;
        self
    }
}

fn part_for_stage(vehicle: &Vehicle, stage: usize) -> Option<String> {
    vehicle
        .parts
        .iter()
        .find(|part| vehicle.stage_of(&part.id) == Some(stage))
        .map(|part| part.id.clone())
}

fn control_surface(vehicle: &Vehicle) -> Option<String> {
    vehicle
        .parts
        .iter()
        .find(|part| {
            matches!(
                part.spec,
                PartSpec::Wing { .. } | PartSpec::FinSet { .. } | PartSpec::ControlSurface { .. }
            )
        })
        .map(|part| part.id.clone())
}

fn radar_part(vehicle: &Vehicle) -> Option<String> {
    vehicle
        .parts
        .iter()
        .find(|part| matches!(part.spec, PartSpec::Sensor { .. }))
        .map(|part| part.id.clone())
}

fn block_index<F>(program: &Program, predicate: F) -> Option<usize>
where
    F: Fn(&Block) -> bool,
{
    program.threads.iter().flatten().position(predicate)
}

fn block_focus(thread: usize, block: usize, label: &str) -> BlockFocus {
    BlockFocus {
        thread,
        block,
        label: label.to_string(),
    }
}

/// Turn one raw report anomaly into a category, a component and, when possible, a program block.
pub(crate) fn analyze_report_anomalies(
    report: &FlightReport,
    vehicle: &Vehicle,
    program: &Program,
) -> Vec<AnomalyInsight> {
    report
        .anomalies
        .iter()
        .map(|message| {
            let lower = message.to_lowercase();
            let stage = stage_number(&lower);
            if lower.contains("control surface faulted") || lower.contains("stability") {
                return AnomalyInsight::new(
                    AnomalyKind::Instability,
                    message,
                    "La stabilité ou une surface de contrôle est mise en cause.",
                )
                .with_component(control_surface(vehicle));
            }
            if lower.contains("lock") || lower.contains("radar") {
                let block = block_index(program, |block| matches!(block, Block::LockTarget));
                return AnomalyInsight::new(
                    AnomalyKind::ProgramIncompatible,
                    message,
                    "Le verrouillage cible dépend du radar et du bloc LOCK TARGET.",
                )
                .with_component(radar_part(vehicle))
                .with_block(block.map(|index| block_focus(0, index, "Lock target")));
            }
            if lower.contains("subroutine") || lower.contains("call") {
                let block = block_index(program, |block| matches!(block, Block::Call { .. }));
                return AnomalyInsight::new(
                    AnomalyKind::ProgramIncompatible,
                    message,
                    "La pile d'appels du sous-programme a atteint sa limite.",
                )
                .with_block(block.map(|index| block_focus(0, index, "Call routine")));
            }
            if lower.contains("timed out") || lower.contains("timeout") {
                let block = block_index(program, |block| matches!(block, Block::WaitUntil { .. }));
                return AnomalyInsight::new(
                    AnomalyKind::Timeout,
                    message,
                    "Une attente conditionnelle a dépassé son délai.",
                )
                .with_block(block.map(|index| block_focus(0, index, "Wait until")));
            }
            if lower.contains("no stage was ever ignited") {
                let block = block_index(program, |block| matches!(block, Block::IgniteStage { .. }));
                return AnomalyInsight::new(
                    AnomalyKind::ResourceShortage,
                    message,
                    "Aucun étage n'a été allumé : vérifiez le programme et les moteurs.",
                )
                .with_component(part_for_stage(vehicle, 0))
                .with_block(block.map(|index| block_focus(0, index, "Ignite stage")));
            }
            if lower.contains("lift off") || lower.contains("thrust-to-weight") {
                let block = block_index(program, |block| matches!(block, Block::IgniteStage { .. }));
                return AnomalyInsight::new(
                    AnomalyKind::Instability,
                    message,
                    "Le rapport poussée/poids est insuffisant pour décoller.",
                )
                .with_component(part_for_stage(vehicle, 0))
                .with_block(block.map(|index| block_focus(0, index, "Ignite stage")));
            }
            if let Some(stage) = stage {
                return AnomalyInsight::new(
                    AnomalyKind::ComponentFailure,
                    message,
                    format!("L'étage {} est concerné.", stage + 1),
                )
                .with_component(part_for_stage(vehicle, stage));
            }
            AnomalyInsight::new(AnomalyKind::Other, message, "Anomalie de vol signalée par le modèle.")
        })
        .collect()
}

fn stage_number(lower: &str) -> Option<usize> {
    let start = lower.find("stage ")? + "stage ".len();
    let digits: String = lower[start..]
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect();
    digits.parse::<usize>().ok().map(|number| number.saturating_sub(1))
}

/// Validation errors that stopped the flight before any report existed.
pub(crate) fn analyze_failure(vehicle: &Vehicle, program: &Program, messages: &[String]) -> Vec<AnomalyInsight> {
    let program_issues = omega_rules::program_issues(program, vehicle);
    let vehicle_issues = vehicle.issues();
    let mut insights = Vec::new();
    for issue in program_issues
        .iter()
        .filter(|issue| issue.severity == IssueSeverity::Error)
    {
        let mut insight = analyze_message(&issue.message);
        if let Some(block) = issue.block {
            insight.block = Some(block_focus(issue.thread, block, "Bloc signalé"));
        }
        insights.push(insight);
    }
    for issue in vehicle_issues
        .iter()
        .filter(|issue| issue.severity == IssueSeverity::Error)
    {
        let mut insight = analyze_vehicle_issue(issue);
        insight.component = issue.part.clone();
        insights.push(insight);
    }
    for message in messages {
        if !insights.iter().any(|insight| insight.message == *message) {
            insights.push(analyze_message(message));
        }
    }
    insights
}

fn analyze_message(message: &str) -> AnomalyInsight {
    let lower = message.to_lowercase();
    let kind = if lower.contains("timeout") || lower.contains("délai") {
        AnomalyKind::Timeout
    } else if lower.contains("stage") || lower.contains("étage") {
        AnomalyKind::ComponentFailure
    } else if lower.contains("moteur") || lower.contains("thrust") || lower.contains("power") {
        AnomalyKind::ResourceShortage
    } else {
        AnomalyKind::ProgramIncompatible
    };
    AnomalyInsight::new(kind, message, "Cette erreur empêche l'essai de produire un rapport.")
}

fn analyze_vehicle_issue(issue: &VehicleIssue) -> AnomalyInsight {
    let lower = issue.message.to_lowercase();
    let kind = if lower.contains("stability") || lower.contains("stable") {
        AnomalyKind::Instability
    } else if lower.contains("power") || lower.contains("computer") || lower.contains("mass") {
        AnomalyKind::ResourceShortage
    } else {
        AnomalyKind::ComponentFailure
    };
    AnomalyInsight::new(
        kind,
        issue.message.clone(),
        "Le composant doit être corrigé dans l'atelier.",
    )
    .with_component(issue.part.clone())
}

/// A contract row captured with the verdict it had at the time of the run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ComplianceRow {
    pub id: String,
    pub label: String,
    pub status: String,
    pub margin: Option<f64>,
    pub hard: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ComplianceSnapshot {
    pub contract_id: u32,
    pub title: String,
    pub score_percent: f64,
    pub accepted: bool,
    pub rows: Vec<ComplianceRow>,
}

impl ComplianceSnapshot {
    pub(crate) fn from_dossier(contract_id: u32, title: &str, dossier: &Dossier) -> Self {
        let rows = dossier
            .matrix
            .iter()
            .map(|row| ComplianceRow {
                id: row.requirement_id.clone(),
                label: row.label.clone(),
                status: match row.status {
                    RequirementStatus::Verified { .. } => "VERIFIED",
                    RequirementStatus::Marginal { .. } => "MARGINAL",
                    RequirementStatus::Failed { .. } => "FAILED",
                    RequirementStatus::Stale => "STALE",
                    RequirementStatus::MethodNotAllowed => "METHOD",
                    RequirementStatus::Unverified => "UNVERIFIED",
                }
                .to_string(),
                margin: margin(&row.status),
                hard: row.hard,
            })
            .collect();
        let accepted = matches!(dossier.decision, DossierDecision::Accepted { .. });
        Self {
            contract_id,
            title: title.to_string(),
            score_percent: dossier.score * 100.0,
            accepted,
            rows,
        }
    }

    pub(crate) fn errors(&self) -> Vec<AnomalyInsight> {
        self.rows
            .iter()
            .filter(|row| row.status == "FAILED" || row.status == "STALE" || row.status == "UNVERIFIED")
            .map(|row| {
                let mut insight = AnomalyInsight::new(
                    AnomalyKind::ContractRequirement,
                    format!("{} : {}", row.label, row.status),
                    format!("Exigence {:?} du contrat #{}.", row.id, self.contract_id),
                );
                insight.detail = if let Some(margin) = row.margin {
                    format!(
                        "Exigence {:?} du contrat #{} : marge {margin:+.2}.",
                        row.id, self.contract_id
                    )
                } else {
                    format!("Exigence {:?} du contrat #{} non satisfaite.", row.id, self.contract_id)
                };
                insight
            })
            .collect()
    }
}

fn margin(status: &RequirementStatus) -> Option<f64> {
    match status {
        RequirementStatus::Verified { margin }
        | RequirementStatus::Marginal { margin }
        | RequirementStatus::Failed { margin } => Some(*margin),
        RequirementStatus::Stale | RequirementStatus::MethodNotAllowed | RequirementStatus::Unverified => None,
    }
}

/// Design-side contract status for the comparison screen. The trial-only requirements remain
/// `TRIAL` until a real flight supplies evidence.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CompliancePreview {
    pub met: usize,
    pub close: usize,
    pub missed: usize,
    pub trial: usize,
    pub checks: Vec<ContractCheck>,
}

impl CompliancePreview {
    pub(crate) fn of(vehicle: &Vehicle, offer: Option<&ContractOffer>) -> Self {
        let Some(offer) = offer else {
            return Self::default();
        };
        let checks = contract_preview(vehicle, offer);
        let count = |status: CheckStatus| checks.iter().filter(|check| check.status == status).count();
        Self {
            met: count(CheckStatus::Met),
            close: count(CheckStatus::Close),
            missed: count(CheckStatus::Missed),
            trial: count(CheckStatus::Unproven),
            checks,
        }
    }

    pub(crate) fn label(&self) -> String {
        format!(
            "{} conformes · {} proches · {} manquantes · {} en essai",
            self.met, self.close, self.missed, self.trial
        )
    }
}

/// Context captured before an action so a later batch completion is still associated with the
/// revision and day that launched it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TrialContext {
    pub day: u32,
    pub revision_id: Option<u32>,
    pub revision_hash: String,
    pub revision_label: String,
    pub change_summary: String,
}

/// One recorded campaign. A single run stores one report summary; a batch stores the aggregate
/// and its exact seed range. `vehicle` and `program` are full snapshots, so replay never depends
/// on the current design.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct TrialHistoryEntry {
    pub id: u64,
    pub label: String,
    pub kind: String,
    pub day: u32,
    pub seed: u64,
    pub runs: usize,
    pub air_launch: bool,
    pub vehicle: Vehicle,
    pub program: Program,
    pub vehicle_hash: String,
    pub program_hash: String,
    pub input_hash: String,
    pub revision_id: Option<u32>,
    pub revision_hash: String,
    pub revision_label: String,
    pub program_version: String,
    pub change_summary: String,
    pub metrics: DesignMetrics,
    pub stats: TrialStats,
    pub phases: Vec<PhaseSummary>,
    pub anomalies: Vec<AnomalyInsight>,
    pub events: Vec<(f64, String)>,
    pub compliance: Option<ComplianceSnapshot>,
    pub relaunched_from: Option<u64>,
}

impl TrialHistoryEntry {
    pub(crate) fn stale(&self, vehicle: &Vehicle, program: &Program) -> bool {
        let vehicle_hash = hash_serializable(vehicle)
            .map(|hash| hash.as_str().to_string())
            .unwrap_or_default();
        let program_hash = hash_serializable(&program.semantic_copy())
            .map(|hash| hash.as_str().to_string())
            .unwrap_or_default();
        self.vehicle_hash != vehicle_hash || self.program_hash != program_hash
    }

    pub(crate) fn stale_reason(&self, vehicle: &Vehicle, program: &Program) -> Option<&'static str> {
        let vehicle_hash = hash_serializable(vehicle)
            .map(|hash| hash.as_str().to_string())
            .unwrap_or_default();
        let program_hash = hash_serializable(&program.semantic_copy())
            .map(|hash| hash.as_str().to_string())
            .unwrap_or_default();
        match (self.vehicle_hash != vehicle_hash, self.program_hash != program_hash) {
            (true, true) => Some("conception et programme modifiés"),
            (true, false) => Some("conception modifiée"),
            (false, true) => Some("programme modifié"),
            (false, false) => None,
        }
    }

    pub(crate) fn seeds_label(&self) -> String {
        if self.runs <= 1 {
            format!("Graine {}", self.seed)
        } else {
            format!(
                "Graines {}–{}",
                self.seed,
                self.seed.saturating_add(self.runs as u64 - 1)
            )
        }
    }

    pub(crate) fn context(&self) -> TrialContext {
        TrialContext {
            day: self.day,
            revision_id: self.revision_id,
            revision_hash: self.revision_hash.clone(),
            revision_label: self.revision_label.clone(),
            change_summary: self.change_summary.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct TrialHistory {
    schema_version: u32,
    next_id: u64,
    entries: Vec<TrialHistoryEntry>,
}

impl Default for TrialHistory {
    fn default() -> Self {
        Self {
            schema_version: HISTORY_SCHEMA,
            next_id: 1,
            entries: Vec::new(),
        }
    }
}

impl TrialHistory {
    pub(crate) fn entries(&self) -> &[TrialHistoryEntry] {
        &self.entries
    }

    pub(crate) fn get(&self, id: u64) -> Option<&TrialHistoryEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub(crate) fn push(&mut self, mut entry: TrialHistoryEntry) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        entry.id = id;
        entry.label = if entry.kind == "BATCH" {
            format!("Campagne #{id} · lot {}", entry.runs)
        } else {
            format!("Campagne #{id}")
        };
        self.entries.push(entry);
        if self.entries.len() > MAX_ENTRIES {
            self.entries.remove(0);
        }
        id
    }

    pub(crate) fn annotate_latest_compliance(&mut self, snapshot: ComplianceSnapshot) {
        if let Some(entry) = self.entries.last_mut() {
            entry.compliance = Some(snapshot);
        }
    }

    pub(crate) fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(self).map_err(|error| error.to_string())
    }

    pub(crate) fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        let schema = value
            .get("schema_version")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        if schema != u64::from(HISTORY_SCHEMA) {
            return Err(format!("unsupported trial history schema {schema}"));
        }
        let mut history: Self = serde_json::from_value(value).map_err(|error| error.to_string())?;
        history.next_id = history
            .next_id
            .max(history.entries.iter().map(|entry| entry.id).max().unwrap_or(0) + 1);
        Ok(history)
    }
}

/// Data needed to build a history entry without touching the UI or the save system.
pub(crate) struct EntryInput<'a> {
    pub context: &'a TrialContext,
    pub vehicle: &'a Vehicle,
    pub program: &'a Program,
    pub seed: u64,
    pub air_launch: bool,
    pub report: &'a FlightReport,
    pub input_hash: String,
}

pub(crate) fn entry_from_report(input: EntryInput<'_>, relaunched_from: Option<u64>) -> TrialHistoryEntry {
    let vehicle_hash = hash_serializable(input.vehicle)
        .map(|hash| hash.as_str().to_string())
        .unwrap_or_default();
    let program_hash = hash_serializable(&input.program.semantic_copy())
        .map(|hash| hash.as_str().to_string())
        .unwrap_or_default();
    TrialHistoryEntry {
        id: 0,
        label: String::new(),
        kind: "SINGLE".to_string(),
        day: input.context.day,
        seed: input.seed,
        runs: 1,
        air_launch: input.air_launch,
        vehicle: input.vehicle.clone(),
        program: input.program.clone(),
        vehicle_hash,
        program_hash,
        input_hash: input.input_hash,
        revision_id: input.context.revision_id,
        revision_hash: input.context.revision_hash.clone(),
        revision_label: input.context.revision_label.clone(),
        program_version: format!("P-{}", short_hash(input.program)),
        change_summary: input.context.change_summary.clone(),
        metrics: DesignMetrics::of(input.vehicle),
        stats: TrialStats::from_report(input.report),
        phases: phases(input.report),
        anomalies: analyze_report_anomalies(input.report, input.vehicle, input.program),
        events: input.report.events.iter().take(40).cloned().collect(),
        compliance: None,
        relaunched_from,
    }
}

pub(crate) struct BatchEntryInput<'a> {
    pub context: &'a TrialContext,
    pub vehicle: &'a Vehicle,
    pub program: &'a Program,
    pub seed: u64,
    pub air_launch: bool,
    pub runs: usize,
    pub input_hash: String,
    pub stats: &'a TrialStats,
    pub phases: &'a [PhaseSummary],
    pub anomalies: &'a [AnomalyInsight],
}

pub(crate) fn entry_from_batch(input: BatchEntryInput<'_>, relaunched_from: Option<u64>) -> TrialHistoryEntry {
    let vehicle_hash = hash_serializable(input.vehicle)
        .map(|hash| hash.as_str().to_string())
        .unwrap_or_default();
    let program_hash = hash_serializable(&input.program.semantic_copy())
        .map(|hash| hash.as_str().to_string())
        .unwrap_or_default();
    TrialHistoryEntry {
        id: 0,
        label: String::new(),
        kind: "BATCH".to_string(),
        day: input.context.day,
        seed: input.seed,
        runs: input.runs,
        air_launch: input.air_launch,
        vehicle: input.vehicle.clone(),
        program: input.program.clone(),
        vehicle_hash,
        program_hash,
        input_hash: input.input_hash,
        revision_id: input.context.revision_id,
        revision_hash: input.context.revision_hash.clone(),
        revision_label: input.context.revision_label.clone(),
        program_version: format!("P-{}", short_hash(input.program)),
        change_summary: input.context.change_summary.clone(),
        metrics: DesignMetrics::of(input.vehicle),
        stats: input.stats.clone(),
        phases: input.phases.to_vec(),
        anomalies: input.anomalies.to_vec(),
        events: Vec::new(),
        compliance: None,
        relaunched_from,
    }
}

pub(crate) fn short_hash(program: &Program) -> String {
    hash_serializable(&program.semantic_copy())
        .map(|hash| hash.as_str().chars().take(8).collect())
        .unwrap_or_else(|_| "--------".to_string())
}

/// Build the evidence for one report, so the dashboard can show that run's own dossier.
pub(crate) fn report_trial(vehicle: &Vehicle, program: &Program, seed: u64, report: &FlightReport) -> MissileTrial {
    let vehicle_hash = hash_serializable(vehicle).unwrap_or_else(|_| omega_data::ContentHash::of_bytes(b"vehicle"));
    let program_hash =
        hash_serializable(&program.semantic_copy()).unwrap_or_else(|_| omega_data::ContentHash::of_bytes(b"program"));
    MissileTrial::from_report(vehicle_hash, program_hash, seed, 1, report)
}

/// Dossier for one run against the selected contract. `None` when no contract is active.
pub(crate) fn compliance_for_report(
    offer: Option<&ContractOffer>,
    vehicle: &Vehicle,
    program: &Program,
    seed: u64,
    report: &FlightReport,
) -> Option<ComplianceSnapshot> {
    let offer = offer?;
    let trial = report_trial(vehicle, program, seed, report);
    let dossier = offer.missile_dossier(std::slice::from_ref(&trial), &trial.vehicle, &trial.program);
    Some(ComplianceSnapshot::from_dossier(offer.id, &offer.title, &dossier))
}

/// Anomalies from a rejected dossier, used when the trial itself flew but the contract failed.
pub(crate) fn contract_anomalies(snapshot: &ComplianceSnapshot) -> Vec<AnomalyInsight> {
    snapshot.errors()
}

/// One line for a compare table, with absolute and percentage variation.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ComparisonRow {
    pub label: &'static str,
    pub a: f64,
    pub b: f64,
}

impl ComparisonRow {
    pub(crate) fn absolute(&self) -> f64 {
        self.b - self.a
    }

    pub(crate) fn percent(&self) -> Option<f64> {
        percent_delta(self.a, self.b)
    }
}

pub(crate) fn compare_rows(a: &DesignMetrics, b: &DesignMetrics) -> Vec<ComparisonRow> {
    a.rows()
        .into_iter()
        .zip(b.rows())
        .map(|((label, a), (_, b))| ComparisonRow { label, a, b })
        .collect()
}

/// Flight-name/profile text used by history rows.
pub(crate) fn flight_label(air_launch: bool) -> &'static str {
    if air_launch { "AIR" } else { "VERTICAL" }
}

/// The input hash a saved entry would produce with its own snapshots. Used by tests and exports.
pub(crate) fn entry_hash(entry: &TrialHistoryEntry) -> String {
    let launch = match (entry.air_launch, entry.seed) {
        (true, seed) => format!("air|200|0|{seed}"),
        (false, seed) => format!("vertical|{seed}"),
    };
    hash_serializable(&(
        &entry.vehicle,
        &entry.program.semantic_copy(),
        FlightConfig::default().dt_s.to_bits(),
        FlightConfig::default().max_time_s.to_bits(),
        launch,
    ))
    .map(|hash| hash.as_str().to_string())
    .unwrap_or_default()
}

/// The launch mode matching an entry.
pub(crate) fn entry_config(entry: &TrialHistoryEntry) -> FlightConfig {
    FlightConfig {
        launch: if entry.air_launch {
            LaunchMode::Air {
                speed_ms: 200.0,
                altitude_m: 0.0,
            }
        } else {
            LaunchMode::Vertical
        },
        seed: entry.seed,
        ..FlightConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparison_rows_give_absolute_and_percentage_variations() {
        let a = DesignMetrics {
            mass_kg: 80.0,
            cost_cr: 100.0,
            stability: 50.0,
            authority: 40.0,
            complexity: 10.0,
            reliability: 90.0,
        };
        let b = DesignMetrics {
            mass_kg: 100.0,
            cost_cr: 80.0,
            stability: 55.0,
            authority: 0.0,
            complexity: 12.0,
            reliability: 90.0,
        };
        let rows = compare_rows(&a, &b);
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[0].label, "Masse");
        assert_eq!(rows[0].absolute(), 20.0);
        assert_eq!(rows[0].percent(), Some(25.0));
        assert_eq!(rows[1].absolute(), -20.0);
        assert_eq!(rows[1].percent(), Some(-20.0));
        assert_eq!(rows[3].percent(), Some(-100.0));
        assert_eq!(rows[5].absolute(), 0.0);
        assert_eq!(rows[5].percent(), Some(0.0));
        assert_eq!(percent_delta(0.0, 10.0), None, "no percentage from a zero baseline");
    }

    #[test]
    fn a_contract_failure_becomes_a_linked_anomaly() {
        let snapshot = ComplianceSnapshot {
            contract_id: 7,
            title: "Essai fictif".to_string(),
            score_percent: 40.0,
            accepted: false,
            rows: vec![ComplianceRow {
                id: "range".to_string(),
                label: "Portée".to_string(),
                status: "FAILED".to_string(),
                margin: Some(-12.5),
                hard: true,
            }],
        };
        let anomalies = contract_anomalies(&snapshot);
        assert_eq!(anomalies.len(), 1);
        assert_eq!(anomalies[0].kind, AnomalyKind::ContractRequirement);
        assert!(anomalies[0].message.contains("Portée"));
        assert!(anomalies[0].detail.contains("contrat #7"));
    }
}
