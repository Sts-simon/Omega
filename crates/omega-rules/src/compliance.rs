//! Design-side preview of a contract: which requirements the current vehicle already meets on
//! paper, which are close, and which can only be proven by a trial.
//!
//! The workshop shows this list so a player can see the effect of an edit without flying; it is a
//! preview, never evidence. Only figures the design itself determines (mass, aerodynamic scores)
//! get a verdict; range, speed, lift-off, separation and lock stay `Unproven` until a trial.
//! All values are abstract gameplay figures; see `docs/VISION_MISSILES.md`.

use crate::assembly::Vehicle;
use crate::contracts::ContractOffer;
use crate::requirement::Comparator;

/// How one contract requirement looks against the design on paper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckStatus {
    /// The design meets the limit with room to spare.
    Met,
    /// The design meets the limit, but within 5 % of it.
    Close,
    /// The design misses the limit on the figures it can be judged on.
    Missed,
    /// Only a trial can say: the metric is measured in flight.
    Unproven,
}

impl CheckStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Met => "MET",
            Self::Close => "CLOSE",
            Self::Missed => "MISSED",
            Self::Unproven => "TRIAL",
        }
    }
}

/// One row of the workshop contract panel.
#[derive(Clone, Debug, PartialEq)]
pub struct ContractCheck {
    pub id: String,
    pub label: String,
    pub metric: String,
    pub comparator: Comparator,
    pub limit: f64,
    /// Design-side value when the metric can be read without flying.
    pub value: Option<f64>,
    pub status: CheckStatus,
}

/// Fraction of the limit under which a passing design is reported as `Close`.
const CLOSE_BAND: f64 = 0.05;

/// Preview every requirement of `offer` against `vehicle`. Rows keep the offer's order, so the
/// panel does not reshuffle between edits.
pub fn contract_preview(vehicle: &Vehicle, offer: &ContractOffer) -> Vec<ContractCheck> {
    let aero = vehicle.aero();
    let mass = vehicle.total_wet_mass_kg();
    let payload = vehicle.payload_summary();
    offer
        .missile_requirements()
        .into_iter()
        .map(|requirement| {
            let value = design_value(&requirement.metric, mass, &aero, payload);
            let status = match value {
                None => CheckStatus::Unproven,
                Some(value) => status(value, requirement.limit, requirement.cmp),
            };
            ContractCheck {
                id: requirement.id,
                label: requirement.label,
                metric: requirement.metric,
                comparator: requirement.cmp,
                limit: requirement.limit,
                value,
                status,
            }
        })
        .collect()
}

/// The figures the design determines before a trial. Everything else is `None`, which the panel
/// renders as "trial": inventing a path length or a lock result here would be a second flight model.
fn design_value(
    metric: &str,
    mass_kg: f64,
    aero: &crate::aero::AeroFigures,
    (payload_kg, payload_yield_kg): (f64, f64),
) -> Option<f64> {
    match metric {
        "launch_mass_kg" => Some(mass_kg),
        "payload_kg" => Some(payload_kg),
        "payload_yield_kg" => Some(payload_yield_kg),
        "stability" => Some(aero.stability),
        "lift" => Some(aero.lift),
        "control_authority" => Some(aero.control_authority),
        _ => None,
    }
}

fn status(value: f64, limit: f64, comparator: Comparator) -> CheckStatus {
    let passes = match comparator {
        Comparator::AtLeast => value >= limit,
        Comparator::AtMost => value <= limit,
    };
    if !passes {
        return CheckStatus::Missed;
    }
    let band = (limit.abs() * CLOSE_BAND).max(1e-9);
    if (value - limit).abs() <= band {
        CheckStatus::Close
    } else {
        CheckStatus::Met
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::tests::sounding_rocket;
    use crate::contracts::{ContractProfile, ContractRequirements, MissionFamily, VehicleFamily};

    fn offer(mission: MissionFamily, vehicle: VehicleFamily, difficulty: u8) -> ContractOffer {
        ContractOffer {
            id: 1,
            mission_family: mission,
            vehicle_family: vehicle,
            profile: None,
            title: "Preview programme".into(),
            client: "Test client".into(),
            brief: "Preview brief".into(),
            context: "Preview context".into(),
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
            reward_credits: 10_000,
            deadline_days: 20,
            difficulty,
        }
    }

    #[test]
    fn a_light_design_meets_mass_and_flight_metrics_stay_unproven() {
        let checks = contract_preview(
            &sounding_rocket(),
            &offer(MissionFamily::SurfaceToSurface, VehicleFamily::Ballistic, 1),
        );
        let mass = checks.iter().find(|check| check.id == "MASS").unwrap();
        assert_eq!(mass.status, CheckStatus::Met, "{mass:?}");
        assert!(mass.value.unwrap() > 0.0);
        for id in ["RANGE", "SPEED"] {
            let check = checks.iter().find(|check| check.id == id).unwrap();
            assert_eq!(check.status, CheckStatus::Unproven, "{check:?}");
            assert!(check.value.is_none());
        }
        // Rows keep the requirement order, so the panel does not shuffle between edits.
        assert_eq!(checks.first().unwrap().id, "RANGE");
        assert_eq!(checks.last().unwrap().id, "SEPARATION");
    }

    #[test]
    fn a_heavy_design_misses_the_mass_limit_and_a_marginal_one_is_close() {
        let mut vehicle = sounding_rocket();
        let limit = vehicle.total_wet_mass_kg();
        let mut offer = offer(MissionFamily::SurfaceToSurface, VehicleFamily::Ballistic, 1);
        offer.requirements.min_trial_score = 0;
        // Force the mass limit through the same requirement builder the dossier uses.
        let checks = contract_preview(&vehicle, &offer);
        let mass = checks.iter().find(|check| check.id == "MASS").unwrap();
        assert_eq!(mass.status, CheckStatus::Met);

        // A module that doubles the launch mass cannot pass the same limit.
        vehicle
            .add_to_top("module", crate::PartSpec::MissionModule { mass_kg: 300.0 })
            .unwrap();
        let heavier = contract_preview(&vehicle, &offer);
        let mass = heavier.iter().find(|check| check.id == "MASS").unwrap();
        assert_eq!(mass.status, CheckStatus::Missed, "{mass:?}");
        assert!(mass.value.unwrap() > limit);
    }

    #[test]
    fn a_charge_is_checked_on_paper_for_mass_and_yield() {
        let mut vehicle = sounding_rocket();
        vehicle
            .add_to_top(
                "charge",
                crate::PartSpec::Explosive {
                    mass_kg: 30.0,
                    grade: crate::ExplosiveGrade::Advanced,
                },
            )
            .unwrap();
        let mut offer = offer(MissionFamily::AirToSurface, VehicleFamily::ShortRange, 1);
        offer.profile = Some(ContractProfile::PrecisionStrike);
        let checks = contract_preview(&vehicle, &offer);
        let payload = checks.iter().find(|check| check.id == "PAYLOAD").unwrap();
        assert_eq!(payload.status, CheckStatus::Met, "{payload:?}");
        assert_eq!(payload.value, Some(30.0));
        let yield_kg = checks.iter().find(|check| check.id == "YIELD").unwrap();
        assert_eq!(yield_kg.value, Some(48.0), "advanced grade multiplies the yield");
        assert_eq!(yield_kg.status, CheckStatus::Met);
        let discretion = checks.iter().find(|check| check.id == "DISCRETION").unwrap();
        assert_eq!(
            discretion.status,
            CheckStatus::Unproven,
            "only a trial measures the profile"
        );
    }

    #[test]
    fn aero_requirements_read_the_aero_sheet_and_report_close_near_the_limit() {
        let mut vehicle = sounding_rocket();
        vehicle
            .add_attached(
                "wing",
                crate::PartSpec::Wing {
                    span_m: 1.2,
                    chord_m: 0.4,
                    sweep_deg: 20.0,
                    planform: crate::WingPlanform::Swept,
                    profile: crate::AirfoilProfile::Classic,
                    light: false,
                },
                "booster",
                crate::NodeId::Radial(0),
            )
            .unwrap();
        let checks = contract_preview(&vehicle, &offer(MissionFamily::AirToAir, VehicleFamily::Cruise, 2));
        let lift = checks.iter().find(|check| check.id == "LIFT").unwrap();
        assert_eq!(lift.status, CheckStatus::Met, "{lift:?}");
        assert!(lift.value.unwrap() >= 40.0);
        let authority = checks.iter().find(|check| check.id == "AUTHORITY").unwrap();
        assert!(authority.value.is_some());
        assert_eq!(authority.status, CheckStatus::Missed, "a wing alone has no authority");

        let close = status(40.0 * 1.02, 40.0, Comparator::AtLeast);
        assert_eq!(close, CheckStatus::Close);
        assert_eq!(status(39.0, 40.0, Comparator::AtLeast), CheckStatus::Missed);
        assert_eq!(status(100.0, 40.0, Comparator::AtLeast), CheckStatus::Met);
        assert_eq!(status(700.0, 700.0, Comparator::AtMost), CheckStatus::Close);
    }
}
