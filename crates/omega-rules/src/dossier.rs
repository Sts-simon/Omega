use crate::requirement::{Evidence, Requirement, RequirementStatus, evaluate_requirement};
use omega_data::ContentHash;
use serde::{Deserialize, Serialize};

/// Minimum weighted score (0..1) for a dossier whose hard requirements are all verified.
pub const ACCEPTANCE_THRESHOLD: f64 = 0.6;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatrixRow {
    pub requirement_id: String,
    pub label: String,
    pub hard: bool,
    pub status: RequirementStatus,
    pub run_id: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum DossierDecision {
    Accepted { score: f64 },
    Rejected { reasons: Vec<String> },
}

/// The deliverable generated from a design and the evidence gathered for it. It is never
/// typed in by the player: it is a pure function of its inputs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dossier {
    pub contract_id: u32,
    pub design: ContentHash,
    pub program: Option<ContentHash>,
    pub matrix: Vec<MatrixRow>,
    pub score: f64,
    pub decision: DossierDecision,
}

fn credit(status: &RequirementStatus, hard: bool) -> f64 {
    match status {
        RequirementStatus::Verified { .. } => 1.0,
        // A thin margin earns partial credit on soft requirements only.
        RequirementStatus::Marginal { .. } if !hard => 0.5,
        _ => 0.0,
    }
}

fn describe(status: &RequirementStatus) -> &'static str {
    match status {
        RequirementStatus::Unverified => "not verified",
        RequirementStatus::Stale => "evidence is stale (design or program changed)",
        RequirementStatus::MethodNotAllowed => "verification method not allowed",
        RequirementStatus::Failed { .. } => "limit exceeded",
        RequirementStatus::Marginal { .. } => "margin too thin for the uncertainty",
        RequirementStatus::Verified { .. } => "verified",
    }
}

/// Build the dossier. When several pieces of evidence exist for one requirement, the most
/// recent run (highest `run_id`) whose design/program hashes match is used; if none matches,
/// the latest one is reported as stale.
pub fn build_dossier(
    contract_id: u32,
    requirements: &[Requirement],
    evidence: &[Evidence],
    design: &ContentHash,
    program: Option<&ContentHash>,
) -> Dossier {
    let mut matrix = Vec::with_capacity(requirements.len());
    let mut weighted = 0.0;
    let mut total_weight = 0.0;
    let mut reasons = Vec::new();

    for req in requirements {
        let for_req: Vec<&Evidence> = evidence.iter().filter(|e| e.requirement_id == req.id).collect();
        let chosen = for_req
            .iter()
            .copied()
            .filter(|e| &e.design == design && e.program.as_ref() == program)
            .max_by_key(|e| e.run_id)
            .or_else(|| for_req.iter().copied().max_by_key(|e| e.run_id));
        let status = evaluate_requirement(req, chosen, design, program);
        let weight = f64::from(req.weight.max(0.0));
        weighted += weight * credit(&status, req.hard);
        total_weight += weight;
        if req.hard && !matches!(status, RequirementStatus::Verified { .. }) {
            reasons.push(format!("{} ({}): {}", req.id, req.label, describe(&status)));
        }
        matrix.push(MatrixRow {
            requirement_id: req.id.clone(),
            label: req.label.clone(),
            hard: req.hard,
            status,
            run_id: chosen.map(|e| e.run_id),
        });
    }

    let score = if total_weight > 0.0 {
        weighted / total_weight
    } else {
        0.0
    };
    if requirements.is_empty() {
        reasons.push("the contract defines no requirements".to_string());
    } else if reasons.is_empty() && score < ACCEPTANCE_THRESHOLD {
        reasons.push(format!(
            "score {:.0}% is below the {:.0}% threshold",
            score * 100.0,
            ACCEPTANCE_THRESHOLD * 100.0
        ));
    }
    let decision = if reasons.is_empty() {
        DossierDecision::Accepted { score }
    } else {
        DossierDecision::Rejected { reasons }
    };
    Dossier {
        contract_id,
        design: design.clone(),
        program: program.cloned(),
        matrix,
        score,
        decision,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::requirement::Comparator;
    use crate::requirement::tests::{evidence, mass_req};

    fn soft(id: &str, limit: f64) -> Requirement {
        let mut r = mass_req();
        r.id = id.into();
        r.hard = false;
        r.limit = limit;
        r
    }

    #[test]
    fn fully_verified_dossier_is_accepted() {
        let d = ContentHash::of_bytes(b"d");
        let dossier = build_dossier(1, &[mass_req()], &[evidence(80.0, 2.0, &d)], &d, None);
        assert!(matches!(dossier.decision, DossierDecision::Accepted { score } if (score - 1.0).abs() < 1e-9));
    }

    #[test]
    fn missing_or_stale_hard_evidence_rejects_with_a_reason() {
        let d = ContentHash::of_bytes(b"d");
        let other = ContentHash::of_bytes(b"other");
        let none = build_dossier(1, &[mass_req()], &[], &d, None);
        assert!(matches!(&none.decision, DossierDecision::Rejected { reasons } if reasons[0].contains("not verified")));
        let stale = build_dossier(1, &[mass_req()], &[evidence(80.0, 2.0, &other)], &d, None);
        assert!(matches!(&stale.decision, DossierDecision::Rejected { reasons } if reasons[0].contains("stale")));
    }

    #[test]
    fn marginal_hard_requirement_rejects_but_marginal_soft_gives_half_credit() {
        let d = ContentHash::of_bytes(b"d");
        let hard = build_dossier(1, &[mass_req()], &[evidence(98.0, 5.0, &d)], &d, None);
        assert!(matches!(hard.decision, DossierDecision::Rejected { .. }));

        let mut ok = mass_req();
        ok.id = "R0".into();
        let mut ev0 = evidence(10.0, 0.0, &d);
        ev0.requirement_id = "R0".into();
        let mut ev_soft = evidence(98.0, 5.0, &d);
        ev_soft.requirement_id = "S1".into();
        let dossier = build_dossier(1, &[ok, soft("S1", 100.0)], &[ev0, ev_soft], &d, None);
        assert!((dossier.score - 0.75).abs() < 1e-9, "score {}", dossier.score);
        assert!(matches!(dossier.decision, DossierDecision::Accepted { .. }));
    }

    #[test]
    fn low_score_on_soft_requirements_is_rejected_by_threshold() {
        let d = ContentHash::of_bytes(b"d");
        let reqs = [soft("S1", 100.0), soft("S2", 100.0)];
        let mut e1 = evidence(50.0, 0.0, &d);
        e1.requirement_id = "S1".into();
        let dossier = build_dossier(1, &reqs, &[e1], &d, None);
        assert!((dossier.score - 0.5).abs() < 1e-9);
        assert!(matches!(&dossier.decision, DossierDecision::Rejected { reasons } if reasons[0].contains("threshold")));
    }

    #[test]
    fn latest_matching_run_wins_and_empty_contract_is_rejected() {
        let d = ContentHash::of_bytes(b"d");
        let mut old = evidence(150.0, 0.0, &d);
        old.run_id = 1;
        let mut new = evidence(60.0, 0.0, &d);
        new.run_id = 2;
        let dossier = build_dossier(1, &[mass_req()], &[old, new], &d, None);
        assert_eq!(dossier.matrix[0].run_id, Some(2));
        assert!(matches!(dossier.decision, DossierDecision::Accepted { .. }));
        let empty = build_dossier(1, &[], &[], &d, None);
        assert!(matches!(empty.decision, DossierDecision::Rejected { .. }));
        let _ = Comparator::AtMost;
    }
}
