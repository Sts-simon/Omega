use crate::design::{
    Assembly, DesignSession, PolygonSketchSession, assembly_content_hash, assembly_metrics,
    build_assembly_mesh_thermal_config, build_polygon_sketch_thermal_config,
};
use crate::jobs::{JobManager, JobState};
use crate::sequence::SequenceWorkspace;
use crate::ui_kit::UiInteraction;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use omega_data::ContentHash;
use omega_rules::{
    Comparator, DossierDecision, Estimate, Evidence, Requirement, RequirementStatus, VerificationMethod,
    evaluate_requirement,
};
use omega_sim_core::refine_tri_mesh_local;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

mod board;
mod verification;

#[cfg(test)]
pub(crate) use board::friendly_capabilities;
pub(crate) use board::{
    ToastText, handle_extra_actions, poll_trial_batch, scroll_program_list, setup_boards, sync_boards,
    update_batch_progress, update_nav_status, update_toast,
};

const CAMPAIGN_SCHEMA: u32 = 1;
const MAX_ACTIVE_CONTRACTS: usize = 2;
const COMPLETED_HISTORY_LIMIT: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FacilityKind {
    Contracts,
    Construction,
    Research,
    Programming,
    Computation,
}

impl FacilityKind {
    fn label(self) -> &'static str {
        match self {
            Self::Contracts => "CONTRACT OFFICE",
            Self::Construction => "ASSEMBLY WORKSHOP",
            Self::Research => "RESEARCH CENTRE",
            Self::Programming => "PROGRAMMING STUDIO",
            Self::Computation => "COMPUTATION LAB",
        }
    }
}

#[derive(Component)]
pub struct BuildingVisual {
    pub facility: FacilityKind,
    pub half_extents: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampusView {
    Island,
    Contracts,
    Construction,
    /// Missile workshop (staged, resizable parts).
    Missile,
    /// Onboard missile program editor.
    Program,
    /// Deterministic flight trials of the current vehicle and program.
    Trials,
    Research,
    Programming,
    Computation,
}

/// Civil and scientific contract families. These represent measurement,
/// environmental observation and cooperative civil-safety instrumentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum ContractDomain {
    ScientificSounding,
    LongDurationObservation,
    GroundWeatherNetwork,
    CivilAirSafety,
    EnvironmentalSurvey,
}

impl ContractDomain {
    const ALL: [Self; 5] = [
        Self::ScientificSounding,
        Self::LongDurationObservation,
        Self::GroundWeatherNetwork,
        Self::CivilAirSafety,
        Self::EnvironmentalSurvey,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::ScientificSounding => "SCIENTIFIC SOUNDING",
            Self::LongDurationObservation => "LONG-DURATION OBSERVATION",
            Self::GroundWeatherNetwork => "GROUND WEATHER NETWORK",
            Self::CivilAirSafety => "COOPERATIVE CIVIL AIR SAFETY",
            Self::EnvironmentalSurvey => "ENVIRONMENTAL AIR-GROUND MAPPING",
        }
    }

    fn client_pool(self) -> &'static [&'static str] {
        match self {
            Self::ScientificSounding => &[
                "Institute for Atmospheric Studies",
                "National Weather Measurement Centre",
                "Scientific Probe Consortium",
            ],
            Self::LongDurationObservation => &[
                "Extreme Environment Observatory",
                "Climate Research Agency",
                "Autonomous Measurement Foundation",
            ],
            Self::GroundWeatherNetwork => &[
                "Island Meteorological Network",
                "Ports and Coastal Authority",
                "Climate Prevention Institute",
            ],
            Self::CivilAirSafety => &[
                "Civil Aviation Consortium",
                "Civil Flight Safety Centre",
                "Research Fleet Operators Association",
            ],
            Self::EnvironmentalSurvey => &[
                "Ecological Mapping Agency",
                "Coastal Monitoring Institute",
                "Ecosystem Observatory",
            ],
        }
    }

    fn title_pool(self) -> &'static [&'static str] {
        match self {
            Self::ScientificSounding => &[
                "Recoverable atmospheric instrument package",
                "Scientific sensor bay",
                "Upper-atmosphere measurement module",
            ],
            Self::LongDurationObservation => &[
                "Long-duration autonomous recorder",
                "Environmental observation module",
                "Low-power monitoring station",
            ],
            Self::GroundWeatherNetwork => &[
                "Distributed weather station set",
                "Coastal multi-sensor station",
                "Synchronized meteorological sensor array",
            ],
            Self::CivilAirSafety => &[
                "Cooperative civil telemetry terminal",
                "Fleet-state recording module",
                "Civil aviation data logger",
            ],
            Self::EnvironmentalSurvey => &[
                "Ecological mapping instrument kit",
                "Georeferenced environmental recorder",
                "Field sampling data module",
            ],
        }
    }

    fn brief(self) -> &'static str {
        match self {
            Self::ScientificSounding => {
                "Deliver a recoverable scientific instrument package for atmospheric measurements. The contract focuses on instrumentation, environmental robustness and data quality."
            }
            Self::LongDurationObservation => {
                "Develop an autonomous recorder for a long measurement campaign, with controlled energy use and reliable data retention."
            }
            Self::GroundWeatherNetwork => {
                "Supply compatible weather stations that operate outdoors and produce consistent environmental readings."
            }
            Self::CivilAirSafety => {
                "Develop cooperative civil telemetry and state-logging modules to improve the safety and interoperability of a research fleet."
            }
            Self::EnvironmentalSurvey => {
                "Create an instrument kit for mapping natural areas and recording georeferenced environmental measurements."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct ContractRequirements {
    max_unit_mass_kg: u8,
    max_unit_power_w: u16,
    temperature_min_c: i16,
    temperature_max_c: i16,
    min_uptime_percent: u8,
    min_data_completeness_percent: u8,
    min_storage_gb: u16,
    required_units: u8,
    max_unit_cost_cr: u64,
    /// Geometry-derived balance requirement: distance between the centre of gravity and the
    /// envelope centre, as a fraction of the design length. Absent in pre-0.22 campaigns.
    #[serde(default)]
    max_cg_offset_ratio: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ContractOffer {
    id: u32,
    domain: ContractDomain,
    title: String,
    client: String,
    brief: String,
    requirements: ContractRequirements,
    reward: u64,
    deadline_days: u16,
    difficulty: u8,
}

/// Prototype electronics and instrumentation configuration. It is deliberately
/// separate from weapon systems and describes only civilian measurement devices.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstrumentProfile {
    pub power_draw_w: u16,
    pub uptime_percent: u8,
    pub data_completeness_percent: u8,
    pub storage_gb: u16,
    pub operating_min_c: i16,
    pub operating_max_c: i16,
}

impl Default for InstrumentProfile {
    fn default() -> Self {
        Self {
            power_draw_w: 24,
            uptime_percent: 96,
            data_completeness_percent: 96,
            storage_gb: 128,
            operating_min_c: -55,
            operating_max_c: 80,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum ContractPhase {
    Accepted,
    NeedsRevision,
    DesignSubmitted,
    AnalysisQueued,
    AnalysisPassed,
    AnalysisFailed,
    TestPassed,
    TestFailed,
}

impl ContractPhase {
    fn label(self) -> &'static str {
        match self {
            Self::Accepted => "ACCEPTED - DESIGN REQUIRED",
            Self::NeedsRevision => "DESIGN DOES NOT MEET REQUIREMENTS",
            Self::DesignSubmitted => "DESIGN SUBMITTED - ANALYSIS REQUIRED",
            Self::AnalysisQueued => "SCIENTIFIC ANALYSIS RUNNING",
            Self::AnalysisPassed => "ANALYSIS PASSED - FIELD TEST REQUIRED",
            Self::AnalysisFailed => "ANALYSIS FAILED - RETRY REQUIRED",
            Self::TestPassed => "FIELD TEST PASSED - READY FOR DELIVERY",
            Self::TestFailed => "FIELD TEST FAILED - REVISE DESIGN",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProjectSnapshot {
    design_revision: u64,
    profile_revision: u64,
    mass_kg: f64,
    part_count: usize,
    connection_count: usize,
    estimated_unit_cost_cr: u64,
    fingerprint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct FieldTestReport {
    contract_id: u32,
    created_unix_ms: u128,
    design_fingerprint: String,
    random_seed: u64,
    instrument_profile: InstrumentProfile,
    sample_count: u32,
    measured_uptime_percent: f64,
    measured_data_completeness_percent: f64,
    thermal_envelope_passed: bool,
    passed: bool,
    report_path: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ActiveContract {
    offer: ContractOffer,
    phase: ContractPhase,
    snapshot: Option<ProjectSnapshot>,
    analysis_job_id: Option<u64>,
    test_report: Option<FieldTestReport>,
    last_issues: Vec<String>,
    /// Hash-bound proofs gathered for this contract (absent in campaigns saved before 0.23).
    #[serde(default)]
    evidence: Vec<Evidence>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CompletedContract {
    id: u32,
    title: String,
    client: String,
    production_cost_cr: u64,
    payout_cr: u64,
    profit_cr: i64,
    completed_unix_ms: u128,
    /// Weighted score of the accepted dossier (absent for contracts completed before 0.23).
    #[serde(default)]
    dossier_score_percent: Option<u8>,
}

/// Tiny local PRNG for reproducible, non-cryptographic contract generation.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct ContractRng {
    state: u64,
}

impl ContractRng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0xA3E0_1AB5_2026_0001 } else { seed },
        }
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    fn range(&mut self, min: u32, max: u32) -> u32 {
        debug_assert!(min <= max);
        min + (self.next_u32() % (max - min + 1))
    }

    fn choose<T: Copy>(&mut self, values: &[T]) -> T {
        let index = self.range(0, values.len().saturating_sub(1) as u32) as usize;
        values[index]
    }
}

/// Harder contracts demand a better balanced design (limit shrinks from 26% to 10%).
fn balance_limit_for_difficulty(difficulty: u8) -> f64 {
    0.30 - 0.04 * f64::from(difficulty.clamp(1, 5))
}

/// Check the balance requirement against the real geometry through the rules crate.
fn check_balance(assembly: &Assembly, limit_ratio: f64, issues: &mut Vec<String>) {
    let Ok(metrics) = assembly_metrics(assembly) else {
        return; // empty or invalid geometry is already reported by the caller
    };
    let Some(estimate) = metrics.get("cg_offset_ratio") else {
        return;
    };
    let Ok(design_hash) = assembly_content_hash(assembly) else {
        return;
    };
    let requirement = Requirement {
        id: "BALANCE".to_string(),
        label: "Centre-of-gravity balance".to_string(),
        metric: "cg_offset_ratio".to_string(),
        cmp: Comparator::AtMost,
        limit: limit_ratio,
        weight: 1.0,
        hard: true,
        allowed_methods: vec![VerificationMethod::Analysis],
    };
    let evidence = Evidence {
        requirement_id: requirement.id.clone(),
        method: VerificationMethod::Analysis,
        run_id: 0,
        design: design_hash.clone(),
        program: None,
        estimate: Estimate {
            value: estimate.value,
            uncertainty: estimate.uncertainty,
        },
    };
    match evaluate_requirement(&requirement, Some(&evidence), &design_hash, None) {
        RequirementStatus::Failed { .. } => issues.push(format!(
            "Centre of gravity is offset {:.1}% of the design length; the limit is {:.1}%",
            estimate.value * 100.0,
            limit_ratio * 100.0
        )),
        RequirementStatus::Marginal { .. } => issues.push(format!(
            "Balance margin is too thin: offset {:.1}% +/- {:.1}% against a {:.1}% limit",
            estimate.value * 100.0,
            estimate.uncertainty * 100.0,
            limit_ratio * 100.0
        )),
        _ => {}
    }
}

fn make_contract(id: u32, domain: ContractDomain, rng: &mut ContractRng) -> ContractOffer {
    let difficulty = rng.range(1, 5) as u8;
    let d = u32::from(difficulty);
    let (base_mass, base_power, base_unit_budget, unit_range) = match domain {
        ContractDomain::ScientificSounding => (26_u32, 62_u32, 7_000_u32, (1, 1)),
        ContractDomain::LongDurationObservation => (30, 52, 9_000, (1, 2)),
        ContractDomain::GroundWeatherNetwork => (24, 42, 6_500, (3, 8)),
        ContractDomain::CivilAirSafety => (20, 38, 8_000, (2, 6)),
        ContractDomain::EnvironmentalSurvey => (25, 56, 7_500, (1, 3)),
    };

    // Higher difficulty tightens mass/power limits and increases reliability and
    // data-quality requirements. These are civilian instrument specifications.
    let max_unit_mass_kg = (base_mass.saturating_sub(d * 2) + rng.range(0, 3)).max(8) as u8;
    let max_unit_power_w = (base_power.saturating_sub(d * 4) + rng.range(0, 8)).max(8) as u16;
    let temperature_span = 45 + d * 6 + rng.range(0, 12);
    let temperature_min_c = -(temperature_span as i16 / 2) - rng.range(0, 8) as i16;
    let temperature_max_c = temperature_span as i16 / 2 + 25;
    let min_uptime_percent = (90 + d * 2 + rng.range(0, 2)).min(99) as u8;
    let min_data_completeness_percent = (92 + d + rng.range(0, 3)).min(99) as u8;
    let min_storage_gb = rng.choose(&[16_u16, 32, 64, 128, 256]);
    let required_units = rng.range(unit_range.0, unit_range.1) as u8;
    let max_unit_cost_cr = u64::from(base_unit_budget + d * 1_500 + rng.range(0, 2_500));
    let total_budget = max_unit_cost_cr.saturating_mul(u64::from(required_units));
    let deadline_days = (48_i32 - i32::from(difficulty) * 3 + rng.range(0, 9) as i32).clamp(20, 60) as u16;
    let design_bonus = u64::from(difficulty) * 1_250 * u64::from(required_units);
    let reward = total_budget + total_budget / 3 + design_bonus;

    ContractOffer {
        id,
        domain,
        title: rng.choose(domain.title_pool()).to_string(),
        client: rng.choose(domain.client_pool()).to_string(),
        brief: domain.brief().to_string(),
        requirements: ContractRequirements {
            max_unit_mass_kg,
            max_unit_power_w,
            temperature_min_c,
            temperature_max_c,
            min_uptime_percent,
            min_data_completeness_percent,
            min_storage_gb,
            required_units,
            max_unit_cost_cr,
            max_cg_offset_ratio: Some(balance_limit_for_difficulty(difficulty)),
        },
        reward,
        deadline_days,
        difficulty,
    }
}

fn generate_board(rng: &mut ContractRng, next_id: &mut u32) -> Vec<ContractOffer> {
    let mut domains = ContractDomain::ALL;
    for index in (1..domains.len()).rev() {
        let other = rng.range(0, index as u32) as usize;
        domains.swap(index, other);
    }

    domains
        .into_iter()
        .map(|domain| {
            let id = *next_id;
            *next_id = (*next_id).wrapping_add(1).max(1);
            make_contract(id, domain, rng)
        })
        .collect()
}

fn system_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0xA3E0_1AB5_2026_0001)
}

#[derive(Serialize, Deserialize)]
struct CampaignSave {
    schema_version: u32,
    credits: u64,
    available_contracts: Vec<ContractOffer>,
    accepted_contracts: Vec<ActiveContract>,
    completed_contracts: Vec<CompletedContract>,
    next_contract_id: u32,
    rng: ContractRng,
    board_refreshes: u32,
    instrument_profile: InstrumentProfile,
    profile_revision: u64,
    selected_contract_id: Option<u32>,
    #[serde(default)]
    research: omega_rules::ResearchState,
    #[serde(default)]
    day: u32,
}

#[derive(Resource)]
pub struct CampusState {
    pub view: CampusView,
    pub credits: u64,
    available_contracts: Vec<ContractOffer>,
    accepted_contracts: Vec<ActiveContract>,
    completed_contracts: Vec<CompletedContract>,
    next_contract_id: u32,
    rng: ContractRng,
    board_refreshes: u32,
    message: String,
    pub instrument_profile: InstrumentProfile,
    profile_revision: u64,
    selected_contract_id: Option<u32>,
    pub profile_editor_open: bool,
    pub(crate) research: omega_rules::ResearchState,
    pub(crate) day: u32,
    /// The civil contract office stays reachable until lot 5 removes it.
    pub(crate) civil_legacy_open: bool,
    dirty: bool,
}

impl Default for CampusState {
    fn default() -> Self {
        if let Some(mut save) = load_campaign() {
            let mut interrupted_analysis = false;
            for active in &mut save.accepted_contracts {
                if active.phase == ContractPhase::AnalysisQueued {
                    active.phase = ContractPhase::AnalysisFailed;
                    active.analysis_job_id = None;
                    interrupted_analysis = true;
                }
            }
            if !save
                .accepted_contracts
                .iter()
                .any(|item| Some(item.offer.id) == save.selected_contract_id)
            {
                save.selected_contract_id = save.accepted_contracts.first().map(|item| item.offer.id);
            }
            return Self {
                view: CampusView::Island,
                credits: save.credits,
                available_contracts: save.available_contracts,
                accepted_contracts: save.accepted_contracts,
                completed_contracts: save.completed_contracts,
                next_contract_id: save.next_contract_id,
                rng: save.rng,
                board_refreshes: save.board_refreshes,
                message: if interrupted_analysis {
                    "Campaign loaded. An unfinished background analysis was marked for retry.".to_string()
                } else {
                    "Campaign loaded from saves/slot_1/campaign.json".to_string()
                },
                instrument_profile: save.instrument_profile,
                profile_revision: save.profile_revision,
                selected_contract_id: save.selected_contract_id,
                profile_editor_open: false,
                research: save.research,
                day: save.day.max(1),
                civil_legacy_open: false,
                dirty: interrupted_analysis,
            };
        }

        let mut rng = ContractRng::new(system_seed());
        let mut next_contract_id = 1;
        let available_contracts = generate_board(&mut rng, &mut next_contract_id);
        Self {
            view: CampusView::Island,
            credits: 25_000,
            available_contracts,
            accepted_contracts: Vec::new(),
            completed_contracts: Vec::new(),
            next_contract_id,
            rng,
            board_refreshes: 0,
            message: "Select a facility on the island".to_string(),
            instrument_profile: InstrumentProfile::default(),
            profile_revision: 1,
            selected_contract_id: None,
            profile_editor_open: false,
            research: omega_rules::ResearchState::default(),
            day: 1,
            civil_legacy_open: false,
            dirty: true,
        }
    }
}

impl CampusState {
    fn selected_contract_index(&self) -> Option<usize> {
        self.selected_contract_id.and_then(|id| {
            self.accepted_contracts
                .iter()
                .position(|contract| contract.offer.id == id)
        })
    }

    fn selected_contract(&self) -> Option<&ActiveContract> {
        self.selected_contract_index()
            .and_then(|index| self.accepted_contracts.get(index))
    }

    pub(crate) fn ui_computation_summary(&self) -> String {
        let project = self
            .selected_contract()
            .map(|active| format!("Selected project: {} | {}", active.offer.title, active.phase.label()))
            .unwrap_or_else(|| "No active project selected".to_string());
        format!("{}\nCredits: {} cr\n{}", project, self.credits, self.message)
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn cycle_selected_contract(&mut self, reverse: bool) {
        if self.accepted_contracts.is_empty() {
            self.selected_contract_id = None;
            self.message = "No active contract to select".to_string();
            return;
        }
        let current = self.selected_contract_index().unwrap_or(0);
        let next = if reverse {
            if current == 0 {
                self.accepted_contracts.len() - 1
            } else {
                current - 1
            }
        } else {
            (current + 1) % self.accepted_contracts.len()
        };
        self.selected_contract_id = Some(self.accepted_contracts[next].offer.id);
        self.message = format!("Selected contract #{}", self.accepted_contracts[next].offer.id);
        self.mark_dirty();
    }

    pub fn queue_selected_analysis(&mut self, jobs: &mut JobManager, design: &DesignSession) {
        let Some(index) = self.selected_contract_index() else {
            self.message = "Accept a contract before requesting a project analysis".to_string();
            return;
        };

        let (contract_id, phase, snapshot) = {
            let active = &self.accepted_contracts[index];
            (active.offer.id, active.phase, active.snapshot.clone())
        };
        let Some(snapshot) = snapshot else {
            self.message = "Submit a compliant design with Ctrl+Enter in the workshop first".to_string();
            return;
        };
        if !matches!(phase, ContractPhase::DesignSubmitted | ContractPhase::AnalysisFailed) {
            self.message = format!("Analysis unavailable while contract is {}", phase.label());
            return;
        }
        if snapshot.fingerprint != assembly_fingerprint(&design.assembly)
            || snapshot.profile_revision != self.profile_revision
        {
            self.accepted_contracts[index].phase = ContractPhase::NeedsRevision;
            self.accepted_contracts[index].analysis_job_id = None;
            self.message = "Design or instrument profile changed. Submit the design again.".to_string();
            self.mark_dirty();
            return;
        }

        let job_id = jobs.enqueue_contract_thermal_demo(contract_id);
        self.accepted_contracts[index].analysis_job_id = Some(job_id);
        self.accepted_contracts[index].phase = ContractPhase::AnalysisQueued;
        self.message = format!("Thermal qualification queued as laboratory job #{job_id}");
        self.mark_dirty();
    }

    pub fn run_selected_field_test(&mut self, design: &DesignSession) {
        let Some(index) = self.selected_contract_index() else {
            self.message = "No active contract selected".to_string();
            return;
        };
        let (phase, offer, snapshot) = {
            let active = &self.accepted_contracts[index];
            (active.phase, active.offer.clone(), active.snapshot.clone())
        };
        if phase != ContractPhase::AnalysisPassed {
            self.message = "A successful laboratory analysis is required before the field test".to_string();
            return;
        }
        let Some(snapshot) = snapshot else {
            self.message = "No submitted design snapshot exists".to_string();
            return;
        };
        if snapshot.fingerprint != assembly_fingerprint(&design.assembly)
            || snapshot.profile_revision != self.profile_revision
        {
            self.accepted_contracts[index].phase = ContractPhase::NeedsRevision;
            self.accepted_contracts[index].analysis_job_id = None;
            self.message = "The design changed after analysis. Submit it again and repeat the analysis.".to_string();
            self.mark_dirty();
            return;
        }

        let profile = self.instrument_profile.clone();
        let test_seed = u64::from(offer.id)
            ^ snapshot.design_revision.rotate_left(17)
            ^ snapshot.profile_revision.rotate_left(31)
            ^ snapshot
                .fingerprint
                .bytes()
                .fold(0_u64, |acc, byte| acc.wrapping_mul(131).wrapping_add(u64::from(byte)));
        let mut rng = ContractRng::new(test_seed);
        let structural_penalty = snapshot.part_count.saturating_sub(8).min(4) as u8;
        let uptime_probability = profile.uptime_percent.saturating_sub(structural_penalty);
        let mut uptime_samples = 0_u32;
        let mut complete_samples = 0_u32;
        for _ in 0..1_000 {
            if rng.range(0, 99) < u32::from(uptime_probability) {
                uptime_samples += 1;
            }
            if rng.range(0, 99) < u32::from(profile.data_completeness_percent) {
                complete_samples += 1;
            }
        }
        let measured_uptime = f64::from(uptime_samples) / 10.0;
        let measured_data = f64::from(complete_samples) / 10.0;
        let thermal_passed = profile.operating_min_c <= offer.requirements.temperature_min_c
            && profile.operating_max_c >= offer.requirements.temperature_max_c;
        let passed = measured_uptime >= f64::from(offer.requirements.min_uptime_percent)
            && measured_data >= f64::from(offer.requirements.min_data_completeness_percent)
            && thermal_passed;

        let mut report = FieldTestReport {
            contract_id: offer.id,
            created_unix_ms: unix_ms(),
            design_fingerprint: snapshot.fingerprint.clone(),
            random_seed: test_seed,
            instrument_profile: profile.clone(),
            sample_count: 1_000,
            measured_uptime_percent: measured_uptime,
            measured_data_completeness_percent: measured_data,
            thermal_envelope_passed: thermal_passed,
            passed,
            report_path: None,
        };
        if let Err(error) = write_field_test_report(&mut report) {
            self.message = format!("Field test ran, but report could not be saved: {error}");
        }

        let design_hash = ContentHash(snapshot.fingerprint.clone());
        let active = &mut self.accepted_contracts[index];
        let run_id = verification::next_run_id(&active.evidence);
        active
            .evidence
            .retain(|e| e.requirement_id != "UPTIME" && e.requirement_id != "DATA");
        active
            .evidence
            .extend(verification::field_test_evidence(&report, &design_hash, run_id));
        active.phase = if passed {
            ContractPhase::TestPassed
        } else {
            ContractPhase::TestFailed
        };
        active.test_report = Some(report.clone());
        active.last_issues.clear();
        if measured_uptime < f64::from(offer.requirements.min_uptime_percent) {
            active.last_issues.push(format!(
                "Measured uptime {:.1}% is below the required {}%",
                measured_uptime, offer.requirements.min_uptime_percent
            ));
        }
        if measured_data < f64::from(offer.requirements.min_data_completeness_percent) {
            active.last_issues.push(format!(
                "Measured data completeness {:.1}% is below the required {}%",
                measured_data, offer.requirements.min_data_completeness_percent
            ));
        }
        if !thermal_passed {
            active
                .last_issues
                .push("Instrument operating-temperature envelope is insufficient".to_string());
        }
        self.message = if passed {
            format!(
                "Contract #{} passed its 1,000-sample field test. Return to the contract office to deliver it.",
                offer.id
            )
        } else {
            format!(
                "Contract #{} failed its field test. Adjust the profile and resubmit the design.",
                offer.id
            )
        };
        self.mark_dirty();
    }

    fn submit_design(&mut self, design: &DesignSession) {
        let Some(index) = self.selected_contract_index() else {
            self.message = "Accept and select a contract before submitting a design".to_string();
            return;
        };
        let offer = self.accepted_contracts[index].offer.clone();
        let (snapshot, issues) = evaluate_design(
            &offer,
            &design.assembly,
            &self.instrument_profile,
            design.revision,
            self.profile_revision,
        );
        let passed = issues.is_empty();
        let design_hash = ContentHash(snapshot.fingerprint.clone());
        let evidence =
            verification::analysis_evidence(&offer, &design.assembly, &self.instrument_profile, &design_hash, 1)
                .unwrap_or_default();
        let active = &mut self.accepted_contracts[index];
        active.evidence = evidence;
        active.snapshot = Some(snapshot);
        active.analysis_job_id = None;
        active.test_report = None;
        active.last_issues = issues.clone();
        active.phase = if passed {
            ContractPhase::DesignSubmitted
        } else {
            ContractPhase::NeedsRevision
        };
        self.message = if passed {
            format!("Design for contract #{} accepted for scientific analysis", offer.id)
        } else {
            format!("Design for contract #{} needs {} correction(s)", offer.id, issues.len())
        };
        self.mark_dirty();
    }

    fn complete_selected_contract(&mut self, design: &DesignSession) {
        let Some(index) = self.selected_contract_index() else {
            self.message = "No active contract selected".to_string();
            return;
        };
        let (phase, offer, snapshot) = {
            let active = &self.accepted_contracts[index];
            (active.phase, active.offer.clone(), active.snapshot.clone())
        };
        if phase != ContractPhase::TestPassed {
            self.message = "Delivery requires a passed field test".to_string();
            return;
        }
        let Some(snapshot) = snapshot else {
            self.message = "No valid project snapshot available".to_string();
            return;
        };
        if snapshot.fingerprint != assembly_fingerprint(&design.assembly)
            || snapshot.profile_revision != self.profile_revision
        {
            self.accepted_contracts[index].phase = ContractPhase::NeedsRevision;
            self.accepted_contracts[index].analysis_job_id = None;
            self.message = "The design changed after the field test. Submit, analyze and test it again.".to_string();
            self.mark_dirty();
            return;
        }
        let design_hash = ContentHash(snapshot.fingerprint.clone());
        if self.accepted_contracts[index].evidence.is_empty() {
            // Campaign saved before evidence was stored: rebuild it from the current state.
            let rebuilt = verification::rebuild_evidence(
                &self.accepted_contracts[index],
                &design.assembly,
                &self.instrument_profile,
                &design_hash,
            );
            self.accepted_contracts[index].evidence = rebuilt;
        }
        let dossier = verification::dossier_for(&self.accepted_contracts[index], &design_hash);
        if let DossierDecision::Rejected { reasons } = &dossier.decision {
            self.message = format!("Dossier rejected: {}", reasons.join("; "));
            self.mark_dirty();
            return;
        }
        let dossier_score_percent = Some((dossier.score * 100.0).round().clamp(0.0, 100.0) as u8);
        let production_cost = snapshot
            .estimated_unit_cost_cr
            .saturating_mul(u64::from(offer.requirements.required_units));
        if production_cost > self.credits {
            self.message = format!(
                "Insufficient funds: production requires {production_cost} cr, available {} cr",
                self.credits
            );
            return;
        }

        self.credits -= production_cost;
        self.credits = self.credits.saturating_add(offer.reward);
        let profit = offer.reward as i128 - production_cost as i128;
        self.completed_contracts.push(CompletedContract {
            id: offer.id,
            title: offer.title.clone(),
            client: offer.client.clone(),
            production_cost_cr: production_cost,
            payout_cr: offer.reward,
            profit_cr: profit.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64,
            completed_unix_ms: unix_ms(),
            dossier_score_percent,
        });
        if self.completed_contracts.len() > COMPLETED_HISTORY_LIMIT {
            let excess = self.completed_contracts.len() - COMPLETED_HISTORY_LIMIT;
            self.completed_contracts.drain(0..excess);
        }
        self.accepted_contracts.remove(index);
        self.selected_contract_id = self.accepted_contracts.first().map(|item| item.offer.id);
        self.message = format!(
            "Delivered contract #{}: production cost {} cr, client payment {} cr, net result {} cr",
            offer.id, production_cost, offer.reward, profit
        );
        self.mark_dirty();
    }

    pub(crate) fn ui_start_research(&mut self, index: usize) {
        let tree = omega_rules::default_tree();
        let Some(tech) = tree.get(index) else { return };
        self.message = match self.research.start(&tree, &tech.id, self.credits) {
            Ok(cost) => {
                self.credits -= cost;
                self.mark_dirty();
                format!(
                    "Research started: {} ({} days, {} cr)",
                    tech.label, tech.duration_days, cost
                )
            }
            Err(error) => format!("Cannot research {}: {error}", tech.label),
        };
    }

    pub(crate) fn ui_next_day(&mut self) {
        self.day += 1;
        let finished = self.research.advance_days(1);
        let tree = omega_rules::default_tree();
        self.message = match finished.first().and_then(|id| tree.iter().find(|t| &t.id == id)) {
            Some(tech) => format!("Day {}: research completed - {}", self.day, tech.label),
            None => format!("Day {}", self.day),
        };
        self.mark_dirty();
    }

    pub(crate) fn owned_capabilities(&self) -> std::collections::BTreeSet<String> {
        self.research.owned_capabilities(&omega_rules::default_tree())
    }

    pub(crate) fn ui_navigate(&mut self, view: CampusView) {
        self.view = view;
        if view != CampusView::Construction {
            self.profile_editor_open = false;
        }
        self.message = match view {
            CampusView::Island => "Returned to the research island".to_string(),
            CampusView::Contracts => "Opened the contract office".to_string(),
            CampusView::Construction => "Opened the legacy assembly bench".to_string(),
            CampusView::Missile => "Opened the missile workshop".to_string(),
            CampusView::Program => "Opened the missile program editor".to_string(),
            CampusView::Trials => "Opened the trial range".to_string(),
            CampusView::Research => "Opened the research centre".to_string(),
            CampusView::Programming => "Opened the programming studio".to_string(),
            CampusView::Computation => "Opened the computation laboratory".to_string(),
        };
    }

    pub(crate) fn ui_refresh_contracts(&mut self) {
        refresh_contract_board(self);
    }

    pub(crate) fn ui_accept_offer(&mut self, index: usize) {
        accept_contract(self, index);
    }

    pub(crate) fn ui_select_contract(&mut self, id: u32) {
        if self.accepted_contracts.iter().any(|active| active.offer.id == id) {
            self.selected_contract_id = Some(id);
            self.mark_dirty();
        }
    }

    pub(crate) fn ui_submit_design(&mut self, design: &DesignSession) {
        self.submit_design(design);
    }

    pub(crate) fn ui_deliver_selected(&mut self, design: &DesignSession) {
        self.complete_selected_contract(design);
    }

    fn edit_instrument_profile(&mut self, keys: &ButtonInput<KeyCode>) {
        if keys.just_pressed(KeyCode::F11) {
            self.profile_editor_open = false;
            self.message = "Instrument profile editor closed".to_string();
            return;
        }

        let profile = &mut self.instrument_profile;
        let mut changed = true;
        match () {
            _ if keys.just_pressed(KeyCode::F1) => profile.power_draw_w = profile.power_draw_w.saturating_sub(2).max(2),
            _ if keys.just_pressed(KeyCode::F2) => {
                profile.power_draw_w = profile.power_draw_w.saturating_add(2).min(250)
            }
            _ if keys.just_pressed(KeyCode::F3) => profile.storage_gb = profile.storage_gb.saturating_sub(16).max(16),
            _ if keys.just_pressed(KeyCode::F4) => {
                profile.storage_gb = profile.storage_gb.saturating_add(16).min(2_048)
            }
            _ if keys.just_pressed(KeyCode::F5) => {
                profile.uptime_percent = profile.uptime_percent.saturating_sub(1).max(80)
            }
            _ if keys.just_pressed(KeyCode::F7) => {
                profile.uptime_percent = profile.uptime_percent.saturating_add(1).min(100)
            }
            _ if keys.just_pressed(KeyCode::F8) => {
                profile.data_completeness_percent = profile.data_completeness_percent.saturating_sub(1).max(80)
            }
            _ if keys.just_pressed(KeyCode::F9) => {
                profile.data_completeness_percent = profile.data_completeness_percent.saturating_add(1).min(100)
            }
            _ if keys.just_pressed(KeyCode::F10) => {
                (profile.operating_min_c, profile.operating_max_c) =
                    match (profile.operating_min_c, profile.operating_max_c) {
                        (-55, 80) => (-40, 70),
                        (-40, 70) => (-30, 60),
                        (-30, 60) => (-60, 100),
                        _ => (-55, 80),
                    };
            }
            _ => changed = false,
        }
        if changed {
            self.profile_revision = self.profile_revision.saturating_add(1);
            self.message = "Instrument profile changed. Resubmit the design before continuing.".to_string();
            self.mark_dirty();
        }
    }
}

/// Unit cost model shared by design evaluation and verification evidence.
fn estimated_unit_cost_cr(mass_kg: f64, part_count: usize, profile: &InstrumentProfile) -> u64 {
    (1_600.0
        + mass_kg.max(0.0) * 85.0
        + part_count as f64 * 120.0
        + f64::from(profile.power_draw_w) * 16.0
        + f64::from(profile.storage_gb) * 4.0)
        .ceil() as u64
}

fn evaluate_design(
    offer: &ContractOffer,
    assembly: &Assembly,
    profile: &InstrumentProfile,
    design_revision: u64,
    profile_revision: u64,
) -> (ProjectSnapshot, Vec<String>) {
    let mut issues = Vec::new();
    if let Err(validation_issues) = assembly.validate() {
        issues.extend(validation_issues);
    }
    if assembly.parts.is_empty() {
        issues.push("Assembly must contain at least one component".to_string());
    }

    let mass_kg = assembly.estimated_mass_kg();
    let part_count = assembly.parts.len();
    let connection_count = assembly.connections.len();
    let estimated_unit_cost_cr = estimated_unit_cost_cr(mass_kg, part_count, profile);
    let requirements = offer.requirements;

    if mass_kg > f64::from(requirements.max_unit_mass_kg) {
        issues.push(format!(
            "Unit mass {:.2} kg exceeds the {} kg limit",
            mass_kg, requirements.max_unit_mass_kg
        ));
    }
    if profile.power_draw_w > requirements.max_unit_power_w {
        issues.push(format!(
            "Power draw {} W exceeds the {} W limit",
            profile.power_draw_w, requirements.max_unit_power_w
        ));
    }
    if profile.operating_min_c > requirements.temperature_min_c
        || profile.operating_max_c < requirements.temperature_max_c
    {
        issues.push(format!(
            "Operating envelope {}..{} C does not cover {}..{} C",
            profile.operating_min_c,
            profile.operating_max_c,
            requirements.temperature_min_c,
            requirements.temperature_max_c
        ));
    }
    if profile.uptime_percent < requirements.min_uptime_percent {
        issues.push(format!(
            "Uptime {}% is below {}%",
            profile.uptime_percent, requirements.min_uptime_percent
        ));
    }
    if profile.data_completeness_percent < requirements.min_data_completeness_percent {
        issues.push(format!(
            "Data completeness {}% is below {}%",
            profile.data_completeness_percent, requirements.min_data_completeness_percent
        ));
    }
    if profile.storage_gb < requirements.min_storage_gb {
        issues.push(format!(
            "Storage {} GB is below {} GB",
            profile.storage_gb, requirements.min_storage_gb
        ));
    }
    if estimated_unit_cost_cr > requirements.max_unit_cost_cr {
        issues.push(format!(
            "Estimated unit cost {estimated_unit_cost_cr} cr exceeds the {} cr limit",
            requirements.max_unit_cost_cr
        ));
    }
    if let Some(limit) = requirements.max_cg_offset_ratio {
        check_balance(assembly, limit, &mut issues);
    }
    verification::thin_margin_issues(offer, assembly, profile, &mut issues);

    let fingerprint = assembly_fingerprint(assembly);
    (
        ProjectSnapshot {
            design_revision,
            profile_revision,
            mass_kg,
            part_count,
            connection_count,
            estimated_unit_cost_cr,
            fingerprint,
        },
        issues,
    )
}

fn assembly_fingerprint(assembly: &Assembly) -> String {
    let bytes = serde_json::to_vec(assembly).unwrap_or_default();
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |state, byte| {
        (state ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01B3)
    });
    format!("{hash:016x}")
}

fn load_campaign() -> Option<CampaignSave> {
    let bytes = match crate::save::read(crate::save::SaveFile::Campaign) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return None,
        Err(error) => {
            warn!("Could not read the campaign save: {error}");
            return None;
        }
    };
    let save: CampaignSave = serde_json::from_slice(&bytes).ok()?;
    (save.schema_version == CAMPAIGN_SCHEMA).then_some(save)
}

fn write_field_test_report(report: &mut FieldTestReport) -> Result<std::path::PathBuf, String> {
    let directory = Path::new("results");
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let path = directory.join(format!(
        "contract_test_{:04}_{}.json",
        report.contract_id, report.created_unix_ms
    ));
    report.report_path = Some(path.display().to_string());
    let json = serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?;
    fs::write(&path, json).map_err(|error| error.to_string())?;
    Ok(path)
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

#[derive(Component)]
pub struct CampusHud;

pub fn setup_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("OMEGA | ISLAND\nSelect a facility"),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::srgb(0.93, 0.96, 0.98)),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(18.0),
            bottom: Val::Px(18.0),
            width: Val::Px(660.0),
            max_height: Val::Percent(82.0),
            padding: UiRect::all(Val::Px(14.0)),
            overflow: Overflow::scroll_y(),
            ..default()
        },
        UiInteraction::None,
        ScrollPosition::default(),
        BackgroundColor(Color::srgba(0.018, 0.032, 0.047, 0.94)),
        CampusHud,
    ));
}

pub fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    buildings: Query<(&GlobalTransform, &BuildingVisual)>,
    mut campus: ResMut<CampusState>,
    design: Res<DesignSession>,
    sequence: Res<SequenceWorkspace>,
    sketch: Res<PolygonSketchSession>,
    missile: Res<crate::missile::MissileSession>,
    program: Res<crate::program_editor::MissileProgramSession>,
    mut procurement: ResMut<crate::procurement::Procurement>,
    ui_buttons: Query<&UiInteraction>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        if campus.view == CampusView::Construction && sketch.active {
            return;
        }
        if campus.profile_editor_open {
            campus.profile_editor_open = false;
            campus.message = "Instrument profile editor closed".to_string();
            return;
        }
        if campus.view == CampusView::Programming && sequence.graph_is_open() {
            return;
        }
        if campus.view != CampusView::Island {
            campus.view = CampusView::Island;
            campus.message = "Returned to the island".to_string();
        }
        return;
    }

    match campus.view {
        CampusView::Island => {
            if keys.just_pressed(KeyCode::F1) {
                enter_facility(&mut campus, FacilityKind::Contracts);
                return;
            }
            if keys.just_pressed(KeyCode::F2) {
                enter_facility(&mut campus, FacilityKind::Construction);
                return;
            }
            if keys.just_pressed(KeyCode::F3) {
                enter_facility(&mut campus, FacilityKind::Research);
                return;
            }
            if keys.just_pressed(KeyCode::F4) {
                enter_facility(&mut campus, FacilityKind::Programming);
                return;
            }
            if keys.just_pressed(KeyCode::F5) {
                enter_facility(&mut campus, FacilityKind::Computation);
                return;
            }

            if !mouse.just_pressed(MouseButton::Left)
                || sequence.graph_is_open()
                || ui_buttons.iter().any(|interaction| *interaction != UiInteraction::None)
            {
                return;
            }
            let Some(cursor) = window.cursor_position() else {
                return;
            };
            let (camera, camera_transform) = *camera;
            let Ok(ray) = camera.viewport_to_world(camera_transform, cursor) else {
                return;
            };
            let origin = ray.origin;
            let direction = ray.direction.as_vec3();
            let mut closest: Option<(f32, FacilityKind)> = None;
            for (transform, building) in &buildings {
                let inverse = transform.to_matrix().inverse();
                let local_origin = inverse.transform_point3(origin);
                let local_direction = inverse.transform_vector3(direction);
                if let Some(distance) = ray_aabb(
                    local_origin,
                    local_direction,
                    -building.half_extents,
                    building.half_extents,
                ) && closest.as_ref().is_none_or(|(best, _)| distance < *best)
                {
                    closest = Some((distance, building.facility));
                }
            }
            if let Some((_, facility)) = closest {
                enter_facility(&mut campus, facility);
            }
        }
        CampusView::Contracts => {
            if !campus.civil_legacy_open {
                if keys.just_pressed(KeyCode::KeyV) {
                    deliver_missile_contract(&mut campus, &mut procurement, &missile.vehicle, &program.program);
                    return;
                }
                for (key, index) in [
                    (KeyCode::Digit1, 0),
                    (KeyCode::Digit2, 1),
                    (KeyCode::Digit3, 2),
                    (KeyCode::Digit4, 3),
                ] {
                    if keys.just_pressed(key) {
                        accept_missile_offer(&mut campus, &mut procurement, index);
                        return;
                    }
                }
                return;
            }
            if keys.just_pressed(KeyCode::KeyN) {
                refresh_contract_board(&mut campus);
                return;
            }
            if keys.just_pressed(KeyCode::Tab) {
                let reverse = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
                campus.cycle_selected_contract(reverse);
                return;
            }
            if keys.just_pressed(KeyCode::KeyV) {
                campus.complete_selected_contract(&design);
                return;
            }
            for (key, index) in [
                (KeyCode::Digit1, 0),
                (KeyCode::Digit2, 1),
                (KeyCode::Digit3, 2),
                (KeyCode::Digit4, 3),
                (KeyCode::Digit5, 4),
            ] {
                if keys.just_pressed(key) {
                    accept_contract(&mut campus, index);
                    return;
                }
            }
        }
        CampusView::Construction => {
            if sequence.graph_is_open() || sketch.active {
                return;
            }
            if campus.profile_editor_open {
                campus.edit_instrument_profile(&keys);
                return;
            }
            if keys.just_pressed(KeyCode::F11) {
                campus.profile_editor_open = true;
                campus.message = "Instrument profile editor opened".to_string();
                return;
            }
            let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
            if control && keys.just_pressed(KeyCode::Tab) {
                let reverse = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
                campus.cycle_selected_contract(reverse);
                return;
            }
            if control && keys.just_pressed(KeyCode::Enter) {
                campus.submit_design(&design);
            }
        }
        CampusView::Research
        | CampusView::Programming
        | CampusView::Computation
        | CampusView::Missile
        | CampusView::Program
        | CampusView::Trials => {}
    }
}

pub fn handle_computation_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut campus: ResMut<CampusState>,
    mut jobs: ResMut<JobManager>,
    design: Res<DesignSession>,
    sketch: Res<PolygonSketchSession>,
) {
    if campus.view != CampusView::Computation {
        return;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        if campus.selected_contract_id.is_some() {
            campus.queue_selected_analysis(&mut jobs, &design);
        } else {
            let id = jobs.enqueue_thermal_demo();
            info!("Generic thermal research job #{id} queued.");
        }
    }
    if keys.just_pressed(KeyCode::KeyG) {
        let id = jobs.enqueue_truss_demo();
        info!("Civil structural visualization job #{id} queued.");
    }
    if keys.just_pressed(KeyCode::KeyP) {
        match build_polygon_sketch_thermal_config(&sketch.document.regions) {
            Ok(config) => {
                let region_count = sketch.document.regions.len();
                let id = jobs.enqueue_mesh_thermal(config);
                campus.message = format!(
                    "Polygon sketch study #{id} queued from {region_count} region(s). Results will be written to results/."
                );
                info!("Polygon sketch thermal study #{id} queued.");
            }
            Err(message) => {
                campus.message = format!("Polygon sketch could not be meshed: {message}");
                warn!("Polygon sketch meshing failed: {message}");
            }
        }
    }
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if keys.just_pressed(KeyCode::KeyM) {
        match build_assembly_mesh_thermal_config(&design.assembly, 0.05) {
            Ok(mut built) => {
                let meshed_count = built.meshed_part_ids.len();
                let skipped_count = built.skipped_part_ids.len();
                let mut refinement_summary = String::new();
                if shift {
                    let min_x = built
                        .config
                        .mesh
                        .nodes
                        .iter()
                        .map(|node| node.position_m[0])
                        .fold(f64::INFINITY, f64::min);
                    let max_x = built
                        .config
                        .mesh
                        .nodes
                        .iter()
                        .map(|node| node.position_m[0])
                        .fold(f64::NEG_INFINITY, f64::max);
                    let min_y = built
                        .config
                        .mesh
                        .nodes
                        .iter()
                        .map(|node| node.position_m[1])
                        .fold(f64::INFINITY, f64::min);
                    let max_y = built
                        .config
                        .mesh
                        .nodes
                        .iter()
                        .map(|node| node.position_m[1])
                        .fold(f64::NEG_INFINITY, f64::max);
                    let centre = [0.5 * (min_x + max_x), 0.5 * (min_y + max_y)];
                    let half_extent = [0.20 * (max_x - min_x), 0.20 * (max_y - min_y)];
                    let target = 0.025;
                    match refine_tri_mesh_local(
                        &built.config.mesh,
                        [centre[0] - half_extent[0], centre[1] - half_extent[1]],
                        [centre[0] + half_extent[0], centre[1] + half_extent[1]],
                        target,
                        12,
                    ) {
                        Ok(refinement) => {
                            refinement_summary = format!(
                                "; local refinement {} rounds, {} triangles, min angle {:.1} deg",
                                refinement.rounds,
                                refinement.stats.triangle_count,
                                refinement.quality.minimum_angle_deg,
                            );
                            built.config.mesh = refinement.mesh;
                        }
                        Err(message) => {
                            campus.message = format!("Local mesh refinement failed: {message}");
                            warn!("Local mesh refinement failed: {message}");
                            return;
                        }
                    }
                }
                let node_count = built.config.mesh.nodes.len();
                let triangle_count = built.config.mesh.triangles.len();
                let id = jobs.enqueue_mesh_thermal(built.config);
                let study_kind = if shift { "locally refined" } else { "standard" };
                campus.message = format!(
                    "Queued {study_kind} workshop mesh study #{id} on Y={:.3} m: {meshed_count} planar parts, {node_count} nodes, {triangle_count} triangles, {skipped_count} unsupported parts skipped{refinement_summary}.",
                    built.workshop_plane_y_m
                );
                info!("Workshop {study_kind} geometry meshing job #{id} queued.");
            }
            Err(message) => {
                campus.message = format!("Cannot mesh this assembly: {message}");
                warn!("Workshop geometry meshing failed: {message}");
            }
        }
    }
    if keys.just_pressed(KeyCode::KeyO) {
        match jobs.open_latest_report() {
            Ok(()) => info!("Opened latest scientific visualization report."),
            Err(message) => warn!("{message}"),
        }
    }
    if keys.just_pressed(KeyCode::KeyT) {
        campus.run_selected_field_test(&design);
    }
    if keys.just_pressed(KeyCode::KeyC) {
        match jobs.cancel_latest() {
            Some(id) => info!("Cancellation requested for research job #{id}."),
            None => info!("No queued or running research job to cancel."),
        }
    }
}

pub fn sync_jobs(mut campus: ResMut<CampusState>, jobs: Res<JobManager>) {
    let mut changes = Vec::new();
    for record in &jobs.records {
        let Some(contract_id) = record.linked_contract_id else {
            continue;
        };
        let Some(active) = campus
            .accepted_contracts
            .iter()
            .find(|item| item.offer.id == contract_id)
        else {
            continue;
        };
        if active.analysis_job_id != Some(record.id) || active.phase != ContractPhase::AnalysisQueued {
            continue;
        }
        match &record.state {
            JobState::Completed => changes.push((
                contract_id,
                ContractPhase::AnalysisPassed,
                "Laboratory material-screening calculation completed",
            )),
            JobState::Failed | JobState::Cancelled => changes.push((
                contract_id,
                ContractPhase::AnalysisFailed,
                "Laboratory analysis failed or was cancelled",
            )),
            JobState::Queued | JobState::Running | JobState::Cancelling => {}
        }
    }
    for (contract_id, phase, message) in changes {
        if let Some(active) = campus
            .accepted_contracts
            .iter_mut()
            .find(|item| item.offer.id == contract_id)
        {
            active.phase = phase;
        }
        campus.message = format!("Contract #{contract_id}: {message}");
        campus.mark_dirty();
    }
}

pub fn persist_campaign(mut campus: ResMut<CampusState>) {
    if !campus.dirty {
        return;
    }
    let save = CampaignSave {
        schema_version: CAMPAIGN_SCHEMA,
        credits: campus.credits,
        available_contracts: campus.available_contracts.clone(),
        accepted_contracts: campus.accepted_contracts.clone(),
        completed_contracts: campus.completed_contracts.clone(),
        next_contract_id: campus.next_contract_id,
        rng: campus.rng,
        board_refreshes: campus.board_refreshes,
        instrument_profile: campus.instrument_profile.clone(),
        profile_revision: campus.profile_revision,
        selected_contract_id: campus.selected_contract_id,
        research: campus.research.clone(),
        day: campus.day,
    };
    let result = serde_json::to_vec_pretty(&save)
        .map_err(|error| error.to_string())
        .and_then(|bytes| crate::save::write(crate::save::SaveFile::Campaign, &bytes));
    match result {
        Ok(()) => campus.dirty = false,
        Err(error) => warn!("Could not save campaign: {error}"),
    }
}

fn enter_facility(campus: &mut CampusState, facility: FacilityKind) {
    campus.view = match facility {
        FacilityKind::Contracts => CampusView::Contracts,
        FacilityKind::Construction => CampusView::Missile,
        FacilityKind::Research => CampusView::Research,
        FacilityKind::Programming => CampusView::Program,
        FacilityKind::Computation => CampusView::Computation,
    };
    campus.message = format!("Entered {}", facility.label());
}

/// Accept an offer from the missile contract office. Shared by the board and the number keys.
pub(crate) fn accept_missile_offer(
    campus: &mut CampusState,
    procurement: &mut crate::procurement::Procurement,
    index: usize,
) {
    campus.message = procurement.accept(index, campus.day);
}

/// Deliver the active missile contract and credit the reward. Refused dossiers keep the contract.
pub(crate) fn deliver_missile_contract(
    campus: &mut CampusState,
    procurement: &mut crate::procurement::Procurement,
    vehicle: &omega_rules::Vehicle,
    program: &omega_rules::Program,
) {
    match procurement.deliver(campus.day, vehicle, program) {
        Ok(delivery) => {
            campus.credits = campus.credits.saturating_add(delivery.reward);
            campus.message = format!(
                "Contract #{} delivered: +{} cr ({}% dossier)",
                delivery.id, delivery.reward, delivery.score_percent
            );
            campus.mark_dirty();
        }
        Err(message) => campus.message = message,
    }
}

fn refresh_contract_board(campus: &mut CampusState) {
    let new_board = generate_board(&mut campus.rng, &mut campus.next_contract_id);
    campus.available_contracts = new_board;
    campus.board_refreshes = campus.board_refreshes.saturating_add(1);
    campus.message = format!(
        "New randomized board generated (refresh #{}). Active projects were kept.",
        campus.board_refreshes
    );
    campus.mark_dirty();
}

fn accept_contract(campus: &mut CampusState, offer_index: usize) {
    if campus.accepted_contracts.len() >= MAX_ACTIVE_CONTRACTS {
        campus.message = format!("Maximum of {MAX_ACTIVE_CONTRACTS} active contracts reached");
        return;
    }
    if offer_index >= campus.available_contracts.len() {
        campus.message = "No offer at this position. Refresh the board with N.".to_string();
        return;
    }
    let offer = campus.available_contracts.remove(offer_index);
    if campus
        .accepted_contracts
        .iter()
        .any(|active| active.offer.id == offer.id)
    {
        campus.message = format!("Contract #{} is already active", offer.id);
        return;
    }
    let id = offer.id;
    let title = offer.title.clone();
    campus.accepted_contracts.push(ActiveContract {
        offer,
        phase: ContractPhase::Accepted,
        snapshot: None,
        analysis_job_id: None,
        test_report: None,
        last_issues: Vec::new(),
        evidence: Vec::new(),
    });
    if campus.selected_contract_id.is_none() {
        campus.selected_contract_id = Some(id);
    }
    campus.message = format!("Accepted contract #{id}: {title}. Select it in the workshop to begin.");
    campus.mark_dirty();
}

fn format_offer(index: usize, offer: &ContractOffer) -> String {
    let requirements = offer.requirements;
    format!(
        "[{}] #{:04} | {} | difficulty {}/5\n    {} | {}\n    {}\n    Mass <= {} kg; power <= {} W; temperature {}..{} C\n    Uptime >= {}%; data >= {}%; storage >= {} GB; units: {}\n    Balance: CG offset <= {}% of length | unit budget <= {} cr | total payout {} cr | deadline {} days",
        index + 1,
        offer.id,
        offer.domain.label(),
        offer.difficulty,
        offer.title,
        offer.client,
        offer.brief,
        requirements.max_unit_mass_kg,
        requirements.max_unit_power_w,
        requirements.temperature_min_c,
        requirements.temperature_max_c,
        requirements.min_uptime_percent,
        requirements.min_data_completeness_percent,
        requirements.min_storage_gb,
        requirements.required_units,
        requirements
            .max_cg_offset_ratio
            .map_or("n/a".to_string(), |r| format!("{:.0}", r * 100.0)),
        requirements.max_unit_cost_cr,
        offer.reward,
        offer.deadline_days,
    )
}

fn format_active_contract(active: &ActiveContract, selected: bool, current_design: Option<&ContentHash>) -> String {
    let mut lines = vec![format!(
        "{} #{:04} | {} | {}",
        if selected { ">" } else { " " },
        active.offer.id,
        active.offer.title,
        active.phase.label()
    )];
    if let Some(snapshot) = &active.snapshot {
        lines.push(format!(
            "    Design: {:.2} kg, {} components, {} links, ~{} cr/unit | rev {} | hash {}",
            snapshot.mass_kg,
            snapshot.part_count,
            snapshot.connection_count,
            snapshot.estimated_unit_cost_cr,
            snapshot.design_revision,
            snapshot.fingerprint
        ));
    }
    if let Some(report) = &active.test_report {
        lines.push(format!(
            "    Field test: uptime {:.1}%, data {:.1}%, thermal envelope {}, {} samples",
            report.measured_uptime_percent,
            report.measured_data_completeness_percent,
            if report.thermal_envelope_passed { "PASS" } else { "FAIL" },
            report.sample_count
        ));
    }
    for issue in &active.last_issues {
        lines.push(format!("    - {issue}"));
    }
    if selected
        && active.snapshot.is_some()
        && let Some(design) = current_design
    {
        lines.extend(verification::matrix_lines(active, design));
    }
    lines.join("\n")
}

pub fn update_hud(
    campus: Res<CampusState>,
    sketch: Res<PolygonSketchSession>,
    design: Res<DesignSession>,
    mut hud: Query<(&mut Text, &mut Node), With<CampusHud>>,
) {
    let Ok((mut text, mut node)) = hud.single_mut() else {
        return;
    };
    let current_design = assembly_content_hash(&design.assembly).ok();
    // Boards (`campus::board`) now present every view; this text panel only remains for the
    // instrument profile editor.
    let show_profile_text = campus.view == CampusView::Construction && campus.profile_editor_open && !sketch.active;
    node.display = if show_profile_text {
        Display::Flex
    } else {
        Display::None
    };
    if !show_profile_text {
        return;
    }

    text.0 = match campus.view {
        CampusView::Island => format!(
            "OMEGA 0.26.0 | RESEARCH ISLAND\n\nFive dedicated facilities:\n\n[F1] CONTRACT OFFICE\nClient offers, project requirements, deadlines and payments.\n\n[F2] CONSTRUCTION WORKSHOP\n3D assembly editor and component library.\n\n[F3] RESEARCH CENTRE\nTechnology development and unlock progression.\n\n[F4] PROGRAMMING STUDIO\nVisual event-and-data sequence editor.\n\n[F5] COMPUTATION CENTRE\nScientific studies, simulations and reports.\n\nCredits: {} cr | Active contracts: {}/{} | Completed: {}\n\nRight drag: orbit | middle drag: pan | wheel: zoom\nClick a building or use the navigation bar",
            campus.credits,
            campus.accepted_contracts.len(),
            MAX_ACTIVE_CONTRACTS,
            campus.completed_contracts.len()
        ),
        CampusView::Contracts => {
            let mut lines = vec![
                "OMEGA | CONTRACT OFFICE".to_string(),
                format!("Available funds: {} cr", campus.credits),
                "Five randomized civilian/scientific domains; one offer per domain.".to_string(),
                "Accept up to two projects. Payment follows design, analysis and field test.".to_string(),
                String::new(),
                "AVAILABLE OFFERS".to_string(),
            ];
            if campus.available_contracts.is_empty() {
                lines.push("No offers remain. Press N to generate a new board.".to_string());
            } else {
                for (index, offer) in campus.available_contracts.iter().enumerate() {
                    lines.push(format_offer(index, offer));
                    lines.push(String::new());
                }
            }
            lines.push("ACTIVE PROJECTS (Tab / Shift+Tab selects)".to_string());
            if campus.accepted_contracts.is_empty() {
                lines.push("None".to_string());
            } else {
                for active in &campus.accepted_contracts {
                    lines.push(format_active_contract(
                        active,
                        Some(active.offer.id) == campus.selected_contract_id,
                        current_design.as_ref(),
                    ));
                }
            }
            lines.push("\nCOMPLETED CONTRACTS".to_string());
            for completed in campus.completed_contracts.iter().rev().take(3) {
                lines.push(format!(
                    "#{:04} {} | cost {} cr | payout {} cr | net {} cr{}",
                    completed.id,
                    completed.title,
                    completed.production_cost_cr,
                    completed.payout_cr,
                    completed.profit_cr,
                    completed
                        .dossier_score_percent
                        .map_or(String::new(), |score| format!(" | dossier {score}%"))
                ));
            }
            lines.push(String::new());
            lines.push(
                "1-5: accept | Tab: select active | V: deliver passed project | N: refresh | Esc: island".to_string(),
            );
            lines.push(format!("\n{}", campus.message));
            lines.join("\n")
        }
        CampusView::Construction => {
            if campus.profile_editor_open {
                format!(
                    "OMEGA | INSTRUMENT PROFILE\n\nF1/F2: power draw -/+ 2 W\nF3/F4: storage -/+ 16 GB\nF5/F7: uptime -/+ 1%\nF8/F9: data completeness -/+ 1%\nF10: cycle operating-temperature envelope\nF11 or Esc: close profile editor\n\nPower: {} W\nStorage: {} GB\nUptime target: {}%\nData completeness target: {}%\nOperating range: {}..{} C\n\n{}",
                    campus.instrument_profile.power_draw_w,
                    campus.instrument_profile.storage_gb,
                    campus.instrument_profile.uptime_percent,
                    campus.instrument_profile.data_completeness_percent,
                    campus.instrument_profile.operating_min_c,
                    campus.instrument_profile.operating_max_c,
                    campus.message
                )
            } else {
                let selected = campus
                    .selected_contract()
                    .map(|active| format_active_contract(active, true, current_design.as_ref()))
                    .unwrap_or_else(|| "No active contract selected. Accept one at the contract office.".to_string());
                format!(
                    "OMEGA | ASSEMBLY WORKSHOP\n\nCURRENT PROJECT\n{}\n\nCtrl+Tab: switch active project\nCtrl+Enter: evaluate and submit current design\nF11: configure civilian instrument profile\nV: draw polygon surfaces (Enter closes, H adds holes, M changes material)\nCtrl+S: save assembly | Ctrl+L: load assembly\nCtrl+B: record revision | Ctrl+Backspace: restore latest revision\n\nInstrument profile: {} W, {} GB, uptime {}%, data {}%, {}..{} C\n\nA submitted design is invalidated if its assembly or profile changes.\n\n{}",
                    selected,
                    campus.instrument_profile.power_draw_w,
                    campus.instrument_profile.storage_gb,
                    campus.instrument_profile.uptime_percent,
                    campus.instrument_profile.data_completeness_percent,
                    campus.instrument_profile.operating_min_c,
                    campus.instrument_profile.operating_max_c,
                    campus.message
                )
            }
        }
        CampusView::Missile | CampusView::Program => String::new(),
        CampusView::Trials => format!(
            "OMEGA | TRIAL RANGE\n\nDeterministic flight test of the current vehicle and its onboard program.\nFour plots (speed, altitude, mass, thrust), the event timeline and the anomalies that explain a failure.\nSeeded batches (10 / 100 / 1000) and A/B comparison need the Monte Carlo analysis research grant.\n\n{}",
            campus.message
        ),
        CampusView::Research => format!(
            "OMEGA | RESEARCH CENTRE\n\nTECHNOLOGY DEPARTMENTS\n\nVEHICLE FAMILIES\nStructural concepts, configurations and design envelopes.\n\nMATERIALS\nDensity, stiffness, thermal resistance and durability.\n\nSCIENTIFIC SENSORS\nEnvironmental measurement, sampling and data quality.\n\nEMBEDDED COMPUTING\nProcessing capacity, memory and program complexity.\n\nMANUFACTURING\nProcesses, tolerances and production methods.\n\nResearch progression and persistent unlocks are planned for version 0.25.\n\n{}",
            campus.message
        ),
        CampusView::Programming => format!(
            "OMEGA | LEGACY SEQUENCER\n\nVISUAL SEQUENCE EDITOR (0.6 prototype)\nBuild event-driven programs using start, delay, read-data, record, condition and repeat blocks.\n\nF6: open / close the visual editor\nThe sandboxed runtime executes abstract measurement and data-processing sequences only.\nThe missile program editor is the PROGRAM tab.\n\n{}",
            campus.message
        ),
        CampusView::Computation => {
            let selected = campus
                .selected_contract()
                .map(|active| format_active_contract(active, true, current_design.as_ref()))
                .unwrap_or_else(|| {
                    "No active project selected. Generic material-conduction study is available.".to_string()
                });
            format!(
                "OMEGA | COMPUTATION CENTRE\n\nCURRENT PROJECT\n{}\n\nR: generic rectangular thermal study\nG: generic civil truss study\nM: mesh workshop primitives | P: mesh drawn polygon regions and run thermal study\nO: open latest HTML visualization report\nT: perform 1,000-sample field test after analysis\nC: cancel newest active calculation\n\nAll studies run on background CPU workers. Mesh studies use workshop geometry or the saved multi-material polygon sketch, then export a triangular heat map, full CSV data and numerical diagnostics.\n\n{}",
                selected, campus.message
            )
        }
    };
}

fn ray_aabb(origin: Vec3, direction: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let mut near = f32::NEG_INFINITY;
    let mut far = f32::INFINITY;
    for axis in 0..3 {
        let o = origin[axis];
        let d = direction[axis];
        if d.abs() < 1e-6 {
            if o < min[axis] || o > max[axis] {
                return None;
            }
            continue;
        }
        let mut t1 = (min[axis] - o) / d;
        let mut t2 = (max[axis] - o) / d;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        near = near.max(t1);
        far = far.min(t2);
        if near > far {
            return None;
        }
    }
    if far < 0.0 { None } else { Some(near.max(0.0)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deterministic_board(seed: u64) -> Vec<ContractOffer> {
        let mut rng = ContractRng::new(seed);
        let mut next_id = 1;
        generate_board(&mut rng, &mut next_id)
    }

    fn fresh_campus_for_test(seed: u64) -> CampusState {
        let mut rng = ContractRng::new(seed);
        let mut next_contract_id = 1;
        let available_contracts = generate_board(&mut rng, &mut next_contract_id);
        CampusState {
            view: CampusView::Island,
            credits: 25_000,
            available_contracts,
            accepted_contracts: Vec::new(),
            completed_contracts: Vec::new(),
            next_contract_id,
            rng,
            board_refreshes: 0,
            message: String::new(),
            instrument_profile: InstrumentProfile::default(),
            profile_revision: 1,
            selected_contract_id: None,
            profile_editor_open: false,
            research: omega_rules::ResearchState::default(),
            day: 1,
            civil_legacy_open: false,
            dirty: false,
        }
    }

    #[test]
    fn each_board_contains_all_five_safe_contract_domains() {
        let board = deterministic_board(42);
        assert_eq!(board.len(), 5);
        for domain in ContractDomain::ALL {
            assert_eq!(board.iter().filter(|offer| offer.domain == domain).count(), 1);
        }
    }

    #[test]
    fn contract_generation_is_reproducible_for_a_given_seed() {
        let first = deterministic_board(123_456);
        let second = deterministic_board(123_456);
        for (a, b) in first.iter().zip(second.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.domain, b.domain);
            assert_eq!(a.title, b.title);
            assert_eq!(a.reward, b.reward);
            assert_eq!(a.requirements.max_unit_cost_cr, b.requirements.max_unit_cost_cr);
        }
    }

    #[test]
    fn generated_contract_constraints_are_in_sane_ranges() {
        for seed in 1..200 {
            for offer in deterministic_board(seed) {
                let requirements = offer.requirements;
                assert!((1..=5).contains(&offer.difficulty));
                assert!(requirements.max_unit_mass_kg >= 8);
                assert!(requirements.max_unit_power_w >= 8);
                assert!(requirements.temperature_min_c < requirements.temperature_max_c);
                assert!((92..=99).contains(&requirements.min_uptime_percent));
                assert!((93..=99).contains(&requirements.min_data_completeness_percent));
                assert!((1..=8).contains(&requirements.required_units));
                assert!(requirements.max_unit_cost_cr > 0);
                assert!(offer.reward > requirements.max_unit_cost_cr * u64::from(requirements.required_units));
                assert!((20..=60).contains(&offer.deadline_days));
            }
        }
    }

    #[test]
    fn prototype_design_snapshot_uses_physical_mass_and_current_revisions() {
        let offer = deterministic_board(42).remove(0);
        let design = DesignSession::default();
        let profile = InstrumentProfile::default();
        let (snapshot, _) = evaluate_design(&offer, &design.assembly, &profile, design.revision, 1);
        assert!(snapshot.mass_kg > 0.0);
        assert_eq!(snapshot.part_count, design.assembly.parts.len());
        assert_eq!(snapshot.design_revision, design.revision);
        assert_eq!(snapshot.profile_revision, 1);
        assert_eq!(snapshot.fingerprint.len(), 16);
    }

    #[test]
    fn accepting_contracts_removes_offers_and_enforces_two_active_limit() {
        let mut campus = fresh_campus_for_test(777);
        let initial_available = campus.available_contracts.len();
        accept_contract(&mut campus, 0);
        accept_contract(&mut campus, 0);
        assert_eq!(campus.accepted_contracts.len(), 2);
        assert_eq!(campus.available_contracts.len(), initial_available - 2);
        let before = campus.accepted_contracts.len();
        accept_contract(&mut campus, 0);
        assert_eq!(campus.accepted_contracts.len(), before);
    }

    #[test]
    fn refreshing_board_keeps_active_contracts_and_assigns_new_ids() {
        let mut campus = fresh_campus_for_test(1);
        accept_contract(&mut campus, 0);
        let active_id = campus.accepted_contracts[0].offer.id;
        let old_ids: Vec<u32> = campus.available_contracts.iter().map(|offer| offer.id).collect();
        refresh_contract_board(&mut campus);
        assert_eq!(campus.accepted_contracts.len(), 1);
        assert_eq!(campus.accepted_contracts[0].offer.id, active_id);
        assert!(
            campus
                .available_contracts
                .iter()
                .all(|offer| !old_ids.contains(&offer.id))
        );
    }

    #[test]
    fn campaign_save_round_trips_contract_and_profile_data() {
        let offer = deterministic_board(99).remove(0);
        let save = CampaignSave {
            schema_version: CAMPAIGN_SCHEMA,
            credits: 123_456,
            available_contracts: vec![offer],
            accepted_contracts: Vec::new(),
            completed_contracts: Vec::new(),
            next_contract_id: 20,
            rng: ContractRng::new(123),
            board_refreshes: 4,
            instrument_profile: InstrumentProfile::default(),
            profile_revision: 3,
            selected_contract_id: None,
            research: omega_rules::ResearchState::default(),
            day: 1,
        };
        let bytes = serde_json::to_vec(&save).unwrap();
        let loaded: CampaignSave = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(loaded.credits, save.credits);
        assert_eq!(loaded.available_contracts[0].id, save.available_contracts[0].id);
        assert_eq!(loaded.profile_revision, save.profile_revision);
        assert_eq!(loaded.instrument_profile.storage_gb, save.instrument_profile.storage_gb);
    }

    #[test]
    fn ray_aabb_detects_and_rejects_hits() {
        assert_eq!(
            ray_aabb(
                Vec3::new(0.0, 0.0, 5.0),
                Vec3::NEG_Z,
                Vec3::splat(-1.0),
                Vec3::splat(1.0)
            ),
            Some(4.0)
        );
        assert_eq!(
            ray_aabb(
                Vec3::new(3.0, 0.0, 5.0),
                Vec3::NEG_Z,
                Vec3::splat(-1.0),
                Vec3::splat(1.0)
            ),
            None
        );
    }

    fn balance_part(
        id: &str,
        kind: crate::design::PartKind,
        material: crate::design::MaterialKind,
        dims: [f64; 3],
        x: f64,
    ) -> crate::design::Part {
        crate::design::Part {
            id: id.to_string(),
            label: id.to_string(),
            kind,
            dimensions_m: dims,
            position_m: [x, 0.0, 0.0],
            rotation_rad: [0.0; 3],
            material,
        }
    }

    fn balance_assembly(heavy_x: f64) -> Assembly {
        use crate::design::{MaterialKind, PartKind};
        let mut assembly = Assembly::demo();
        assembly.parts = vec![
            balance_part("beam", PartKind::Beam, MaterialKind::Polymer, [4.0, 0.1, 0.1], 0.0),
            balance_part(
                "block",
                PartKind::Enclosure,
                MaterialKind::Steel,
                [0.5, 0.5, 0.5],
                heavy_x,
            ),
        ];
        assembly.connections.clear();
        assembly
    }

    #[test]
    fn balance_requirement_depends_on_the_real_geometry() {
        let mut centred = Vec::new();
        check_balance(&balance_assembly(0.0), 0.10, &mut centred);
        assert!(centred.is_empty(), "{centred:?}");

        let mut shifted = Vec::new();
        check_balance(&balance_assembly(1.5), 0.10, &mut shifted);
        assert_eq!(shifted.len(), 1, "{shifted:?}");
        assert!(shifted[0].contains("Centre of gravity"));
    }

    #[test]
    fn balance_limit_tightens_with_difficulty() {
        assert!(balance_limit_for_difficulty(5) < balance_limit_for_difficulty(1));
        assert!(balance_limit_for_difficulty(5) > 0.0);
    }

    #[test]
    fn pre_022_contract_requirements_still_deserialize() {
        let json = r#"{"max_unit_mass_kg":20,"max_unit_power_w":40,"temperature_min_c":-30,"temperature_max_c":50,
            "min_uptime_percent":95,"min_data_completeness_percent":95,"min_storage_gb":64,
            "required_units":2,"max_unit_cost_cr":9000}"#;
        let requirements: ContractRequirements = serde_json::from_str(json).unwrap();
        assert_eq!(requirements.max_cg_offset_ratio, None);
    }

    #[test]
    fn generated_contracts_carry_a_balance_limit() {
        for offer in deterministic_board(99) {
            assert!(offer.requirements.max_cg_offset_ratio.is_some());
        }
    }
}
