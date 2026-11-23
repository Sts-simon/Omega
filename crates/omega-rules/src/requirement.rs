use omega_data::ContentHash;
use serde::{Deserialize, Serialize};

/// How a requirement is proven. Cheap methods carry wider uncertainty.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMethod {
    Analysis,
    Demonstration,
    Test,
}

impl VerificationMethod {
    /// Multiplier applied to the baseline (analysis-level) uncertainty of a metric.
    pub fn uncertainty_scale(self) -> f64 {
        match self {
            Self::Analysis => 1.0,
            Self::Demonstration => 0.5,
            Self::Test => 0.25,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationCost {
    pub credits: u64,
    pub days: u16,
}

/// Gameplay constants for the verification budget (not physical quantities).
pub fn verification_cost(method: VerificationMethod) -> VerificationCost {
    match method {
        VerificationMethod::Analysis => VerificationCost { credits: 200, days: 1 },
        VerificationMethod::Demonstration => VerificationCost { credits: 900, days: 3 },
        VerificationMethod::Test => VerificationCost { credits: 2_500, days: 7 },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparator {
    AtMost,
    AtLeast,
}

/// A measured or estimated value with a symmetric half-width uncertainty (>= 0).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Estimate {
    pub value: f64,
    pub uncertainty: f64,
}

impl Estimate {
    pub fn new(value: f64, uncertainty: f64) -> Result<Self, String> {
        if !value.is_finite() || !uncertainty.is_finite() || uncertainty < 0.0 {
            return Err("estimate must be finite with a non-negative uncertainty".to_string());
        }
        Ok(Self { value, uncertainty })
    }

    /// The same estimate as obtained by a different verification method.
    pub fn rescaled_for(self, method: VerificationMethod) -> Self {
        Self { value: self.value, uncertainty: self.uncertainty * method.uncertainty_scale() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Requirement {
    pub id: String,
    pub label: String,
    pub metric: String,
    pub cmp: Comparator,
    pub limit: f64,
    pub weight: f32,
    /// A hard requirement must be fully verified for the dossier to be accepted.
    pub hard: bool,
    pub allowed_methods: Vec<VerificationMethod>,
}

/// An immutable piece of proof: which design (and program) it was produced for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub requirement_id: String,
    pub method: VerificationMethod,
    pub run_id: u64,
    pub design: ContentHash,
    pub program: Option<ContentHash>,
    pub estimate: Estimate,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RequirementStatus {
    /// No usable evidence.
    Unverified,
    /// Evidence exists but was produced for a different design/program.
    Stale,
    /// Evidence used a method this requirement does not allow.
    MethodNotAllowed,
    /// Central value violates the limit.
    Failed { margin: f64 },
    /// Central value passes but the uncertainty band crosses the limit.
    Marginal { margin: f64 },
    /// The whole uncertainty band satisfies the limit.
    Verified { margin: f64 },
}

/// Signed distance to the limit of the *conservative* bound (positive = satisfied).
fn margins(req: &Requirement, estimate: Estimate) -> (f64, f64) {
    match req.cmp {
        Comparator::AtMost => (req.limit - estimate.value, req.limit - (estimate.value + estimate.uncertainty)),
        Comparator::AtLeast => (estimate.value - req.limit, (estimate.value - estimate.uncertainty) - req.limit),
    }
}

pub fn evaluate_requirement(
    req: &Requirement,
    evidence: Option<&Evidence>,
    design: &ContentHash,
    program: Option<&ContentHash>,
) -> RequirementStatus {
    let Some(ev) = evidence else {
        return RequirementStatus::Unverified;
    };
    if &ev.design != design || ev.program.as_ref() != program {
        return RequirementStatus::Stale;
    }
    if !req.allowed_methods.contains(&ev.method) {
        return RequirementStatus::MethodNotAllowed;
    }
    let (central, conservative) = margins(req, ev.estimate);
    if central < 0.0 {
        RequirementStatus::Failed { margin: central }
    } else if conservative < 0.0 {
        RequirementStatus::Marginal { margin: conservative }
    } else {
        RequirementStatus::Verified { margin: conservative }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn mass_req() -> Requirement {
        Requirement {
            id: "R1".into(),
            label: "Mass".into(),
            metric: "mass_kg".into(),
            cmp: Comparator::AtMost,
            limit: 100.0,
            weight: 1.0,
            hard: true,
            allowed_methods: vec![VerificationMethod::Analysis, VerificationMethod::Test],
        }
    }

    pub fn evidence(value: f64, unc: f64, design: &ContentHash) -> Evidence {
        Evidence {
            requirement_id: "R1".into(),
            method: VerificationMethod::Analysis,
            run_id: 1,
            design: design.clone(),
            program: None,
            estimate: Estimate::new(value, unc).unwrap(),
        }
    }

    #[test]
    fn at_most_distinguishes_verified_marginal_failed() {
        let d = ContentHash::of_bytes(b"d");
        let req = mass_req();
        let s = |v, u| evaluate_requirement(&req, Some(&evidence(v, u, &d)), &d, None);
        assert!(matches!(s(90.0, 5.0), RequirementStatus::Verified { margin } if (margin - 5.0).abs() < 1e-9));
        assert!(matches!(s(98.0, 5.0), RequirementStatus::Marginal { .. }));
        assert!(matches!(s(101.0, 0.0), RequirementStatus::Failed { .. }));
    }

    #[test]
    fn at_least_uses_the_lower_bound() {
        let d = ContentHash::of_bytes(b"d");
        let mut req = mass_req();
        req.cmp = Comparator::AtLeast;
        req.limit = 10.0;
        let s = |v, u| evaluate_requirement(&req, Some(&evidence(v, u, &d)), &d, None);
        assert!(matches!(s(15.0, 2.0), RequirementStatus::Verified { .. }));
        assert!(matches!(s(11.0, 2.0), RequirementStatus::Marginal { .. }));
        assert!(matches!(s(9.0, 0.0), RequirementStatus::Failed { .. }));
    }

    #[test]
    fn changed_design_or_program_makes_evidence_stale() {
        let d = ContentHash::of_bytes(b"d");
        let other = ContentHash::of_bytes(b"other");
        let req = mass_req();
        let ev = evidence(50.0, 1.0, &d);
        assert_eq!(evaluate_requirement(&req, Some(&ev), &other, None), RequirementStatus::Stale);
        assert_eq!(evaluate_requirement(&req, Some(&ev), &d, Some(&other)), RequirementStatus::Stale);
        assert_eq!(evaluate_requirement(&req, None, &d, None), RequirementStatus::Unverified);
    }

    #[test]
    fn disallowed_method_is_flagged() {
        let d = ContentHash::of_bytes(b"d");
        let req = mass_req();
        let mut ev = evidence(50.0, 1.0, &d);
        ev.method = VerificationMethod::Demonstration;
        assert_eq!(evaluate_requirement(&req, Some(&ev), &d, None), RequirementStatus::MethodNotAllowed);
    }

    #[test]
    fn tests_tighten_uncertainty_and_cost_more_than_analysis() {
        let e = Estimate::new(97.0, 4.0).unwrap();
        assert!(e.rescaled_for(VerificationMethod::Test).uncertainty < e.uncertainty);
        assert!(verification_cost(VerificationMethod::Test).credits > verification_cost(VerificationMethod::Analysis).credits);
        assert!(verification_cost(VerificationMethod::Test).days > verification_cost(VerificationMethod::Analysis).days);
    }

    #[test]
    fn invalid_estimates_are_rejected() {
        assert!(Estimate::new(f64::NAN, 1.0).is_err());
        assert!(Estimate::new(1.0, -0.1).is_err());
    }
}
