//! Bevy-independent gameplay rules for Omega.
//!
//! The crate answers one question: *given a design, what has actually been proven about it,
//! and does that satisfy a contract?* All quantities are abstract gameplay values.

pub mod aero;
pub mod assembly;
pub mod avionics;
pub mod compliance;
pub mod contracts;
pub mod diff;
pub mod dossier;
pub mod flight;
pub mod metrics;
pub mod program;
pub mod requirement;
pub mod research;
pub mod vehicle;

pub use aero::{AeroFigures, AeroSummary, SurfaceContribution};
pub use assembly::{
    Attach, IssueSeverity, NodeId, PartPose, PlacedPart, TreeEntry, VEHICLE_SCHEMA, Vehicle, VehicleIssue,
    VehicleWarning,
};
pub use avionics::{AvionicsCategory, AvionicsFigures, AvionicsModule, AvionicsSpec};
pub use compliance::{CheckStatus, ContractCheck, contract_preview};
pub use contracts::{
    ContractEvaluation, ContractOffer, ContractPhase, ContractProgress, ContractRequirements, MissionFamily,
    VehicleFamily, generate_contract_board,
};
pub use diff::{DesignDiff, diff_parts};
pub use dossier::{ACCEPTANCE_THRESHOLD, Dossier, DossierDecision, MatrixRow, build_dossier};
pub use flight::{FlightConfig, FlightReport, LaunchMode, LockResult, Sample, simulate};
pub use metrics::{DerivedMetrics, PartGeometry, derive_metrics};
pub use program::{Block, Program, validate_program};
pub use requirement::{
    Comparator, Estimate, Evidence, Requirement, RequirementStatus, VerificationCost, VerificationMethod,
    evaluate_requirement, verification_cost,
};
pub use research::{
    ActiveResearch, Department, Grant, ResearchError, ResearchState, TechNode, default_tree, starting_capabilities,
    validate_tree,
};
pub use vehicle::{
    AirfoilProfile, ComputerClass, ControlKind, FinLayout, Part, PartCategory, PartSpec, PartStats, RadarClass,
    SensorKind, StabilizerRole, Stage, StageReport, WingPlanform,
};
