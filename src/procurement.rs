//! Missile contract office. Offers name a missile family, and the delivery dossier is built from
//! the player's own vehicle, onboard program and trials. Every proof is bound to the vehicle and
//! program hashes that produced it, so editing either after a trial makes the dossier stale.
//!
//! Everything here is fictional and abstract; see `docs/VISION_MISSILES.md`.

use crate::save::{self, SaveFile};
use crate::trials::TrialSession;
use bevy::prelude::*;
use omega_data::{ContentHash, hash_serializable};
use omega_rules::contracts::{ContractOffer, MissileTrial, generate_contract_board};
use omega_rules::{Dossier, DossierDecision, Program, Vehicle};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: u32 = 1;

fn system_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0xA3E0_1AB5_2026_0001)
}

/// The vehicle and program hashes a dossier is checked against.
pub(crate) fn hashes(vehicle: &Vehicle, program: &Program) -> (ContentHash, ContentHash) {
    let vehicle = hash_serializable(vehicle).unwrap_or_else(|_| ContentHash::of_bytes(b"vehicle"));
    let program = hash_serializable(program).unwrap_or_else(|_| ContentHash::of_bytes(b"program"));
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
}

#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Procurement {
    schema_version: u32,
    seed: u64,
    next_id: u32,
    offers: Vec<ContractOffer>,
    active: Option<ActiveContract>,
    completed: Vec<CompletedContract>,
    /// Last trial already folded into evidence, so a report is not recorded twice.
    last_recorded: Option<String>,
    #[serde(skip)]
    pub(crate) message: String,
    #[serde(skip)]
    dirty: bool,
}

impl Default for Procurement {
    fn default() -> Self {
        match save::read(SaveFile::Procurement) {
            Ok(Some(bytes)) => match Self::from_bytes(&bytes) {
                Ok(loaded) => {
                    info!("Loaded {} missile contract(s)", loaded.completed.len());
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

impl Procurement {
    pub(crate) fn fresh(seed: u64) -> Self {
        let offers = generate_contract_board(seed, 1);
        let next_id = 1 + offers.len() as u32;
        Self {
            schema_version: SCHEMA_VERSION,
            seed,
            next_id,
            offers,
            active: None,
            completed: Vec::new(),
            last_recorded: None,
            message: "Accept a missile contract, then fly its trial.".to_string(),
            dirty: true,
        }
    }

    pub fn offers(&self) -> &[ContractOffer] {
        &self.offers
    }

    pub fn active(&self) -> Option<&ActiveContract> {
        self.active.as_ref()
    }

    pub fn completed(&self) -> &[CompletedContract] {
        &self.completed
    }

    /// Which contract is waiting for the current design and program.
    pub(crate) fn evidence_status(&self, vehicle: &Vehicle, program: &Program) -> EvidenceStatus {
        let Some(active) = &self.active else {
            return EvidenceStatus::NoContract;
        };
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

    /// Everything the contract board shows, so it rebuilds only when something changed.
    pub(crate) fn signature(&self, vehicle: &Vehicle, program: &Program) -> String {
        let active = self.active.as_ref().map(|active| {
            format!(
                "{}:{}:{}",
                active.offer.id,
                active.trials.len(),
                active.trials.last().map_or(String::new(), |trial| format!(
                    "{}-{}",
                    trial.vehicle.0, trial.program.0
                ))
            )
        });
        format!(
            "{:?}|{:?}|{:?}|{:?}|{:?}",
            active,
            self.offers.iter().map(|offer| offer.id).collect::<Vec<_>>(),
            self.completed.len(),
            hashes(vehicle, program),
            self.evidence_status(vehicle, program)
        )
    }

    pub(crate) fn dossier(&self, vehicle: &Vehicle, program: &Program) -> Option<Dossier> {
        let active = self.active.as_ref()?;
        let (vehicle_hash, program_hash) = hashes(vehicle, program);
        Some(
            active
                .offer
                .missile_dossier(&active.trials, &vehicle_hash, &program_hash),
        )
    }

    pub(crate) fn accept(&mut self, index: usize, day: u32) -> String {
        if self.active.is_some() {
            return "Deliver the active contract first.".to_string();
        }
        if index >= self.offers.len() {
            return "No offer at this position.".to_string();
        }
        let offer = self.offers.remove(index);
        let id = offer.id;
        self.active = Some(ActiveContract {
            offer,
            accepted_day: day,
            trials: Vec::new(),
        });
        self.dirty = true;
        format!("Accepted contract #{id}")
    }

    /// Fold a finished trial into the active contract. Returns a message when it was recorded.
    pub(crate) fn record_trial(&mut self, trial: MissileTrial) -> Option<String> {
        let active = self.active.as_mut()?;
        let duplicate = active
            .trials
            .iter()
            .any(|known| known.vehicle == trial.vehicle && known.program == trial.program && known.seed == trial.seed);
        if duplicate {
            return None;
        }
        active.trials.push(trial);
        self.dirty = true;
        Some(format!("Trial evidence saved ({} run)", active.trials.len()))
    }

    pub(crate) fn deliver(&mut self, day: u32, vehicle: &Vehicle, program: &Program) -> Result<Delivery, String> {
        let Some(active) = self.active.as_ref() else {
            return Err("No active contract.".to_string());
        };
        if day > active.due_day() {
            return Err(format!("Deadline passed on day {}.", active.due_day()));
        }
        let dossier = self
            .dossier(vehicle, program)
            .ok_or_else(|| "No active contract.".to_string())?;
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
        let active = self.active.take().expect("checked above");
        let delivery = Delivery {
            id: active.offer.id,
            reward: active.offer.reward_credits,
            score_percent,
        };
        self.completed.push(CompletedContract {
            id: active.offer.id,
            title: active.offer.title.clone(),
            client: active.offer.client.clone(),
            reward: active.offer.reward_credits,
            completed_day: day,
            score_percent,
        });
        // A new board so the office never runs dry; ids keep growing.
        self.seed = self.seed.wrapping_add(1);
        self.offers = generate_contract_board(self.seed, self.next_id);
        self.next_id = self.next_id.saturating_add(self.offers.len() as u32);
        self.dirty = true;
        Ok(delivery)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(self).map_err(|error| error.to_string())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let loaded: Self = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        if loaded.schema_version != SCHEMA_VERSION {
            return Err(format!("unsupported missile contract schema {}", loaded.schema_version));
        }
        Ok(loaded)
    }
}

/// Persist the office when it changed.
pub(crate) fn persist_procurement(mut procurement: ResMut<Procurement>) {
    if !procurement.dirty {
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
    let Some(active) = procurement.active.as_ref() else {
        return;
    };
    let run_id = active.trials.len() as u64 + 1;
    let trial = MissileTrial::from_report(
        ContentHash(run.vehicle_hash.clone()),
        ContentHash(run.program_hash.clone()),
        run.seed,
        run_id,
        &run.report,
    );
    if let Some(message) = procurement.record_trial(trial) {
        procurement.message = message;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_rules::contracts::{ContractRequirements, MissionFamily, VehicleFamily};
    use omega_rules::{ComputerClass, Part, PartSpec, Stage};

    fn offer_for(family: VehicleFamily) -> ContractOffer {
        ContractOffer {
            id: 7,
            mission_family: MissionFamily::AirToSurface,
            vehicle_family: family,
            title: "Test programme".into(),
            client: "Test client".into(),
            brief: "Test brief".into(),
            requirements: ContractRequirements {
                min_design_score: 0,
                min_program_score: 0,
                min_trial_score: 0,
                required_capabilities: Vec::new(),
            },
            reward_credits: 9_000,
            deadline_days: 10,
            difficulty: 1,
        }
    }

    fn vehicle() -> Vehicle {
        Vehicle {
            name: "Test vehicle".into(),
            stages: vec![Stage {
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
        }
    }

    fn program() -> Program {
        Program::single(vec![omega_rules::Block::IgniteStage { stage: 0 }])
    }

    fn trial(vehicle: &Vehicle, program: &Program, run_id: u64) -> MissileTrial {
        let (vehicle_hash, program_hash) = hashes(vehicle, program);
        MissileTrial {
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
        }
    }

    fn office() -> Procurement {
        let mut procurement = Procurement::fresh(11);
        procurement.offers = vec![offer_for(VehicleFamily::ShortRange)];
        procurement.next_id = 8;
        procurement
    }

    #[test]
    fn accepting_moves_an_offer_to_the_active_contract() {
        let mut procurement = office();
        assert_eq!(
            procurement.evidence_status(&vehicle(), &program()),
            EvidenceStatus::NoContract
        );
        let message = procurement.accept(0, 3);
        assert!(message.contains("#7"), "{message}");
        assert!(procurement.offers().is_empty());
        assert_eq!(procurement.active().unwrap().due_day(), 13);
        assert!(procurement.accept(0, 3).contains("Deliver"));
    }

    #[test]
    fn a_trial_becomes_evidence_and_a_delivery_is_credited() {
        let mut procurement = office();
        procurement.accept(0, 3);
        let recorded = procurement.record_trial(trial(&vehicle(), &program(), 1));
        assert!(recorded.is_some());
        assert_eq!(
            procurement.evidence_status(&vehicle(), &program()),
            EvidenceStatus::Current
        );
        let dossier = procurement.dossier(&vehicle(), &program()).unwrap();
        assert!(matches!(dossier.decision, DossierDecision::Accepted { .. }));
        let delivery = procurement.deliver(4, &vehicle(), &program()).unwrap();
        assert_eq!(delivery.reward, 9_000);
        assert_eq!(procurement.completed().len(), 1);
        assert!(procurement.active().is_none());
        assert_eq!(procurement.offers().len(), 4, "a fresh board follows");
    }

    #[test]
    fn editing_the_vehicle_after_the_trial_makes_the_dossier_stale() {
        let mut procurement = office();
        procurement.accept(0, 3);
        procurement.record_trial(trial(&vehicle(), &program(), 1));
        let mut edited = vehicle();
        edited.stages[0].parts[0].spec = PartSpec::SolidMotor {
            length_m: 2.2,
            diameter_m: 0.4,
            burn_time_s: 20.0,
        };
        assert_eq!(procurement.evidence_status(&edited, &program()), EvidenceStatus::Stale);
        let error = procurement.deliver(4, &edited, &program()).unwrap_err();
        assert!(error.contains("stale") || error.contains("Dossier"), "{error}");
        assert!(procurement.active().is_some(), "a refused dossier keeps the contract");
    }

    #[test]
    fn editing_the_program_and_late_delivery_are_refused() {
        let mut procurement = office();
        procurement.accept(0, 3);
        procurement.record_trial(trial(&vehicle(), &program(), 1));
        let other = Program::single(vec![omega_rules::Block::IgniteStage { stage: 0 }]);
        let _ = other;
        let edited = Program::single(vec![
            omega_rules::Block::IgniteStage { stage: 0 },
            omega_rules::Block::Wait { seconds: 5.0 },
        ]);
        assert_eq!(procurement.evidence_status(&vehicle(), &edited), EvidenceStatus::Stale);
        let late = procurement.deliver(14, &vehicle(), &program()).unwrap_err();
        assert!(late.contains("Deadline"), "{late}");
    }

    #[test]
    fn the_same_trial_is_not_recorded_twice_and_the_office_round_trips() {
        let mut procurement = office();
        procurement.accept(0, 3);
        assert!(procurement.record_trial(trial(&vehicle(), &program(), 1)).is_some());
        assert!(procurement.record_trial(trial(&vehicle(), &program(), 2)).is_none());
        let bytes = procurement.to_bytes().unwrap();
        let loaded = Procurement::from_bytes(&bytes).unwrap();
        assert_eq!(loaded.active(), procurement.active());
        assert_eq!(loaded.completed(), procurement.completed());
        let mut future = procurement.clone();
        future.schema_version = 99;
        assert!(Procurement::from_bytes(&future.to_bytes().unwrap()).is_err());
    }

    #[test]
    fn a_fresh_office_has_four_offers() {
        let procurement = Procurement::fresh(5);
        assert_eq!(procurement.offers().len(), 4);
        assert!(procurement.completed().is_empty());
        assert!(procurement.active().is_none());
    }
}
