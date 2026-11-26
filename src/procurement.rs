//! Missile contract office. Offers name a missile family, and the delivery dossier is built from
//! the player's own vehicle, onboard program and trials. Every proof is bound to the vehicle and
//! program hashes that produced it, so editing either after a trial makes the dossier stale.
//!
//! Research gates the office: an offer only appears when its mission family (and, for a cruiser,
//! the cruise family) is owned, and the number of active contracts follows the
//! `ExtraActiveProjects` grant.
//!
//! Everything here is fictional and abstract; see `docs/VISION_MISSILES.md`.

use crate::save::{self, SaveFile};
use crate::trials::TrialSession;
use bevy::prelude::*;
use omega_data::{ContentHash, hash_serializable};
use omega_rules::contracts::{
    ContractOffer, ContractProfile, MAX_PROFILES_PER_FAMILY, MissileTrial, MissionFamily, generate_available_board,
    offer_for_profile,
};
use omega_rules::research::capability_label;
use omega_rules::{Dossier, DossierDecision, NodeId, PartSpec, PlacedPart, Program, ResearchState, Vehicle};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: u32 = 2;

fn system_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0xA3E0_1AB5_2026_0001)
}

/// A part's attachment, without the parent id: the tree position is represented by where the part
/// sits in [`StructuralPart`], not by a mutable identifier.
#[derive(Serialize)]
struct StructuralAttachment {
    node: NodeId,
    angle_deg: f64,
    separation: bool,
    offset_x_m: f64,
    offset_y_m: f64,
    offset_z_m: f64,
    rotation_x_deg: f64,
    rotation_y_deg: f64,
    rotation_z_deg: f64,
}

/// The shape of a design with ids and display metadata removed. Children are sorted by their own
/// serialized shape, so neither part order nor renamed ids can make an identical craft look new.
#[derive(Serialize)]
struct StructuralPart {
    spec: PartSpec,
    attachment: Option<StructuralAttachment>,
    children: Vec<StructuralPart>,
}

#[derive(Serialize)]
struct StructuralVehicle {
    roots: Vec<StructuralPart>,
}

fn structural_part(part: &PlacedPart, parts: &[PlacedPart], seen: &mut BTreeSet<String>) -> Option<StructuralPart> {
    if !seen.insert(part.id.clone()) {
        return None;
    }
    let mut children = Vec::new();
    for child in parts {
        let Some(attach) = &child.attach else {
            continue;
        };
        if attach.parent == part.id {
            children.push(structural_part(child, parts, seen)?);
        }
    }
    seen.remove(&part.id);
    children.sort_by_key(|child| serde_json::to_string(child).unwrap_or_default());
    let attachment = part.attach.as_ref().map(|attach| StructuralAttachment {
        node: attach.node,
        angle_deg: attach.angle_deg,
        separation: attach.separation,
        offset_x_m: attach.offset_x_m,
        offset_y_m: attach.offset_y_m,
        offset_z_m: attach.offset_z_m,
        rotation_x_deg: attach.rotation_x_deg,
        rotation_y_deg: attach.rotation_y_deg,
        rotation_z_deg: attach.rotation_z_deg,
    });
    Some(StructuralPart {
        spec: part.spec,
        attachment,
        children,
    })
}

fn structural_design(vehicle: &Vehicle) -> Option<StructuralVehicle> {
    let mut roots = Vec::new();
    for root in vehicle.parts.iter().filter(|part| part.attach.is_none()) {
        roots.push(structural_part(root, &vehicle.parts, &mut BTreeSet::new())?);
    }
    if roots.is_empty() {
        return None;
    }
    roots.sort_by_key(|root| serde_json::to_string(root).unwrap_or_default());
    Some(StructuralVehicle { roots })
}

/// Identity of the airframe itself, independent of its display name and of the ids assigned to its
/// parts. Copying a design, or renaming its parts, cannot create a second craft for the
/// one-design-one-contract rule.
fn design_fingerprint(vehicle: &Vehicle) -> ContentHash {
    if let Some(hash) = structural_design(vehicle).and_then(|design| hash_serializable(&design).ok()) {
        return hash;
    }
    // Invalid or cyclic assemblies have no structural tree; keep the old full-design fallback
    // rather than making every invalid design share one fingerprint.
    let mut fallback = vehicle.clone();
    fallback.name.clear();
    hash_serializable(&fallback).unwrap_or_else(|_| ContentHash::of_bytes(b"vehicle"))
}

/// The vehicle and program hashes a dossier is checked against.
pub(crate) fn hashes(vehicle: &Vehicle, program: &Program) -> (ContentHash, ContentHash) {
    let vehicle = hash_serializable(vehicle).unwrap_or_else(|_| ContentHash::of_bytes(b"vehicle"));
    let program = hash_serializable(&program.semantic_copy()).unwrap_or_else(|_| ContentHash::of_bytes(b"program"));
    (vehicle, program)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ActiveContract {
    pub offer: ContractOffer,
    pub accepted_day: u32,
    pub trials: Vec<MissileTrial>,
}

impl ActiveContract {
    pub fn due_day(&self) -> u32 {
        self.accepted_day.saturating_add(u32::from(self.offer.deadline_days))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct CompletedContract {
    pub id: u32,
    pub title: String,
    pub client: String,
    pub reward: u64,
    pub completed_day: u32,
    pub score_percent: u8,
    /// Revision submitted with the dossier; old deliveries have no revision link.
    #[serde(default)]
    pub revision_id: Option<u32>,
    #[serde(default)]
    pub secondary_bonus: u64,
    /// Structural design fingerprint delivered. Used to stop one craft from being sold to several
    /// contracts, even after renaming it or its parts. Older saves hold the full vehicle hash,
    /// which still matches a same-name design.
    #[serde(default)]
    pub vehicle: Option<ContentHash>,
}

/// Whether the evidence on file still describes the current design and program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EvidenceStatus {
    NoContract,
    NoTrial,
    Current,
    Stale,
}

impl EvidenceStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::NoContract => "NO CONTRACT",
            Self::NoTrial => "TRIAL REQUIRED",
            Self::Current => "EVIDENCE CURRENT",
            Self::Stale => "STALE",
        }
    }
}

/// Result of a successful delivery, so the caller can credit the campaign.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Delivery {
    pub id: u32,
    pub reward: u64,
    pub score_percent: u8,
    pub secondary_bonus: u64,
}

#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Procurement {
    schema_version: u32,
    seed: u64,
    next_id: u32,
    offers: Vec<ContractOffer>,
    active: Vec<ActiveContract>,
    #[serde(default)]
    selected: Option<u32>,
    completed: Vec<CompletedContract>,
    /// Last trial already folded into evidence, so a report is not recorded twice.
    last_recorded: Option<String>,
    /// Capabilities the current board was generated for.
    #[serde(default)]
    known_caps: BTreeSet<String>,
    #[serde(skip)]
    pub(crate) message: String,
    #[serde(skip)]
    dirty: bool,
}

#[derive(Deserialize)]
struct LegacyProcurement {
    seed: u64,
    next_id: u32,
    offers: Vec<ContractOffer>,
    active: Option<ActiveContract>,
    completed: Vec<CompletedContract>,
    last_recorded: Option<String>,
}

impl Default for Procurement {
    fn default() -> Self {
        match save::read(SaveFile::Procurement) {
            Ok(Some(bytes)) => match Self::from_bytes(&bytes) {
                Ok(loaded) => {
                    info!(
                        "Loaded {} missile contract(s), {} active",
                        loaded.completed.len(),
                        loaded.active.len()
                    );
                    return loaded;
                }
                Err(error) => warn!("Could not read the missile contracts: {error}"),
            },
            Ok(None) => {}
            Err(error) => warn!("Could not read the missile contracts: {error}"),
        }
        Self::fresh(system_seed())
    }
}

fn starting_capabilities() -> BTreeSet<String> {
    ResearchState::default().owned_capabilities(&omega_rules::default_tree())
}

impl Procurement {
    pub(crate) fn load_active(&mut self) {
        *self = match save::read(SaveFile::Procurement) {
            Ok(Some(bytes)) => match Self::from_bytes(&bytes) {
                Ok(loaded) => loaded,
                Err(error) => {
                    warn!("Could not read the missile contracts: {error}");
                    Self::fresh(system_seed())
                }
            },
            Ok(None) => Self::fresh(system_seed()),
            Err(error) => {
                warn!("Could not read the missile contracts: {error}");
                Self::fresh(system_seed())
            }
        };
    }

    pub(crate) fn new_campaign(&mut self) {
        *self = Self::fresh(system_seed());
    }

    pub(crate) fn fresh(seed: u64) -> Self {
        let caps = starting_capabilities();
        let offers = generate_available_board(seed, 1, &caps);
        let next_id = 1 + offers.len() as u32;
        Self {
            schema_version: SCHEMA_VERSION,
            seed,
            next_id,
            offers,
            active: Vec::new(),
            selected: None,
            completed: Vec::new(),
            last_recorded: None,
            known_caps: caps,
            message: "Accept a missile contract, then fly its trial.".to_string(),
            dirty: true,
        }
    }

    pub fn offers(&self) -> &[ContractOffer] {
        &self.offers
    }

    pub fn active(&self) -> &[ActiveContract] {
        &self.active
    }

    pub fn completed(&self) -> &[CompletedContract] {
        &self.completed
    }

    pub(crate) fn selected_id(&self) -> Option<u32> {
        self.selected
            .filter(|id| self.active.iter().any(|active| active.offer.id == *id))
            .or_else(|| self.active.first().map(|active| active.offer.id))
    }

    pub(crate) fn selected(&self) -> Option<&ActiveContract> {
        let id = self.selected_id()?;
        self.active.iter().find(|active| active.offer.id == id)
    }

    /// Select one active contract for the dossier panel. Returns a status message.
    pub(crate) fn select(&mut self, id: u32) -> String {
        match self.active.iter().find(|active| active.offer.id == id) {
            Some(active) => {
                self.selected = Some(id);
                format!("Selected contract #{}", active.offer.id)
            }
            None => "That contract is not active.".to_string(),
        }
    }

    pub(crate) fn status_for(&self, active: &ActiveContract, vehicle: &Vehicle, program: &Program) -> EvidenceStatus {
        let Some(latest) = active.trials.last() else {
            return EvidenceStatus::NoTrial;
        };
        let (vehicle_hash, program_hash) = hashes(vehicle, program);
        if latest.vehicle == vehicle_hash && latest.program == program_hash {
            EvidenceStatus::Current
        } else {
            EvidenceStatus::Stale
        }
    }

    /// Evidence state of the selected contract, or `NoContract` when nothing is active.
    pub(crate) fn evidence_status(&self, vehicle: &Vehicle, program: &Program) -> EvidenceStatus {
        match self.selected() {
            None => EvidenceStatus::NoContract,
            Some(active) => self.status_for(active, vehicle, program),
        }
    }

    /// Everything the contract board shows, so it rebuilds only when something changed.
    pub(crate) fn signature(&self, vehicle: &Vehicle, program: &Program) -> String {
        let active: Vec<String> = self
            .active
            .iter()
            .map(|active| {
                format!(
                    "{}:{}:{}:{:?}:{}",
                    active.offer.id,
                    active.trials.len(),
                    active.trials.last().map_or(String::new(), |trial| format!(
                        "{}-{}",
                        trial.vehicle.0, trial.program.0
                    )),
                    self.selected_id(),
                    self.status_for(active, vehicle, program) as u8
                )
            })
            .collect();
        format!(
            "{:?}|{:?}|{:?}|{:?}|{:?}",
            active,
            self.offers.iter().map(|offer| offer.id).collect::<Vec<_>>(),
            self.completed.len(),
            hashes(vehicle, program),
            self.known_caps
        )
    }

    pub(crate) fn dossier_for(&self, active: &ActiveContract, vehicle: &Vehicle, program: &Program) -> Dossier {
        let (vehicle_hash, program_hash) = hashes(vehicle, program);
        active
            .offer
            .missile_dossier(&active.trials, &vehicle_hash, &program_hash)
    }

    /// Accept an offer. A family that is not researched is refused by the name of the technology
    /// that unlocks it, and the office never holds more projects than the research grants allow.
    pub(crate) fn accept(&mut self, index: usize, day: u32, max_active: usize, owned: &BTreeSet<String>) -> String {
        if index >= self.offers.len() {
            return "No offer at this position.".to_string();
        }
        let offer = self.offers[index].clone();
        let missing: Vec<&str> = offer
            .required_family_capabilities()
            .into_iter()
            .filter(|capability| !owned.contains(*capability))
            .collect();
        if !missing.is_empty() {
            let names: Vec<String> = missing
                .iter()
                .map(|capability| format!("\"{}\" (Research Centre)", capability_label(capability)))
                .collect();
            return format!("Contract #{} needs {} researched first.", offer.id, names.join(" and "));
        }
        if self.active.len() >= max_active {
            return format!(
                "All {max_active} contract slot(s) are in use. Deliver one, or research \"Parallel project management\" for another."
            );
        }
        self.offers.remove(index);
        let id = offer.id;
        self.active.push(ActiveContract {
            offer,
            accepted_day: day,
            trials: Vec::new(),
        });
        self.selected = Some(id);
        self.dirty = true;
        format!("Accepted contract #{id}")
    }

    /// Fold a finished trial into every active contract that accepts its flight profile. A trial
    /// flown by the wrong vehicle family is refused by those contracts instead of becoming
    /// evidence for an unrelated mission.
    pub(crate) fn record_trial(&mut self, trial: MissileTrial) -> Option<String> {
        let mut recorded = 0;
        let mut incompatible = 0;
        for active in &mut self.active {
            if !active.offer.accepts_trial(&trial) {
                incompatible += 1;
                continue;
            }
            let duplicate = active.trials.iter().any(|known| {
                known.vehicle == trial.vehicle && known.program == trial.program && known.seed == trial.seed
            });
            if duplicate {
                continue;
            }
            active.trials.push(trial.clone());
            recorded += 1;
        }
        if recorded == 0 && incompatible == 0 {
            return None;
        }
        self.dirty = true;
        let skipped = if incompatible > 0 {
            format!(" ({incompatible} refused: airframe profile mismatch)")
        } else {
            String::new()
        };
        Some(format!("Trial evidence saved on {recorded} contract(s){skipped}"))
    }

    pub(crate) fn deliver(
        &mut self,
        day: u32,
        contract_id: Option<u32>,
        vehicle: &Vehicle,
        program: &Program,
        revision_id: Option<u32>,
    ) -> Result<Delivery, String> {
        let id = contract_id
            .or_else(|| self.selected_id())
            .ok_or_else(|| "No active contract.".to_string())?;
        let index = self
            .active
            .iter()
            .position(|active| active.offer.id == id)
            .ok_or_else(|| format!("Contract #{id} is not active."))?;
        let active = &self.active[index];
        if day > active.due_day() {
            return Err(format!("Deadline passed on day {}.", active.due_day()));
        }
        let metrics = crate::lab::DesignMetrics::of(vehicle);
        if !crate::lab::budget_ok(&active.offer, &metrics) {
            return Err(format!(
                "Design budget exceeded: {:.0} cr spent, {} cr allowed.",
                metrics.cost_cr, active.offer.design_budget_cr
            ));
        }
        let primary = crate::lab::goal_statuses(&active.offer.primary_goals, &metrics);
        let failed: Vec<String> = primary
            .iter()
            .filter(|goal| !goal.met)
            .map(|goal| goal.text())
            .collect();
        if !failed.is_empty() {
            return Err(format!("Primary design goals not met: {}.", failed.join("; ")));
        }
        let secondary = crate::lab::goal_statuses(&active.offer.secondary_goals, &metrics);
        let secondary_bonus = if !secondary.is_empty() && secondary.iter().all(|goal| goal.met) {
            active.offer.secondary_bonus_cr
        } else {
            0
        };
        let (vehicle_hash, _) = hashes(vehicle, program);
        let design = design_fingerprint(vehicle);
        if let Some(previous) = self.completed.iter().find(|contract| {
            contract.vehicle.as_ref() == Some(&design) || contract.vehicle.as_ref() == Some(&vehicle_hash)
        }) {
            return Err(format!(
                "This exact design was already delivered to contract #{}. Build a distinct airframe for another contract.",
                previous.id
            ));
        }
        let dossier = self.dossier_for(active, vehicle, program);
        if !active.offer.accepts_dossier(&dossier) {
            let reason = match &dossier.decision {
                DossierDecision::Rejected { reasons } => reasons.join("; "),
                DossierDecision::Accepted { score } => format!(
                    "score {:.0}% is below the required {}%",
                    score * 100.0,
                    active.offer.requirements.min_trial_score
                ),
            };
            return Err(format!("Dossier refused: {reason}"));
        }
        let score_percent = (dossier.score * 100.0).round().clamp(0.0, 100.0) as u8;
        let reward = quality_reward(active.offer.reward_credits, dossier.score) + secondary_bonus;
        let active = self.active.remove(index);
        let delivery = Delivery {
            id: active.offer.id,
            reward,
            score_percent,
            secondary_bonus,
        };
        self.completed.push(CompletedContract {
            id: active.offer.id,
            title: active.offer.title.clone(),
            client: active.offer.client.clone(),
            reward,
            completed_day: day,
            score_percent,
            revision_id,
            secondary_bonus,
            vehicle: Some(design),
        });
        self.selected = self.active.first().map(|active| active.offer.id);
        self.refill();
        self.dirty = true;
        Ok(delivery)
    }

    /// Drop offers the research no longer supports (never normally) and top each available family
    /// back up to two profile offers, without duplicating one already on the board or under contract.
    pub(crate) fn refill(&mut self) {
        self.offers.retain(|offer| offer.is_available(&self.known_caps));
        for family in MissionFamily::ALL {
            if !(1..=5u8).any(|difficulty| self.known_caps.contains(family.capability(difficulty))) {
                continue;
            }
            let mut used: Vec<ContractProfile> = self
                .offers
                .iter()
                .chain(self.active.iter().map(|active| &active.offer))
                .filter_map(|offer| offer.profile)
                .filter(|profile| profile.mission_family() == family)
                .collect();
            for profile in ContractProfile::available_for_family(family, &self.known_caps) {
                if used.len() >= MAX_PROFILES_PER_FAMILY {
                    break;
                }
                if used.contains(&profile) {
                    continue;
                }
                let id = self.next_id;
                self.next_id = self.next_id.saturating_add(1);
                self.offers
                    .push(offer_for_profile(self.seed, id, profile, &self.known_caps));
                used.push(profile);
            }
        }
    }

    /// Follow the research tree: newly unlocked families join the board, and the board is refilled
    /// after a delivery. Returns whether anything changed.
    pub(crate) fn sync_offers(&mut self, owned: &BTreeSet<String>) -> bool {
        if &self.known_caps == owned {
            return false;
        }
        self.known_caps = owned.clone();
        self.refill();
        self.dirty = true;
        true
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(self).map_err(|error| error.to_string())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        let schema = value
            .get("schema_version")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        if schema == 1 {
            let legacy: LegacyProcurement = serde_json::from_value(value).map_err(|error| error.to_string())?;
            let active = legacy.active.into_iter().collect();
            let mut procurement = Self {
                schema_version: SCHEMA_VERSION,
                seed: legacy.seed,
                next_id: legacy.next_id,
                offers: legacy.offers,
                active,
                selected: None,
                completed: legacy.completed,
                last_recorded: legacy.last_recorded,
                known_caps: starting_capabilities(),
                message: String::new(),
                dirty: false,
            };
            procurement.selected = procurement.selected_id();
            return Ok(procurement);
        }
        if schema != u64::from(SCHEMA_VERSION) {
            return Err(format!("unsupported missile contract schema {schema}"));
        }
        let mut loaded: Self = serde_json::from_value(value).map_err(|error| error.to_string())?;
        loaded.selected = loaded.selected_id();
        Ok(loaded)
    }
}

/// Persist the office when it changed.
pub(crate) fn persist_procurement(mut procurement: ResMut<Procurement>) {
    if !procurement.dirty || crate::save::active_slot().is_none() {
        return;
    }
    match procurement
        .to_bytes()
        .and_then(|bytes| save::write(SaveFile::Procurement, &bytes))
    {
        Ok(()) => procurement.dirty = false,
        Err(error) => warn!("Could not save the missile contracts: {error}"),
    }
}

/// Keep the board in step with the research tree (and the save, which is applied on the first frame).
pub(crate) fn sync_offers(mut procurement: ResMut<Procurement>, campus: Res<crate::campus::CampusState>) {
    procurement.sync_offers(&campus.owned_capabilities());
}

/// Turn a finished trial into contract evidence, bound to the exact design it flew.
pub(crate) fn record_trial_evidence(mut procurement: ResMut<Procurement>, trials: Res<TrialSession>) {
    let Some(run) = trials.report.as_ref() else {
        return;
    };
    // The run counter distinguishes an identical rerun from the report it replaced.
    let key = format!("{}#{}", run.input_hash, trials.runs);
    if procurement.last_recorded.as_deref() == Some(key.as_str()) {
        return;
    }
    procurement.last_recorded = Some(key);
    if procurement.active.is_empty() {
        return;
    }
    let trial = MissileTrial::from_report(
        ContentHash(run.vehicle_hash.clone()),
        ContentHash(run.program_hash.clone()),
        run.seed,
        trial_run_id(&procurement),
        &run.report,
    );
    if let Some(message) = procurement.record_trial(trial) {
        procurement.message = message;
    }
}

/// The dossier score pays: a design that only reaches the acceptance threshold earns the nominal
/// fee, and every point above it up to a perfect dossier adds a bonus of at most 15 %.
fn quality_reward(base: u64, score: f64) -> u64 {
    let above = ((score - omega_rules::dossier::ACCEPTANCE_THRESHOLD)
        / (1.0 - omega_rules::dossier::ACCEPTANCE_THRESHOLD))
        .clamp(0.0, 1.0);
    (base as f64 * (1.0 + 0.15 * above)).round() as u64
}

fn trial_run_id(procurement: &Procurement) -> u64 {
    procurement
        .active
        .iter()
        .map(|active| active.trials.len() as u64)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab::DesignMetrics;
    use omega_rules::contracts::{ContractProfile, ContractRequirements, DesignGoal, DesignGoalMetric, VehicleFamily};
    use omega_rules::{ComputerClass, Part, PartSpec, Stage};

    fn offer_for(family: VehicleFamily) -> ContractOffer {
        ContractOffer {
            id: 7,
            mission_family: MissionFamily::AirToSurface,
            vehicle_family: family,
            profile: None,
            title: "Test programme".into(),
            client: "Test client".into(),
            brief: "Test brief".into(),
            context: "Test context".into(),
            requirements: ContractRequirements {
                min_design_score: 0,
                min_program_score: 0,
                min_trial_score: 0,
                required_capabilities: Vec::new(),
            },
            design_budget_cr: 0,
            primary_goals: Vec::new(),
            secondary_goals: Vec::new(),
            secondary_bonus_cr: 0,
            requested_dossier: Vec::new(),
            reward_credits: 9_000,
            deadline_days: 10,
            difficulty: 1,
        }
    }

    fn vehicle() -> Vehicle {
        Vehicle::from_stages(
            "Test vehicle",
            vec![Stage {
                parts: vec![
                    Part {
                        id: "motor".into(),
                        spec: PartSpec::SolidMotor {
                            length_m: 2.0,
                            diameter_m: 0.4,
                            burn_time_s: 20.0,
                        },
                    },
                    Part {
                        id: "computer".into(),
                        spec: PartSpec::Computer {
                            class: ComputerClass::Basic,
                        },
                    },
                ],
            }],
        )
    }

    fn program() -> Program {
        Program::single(vec![omega_rules::Block::IgniteStage { stage: 0 }])
    }

    fn trial(vehicle: &Vehicle, program: &Program, run_id: u64) -> MissileTrial {
        let (vehicle_hash, program_hash) = hashes(vehicle, program);
        MissileTrial {
            stability: 0.0,
            lift: 0.0,
            control_authority: 0.0,
            run_id,
            vehicle: vehicle_hash,
            program: program_hash,
            seed: 1,
            path_length_m: 20_000.0,
            max_speed_ms: 400.0,
            launch_mass_kg: 120.0,
            lifted_off: true,
            separated: true,
            lock_acquired: None,
            vehicle_family: None,
            max_altitude_m: 0.0,
            discretion: 0.0,
            payload_kg: 0.0,
            payload_yield_kg: 0.0,
        }
    }

    fn profile_offer(profile: ContractProfile) -> ContractOffer {
        let mut offer = offer_for(profile.vehicle_family());
        offer.mission_family = profile.mission_family();
        offer.profile = Some(profile);
        offer.requirements.min_trial_score = 0;
        offer
    }

    fn office() -> Procurement {
        let mut procurement = Procurement::fresh(11);
        let mut offer = offer_for(VehicleFamily::ShortRange);
        offer.id = 7;
        procurement.offers = vec![offer];
        procurement.next_id = 8;
        procurement.known_caps = starting_capabilities();
        procurement
    }

    fn caps(extra: &[&str]) -> BTreeSet<String> {
        let mut owned = starting_capabilities();
        owned.extend(extra.iter().map(|c| (*c).to_string()));
        owned
    }

    #[test]
    fn accepting_moves_an_offer_to_the_active_contract() {
        let mut procurement = office();
        let owned = caps(&[]);
        assert_eq!(
            procurement.evidence_status(&vehicle(), &program()),
            EvidenceStatus::NoContract
        );
        let message = procurement.accept(0, 3, 1, &owned);
        assert!(message.contains("#7"), "{message}");
        assert!(procurement.offers().is_empty());
        assert_eq!(procurement.active()[0].due_day(), 13);
        let message = procurement.accept(0, 3, 1, &owned);
        assert!(message.contains("No offer"), "{message}");
    }

    #[test]
    fn a_second_contract_needs_the_parallel_projects_grant() {
        let mut procurement = office();
        procurement.offers.push(offer_for(VehicleFamily::ShortRange));
        procurement.offers[1].id = 8;
        let owned = caps(&[]);
        procurement.accept(0, 3, 1, &owned);
        let message = procurement.accept(0, 3, 1, &owned);
        assert!(message.contains("slot"), "{message}");
        assert_eq!(procurement.active().len(), 1);

        procurement.offers.push(offer_for(VehicleFamily::ShortRange));
        procurement.offers[0].id = 9;
        let message = procurement.accept(0, 3, 2, &owned);
        assert!(message.contains("#9"), "{message}");
        assert_eq!(procurement.active().len(), 2);
        assert_eq!(procurement.selected_id(), Some(9));
    }

    #[test]
    fn an_unresearched_family_is_refused_with_the_technology_name() {
        let mut procurement = office();
        let mut locked = offer_for(VehicleFamily::ShortRange);
        locked.id = 12;
        locked.mission_family = MissionFamily::SurfaceToAir;
        procurement.offers = vec![locked];
        let message = procurement.accept(0, 3, 1, &caps(&[]));
        assert!(message.contains("Surface-to-air family"), "{message}");
        assert!(procurement.active().is_empty());
        assert_eq!(procurement.offers().len(), 1, "the offer stays on the board");

        let message = procurement.accept(0, 3, 1, &caps(&["family:surface_to_air"]));
        assert!(message.contains("#12"), "{message}");
    }

    #[test]
    fn sync_adds_offers_only_for_newly_researched_families() {
        let mut procurement = Procurement::fresh(3);
        let before: Vec<u32> = procurement.offers().iter().map(|o| o.id).collect();
        assert!(before.len() < MissionFamily::ALL.len());
        assert!(!procurement.sync_offers(&starting_capabilities()));
        assert_eq!(
            procurement.offers().iter().map(|o| o.id).collect::<Vec<_>>(),
            before,
            "an unchanged tree keeps the board"
        );
        assert!(procurement.sync_offers(&caps(&["family:surface_to_air", "family:air_to_air"])));
        assert!(procurement.offers().len() > before.len());
        assert!(
            procurement
                .offers()
                .iter()
                .all(|offer| offer.mission_family != MissionFamily::SurfaceToAir
                    || offer.is_available(&caps(&["family:surface_to_air"])))
        );
    }

    #[test]
    fn a_refilled_board_keeps_two_profiles_per_family_at_most() {
        let mut procurement = Procurement::fresh(3);
        procurement.sync_offers(&caps(&[
            "family:cruise",
            "family:surface_to_air",
            "family:air_to_air",
            "family:anti_ship",
        ]));
        for family in MissionFamily::ALL {
            let offers: Vec<_> = procurement
                .offers()
                .iter()
                .filter(|offer| offer.mission_family == family)
                .collect();
            assert_eq!(offers.len(), 2, "{family:?}: {offers:?}");
            let profiles: Vec<ContractProfile> = offers.iter().filter_map(|offer| offer.profile).collect();
            assert_eq!(profiles.len(), 2);
            assert_ne!(profiles[0], profiles[1], "a family must not repeat a profile");
        }
    }

    #[test]
    fn a_trial_becomes_evidence_and_a_delivery_is_credited() {
        let mut procurement = office();
        let owned = caps(&[]);
        procurement.accept(0, 3, 1, &owned);
        let recorded = procurement.record_trial(trial(&vehicle(), &program(), 1));
        assert!(recorded.is_some());
        assert_eq!(
            procurement.evidence_status(&vehicle(), &program()),
            EvidenceStatus::Current
        );
        let active = procurement.selected().unwrap();
        let dossier = procurement.dossier_for(active, &vehicle(), &program());
        assert!(matches!(dossier.decision, DossierDecision::Accepted { .. }));
        let delivery = procurement.deliver(4, Some(7), &vehicle(), &program(), None).unwrap();
        assert_eq!(delivery.reward, 10_350, "a perfect dossier pays a 15 % bonus");
        assert_eq!(procurement.completed()[0].reward, delivery.reward);
        assert_eq!(procurement.completed().len(), 1);
        assert!(procurement.active().is_empty());
        assert!(
            procurement
                .offers()
                .iter()
                .any(|offer| offer.mission_family == MissionFamily::AirToSurface),
            "the delivered family gets a fresh offer"
        );
    }

    fn goal(id: &str, metric: DesignGoalMetric, limit: u32, at_most: bool) -> DesignGoal {
        DesignGoal {
            id: id.to_string(),
            label: id.to_string(),
            metric,
            limit,
            at_most,
        }
    }

    #[test]
    fn combined_goals_gate_delivery_and_secondary_goals_pay_a_bonus() {
        let metrics = DesignMetrics::of(&vehicle());
        let mut procurement = office();
        let offer = &mut procurement.offers[0];
        offer.design_budget_cr = metrics.cost_cr.ceil() as u64;
        offer.primary_goals = vec![
            goal(
                "RELIABILITY",
                DesignGoalMetric::Reliability,
                metrics.reliability.floor() as u32,
                false,
            ),
            goal(
                "COMPLEXITY",
                DesignGoalMetric::Complexity,
                metrics.complexity.ceil() as u32,
                true,
            ),
            goal("MASS", DesignGoalMetric::MassKg, metrics.mass_kg.ceil() as u32, true),
        ];
        offer.secondary_goals = vec![goal(
            "SECONDARY",
            DesignGoalMetric::Reliability,
            metrics.reliability.floor() as u32,
            false,
        )];
        offer.secondary_bonus_cr = 1_250;
        procurement.accept(0, 3, 1, &caps(&[]));
        procurement.record_trial(trial(&vehicle(), &program(), 1));
        let delivery = procurement
            .deliver(4, Some(7), &vehicle(), &program(), Some(12))
            .unwrap();
        assert_eq!(delivery.secondary_bonus, 1_250);
        assert_eq!(delivery.reward, 10_350 + 1_250);
        assert_eq!(procurement.completed()[0].revision_id, Some(12));
        assert_eq!(procurement.completed()[0].secondary_bonus, 1_250);
    }

    #[test]
    fn a_missed_primary_goal_or_budget_refuses_the_delivery() {
        let metrics = DesignMetrics::of(&vehicle());
        let mut procurement = office();
        assert!(metrics.cost_cr > 1.0, "the test design must cost credits");
        procurement.accept(0, 3, 1, &caps(&[]));
        procurement.active[0].offer.design_budget_cr = (metrics.cost_cr.floor() as u64).saturating_sub(1);
        procurement.record_trial(trial(&vehicle(), &program(), 1));
        let error = procurement
            .deliver(4, Some(7), &vehicle(), &program(), None)
            .unwrap_err();
        assert!(error.contains("budget"), "{error}");
        assert_eq!(procurement.active().len(), 1, "a refused budget keeps the contract");

        procurement.active[0].offer.design_budget_cr = 0;
        procurement.active[0].offer.primary_goals = vec![goal("IMPOSSIBLE", DesignGoalMetric::Reliability, 101, false)];
        procurement.active[0].offer.secondary_bonus_cr = 0;
        let error = procurement
            .deliver(4, Some(7), &vehicle(), &program(), None)
            .unwrap_err();
        assert!(error.contains("Primary design goals"), "{error}");
    }

    #[test]
    fn the_dossier_score_scales_the_fee_from_nominal_to_a_bonus() {
        let base = 9_000;
        assert_eq!(quality_reward(base, 0.0), base, "no bonus below the threshold");
        assert_eq!(quality_reward(base, 0.6), base, "the threshold pays the nominal fee");
        assert_eq!(quality_reward(base, 1.0), 10_350);
        assert!(quality_reward(base, 0.8) > base && quality_reward(base, 0.8) < 10_350);
        assert!(quality_reward(base, 0.8) < quality_reward(base, 0.9));
    }

    #[test]
    fn a_trial_feeds_every_active_contract() {
        let mut procurement = office();
        let owned = caps(&[]);
        procurement.offers.push(offer_for(VehicleFamily::ShortRange));
        procurement.offers[1].id = 8;
        procurement.accept(0, 3, 2, &owned);
        procurement.accept(0, 3, 2, &owned);
        assert_eq!(procurement.active().len(), 2);
        procurement.record_trial(trial(&vehicle(), &program(), 1));
        assert!(procurement.active().iter().all(|active| active.trials.len() == 1));
    }

    #[test]
    fn editing_the_vehicle_after_the_trial_makes_the_dossier_stale() {
        let mut procurement = office();
        procurement.accept(0, 3, 1, &caps(&[]));
        procurement.record_trial(trial(&vehicle(), &program(), 1));
        let mut edited = vehicle();
        edited.replace_part(
            "motor",
            PartSpec::SolidMotor {
                length_m: 2.2,
                diameter_m: 0.4,
                burn_time_s: 20.0,
            },
        );
        assert_eq!(procurement.evidence_status(&edited, &program()), EvidenceStatus::Stale);
        let error = procurement.deliver(4, Some(7), &edited, &program(), None).unwrap_err();
        assert!(error.contains("stale") || error.contains("Dossier"), "{error}");
        assert_eq!(procurement.active().len(), 1, "a refused dossier keeps the contract");
    }

    #[test]
    fn restoring_the_exact_program_revalidates_evidence_and_allows_delivery() {
        let mut procurement = office();
        procurement.accept(0, 3, 1, &caps(&[]));
        let original_program = program();
        procurement.record_trial(trial(&vehicle(), &original_program, 1));

        let edited_program = Program::single(vec![omega_rules::Block::Wait { seconds: 5.0 }]);
        assert_eq!(
            procurement.evidence_status(&vehicle(), &edited_program),
            EvidenceStatus::Stale
        );
        assert!(
            procurement
                .deliver(4, Some(7), &vehicle(), &edited_program, None)
                .is_err()
        );
        assert_eq!(procurement.active().len(), 1);

        assert_eq!(
            procurement.evidence_status(&vehicle(), &original_program),
            EvidenceStatus::Current
        );
        let delivery = procurement
            .deliver(4, Some(7), &vehicle(), &original_program, None)
            .unwrap();
        assert_eq!(delivery.reward, 10_350);
        assert!(procurement.active().is_empty());
        assert_eq!(procurement.completed().len(), 1);
    }

    #[test]
    fn a_trial_is_refused_by_a_contract_with_another_flight_profile() {
        let mut procurement = office();
        procurement.offers = vec![profile_offer(ContractProfile::LowObservableRaid)];
        procurement.offers[0].id = 7;
        procurement.accept(0, 3, 1, &caps(&["family:cruise"]));
        let mut ballistic = trial(&vehicle(), &program(), 1);
        ballistic.vehicle_family = Some(VehicleFamily::Ballistic);
        procurement.record_trial(ballistic);
        assert!(
            procurement.active()[0].trials.is_empty(),
            "a ballistic trial cannot prove a cruise raid"
        );
        let mut cruise = trial(&vehicle(), &program(), 2);
        cruise.vehicle_family = Some(VehicleFamily::Cruise);
        procurement.record_trial(cruise);
        assert_eq!(procurement.active()[0].trials.len(), 1);
    }

    #[test]
    fn one_exact_design_cannot_be_delivered_to_two_contracts() {
        let mut procurement = office();
        procurement.offers.push(offer_for(VehicleFamily::ShortRange));
        procurement.offers[1].id = 8;
        procurement.accept(0, 3, 2, &caps(&[]));
        procurement.accept(0, 3, 2, &caps(&[]));
        procurement.record_trial(trial(&vehicle(), &program(), 1));
        procurement.deliver(4, Some(7), &vehicle(), &program(), None).unwrap();
        let error = procurement
            .deliver(4, Some(8), &vehicle(), &program(), None)
            .unwrap_err();
        assert!(error.contains("already delivered"), "{error}");
        let mut renamed = vehicle();
        renamed.name = "A different label".into();
        let error = procurement.deliver(4, Some(8), &renamed, &program(), None).unwrap_err();
        assert!(
            error.contains("already delivered"),
            "renaming must not bypass the rule: {error}"
        );
        let mut copied = vehicle();
        for part in &mut copied.parts {
            let old_id = part.id.clone();
            if let Some(attach) = &mut part.attach {
                attach.parent = format!("copy-{}", attach.parent);
            }
            part.id = format!("copy-{old_id}");
        }
        let error = procurement.deliver(4, Some(8), &copied, &program(), None).unwrap_err();
        assert!(
            error.contains("already delivered"),
            "renaming every part must not bypass the rule: {error}"
        );
        assert_eq!(procurement.completed().len(), 1);
    }

    #[test]
    fn editing_the_program_and_late_delivery_are_refused() {
        let mut procurement = office();
        procurement.accept(0, 3, 1, &caps(&[]));
        procurement.record_trial(trial(&vehicle(), &program(), 1));
        let edited = Program::single(vec![
            omega_rules::Block::IgniteStage { stage: 0 },
            omega_rules::Block::Wait { seconds: 5.0 },
        ]);
        assert_eq!(procurement.evidence_status(&vehicle(), &edited), EvidenceStatus::Stale);
        let late = procurement
            .deliver(14, Some(7), &vehicle(), &program(), None)
            .unwrap_err();
        assert!(late.contains("Deadline"), "{late}");
    }

    #[test]
    fn the_same_trial_is_not_recorded_twice_and_the_office_round_trips() {
        let mut procurement = office();
        procurement.accept(0, 3, 1, &caps(&[]));
        assert!(procurement.record_trial(trial(&vehicle(), &program(), 1)).is_some());
        assert!(procurement.record_trial(trial(&vehicle(), &program(), 2)).is_none());
        let bytes = procurement.to_bytes().unwrap();
        let loaded = Procurement::from_bytes(&bytes).unwrap();
        assert_eq!(loaded.active(), procurement.active());
        assert_eq!(loaded.completed(), procurement.completed());
        assert_eq!(loaded.selected_id(), procurement.selected_id());
        let mut future = procurement.clone();
        future.schema_version = 99;
        assert!(Procurement::from_bytes(&future.to_bytes().unwrap()).is_err());
    }

    #[test]
    fn a_new_campaign_goes_from_offer_to_trial_revision_and_delivery() {
        let mut procurement = office();
        let owned = caps(&[]);
        procurement.accept(0, 3, 1, &owned);
        let vehicle = vehicle();
        let program = program();
        procurement.record_trial(trial(&vehicle, &program, 1));

        // The revision log is the workshop side of the same loop: the delivered airframe is the
        // one that was recorded, named and marked as submitted.
        let mut revisions = crate::revisions::RevisionLog::from_bytes(
            br#"{"schema_version":3,"revisions":[],"next_id":1,"head":null,"submitted":null}"#,
        )
        .unwrap();
        let revision = revisions.commit_vehicle(&vehicle).unwrap();
        revisions.rename(revision, "Campagne de test").unwrap();
        revisions.mark_submitted(revision);
        assert_eq!(revisions.submitted().unwrap().id, revision);

        let delivery = procurement
            .deliver(4, Some(7), &vehicle, &program, Some(revision))
            .unwrap();
        assert_eq!(delivery.reward, 10_350);
        assert_eq!(
            procurement.evidence_status(&vehicle, &program),
            EvidenceStatus::NoContract,
            "the contract is closed but the evidence chain remains replayable"
        );
        assert_eq!(procurement.completed().len(), 1);
        assert_eq!(revisions.len(), 1);
        assert_eq!(revisions.head().unwrap().label, "Campagne de test");
    }

    #[test]
    fn a_schema_one_office_with_one_active_contract_still_loads() {
        let legacy = serde_json::json!({
            "schema_version": 1,
            "seed": 5,
            "next_id": 9,
            "offers": [],
            "active": {
                "offer": offer_for(VehicleFamily::ShortRange),
                "accepted_day": 2,
                "trials": []
            },
            "completed": [],
            "last_recorded": null
        });
        let procurement = Procurement::from_bytes(legacy.to_string().as_bytes()).unwrap();
        assert_eq!(procurement.active().len(), 1);
        assert_eq!(procurement.selected_id(), Some(7));
        assert_eq!(procurement.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn a_fresh_office_lists_only_researched_families() {
        let procurement = Procurement::fresh(5);
        assert!(!procurement.offers().is_empty());
        assert!(
            procurement
                .offers()
                .iter()
                .all(|offer| offer.is_available(&starting_capabilities()))
        );
        assert!(procurement.completed().is_empty());
        assert!(procurement.active().is_empty());
    }
}
