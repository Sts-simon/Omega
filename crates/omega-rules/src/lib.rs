//! Bevy-independent gameplay rules for Omega.
//!
//! The crate answers one question: *given a design, what has actually been proven about it,
//! and does that satisfy a contract?* All quantities are abstract gameplay values.

pub mod contracts;
pub mod diff;
pub mod dossier;
pub mod flight;
pub mod metrics;
pub mod program;
pub mod requirement;
pub mod research;
pub mod vehicle;

pub use contracts::{
    generate_contract_board, ContractEvaluation, ContractOffer, ContractPhase, ContractProgress,
    ContractRequirements, MissionFamily, VehicleFamily,
};
pub use diff::{diff_parts, DesignDiff};
pub use dossier::{build_dossier, Dossier, DossierDecision, MatrixRow, ACCEPTANCE_THRESHOLD};
pub use metrics::{derive_metrics, DerivedMetrics, PartGeometry};
pub use flight::{simulate, FlightConfig, FlightReport, LaunchMode, LockResult, Sample};
pub use program::{validate_program, Block, Program};
pub use vehicle::{ComputerClass, Part, PartSpec, PartStats, RadarClass, SensorKind, Stage, StageReport, Vehicle};
pub use research::{default_tree, starting_capabilities, validate_tree, ActiveResearch, Department, Grant, ResearchError, ResearchState, TechNode};
pub use requirement::{
    evaluate_requirement, verification_cost, Comparator, Estimate, Evidence, Requirement,
    RequirementStatus, VerificationCost, VerificationMethod,
};
