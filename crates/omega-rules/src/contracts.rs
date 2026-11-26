//! Fictional military contract generation and progression primitives.
//!
//! Contract values are abstract game ratings. This module deliberately does not model
//! real-world mission planning, targeting, guidance, or weapon performance.

use crate::dossier::{Dossier, DossierDecision, build_dossier};
use crate::flight::FlightReport;
use crate::requirement::{Comparator, Estimate, Evidence, Requirement, VerificationMethod};
use omega_data::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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

    /// Research capability that unlocks offers for this mission family. A ballistic vehicle is
    /// the starting family; hard surface-to-surface work needs the anti-ship family.
    pub fn capability(self, difficulty: u8) -> &'static str {
        match self {
            Self::AirToAir => "family:air_to_air",
            Self::AirToSurface => "family:ballistic",
            Self::SurfaceToAir => "family:surface_to_air",
            Self::SurfaceToSurface if difficulty >= 4 => "family:anti_ship",
            Self::SurfaceToSurface => "family:ballistic",
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

    /// Research capability a cruiser needs; ballistic and short-range work from day one.
    pub fn capability(self) -> Option<&'static str> {
        match self {
            Self::Ballistic => Some("family:ballistic"),
            Self::Cruise => Some("family:cruise"),
            Self::ShortRange => None,
        }
    }

    /// Classify a design for contract compatibility. A vehicle with an air-breathing engine is a
    /// cruiser; otherwise a multi-stage stack is the ballistic family and a single stage is the
    /// short-range family. This is a gameplay rule, not a real classification.
    pub fn of_vehicle(vehicle: &crate::assembly::Vehicle) -> Self {
        if vehicle
            .parts
            .iter()
            .any(|part| matches!(part.spec, crate::vehicle::PartSpec::AirBreathing { .. }))
        {
            Self::Cruise
        } else if vehicle.stage_count() >= 2 {
            Self::Ballistic
        } else {
            Self::ShortRange
        }
    }
}

/// A mission package layered over the broad mission family. Two offers in the same family can
/// demand opposite flight profiles, so one airframe cannot satisfy both.
/// The board shows a readable sample of a family's profiles: one low/high or short/long pair.
pub const MAX_PROFILES_PER_FAMILY: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractProfile {
    DeepStrike,
    LowObservableRaid,
    PrecisionStrike,
    AirSuperiority,
    LongRangeIntercept,
    PointDefence,
    AreaDefence,
    TheatreStrike,
    SeaSkimmer,
}

impl ContractProfile {
    pub const ALL: [Self; 9] = [
        Self::DeepStrike,
        Self::LowObservableRaid,
        Self::PrecisionStrike,
        Self::AirSuperiority,
        Self::LongRangeIntercept,
        Self::PointDefence,
        Self::AreaDefence,
        Self::TheatreStrike,
        Self::SeaSkimmer,
    ];

    pub fn for_family(family: MissionFamily) -> &'static [Self] {
        match family {
            MissionFamily::AirToAir => &[Self::AirSuperiority, Self::LongRangeIntercept],
            MissionFamily::AirToSurface => &[Self::LowObservableRaid, Self::DeepStrike, Self::PrecisionStrike],
            MissionFamily::SurfaceToAir => &[Self::PointDefence, Self::AreaDefence],
            MissionFamily::SurfaceToSurface => &[Self::TheatreStrike, Self::SeaSkimmer],
        }
    }

    pub fn available_for_family(family: MissionFamily, owned: &BTreeSet<String>) -> Vec<Self> {
        Self::for_family(family)
            .iter()
            .copied()
            .filter(|profile| profile.is_available(owned))
            .collect()
    }

    pub fn mission_family(self) -> MissionFamily {
        match self {
            Self::AirSuperiority | Self::LongRangeIntercept => MissionFamily::AirToAir,
            Self::DeepStrike | Self::LowObservableRaid | Self::PrecisionStrike => MissionFamily::AirToSurface,
            Self::PointDefence | Self::AreaDefence => MissionFamily::SurfaceToAir,
            Self::TheatreStrike | Self::SeaSkimmer => MissionFamily::SurfaceToSurface,
        }
    }

    pub fn vehicle_family(self) -> VehicleFamily {
        match self {
            Self::DeepStrike | Self::AreaDefence | Self::TheatreStrike => VehicleFamily::Ballistic,
            Self::LowObservableRaid | Self::LongRangeIntercept | Self::SeaSkimmer => VehicleFamily::Cruise,
            Self::PrecisionStrike | Self::AirSuperiority | Self::PointDefence => VehicleFamily::ShortRange,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::DeepStrike => "DEEP STRIKE",
            Self::LowObservableRaid => "VEIL RAID",
            Self::PrecisionStrike => "PRECISION STRIKE",
            Self::AirSuperiority => "AIR SUPERIORITY",
            Self::LongRangeIntercept => "LONG-RANGE INTERCEPT",
            Self::PointDefence => "POINT DEFENCE",
            Self::AreaDefence => "AREA DEFENCE",
            Self::TheatreStrike => "THEATRE STRIKE",
            Self::SeaSkimmer => "SEA SKIMMER",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::DeepStrike => "Deep strike programme",
            Self::LowObservableRaid => "Low-observable raid",
            Self::PrecisionStrike => "Precision strike package",
            Self::AirSuperiority => "Air superiority round",
            Self::LongRangeIntercept => "Long-range intercept",
            Self::PointDefence => "Point-defence interceptor",
            Self::AreaDefence => "Area-defence interceptor",
            Self::TheatreStrike => "Theatre strike programme",
            Self::SeaSkimmer => "Sea-skimmer programme",
        }
    }

    pub fn client(self) -> &'static str {
        match self {
            Self::DeepStrike => "Meridian Strategic Systems",
            Self::LowObservableRaid => "Veil Systems Directorate",
            Self::PrecisionStrike => "Kestrel Munitions Board",
            Self::AirSuperiority => "Aster Defence Directorate",
            Self::LongRangeIntercept => "Halcyon Air Command",
            Self::PointDefence => "Northstar Defence Office",
            Self::AreaDefence => "Cinder Range Authority",
            Self::TheatreStrike => "Orion Procurement Bureau",
            Self::SeaSkimmer => "Tethys Naval Bureau",
        }
    }

    pub fn brief(self) -> &'static str {
        match self {
            Self::DeepStrike => "A high-apogee, long-range round that trades stealth for reach.",
            Self::LowObservableRaid => "A low-flying raid package: the trial must stay discreet.",
            Self::PrecisionStrike => "A compact short-range round with a tightly sized charge.",
            Self::AirSuperiority => "A light, agile round that must acquire its target and turn hard.",
            Self::LongRangeIntercept => "A patient cruiser that loiters low before a long intercept.",
            Self::PointDefence => "A quick-reaction round for close-in defence.",
            Self::AreaDefence => "A high-altitude interceptor that watches a wide sector.",
            Self::TheatreStrike => "A heavy two-stage round with a large payload and a high apogee.",
            Self::SeaSkimmer => "A wave-top cruise package that must remain hard to spot.",
        }
    }

    /// Minimum explosive charge mass the dossier must prove.
    pub fn payload_kg(self) -> f64 {
        match self {
            Self::DeepStrike => 110.0,
            Self::LowObservableRaid => 45.0,
            Self::PrecisionStrike => 25.0,
            Self::AirSuperiority => 12.0,
            Self::LongRangeIntercept => 20.0,
            Self::PointDefence => 8.0,
            Self::AreaDefence => 30.0,
            Self::TheatreStrike => 150.0,
            Self::SeaSkimmer => 70.0,
        }
    }

    /// Minimum abstract game yield, which rewards better charge grades.
    pub fn yield_kg(self) -> f64 {
        match self {
            Self::DeepStrike => 140.0,
            Self::LowObservableRaid => 55.0,
            Self::PrecisionStrike => 32.0,
            Self::AirSuperiority => 16.0,
            Self::LongRangeIntercept => 26.0,
            Self::PointDefence => 10.0,
            Self::AreaDefence => 38.0,
            Self::TheatreStrike => 170.0,
            Self::SeaSkimmer => 88.0,
        }
    }

    /// Low-altitude profiles require a high discretion score.
    pub fn discretion_min(self) -> Option<f64> {
        match self {
            Self::LowObservableRaid => Some(62.0),
            Self::PrecisionStrike => Some(50.0),
            Self::AirSuperiority => Some(50.0),
            Self::LongRangeIntercept => Some(58.0),
            Self::PointDefence => Some(55.0),
            Self::SeaSkimmer => Some(68.0),
            _ => None,
        }
    }

    /// High-apogee profiles require the trial to reach this altitude.
    pub fn ceiling_min(self) -> Option<f64> {
        match self {
            Self::DeepStrike => Some(8_000.0),
            Self::AreaDefence => Some(9_000.0),
            Self::TheatreStrike => Some(10_000.0),
            _ => None,
        }
    }

    pub fn range_factor(self) -> f64 {
        match self {
            Self::DeepStrike => 1.15,
            Self::LowObservableRaid => 1.10,
            Self::PrecisionStrike => 0.85,
            Self::AirSuperiority => 0.80,
            Self::LongRangeIntercept => 1.20,
            Self::PointDefence => 0.75,
            Self::AreaDefence => 0.95,
            Self::TheatreStrike => 1.25,
            Self::SeaSkimmer => 1.10,
        }
    }

    pub fn reward_factor(self) -> f64 {
        match self {
            Self::DeepStrike => 1.15,
            Self::LowObservableRaid => 1.10,
            Self::PrecisionStrike => 0.95,
            Self::AirSuperiority => 1.00,
            Self::LongRangeIntercept => 1.20,
            Self::PointDefence => 0.90,
            Self::AreaDefence => 1.05,
            Self::TheatreStrike => 1.30,
            Self::SeaSkimmer => 1.15,
        }
    }

    pub fn deadline_bonus(self) -> i16 {
        match self {
            Self::TheatreStrike => -2,
            Self::PointDefence => 2,
            Self::PrecisionStrike => 1,
            _ => 0,
        }
    }

    /// Profile-specific emphasis on the design dossier.
    pub fn design_bonus(self) -> i16 {
        match self {
            Self::DeepStrike | Self::TheatreStrike => 8,
            Self::LowObservableRaid | Self::SeaSkimmer => 6,
            Self::AreaDefence => 5,
            Self::PrecisionStrike => 4,
            _ => 0,
        }
    }

    /// Profile-specific emphasis on the onboard program.
    pub fn program_bonus(self) -> i16 {
        match self {
            Self::LowObservableRaid => 8,
            Self::AirSuperiority | Self::LongRangeIntercept => 7,
            Self::SeaSkimmer => 5,
            Self::PointDefence => 4,
            Self::DeepStrike => 3,
            _ => 0,
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
    /// Mission package that adds payload, discretion or apogee requirements. `None` is kept for
    /// offers saved before profiles existed; such an offer keeps the older family-only rubric.
    #[serde(default)]
    pub profile: Option<ContractProfile>,
    pub title: String,
    pub client: String,
    pub brief: String,
    /// Fictional background shown on the offer card and in the technical dossier.
    #[serde(default)]
    pub context: String,
    pub requirements: ContractRequirements,
    /// Fictional design budget checked against the workshop's abstract cost figure.
    #[serde(default)]
    pub design_budget_cr: u64,
    /// Combined design constraints the delivered airframe must satisfy.
    #[serde(default)]
    pub primary_goals: Vec<DesignGoal>,
    /// Optional stretch goals; satisfying all of them pays the bonus.
    #[serde(default)]
    pub secondary_goals: Vec<DesignGoal>,
    #[serde(default)]
    pub secondary_bonus_cr: u64,
    /// Sections the client expects in the technical dossier.
    #[serde(default)]
    pub requested_dossier: Vec<String>,
    pub reward_credits: u64,
    pub deadline_days: u16,
    pub difficulty: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignGoalMetric {
    MassKg,
    Reliability,
    Complexity,
}

/// An integer-valued design goal. Values are compared with the workshop's abstract metrics, not
/// real engineering measurements.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignGoal {
    pub id: String,
    pub label: String,
    pub metric: DesignGoalMetric,
    pub limit: u32,
    pub at_most: bool,
}

impl DesignGoal {
    pub fn text(&self) -> String {
        let comparator = if self.at_most { "≤" } else { "≥" };
        let unit = match self.metric {
            DesignGoalMetric::MassKg => "kg",
            DesignGoalMetric::Reliability | DesignGoalMetric::Complexity => "/100",
        };
        format!("{} {comparator} {} {unit}", self.label, self.limit)
    }
}

/// Standard sections of a delivered dossier, in presentation order.
pub fn standard_dossier_sections() -> Vec<String> {
    [
        "Vue générale du véhicule",
        "Arbre d'assemblage",
        "Équipements",
        "Paramètres de conception",
        "Programme embarqué",
        "Résultats des essais",
        "Exigences validées",
        "Anomalies éventuelles",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

impl ContractOffer {
    pub fn profile_label(&self) -> &'static str {
        self.profile.map_or("GENERAL", ContractProfile::label)
    }

    /// Every research capability this offer needs. An offer without them is not put on the board,
    /// and accepting one that slipped through from an older save is refused by name.
    pub fn required_family_capabilities(&self) -> Vec<&'static str> {
        let mut required = vec![self.mission_family.capability(self.difficulty)];
        if let Some(capability) = self.vehicle_family.capability() {
            required.push(capability);
        }
        required
    }

    pub fn is_available(&self, owned: &BTreeSet<String>) -> bool {
        self.required_family_capabilities()
            .iter()
            .all(|capability| owned.contains(*capability))
            && self
                .requirements
                .required_capabilities
                .iter()
                .all(|capability| owned.contains(capability))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id == 0 {
            return Err("contract id must be non-zero".to_string());
        }
        if let Some(profile) = self.profile
            && (profile.mission_family() != self.mission_family || profile.vehicle_family() != self.vehicle_family)
        {
            return Err("contract profile does not match its mission and vehicle families".to_string());
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

    fn shuffle<T>(&mut self, values: &mut [T]) {
        for index in (1..values.len()).rev() {
            let other = self.range(0, index as u32) as usize;
            values.swap(index, other);
        }
    }
}

impl ContractProfile {
    pub fn is_available(self, owned: &BTreeSet<String>) -> bool {
        self.vehicle_family()
            .capability()
            .is_none_or(|capability| owned.contains(capability))
    }
}

fn make_offer(id: u32, mission_family: MissionFamily, profile: ContractProfile, difficulty: u8) -> ContractOffer {
    let d = u64::from(difficulty);
    let base_reward = 4_000 + d * 2_500;
    let reward_credits = (base_reward as f64 * profile.reward_factor()).round() as u64;
    let deadline = (35_i32 - i32::from(difficulty) * 3 + i32::from(profile.deadline_bonus())).max(12) as u16;
    let reliability = 30 + i32::from(difficulty) * 4;
    let complexity = 95 - i32::from(difficulty) * 2;
    let secondary_reliability = (reliability + 8).min(95);
    let secondary_complexity = (complexity - 8).max(25);
    let mass_limit = 1_200_u32.saturating_sub(u32::from(difficulty) * 80);
    ContractOffer {
        id: id.max(1),
        mission_family,
        vehicle_family: profile.vehicle_family(),
        profile: Some(profile),
        title: profile.title().to_string(),
        client: profile.client().to_string(),
        brief: profile.brief().to_string(),
        context: format!(
            "{} asks for a {} package. The programme is fictional; every figure is a game rating.",
            profile.client(),
            profile.label().to_lowercase()
        ),
        requirements: ContractRequirements {
            min_design_score: (55 + i32::from(difficulty) * 5 + i32::from(profile.design_bonus())).clamp(0, 95) as u8,
            min_program_score: (50 + i32::from(difficulty) * 6 + i32::from(profile.program_bonus())).clamp(0, 95) as u8,
            min_trial_score: (55 + difficulty * 5).min(95),
            required_capabilities: if difficulty >= 4 {
                vec!["research:advanced_validation".to_string()]
            } else {
                Vec::new()
            },
        },
        design_budget_cr: 12_000 + d * 2_000,
        primary_goals: vec![
            DesignGoal {
                id: "RELIABILITY".to_string(),
                label: "Reliability".to_string(),
                metric: DesignGoalMetric::Reliability,
                limit: reliability as u32,
                at_most: false,
            },
            DesignGoal {
                id: "COMPLEXITY".to_string(),
                label: "Complexity".to_string(),
                metric: DesignGoalMetric::Complexity,
                limit: complexity as u32,
                at_most: true,
            },
            DesignGoal {
                id: "MASS".to_string(),
                label: "Mass".to_string(),
                metric: DesignGoalMetric::MassKg,
                limit: mass_limit,
                at_most: true,
            },
        ],
        secondary_goals: vec![
            DesignGoal {
                id: "RELIABILITY_BONUS".to_string(),
                label: "Elite reliability".to_string(),
                metric: DesignGoalMetric::Reliability,
                limit: secondary_reliability as u32,
                at_most: false,
            },
            DesignGoal {
                id: "COMPLEXITY_BONUS".to_string(),
                label: "Clean design".to_string(),
                metric: DesignGoalMetric::Complexity,
                limit: secondary_complexity as u32,
                at_most: true,
            },
        ],
        secondary_bonus_cr: reward_credits / 5,
        requested_dossier: standard_dossier_sections(),
        reward_credits,
        deadline_days: deadline,
        difficulty,
    }
}

/// Generate up to two profile offers for each mission family, then shuffle the board deterministically.
pub fn generate_contract_board(seed: u64, first_id: u32) -> Vec<ContractOffer> {
    let all: BTreeSet<String> = [
        "family:ballistic",
        "family:cruise",
        "family:surface_to_air",
        "family:air_to_air",
        "family:anti_ship",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    generate_available_board(seed, first_id, &all)
}

/// Up to two profile offers per mission family the player has researched, in a deterministic
/// shuffled order.
/// Unresearched families are simply absent from the board; a cruise offer also needs the cruise
/// family, and a hard surface-to-surface offer needs the anti-ship family (difficulty is clamped
/// so an available family never produces an offer it cannot staff).
pub fn generate_available_board(seed: u64, first_id: u32, owned: &BTreeSet<String>) -> Vec<ContractOffer> {
    let mut rng = ContractRng::new(seed);
    let mut families = MissionFamily::ALL;
    rng.shuffle(&mut families);
    let mut offers = Vec::new();
    for mission_family in families {
        // Each family rolls its own difficulty before the board is filtered, so a seeded board
        // stays deterministic per family.
        let difficulty = difficulty_with(&mut rng, mission_family, owned);
        if !can_staff(mission_family, difficulty, owned) {
            continue;
        }
        let mut profiles = ContractProfile::available_for_family(mission_family, owned);
        rng.shuffle(&mut profiles);
        for profile in profiles.into_iter().take(MAX_PROFILES_PER_FAMILY) {
            let offer = make_offer(
                first_id.wrapping_add(offers.len() as u32).max(1),
                mission_family,
                profile,
                difficulty,
            );
            if offer.is_available(owned) {
                offers.push(offer);
            }
        }
    }
    offers
}

fn difficulty_with(rng: &mut ContractRng, mission_family: MissionFamily, owned: &BTreeSet<String>) -> u8 {
    let mut difficulty = rng.range(1, 5) as u8;
    while difficulty > 1 && !can_staff(mission_family, difficulty, owned) {
        difficulty -= 1;
    }
    difficulty
}

/// A mission family can only staff an offer when its family capability is owned and the hard
/// validation research required from difficulty 4 is also present. Keeping this rule in one
/// place makes the board's difficulty progression match `ContractOffer::is_available`.
fn can_staff(mission_family: MissionFamily, difficulty: u8, owned: &BTreeSet<String>) -> bool {
    owned.contains(mission_family.capability(difficulty))
        && (difficulty < 4 || owned.contains("research:advanced_validation"))
}

/// Pick a difficulty compatible with the researched mission family.
fn difficulty_for(seed: u64, mission_family: MissionFamily, owned: &BTreeSet<String>) -> u8 {
    difficulty_with(&mut ContractRng::new(seed), mission_family, owned)
}

/// One fresh offer for a specific profile, used to top up the board after a delivery.
pub fn offer_for_profile(seed: u64, id: u32, profile: ContractProfile, owned: &BTreeSet<String>) -> ContractOffer {
    make_offer(
        id,
        profile.mission_family(),
        profile,
        difficulty_for(seed ^ u64::from(id), profile.mission_family(), owned),
    )
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
    /// Abstract aerodynamic stability the design had during the trial (0..100).
    #[serde(default)]
    pub stability: f64,
    /// Abstract wing lift the design had during the trial (0..100).
    #[serde(default)]
    pub lift: f64,
    /// Abstract control authority the design had during the trial (0..100).
    #[serde(default)]
    pub control_authority: f64,
    /// Family the design belonged to during the trial. `None` marks evidence from an old save.
    #[serde(default)]
    pub vehicle_family: Option<VehicleFamily>,
    /// Highest altitude reached by the trial, in metres.
    #[serde(default)]
    pub max_altitude_m: f64,
    /// Derived low-observability score (0..100): low flight is discreet, high apogee is not.
    #[serde(default)]
    pub discretion: f64,
    /// Explosive charge mass carried during the trial.
    #[serde(default)]
    pub payload_kg: f64,
    /// Abstract game yield of that charge, after its grade multiplier.
    #[serde(default)]
    pub payload_yield_kg: f64,
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
            stability: report.aero.stability,
            lift: report.aero.lift,
            control_authority: report.aero.control_authority,
            vehicle_family: Some(report.vehicle_family),
            max_altitude_m: report.max_altitude_m,
            discretion: report.discretion,
            payload_kg: report.payload_kg,
            payload_yield_kg: report.payload_yield_kg,
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
            proof("STABILITY", self.stability, 2.0),
            proof("LIFT", self.lift, 2.0),
            proof("AUTHORITY", self.control_authority, 2.0),
            proof("PAYLOAD", self.payload_kg, self.payload_kg * 0.01),
            proof("YIELD", self.payload_yield_kg, self.payload_yield_kg * 0.01),
            proof("DISCRETION", self.discretion, 2.0),
            proof("CEILING", self.max_altitude_m, 2.0),
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
        let range_factor = self.profile.map_or(1.0, ContractProfile::range_factor);
        let (range_m, speed_ms, mass_kg) = match self.vehicle_family {
            VehicleFamily::Ballistic => ((26.0 + 4.0 * d) * range_factor * 1_000.0, 550.0 + 40.0 * d, 700.0),
            VehicleFamily::Cruise => ((30.0 + 5.0 * d) * range_factor * 1_000.0, 170.0 + 10.0 * d, 420.0),
            VehicleFamily::ShortRange => ((10.0 + 3.0 * d) * range_factor * 1_000.0, 240.0 + 20.0 * d, 300.0),
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
        ];
        // A short-range or cruise profile may be a single-stage design; only the ballistic family
        // is expected to prove stage separation. Legacy offers without a profile keep their old
        // unconditional requirement so their stored rubrics do not change.
        if self.profile.is_none() || matches!(self.vehicle_family, VehicleFamily::Ballistic) {
            requirements.push(missile_requirement(
                "SEPARATION",
                "Stage separation",
                "separated",
                Comparator::AtLeast,
                1.0,
                1.0,
                true,
            ));
        }
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
        // A surface-launched interceptor wants a stable airframe, but this stays a soft
        // requirement: a marginal design is a compromise, not an impossible contract.
        if matches!(self.mission_family, MissionFamily::SurfaceToAir) {
            requirements.push(missile_requirement(
                "STABILITY",
                "Aerodynamic stability",
                "stability",
                Comparator::AtLeast,
                20.0,
                0.5,
                false,
            ));
        }
        // Each family asks the airframe for the one thing it exists for: a cruise missile needs
        // wings, an air-to-air interceptor needs moving surfaces, a surface-to-air round stability.
        if matches!(self.vehicle_family, VehicleFamily::Cruise) {
            requirements.push(missile_requirement(
                "LIFT",
                "Wing lift",
                "lift",
                Comparator::AtLeast,
                40.0,
                0.5,
                false,
            ));
        }
        if matches!(self.mission_family, MissionFamily::AirToAir) {
            requirements.push(missile_requirement(
                "AUTHORITY",
                "Control authority",
                "control_authority",
                Comparator::AtLeast,
                25.0,
                0.5,
                false,
            ));
        }
        if let Some(profile) = self.profile {
            requirements.push(missile_requirement(
                "PAYLOAD",
                "Explosive charge",
                "payload_kg",
                Comparator::AtLeast,
                profile.payload_kg(),
                1.5,
                true,
            ));
            requirements.push(missile_requirement(
                "YIELD",
                "Charge yield",
                "payload_yield_kg",
                Comparator::AtLeast,
                profile.yield_kg(),
                1.0,
                true,
            ));
            if let Some(limit) = profile.discretion_min() {
                requirements.push(missile_requirement(
                    "DISCRETION",
                    "Discretion",
                    "discretion",
                    Comparator::AtLeast,
                    limit,
                    1.5,
                    true,
                ));
            }
            if let Some(limit) = profile.ceiling_min() {
                requirements.push(missile_requirement(
                    "CEILING",
                    "Apogee",
                    "max_altitude_m",
                    Comparator::AtLeast,
                    limit,
                    1.5,
                    true,
                ));
            }
        }
        requirements
    }

    /// A profile-bearing contract only accepts evidence flown by its own vehicle family. Old
    /// offers without a profile keep the permissive behaviour of the earlier rubric.
    pub fn accepts_trial(&self, trial: &MissileTrial) -> bool {
        self.profile.is_none() || trial.vehicle_family.is_none_or(|family| family == self.vehicle_family)
    }

    /// Build the delivery dossier from the trial evidence gathered for this design and program.
    /// `build_dossier` already picks the latest trial matching both hashes and reports `Stale`
    /// when none matches, so an edited vehicle or program invalidates the dossier.
    pub fn missile_dossier(&self, trials: &[MissileTrial], vehicle: &ContentHash, program: &ContentHash) -> Dossier {
        let requirements = self.missile_requirements();
        let evidence: Vec<Evidence> = trials
            .iter()
            .filter(|trial| self.accepts_trial(trial))
            .flat_map(MissileTrial::evidence)
            .collect();
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
        offer.profile = None;
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
            aero: crate::aero::AeroSummary::default(),
            avionics: crate::avionics::AvionicsSummary::default(),
            surface_fault: false,
            trace: Vec::new(),
            final_flags: [false; crate::program::FLAGS],
            final_mode: 0,
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
            max_altitude_m: 0.0,
            discretion: 100.0,
            vehicle_family: VehicleFamily::Ballistic,
            payload_kg: 0.0,
            payload_yield_kg: 0.0,
        }
    }

    fn trial(vehicle: &ContentHash, program: &ContentHash, report: &FlightReport) -> MissileTrial {
        MissileTrial::from_report(vehicle.clone(), program.clone(), 1, 1, report)
    }

    fn profile_offer(profile: ContractProfile) -> ContractOffer {
        let mut offer = make_offer(1, profile.mission_family(), profile, 1);
        offer.requirements.min_trial_score = 0;
        offer
    }

    #[test]
    fn a_single_stage_short_range_trial_clears_its_profile() {
        let offer = profile_offer(ContractProfile::PrecisionStrike);
        let vehicle = ContentHash::of_bytes(b"short-range vehicle");
        let program = ContentHash::of_bytes(b"short-range program");
        let mut flight = report(20_000.0, 400.0, 250.0, false);
        flight.events.clear();
        flight.vehicle_family = VehicleFamily::ShortRange;
        flight.payload_kg = 40.0;
        flight.payload_yield_kg = 40.0;
        flight.max_altitude_m = 400.0;
        flight.discretion = crate::flight::discretion_from_altitude(400.0);
        let trial = trial(&vehicle, &program, &flight);
        assert!(!trial.separated);
        let dossier = offer.missile_dossier(&[trial], &vehicle, &program);
        assert!(offer.accepts_dossier(&dossier), "{dossier:?}");
    }

    #[test]
    fn short_range_and_cruise_profiles_do_not_require_stage_separation() {
        for profile in [
            ContractProfile::PrecisionStrike,
            ContractProfile::AirSuperiority,
            ContractProfile::PointDefence,
        ] {
            let ids: Vec<String> = profile_offer(profile)
                .missile_requirements()
                .into_iter()
                .map(|requirement| requirement.id)
                .collect();
            assert!(!ids.contains(&"SEPARATION".to_string()), "{profile:?}: {ids:?}");
        }
        let deep = profile_offer(ContractProfile::DeepStrike);
        assert!(
            deep.missile_requirements()
                .iter()
                .any(|requirement| requirement.id == "SEPARATION")
        );
    }

    #[test]
    fn mission_profiles_ask_for_opposite_flight_envelopes() {
        let low = profile_offer(ContractProfile::LowObservableRaid);
        let high = profile_offer(ContractProfile::DeepStrike);
        let ids = |offer: &ContractOffer| -> Vec<String> {
            offer
                .missile_requirements()
                .into_iter()
                .map(|requirement| requirement.id)
                .collect()
        };
        let low_ids = ids(&low);
        assert!(low_ids.contains(&"PAYLOAD".to_string()));
        assert!(low_ids.contains(&"DISCRETION".to_string()));
        assert!(!low_ids.contains(&"CEILING".to_string()));
        let high_ids = ids(&high);
        assert!(high_ids.contains(&"PAYLOAD".to_string()));
        assert!(high_ids.contains(&"CEILING".to_string()));
        assert!(!high_ids.contains(&"DISCRETION".to_string()));

        let vehicle = ContentHash::of_bytes(b"vehicle");
        let program = ContentHash::of_bytes(b"program");
        let mut flight = report(200_000.0, 1_000.0, 200.0, true);
        flight.max_altitude_m = 500.0;
        flight.discretion = crate::flight::discretion_from_altitude(500.0);
        flight.payload_kg = 200.0;
        flight.payload_yield_kg = 200.0;
        let mut low_trial = trial(&vehicle, &program, &flight);
        // Ignore the family gate here: the point is that the altitude profile alone separates
        // the two contracts.
        low_trial.vehicle_family = None;
        let row = |dossier: &Dossier, id: &str| {
            dossier
                .matrix
                .iter()
                .find(|row| row.requirement_id == id)
                .unwrap_or_else(|| panic!("{id} row missing"))
                .status
        };
        let low_dossier = low.missile_dossier(std::slice::from_ref(&low_trial), &vehicle, &program);
        assert!(matches!(
            row(&low_dossier, "DISCRETION"),
            crate::requirement::RequirementStatus::Verified { .. }
        ));
        let high_dossier = high.missile_dossier(std::slice::from_ref(&low_trial), &vehicle, &program);
        assert!(matches!(
            row(&high_dossier, "CEILING"),
            crate::requirement::RequirementStatus::Failed { .. }
        ));
    }

    #[test]
    fn a_contract_refuses_evidence_from_the_wrong_vehicle_family() {
        let offer = profile_offer(ContractProfile::LowObservableRaid);
        let vehicle = ContentHash::of_bytes(b"vehicle");
        let program = ContentHash::of_bytes(b"program");
        let mut flight = report(200_000.0, 1_000.0, 200.0, true);
        flight.vehicle_family = VehicleFamily::Ballistic;
        let ballistic = trial(&vehicle, &program, &flight);
        assert!(!offer.accepts_trial(&ballistic));
        let mut cruise_flight = flight;
        cruise_flight.vehicle_family = VehicleFamily::Cruise;
        assert!(offer.accepts_trial(&trial(&vehicle, &program, &cruise_flight)));
        let dossier = offer.missile_dossier(&[ballistic], &vehicle, &program);
        assert!(matches!(dossier.decision, DossierDecision::Rejected { .. }));
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
    fn surface_to_air_offers_ask_for_a_stable_airframe() {
        let offer = offer_for(MissionFamily::SurfaceToAir, VehicleFamily::ShortRange);
        let stability = offer
            .missile_requirements()
            .into_iter()
            .find(|requirement| requirement.id == "STABILITY")
            .expect("a surface-to-air offer asks for stability");
        assert!(!stability.hard, "stability is a soft compromise");
        assert!(stability.limit > 0.0);
        let ground = offer_for(MissionFamily::AirToSurface, VehicleFamily::Ballistic);
        assert!(
            !ground
                .missile_requirements()
                .iter()
                .any(|requirement| requirement.id == "STABILITY")
        );
    }

    #[test]
    fn each_family_asks_for_the_airframe_trait_it_needs() {
        let cruise = offer_for(MissionFamily::AirToSurface, VehicleFamily::Cruise);
        let lift = cruise
            .missile_requirements()
            .into_iter()
            .find(|requirement| requirement.id == "LIFT")
            .expect("a cruise contract asks for wing lift");
        assert!(!lift.hard && lift.limit > 0.0);
        let interceptor = offer_for(MissionFamily::AirToAir, VehicleFamily::ShortRange);
        let authority = interceptor
            .missile_requirements()
            .into_iter()
            .find(|requirement| requirement.id == "AUTHORITY")
            .expect("an air-to-air contract asks for control authority");
        assert!(!authority.hard && authority.limit > 0.0);

        let vehicle = ContentHash::of_bytes(b"vehicle");
        let program = ContentHash::of_bytes(b"program");
        let mut flight = report(20_000.0, 400.0, 250.0, true);
        flight.aero.lift = 55.0;
        flight.aero.control_authority = 40.0;
        let trial = trial(&vehicle, &program, &flight);
        let evidence = trial.evidence();
        let quoted = |id: &str| {
            evidence
                .iter()
                .find(|e| e.requirement_id == id)
                .unwrap_or_else(|| panic!("{id} proof missing"))
                .estimate
                .value
        };
        assert_eq!(quoted("LIFT"), 55.0);
        assert_eq!(quoted("AUTHORITY"), 40.0);

        let row = |dossier: &Dossier, id: &str| {
            dossier
                .matrix
                .iter()
                .find(|row| row.requirement_id == id)
                .unwrap_or_else(|| panic!("{id} row missing"))
                .clone()
        };
        let cruise_dossier = cruise.missile_dossier(std::slice::from_ref(&trial), &vehicle, &program);
        assert!(matches!(
            row(&cruise_dossier, "LIFT").status,
            crate::requirement::RequirementStatus::Verified { .. }
        ));
        let interceptor_dossier = interceptor.missile_dossier(&[trial], &vehicle, &program);
        assert!(matches!(
            row(&interceptor_dossier, "AUTHORITY").status,
            crate::requirement::RequirementStatus::Verified { .. }
        ));
    }

    #[test]
    fn evidence_quotes_the_aerodynamic_stability_of_the_design() {
        let vehicle = ContentHash::of_bytes(b"vehicle");
        let program = ContentHash::of_bytes(b"program");
        let mut flight = report(20_000.0, 400.0, 250.0, true);
        flight.aero.stability = 63.0;
        let trial = trial(&vehicle, &program, &flight);
        assert_eq!(trial.stability, 63.0);
        let evidence = trial
            .evidence()
            .into_iter()
            .find(|e| e.requirement_id == "STABILITY")
            .unwrap();
        assert_eq!(evidence.estimate.value, 63.0);
        assert_eq!(evidence.design, vehicle);
    }

    #[test]
    fn a_stable_airframe_earns_the_soft_stability_score_without_blocking_the_contract() {
        let offer = offer_for(MissionFamily::SurfaceToAir, VehicleFamily::ShortRange);
        let vehicle = ContentHash::of_bytes(b"vehicle");
        let program = ContentHash::of_bytes(b"program");
        let mut stable = report(20_000.0, 400.0, 250.0, true);
        stable.aero.stability = 63.0;
        let mut unstable = stable.clone();
        unstable.aero.stability = 0.0;
        let dossier =
            |flight: &FlightReport| offer.missile_dossier(&[trial(&vehicle, &program, flight)], &vehicle, &program);
        let stable = dossier(&stable);
        let unstable = dossier(&unstable);
        assert!(
            matches!(stable.decision, DossierDecision::Accepted { .. }),
            "{:?}",
            stable.decision
        );
        assert!(
            matches!(unstable.decision, DossierDecision::Accepted { .. }),
            "a soft requirement must not reject: {:?}",
            unstable.decision
        );
        let row = |dossier: &Dossier| {
            dossier
                .matrix
                .iter()
                .find(|row| row.requirement_id == "STABILITY")
                .expect("the dossier lists stability")
                .clone()
        };
        assert!(!row(&stable).hard);
        assert!(matches!(
            row(&stable).status,
            crate::requirement::RequirementStatus::Verified { .. }
        ));
        assert!(matches!(
            row(&unstable).status,
            crate::requirement::RequirementStatus::Failed { .. }
        ));
        assert!(
            (stable.score - unstable.score - 0.05).abs() < 1e-9,
            "{} vs {}",
            stable.score,
            unstable.score
        );
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

    fn owned(capabilities: &[&str]) -> BTreeSet<String> {
        capabilities.iter().map(|c| (*c).to_string()).collect()
    }

    #[test]
    fn unresearched_families_offer_nothing_and_research_adds_them() {
        let fresh = owned(&["family:ballistic"]);
        let board = generate_available_board(5, 1, &fresh);
        assert!(!board.is_empty());
        assert!(board.iter().all(|offer| {
            offer.mission_family != MissionFamily::SurfaceToAir && offer.mission_family != MissionFamily::AirToAir
        }));
        assert!(board.iter().all(|offer| offer.vehicle_family != VehicleFamily::Cruise));

        let researched = owned(&[
            "family:ballistic",
            "family:surface_to_air",
            "family:air_to_air",
            "family:cruise",
        ]);
        let board = generate_available_board(5, 1, &researched);
        assert_eq!(board.len(), 2 * MissionFamily::ALL.len());
        for family in MissionFamily::ALL {
            assert_eq!(board.iter().filter(|offer| offer.mission_family == family).count(), 2);
        }
        for offer in &board {
            assert!(offer.is_available(&researched), "{offer:?}");
        }
    }

    #[test]
    fn hard_surface_to_surface_offers_need_the_anti_ship_family() {
        let without = owned(&["family:ballistic"]);
        let with = owned(&["family:ballistic", "family:anti_ship"]);
        for seed in 1..40 {
            for offer in generate_available_board(seed, 1, &without) {
                assert!(offer.difficulty <= 3 || offer.mission_family != MissionFamily::SurfaceToSurface);
            }
        }
        let hard = generate_available_board(3, 1, &with)
            .into_iter()
            .find(|offer| offer.mission_family == MissionFamily::SurfaceToSurface)
            .unwrap();
        assert!(hard.is_available(&with));
        assert!(hard.difficulty < 4 || hard.required_family_capabilities().contains(&"family:anti_ship"));
    }

    #[test]
    fn hard_offers_only_appear_after_hard_validation_is_researched() {
        let starting = owned(&[
            "family:ballistic",
            "family:surface_to_air",
            "family:air_to_air",
            "family:cruise",
        ]);
        for seed in 1..30 {
            for offer in generate_available_board(seed, 1, &starting) {
                assert!(
                    offer.difficulty <= 3,
                    "a hard offer escaped without its research: {offer:?}"
                );
                assert!(offer.is_available(&starting));
            }
        }
        let mut with_validation = starting.clone();
        with_validation.insert("research:advanced_validation".to_string());
        with_validation.insert("family:anti_ship".to_string());
        assert!(with_validation.contains("research:advanced_validation"));
        let hard_exists = (1..60).any(|seed| {
            generate_available_board(seed, 1, &with_validation)
                .iter()
                .any(|offer| offer.difficulty >= 4)
        });
        assert!(hard_exists, "advanced validation must actually unlock hard offers");
        for seed in 1..10 {
            for offer in generate_available_board(seed, 1, &with_validation) {
                assert!(offer.is_available(&with_validation));
            }
        }
    }

    #[test]
    fn board_is_deterministic_and_covers_every_mission_family() {
        let first = generate_contract_board(42, 100);
        let second = generate_contract_board(42, 100);
        assert_eq!(first, second);
        assert_eq!(first.len(), 2 * MissionFamily::ALL.len());
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

    #[test]
    fn offers_carry_a_context_budget_combined_goals_and_a_requested_dossier() {
        let offer = generate_contract_board(9, 1).remove(0);
        assert!(!offer.context.is_empty());
        assert!(offer.design_budget_cr > 1_000);
        assert!(
            offer
                .primary_goals
                .iter()
                .any(|goal| goal.metric == DesignGoalMetric::MassKg)
        );
        assert!(
            offer
                .primary_goals
                .iter()
                .any(|goal| goal.metric == DesignGoalMetric::Reliability)
        );
        assert!(
            offer
                .primary_goals
                .iter()
                .any(|goal| goal.metric == DesignGoalMetric::Complexity)
        );
        assert!(!offer.secondary_goals.is_empty());
        assert!(offer.secondary_bonus_cr > 0);
        assert_eq!(offer.requested_dossier.len(), 8);
        assert!(offer.validate().is_ok(), "{offer:?}");
    }

    #[test]
    fn difficulty_raises_rewards_and_pressure_without_gaps() {
        for profile in ContractProfile::ALL {
            let offers: Vec<ContractOffer> = (1..=5)
                .map(|difficulty| make_offer(1, profile.mission_family(), profile, difficulty))
                .collect();
            for pair in offers.windows(2) {
                let (easy, hard) = (&pair[0], &pair[1]);
                assert!(hard.reward_credits > easy.reward_credits, "{profile:?}");
                assert!(hard.secondary_bonus_cr >= easy.secondary_bonus_cr, "{profile:?}");
                assert!(hard.design_budget_cr > easy.design_budget_cr, "{profile:?}");
                assert!(hard.deadline_days <= easy.deadline_days, "{profile:?}");
                assert!(hard.requirements.min_design_score >= easy.requirements.min_design_score);
                assert!(hard.requirements.min_program_score >= easy.requirements.min_program_score);
                assert!(hard.requirements.min_trial_score >= easy.requirements.min_trial_score);
                let goal = |offer: &ContractOffer, metric| {
                    offer
                        .primary_goals
                        .iter()
                        .find(|goal| goal.metric == metric)
                        .unwrap()
                        .limit
                };
                assert!(goal(hard, DesignGoalMetric::Reliability) >= goal(easy, DesignGoalMetric::Reliability));
                assert!(goal(hard, DesignGoalMetric::Complexity) <= goal(easy, DesignGoalMetric::Complexity));
                assert!(goal(hard, DesignGoalMetric::MassKg) <= goal(easy, DesignGoalMetric::MassKg));
            }
        }
    }

    #[test]
    fn every_family_has_several_distinct_profiles_and_vehicle_families() {
        for mission_family in MissionFamily::ALL {
            let profiles = ContractProfile::for_family(mission_family);
            assert!(profiles.len() >= 2, "{mission_family:?}");
            let mut vehicle_families = Vec::new();
            for profile in profiles {
                let offer = make_offer(1, mission_family, *profile, 3);
                assert_eq!(offer.mission_family, mission_family);
                assert!(offer.validate().is_ok(), "{offer:?}");
                if !vehicle_families.contains(&offer.vehicle_family) {
                    vehicle_families.push(offer.vehicle_family);
                }
            }
            assert!(!vehicle_families.is_empty());
        }
        // The three airframe families are all reachable from the profile table, not just one.
        let all_vehicle_families: Vec<_> = ContractProfile::ALL
            .iter()
            .map(|profile| make_offer(1, profile.mission_family(), *profile, 3).vehicle_family)
            .collect();
        for family in VehicleFamily::ALL {
            assert!(all_vehicle_families.contains(&family), "{family:?}");
        }
    }

    #[test]
    fn secondary_goals_are_stricter_and_pay_only_as_a_bonus() {
        for difficulty in 1..=5 {
            let offer = make_offer(
                1,
                MissionFamily::SurfaceToSurface,
                ContractProfile::DeepStrike,
                difficulty,
            );
            let primary = offer
                .primary_goals
                .iter()
                .find(|goal| goal.metric == DesignGoalMetric::Reliability)
                .unwrap();
            let secondary = offer
                .secondary_goals
                .iter()
                .find(|goal| goal.metric == DesignGoalMetric::Reliability)
                .unwrap();
            assert!(secondary.limit >= primary.limit);
            assert_eq!(offer.secondary_bonus_cr, offer.reward_credits / 5);
            assert!(offer.secondary_bonus_cr < offer.reward_credits);
        }
    }
}
