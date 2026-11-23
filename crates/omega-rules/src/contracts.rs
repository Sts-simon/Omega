//! Fictional military contract generation and progression primitives.
//!
//! Contract values are abstract game ratings. This module deliberately does not model
//! real-world mission planning, targeting, guidance, or weapon performance.

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
        if self.required_capabilities.iter().any(|capability| capability.trim().is_empty()) {
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn invalid_scores_are_rejected() {
        let mut offer = generate_contract_board(3, 1).remove(0);
        offer.requirements.min_trial_score = 101;
        assert!(offer.validate().is_err());
    }
}
