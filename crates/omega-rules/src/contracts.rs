//! Fictional military contract generation and progression primitives.
//!
//! Contract values are abstract game ratings. This module deliberately does not model
//! real-world mission planning, targeting, guidance, or weapon performance.

use crate::dossier::{Dossier, DossierDecision, build_dossier};
use crate::flight::FlightReport;
use crate::requirement::{Comparator, Estimate, Evidence, Requirement, VerificationMethod};
use omega_data::ContentHash;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionFamily {
    AirToAir,
    AirToSurface,
    SurfaceToAir,
    SurfaceToSurface,
}

impl MissionFamily {
    pub const ALL: [Self; 4] = [
        Self::AirToAir,
        Self::AirToSurface,
        Self::SurfaceToAir,
        Self::SurfaceToSurface,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::AirToAir => "AIR-TO-AIR",
            Self::AirToSurface => "AIR-TO-SURFACE",
            Self::SurfaceToAir => "SURFACE-TO-AIR",
            Self::SurfaceToSurface => "SURFACE-TO-SURFACE",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VehicleFamily {
    Ballistic,
    Cruise,
    ShortRange,
}

impl VehicleFamily {
    pub const ALL: [Self; 3] = [Self::Ballistic, Self::Cruise, Self::ShortRange];

    pub fn label(self) -> &'static str {
        match self {
            Self::Ballistic => "BALLISTIC",
            Self::Cruise => "CRUISE",
            Self::ShortRange => "SHORT-RANGE",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractRequirements {
    /// Abstract design-quality score, from 0 to 100.
    pub min_design_score: u8,
    /// Abstract program-quality score, from 0 to 100.
    pub min_program_score: u8,
    /// Abstract trial-quality score, from 0 to 100.
    pub min_trial_score: u8,
    /// Research capability identifiers required by the contract.
    pub required_capabilities: Vec<String>,
}

impl ContractRequirements {
    pub fn validate(&self) -> Result<(), String> {
        for (name, score) in [
            ("design", self.min_design_score),
            ("program", self.min_program_score),
            ("trial", self.min_trial_score),
        ] {
            if score > 100 {
                return Err(format!("{name} score must be between 0 and 100"));
            }
        }
        if self
            .required_capabilities
            .iter()
            .any(|capability| capability.trim().is_empty())
        {
            return Err("required capability identifiers cannot be empty".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractOffer {
    pub id: u32,
    pub mission_family: MissionFamily,
    pub vehicle_family: VehicleFamily,
    pub title: String,
    pub client: String,
    pub brief: String,
    pub requirements: ContractRequirements,
    pub reward_credits: u64,
    pub deadline_days: u16,
    pub difficulty: u8,
}

impl ContractOffer {
    pub fn validate(&self) -> Result<(), String> {
        if self.id == 0 {
            return Err("contract id must be non-zero".to_string());
        }
        if self.title.trim().is_empty() || self.client.trim().is_empty() || self.brief.trim().is_empty() {
            return Err("contract title, client and brief must not be empty".to_string());
        }
        if !(1..=5).contains(&self.difficulty) {
            return Err("contract difficulty must be between 1 and 5".to_string());
        }
        if self.reward_credits == 0 || self.deadline_days == 0 {
            return Err("contract reward and deadline must be non-zero".to_string());
        }
        self.requirements.validate()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractPhase {
    Accepted,
    DesignSubmitted,
    ProgramSubmitted,
    TrialsRequired,
    ReadyForDelivery,
    Delivered,
    NeedsRevision,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractProgress {
    pub contract_id: u32,
    pub phase: ContractPhase,
    pub design_revision: u64,
    pub program_revision: u64,
    pub completed_trial_runs: u32,
}

impl ContractProgress {
    pub fn new(contract_id: u32) -> Result<Self, String> {
        if contract_id == 0 {
            return Err("contract id must be non-zero".to_string());
        }
        Ok(Self {
            contract_id,
            phase: ContractPhase::Accepted,
            design_revision: 0,
            program_revision: 0,
            completed_trial_runs: 0,
        })
    }

    /// Any design or program edit invalidates downstream submissions and trial evidence.
    pub fn invalidate_after_edit(&mut self, design_changed: bool, program_changed: bool) {
        if design_changed {
            self.design_revision = self.design_revision.saturating_add(1);
        }
        if program_changed {
            self.program_revision = self.program_revision.saturating_add(1);
        }
        if design_changed || program_changed {
            self.completed_trial_runs = 0;
            self.phase = ContractPhase::NeedsRevision;
        }
    }

    pub fn record_trial(&mut self) {
        self.completed_trial_runs = self.completed_trial_runs.saturating_add(1);
        self.phase = ContractPhase::ReadyForDelivery;
    }
}

/// Small deterministic generator. It is for reproducible game content, not security.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractRng {
    state: u64,
}

impl ContractRng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    fn range(&mut self, min: u32, max: u32) -> u32 {
        min + (self.next_u64() % u64::from(max - min + 1)) as u32
    }

    fn choose<T: Copy>(&mut self, values: &[T]) -> T {
        values[(self.next_u64() % values.len() as u64) as usize]
    }
}

/// Generate one offer for each mission family, then shuffle the board deterministically.
pub fn generate_contract_board(seed: u64, first_id: u32) -> Vec<ContractOffer> {
    let mut rng = ContractRng::new(seed);
    let mut families = MissionFamily::ALL;
    for index in (1..families.len()).rev() {
        let other = rng.range(0, index as u32) as usize;
        families.swap(index, other);
    }

    families
        .into_iter()
        .enumerate()
        .map(|(index, mission_family)| {
            let difficulty = rng.range(1, 5) as u8;
            let vehicle_family = rng.choose(&VehicleFamily::ALL);
            let d = u64::from(difficulty);
            let title = format!("{} development programme", mission_family.label());
            let client = match mission_family {
                MissionFamily::AirToAir => "Aster Defence Directorate",
                MissionFamily::AirToSurface => "Meridian Strategic Systems",
                MissionFamily::SurfaceToAir => "Northstar Defence Office",
                MissionFamily::SurfaceToSurface => "Orion Procurement Bureau",
            };
            let brief = format!(
                "Submit a validated fictional {} vehicle programme. The review evaluates design completeness, software quality and repeatable simulation evidence.",
                vehicle_family.label().to_lowercase()
            );
            ContractOffer {
                id: first_id.wrapping_add(index as u32).max(1),
                mission_family,
                vehicle_family,
                title,
                client: client.to_string(),
                brief,
                requirements: ContractRequirements {
                    min_design_score: (55 + difficulty * 5).min(95),
                    min_program_score: (50 + difficulty * 6).min(95),
                    min_trial_score: (55 + difficulty * 5).min(95),
                    required_capabilities: if difficulty >= 4 {
                        vec!["research:advanced_validation".to_string()]
                    } else {
                        Vec::new()
                    },
                },
                reward_credits: 4_000 + d * 2_500,
                deadline_days: 35_u16.saturating_sub(u16::from(difficulty) * 3),
                difficulty,
            }
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractEvaluation {
    pub design_passed: bool,
    pub program_passed: bool,
    pub trials_passed: bool,
    pub missing_capabilities: Vec<String>,
    pub score_issues: Vec<String>,
    pub accepted: bool,
}

impl ContractOffer {
    /// Evaluate abstract game scores and unlocked research capabilities against this offer.
    /// This is a contract rubric, not a real-world engineering assessment.
    pub fn evaluate_submission(
        &self,
        design_score: u8,
        program_score: u8,
        trial_score: u8,
        capabilities: &[String],
    ) -> ContractEvaluation {
        let missing_capabilities: Vec<String> = self
            .requirements
            .required_capabilities
            .iter()
            .filter(|required| !capabilities.iter().any(|available| available == *required))
            .cloned()
            .collect();

        let mut score_issues = Vec::new();
        if design_score < self.requirements.min_design_score {
            score_issues.push(format!(
                "design score {design_score} is below {}",
                self.requirements.min_design_score
            ));
        }
        if program_score < self.requirements.min_program_score {
            score_issues.push(format!(
                "program score {program_score} is below {}",
                self.requirements.min_program_score
            ));
        }
        if trial_score < self.requirements.min_trial_score {
            score_issues.push(format!(
                "trial score {trial_score} is below {}",
                self.requirements.min_trial_score
            ));
        }

        let design_passed = design_score >= self.requirements.min_design_score;
        let program_passed = program_score >= self.requirements.min_program_score;
        let trials_passed = trial_score >= self.requirements.min_trial_score;
        let accepted = design_passed && program_passed && trials_passed && missing_capabilities.is_empty();

        ContractEvaluation {
            design_passed,
            program_passed,
            trials_passed,
            missing_capabilities,
            score_issues,
            accepted,
        }
    }
}

/// The measurement one missile trial contributes to a contract dossier. It is bound to the
/// vehicle and program hashes that produced it, so any later edit makes the evidence stale.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MissileTrial {
    pub run_id: u64,
    pub vehicle: ContentHash,
    pub program: ContentHash,
    pub seed: u64,
    pub path_length_m: f64,
    pub max_speed_ms: f64,
    pub launch_mass_kg: f64,
    pub lifted_off: bool,
    pub separated: bool,
    pub lock_acquired: Option<bool>,
}

impl MissileTrial {
    pub fn from_report(
        vehicle: ContentHash,
        program: ContentHash,
        seed: u64,
        run_id: u64,
        report: &FlightReport,
    ) -> Self {
        Self {
            run_id,
            vehicle,
            program,
            seed,
            path_length_m: report.path_length_m,
            max_speed_ms: report.max_speed_ms,
            launch_mass_kg: report.samples.first().map_or(0.0, |sample| sample.mass_kg),
            lifted_off: report.lifted_off,
            separated: report.events.iter().any(|(_, label)| label.contains("separated")),
            lock_acquired: report.lock.map(|lock| lock.success),
        }
    }

    /// One proof per missile requirement, all obtained by a test run. Numeric values carry the
    /// trial's small sampling uncertainty; booleans are exact.
    pub fn evidence(&self) -> Vec<Evidence> {
        let proof = |requirement_id: &str, value: f64, uncertainty: f64| Evidence {
            requirement_id: requirement_id.to_string(),
            method: VerificationMethod::Test,
            run_id: self.run_id,
            design: self.vehicle.clone(),
            program: Some(self.program.clone()),
            estimate: Estimate { value, uncertainty },
        };
        let flag = |yes: bool| if yes { 1.0 } else { 0.0 };
        vec![
            proof("RANGE", self.path_length_m, self.path_length_m * 0.005),
            proof("SPEED", self.max_speed_ms, self.max_speed_ms * 0.005),
            proof("MASS", self.launch_mass_kg, self.launch_mass_kg * 0.01),
            proof("LIFTOFF", flag(self.lifted_off), 0.0),
            proof("SEPARATION", flag(self.separated), 0.0),
            proof("LOCK", flag(self.lock_acquired.unwrap_or(false)), 0.0),
        ]
    }
}

fn missile_requirement(
    id: &str,
    label: &str,
    metric: &str,
    cmp: Comparator,
    limit: f64,
    weight: f32,
    hard: bool,
) -> Requirement {
    Requirement {
        id: id.to_string(),
        label: label.to_string(),
        metric: metric.to_string(),
        cmp,
        limit,
        weight,
        hard,
        allowed_methods: vec![VerificationMethod::Test],
    }
}

impl ContractOffer {
    /// Abstract flying requirements for this offer. Families differ in range, speed and mass, and
    /// an air-to-air or surface-to-air contract also requires a target lock.
    pub fn missile_requirements(&self) -> Vec<Requirement> {
        let d = f64::from(self.difficulty);
        let (range_m, speed_ms, mass_kg) = match self.vehicle_family {
            VehicleFamily::Ballistic => ((26.0 + 4.0 * d) * 1_000.0, 550.0 + 40.0 * d, 700.0),
            VehicleFamily::Cruise => ((30.0 + 5.0 * d) * 1_000.0, 170.0 + 10.0 * d, 420.0),
            VehicleFamily::ShortRange => ((10.0 + 3.0 * d) * 1_000.0, 240.0 + 20.0 * d, 300.0),
        };
        let mut requirements = vec![
            missile_requirement(
                "RANGE",
                "Path length",
                "path_length_m",
                Comparator::AtLeast,
                range_m,
                2.0,
                true,
            ),
            missile_requirement(
                "SPEED",
                "Top speed",
                "max_speed_ms",
                Comparator::AtLeast,
                speed_ms,
                1.0,
                false,
            ),
            missile_requirement(
                "MASS",
                "Launch mass",
                "launch_mass_kg",
                Comparator::AtMost,
                mass_kg,
                1.5,
                true,
            ),
            missile_requirement("LIFTOFF", "Lift-off", "lifted_off", Comparator::AtLeast, 1.0, 2.0, true),
            missile_requirement(
                "SEPARATION",
                "Stage separation",
                "separated",
                Comparator::AtLeast,
                1.0,
                1.0,
                true,
            ),
        ];
        if matches!(
            self.mission_family,
            MissionFamily::AirToAir | MissionFamily::SurfaceToAir
        ) {
            requirements.push(missile_requirement(
                "LOCK",
                "Target lock",
                "lock_acquired",
                Comparator::AtLeast,
                1.0,
                2.0,
                true,
            ));
        }
        requirements
    }

    /// Build the delivery dossier from the trial evidence gathered for this design and program.
    /// `build_dossier` already picks the latest trial matching both hashes and reports `Stale`
    /// when none matches, so an edited vehicle or program invalidates the dossier.
    pub fn missile_dossier(&self, trials: &[MissileTrial], vehicle: &ContentHash, program: &ContentHash) -> Dossier {
        let requirements = self.missile_requirements();
        let evidence: Vec<Evidence> = trials.iter().flat_map(MissileTrial::evidence).collect();
        build_dossier(self.id, &requirements, &evidence, vehicle, Some(program))
    }

    /// The dossier must be accepted and reach the offer's trial-quality score.
    pub fn accepts_dossier(&self, dossier: &Dossier) -> bool {
        matches!(dossier.decision, DossierDecision::Accepted { .. })
            && dossier.score * 100.0 >= f64::from(self.requirements.min_trial_score)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flight::{LockResult, Sample};
    use crate::vehicle::RadarClass;

    fn offer_for(mission_family: MissionFamily, vehicle_family: VehicleFamily) -> ContractOffer {
        let mut offer = generate_contract_board(1, 1).remove(0);
        offer.mission_family = mission_family;
        offer.vehicle_family = vehicle_family;
        offer.difficulty = 1;
        offer.requirements.min_trial_score = 0;
        offer
    }

    fn report(path: f64, speed: f64, mass: f64, lock: bool) -> FlightReport {
        FlightReport {
            samples: vec![Sample {
                t_s: 0.1,
                speed_ms: speed,
                path_m: 0.0,
                altitude_m: 0.0,
                mass_kg: mass,
                thrust_n: 1_000.0,
            }],
            events: vec![(1.0, "Stage 1 separated".to_string())],
            anomalies: Vec::new(),
            first_ignition_twr: Some(2.0),
            lifted_off: true,
            max_speed_ms: speed,
            path_length_m: path,
            final_speed_ms: speed,
            flight_time_s: 100.0,
            lock: Some(LockResult {
                sensor: Some(RadarClass::Basic),
                probability: 0.55,
                success: lock,
                speed_ms: speed,
            }),
        }
    }

    fn trial(vehicle: &ContentHash, program: &ContentHash, report: &FlightReport) -> MissileTrial {
        MissileTrial::from_report(vehicle.clone(), program.clone(), 1, 1, report)
    }

    #[test]
    fn missile_requirements_follow_the_family_and_difficulty() {
        let ballistic = offer_for(MissionFamily::AirToSurface, VehicleFamily::Ballistic);
        let mut hard_ballistic = ballistic.clone();
        hard_ballistic.difficulty = 5;
        let short = offer_for(MissionFamily::AirToSurface, VehicleFamily::ShortRange);
        let id = |offer: &ContractOffer, id: &str| {
            offer
                .missile_requirements()
                .into_iter()
                .find(|requirement| requirement.id == id)
                .unwrap()
        };
        assert!(
            id(&hard_ballistic, "RANGE").limit > id(&ballistic, "RANGE").limit,
            "harder offers demand more range"
        );
        assert!(id(&short, "RANGE").limit < id(&ballistic, "RANGE").limit);
        assert!(id(&short, "MASS").limit <= 300.0);
        assert!(!ballistic.missile_requirements().iter().any(|r| r.id == "LOCK"));
    }

    #[test]
    fn air_to_air_contracts_require_a_target_lock() {
        let offer = offer_for(MissionFamily::AirToAir, VehicleFamily::ShortRange);
        let lock = offer
            .missile_requirements()
            .into_iter()
            .find(|requirement| requirement.id == "LOCK")
            .expect("air-to-air needs a lock");
        assert!(lock.hard);
    }

    #[test]
    fn a_covered_trial_accepts_the_dossier_and_edits_make_it_stale() {
        let offer = offer_for(MissionFamily::AirToSurface, VehicleFamily::ShortRange);
        let vehicle = ContentHash::of_bytes(b"vehicle");
        let program = ContentHash::of_bytes(b"program");
        let flight = report(20_000.0, 400.0, 250.0, false);
        let trials = vec![trial(&vehicle, &program, &flight)];
        let dossier = offer.missile_dossier(&trials, &vehicle, &program);
        assert!(
            matches!(dossier.decision, DossierDecision::Accepted { .. }),
            "{:?}",
            dossier.decision
        );
        assert!(offer.accepts_dossier(&dossier));

        let edited_vehicle = ContentHash::of_bytes(b"vehicle-edited");
        let stale = offer.missile_dossier(&trials, &edited_vehicle, &program);
        assert!(matches!(stale.decision, DossierDecision::Rejected { .. }));
        assert!(!offer.accepts_dossier(&stale));
        let edited_program = ContentHash::of_bytes(b"program-edited");
        let stale = offer.missile_dossier(&trials, &vehicle, &edited_program);
        assert!(matches!(stale.decision, DossierDecision::Rejected { .. }));
    }

    #[test]
    fn a_failed_trial_is_refused_with_the_failing_requirement() {
        let offer = offer_for(MissionFamily::AirToAir, VehicleFamily::ShortRange);
        let vehicle = ContentHash::of_bytes(b"vehicle");
        let program = ContentHash::of_bytes(b"program");
        let flight = report(20_000.0, 400.0, 250.0, false);
        let trials = vec![trial(&vehicle, &program, &flight)];
        let dossier = offer.missile_dossier(&trials, &vehicle, &program);
        let DossierDecision::Rejected { reasons } = &dossier.decision else {
            panic!("a failed lock must reject the dossier: {:?}", dossier.decision);
        };
        assert!(reasons.iter().any(|reason| reason.contains("LOCK")), "{reasons:?}");
    }

    #[test]
    fn the_latest_matching_trial_wins() {
        let offer = offer_for(MissionFamily::AirToSurface, VehicleFamily::ShortRange);
        let vehicle = ContentHash::of_bytes(b"vehicle");
        let program = ContentHash::of_bytes(b"program");
        let too_short = MissileTrial::from_report(
            vehicle.clone(),
            program.clone(),
            1,
            1,
            &report(5_000.0, 400.0, 250.0, false),
        );
        let good = MissileTrial::from_report(
            vehicle.clone(),
            program.clone(),
            2,
            2,
            &report(20_000.0, 400.0, 250.0, false),
        );
        let dossier = offer.missile_dossier(&[too_short, good], &vehicle, &program);
        assert!(offer.accepts_dossier(&dossier));
    }

    #[test]
    fn board_is_deterministic_and_covers_every_mission_family() {
        let first = generate_contract_board(42, 100);
        let second = generate_contract_board(42, 100);
        assert_eq!(first, second);
        assert_eq!(first.len(), MissionFamily::ALL.len());
        for family in MissionFamily::ALL {
            assert!(first.iter().any(|offer| offer.mission_family == family));
        }
    }

    #[test]
    fn generated_offers_have_valid_ids_and_requirements() {
        let board = generate_contract_board(7, 1);
        for (index, offer) in board.iter().enumerate() {
            assert_eq!(offer.id, index as u32 + 1);
            assert!(offer.validate().is_ok(), "{offer:?}");
        }
    }

    #[test]
    fn design_or_program_edit_invalidates_trial_progress() {
        let mut progress = ContractProgress::new(1).unwrap();
        progress.record_trial();
        assert_eq!(progress.phase, ContractPhase::ReadyForDelivery);
        progress.invalidate_after_edit(false, true);
        assert_eq!(progress.phase, ContractPhase::NeedsRevision);
        assert_eq!(progress.completed_trial_runs, 0);
        assert_eq!(progress.program_revision, 1);
    }

    #[test]
    fn submission_requires_all_scores_and_research_capabilities() {
        // Pin the capability on the offer: the generated board only requires one for difficulty 4
        // and above, so a seeded board would make this test depend on the roll.
        let mut offer = generate_contract_board(1, 1).remove(0);
        offer.requirements.required_capabilities = vec!["research:advanced_validation".to_string()];
        let capabilities = offer.requirements.required_capabilities.clone();
        assert!(offer.evaluate_submission(100, 100, 100, &capabilities).accepted);
        assert!(!offer.evaluate_submission(0, 100, 100, &capabilities).accepted);
        let missing = offer.evaluate_submission(100, 100, 100, &[]);
        assert!(!missing.accepted);
        assert_eq!(missing.missing_capabilities, offer.requirements.required_capabilities);
    }

    #[test]
    fn invalid_scores_are_rejected() {
        let mut offer = generate_contract_board(3, 1).remove(0);
        offer.requirements.min_trial_score = 101;
        assert!(offer.validate().is_err());
    }
}
