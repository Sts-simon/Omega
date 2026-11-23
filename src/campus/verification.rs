//! Contract verification: contract limits become requirements, every check produces evidence
//! bound to the hash of the design it was made for, and the delivery dossier is generated from
//! that evidence (`omega-rules`). All uncertainties are abstract gameplay values except the
//! field-test one, which is the standard error of the 1,000-sample measurement.

use super::*;
use omega_data::ContentHash;
use omega_rules::{Dossier, DossierDecision, Requirement, VerificationMethod, build_dossier};

fn requirement(
    id: &str,
    label: &str,
    metric: &str,
    cmp: Comparator,
    limit: f64,
    methods: &[VerificationMethod],
) -> Requirement {
    Requirement {
        id: id.to_string(),
        label: label.to_string(),
        metric: metric.to_string(),
        cmp,
        limit,
        weight: 1.0,
        hard: true,
        allowed_methods: methods.to_vec(),
    }
}

/// The requirements a contract imposes. Reliability figures can only be proven by a field test.
pub(super) fn requirements_for(offer: &ContractOffer) -> Vec<Requirement> {
    use Comparator::{AtLeast, AtMost};
    use VerificationMethod::{Analysis, Test};
    let r = &offer.requirements;
    let mut list = vec![
        requirement(
            "MASS",
            "Unit mass (kg)",
            "mass_kg",
            AtMost,
            f64::from(r.max_unit_mass_kg),
            &[Analysis],
        ),
        requirement(
            "COST",
            "Unit cost (cr)",
            "unit_cost_cr",
            AtMost,
            r.max_unit_cost_cr as f64,
            &[Analysis],
        ),
        requirement(
            "POWER",
            "Power draw (W)",
            "power_draw_w",
            AtMost,
            f64::from(r.max_unit_power_w),
            &[Analysis],
        ),
        requirement(
            "TEMP_MIN",
            "Operating minimum (C)",
            "temp_min_c",
            AtMost,
            f64::from(r.temperature_min_c),
            &[Analysis],
        ),
        requirement(
            "TEMP_MAX",
            "Operating maximum (C)",
            "temp_max_c",
            AtLeast,
            f64::from(r.temperature_max_c),
            &[Analysis],
        ),
        requirement(
            "STORAGE",
            "Storage (GB)",
            "storage_gb",
            AtLeast,
            f64::from(r.min_storage_gb),
            &[Analysis],
        ),
        requirement(
            "UPTIME",
            "Uptime (%)",
            "uptime_percent",
            AtLeast,
            f64::from(r.min_uptime_percent),
            &[Test],
        ),
        requirement(
            "DATA",
            "Data completeness (%)",
            "data_completeness_percent",
            AtLeast,
            f64::from(r.min_data_completeness_percent),
            &[Test],
        ),
    ];
    if let Some(limit) = r.max_cg_offset_ratio {
        list.insert(
            1,
            requirement(
                "BALANCE",
                "CG offset / length",
                "cg_offset_ratio",
                AtMost,
                limit,
                &[Analysis],
            ),
        );
    }
    list
}

fn evidence(
    id: &str,
    method: VerificationMethod,
    run_id: u64,
    design: &ContentHash,
    value: f64,
    uncertainty: f64,
) -> Evidence {
    Evidence {
        requirement_id: id.to_string(),
        method,
        run_id,
        design: design.clone(),
        program: None,
        estimate: Estimate {
            value,
            uncertainty: uncertainty.max(0.0),
        },
    }
}

/// Evidence obtainable at design submission: geometry-derived figures (with their analysis
/// uncertainty) and the profile values the player declared (exact).
pub(super) fn analysis_evidence(
    offer: &ContractOffer,
    assembly: &Assembly,
    profile: &InstrumentProfile,
    design: &ContentHash,
    run_id: u64,
) -> Result<Vec<Evidence>, String> {
    let metrics = assembly_metrics(assembly)?;
    let mass = metrics.get("mass_kg").ok_or("mass is not available")?;
    let cost = estimated_unit_cost_cr(mass.value, assembly.parts.len(), profile) as f64;
    let a = VerificationMethod::Analysis;
    let mut list = vec![
        evidence("MASS", a, run_id, design, mass.value, mass.uncertainty),
        // The cost model is linear in mass (85 cr/kg), so mass uncertainty carries over.
        evidence("COST", a, run_id, design, cost, mass.uncertainty * 85.0),
        evidence("POWER", a, run_id, design, f64::from(profile.power_draw_w), 0.0),
        evidence("TEMP_MIN", a, run_id, design, f64::from(profile.operating_min_c), 0.0),
        evidence("TEMP_MAX", a, run_id, design, f64::from(profile.operating_max_c), 0.0),
        evidence("STORAGE", a, run_id, design, f64::from(profile.storage_gb), 0.0),
    ];
    if offer.requirements.max_cg_offset_ratio.is_some()
        && let Some(balance) = metrics.get("cg_offset_ratio")
    {
        list.push(evidence(
            "BALANCE",
            a,
            run_id,
            design,
            balance.value,
            balance.uncertainty,
        ));
    }
    Ok(list)
}

/// Standard error, in percentage points, of a proportion measured over `samples` samples.
pub(super) fn proportion_standard_error(percent: f64, samples: u32) -> f64 {
    if samples == 0 {
        return 0.0;
    }
    let p = (percent / 100.0).clamp(0.0, 1.0);
    (p * (1.0 - p) / f64::from(samples)).sqrt() * 100.0
}

/// Evidence produced by a field test: measured values with their sampling uncertainty.
pub(super) fn field_test_evidence(report: &FieldTestReport, design: &ContentHash, run_id: u64) -> Vec<Evidence> {
    let t = VerificationMethod::Test;
    vec![
        evidence(
            "UPTIME",
            t,
            run_id,
            design,
            report.measured_uptime_percent,
            proportion_standard_error(report.measured_uptime_percent, report.sample_count),
        ),
        evidence(
            "DATA",
            t,
            run_id,
            design,
            report.measured_data_completeness_percent,
            proportion_standard_error(report.measured_data_completeness_percent, report.sample_count),
        ),
    ]
}

pub(super) fn next_run_id(evidence: &[Evidence]) -> u64 {
    evidence.iter().map(|e| e.run_id).max().unwrap_or(0) + 1
}

/// Rebuild evidence for a contract saved before evidence was stored (0.22 campaigns).
pub(super) fn rebuild_evidence(
    active: &ActiveContract,
    assembly: &Assembly,
    profile: &InstrumentProfile,
    design: &ContentHash,
) -> Vec<Evidence> {
    let mut list = analysis_evidence(&active.offer, assembly, profile, design, 1).unwrap_or_default();
    if let Some(report) = &active.test_report
        && report.design_fingerprint == design.as_str()
    {
        list.extend(field_test_evidence(report, design, 2));
    }
    list
}

pub(super) fn dossier_for(active: &ActiveContract, design: &ContentHash) -> Dossier {
    build_dossier(
        active.offer.id,
        &requirements_for(&active.offer),
        &active.evidence,
        design,
        None,
    )
}

fn badge(status: &RequirementStatus) -> &'static str {
    match status {
        RequirementStatus::Verified { .. } => "OK",
        RequirementStatus::Marginal { .. } => "THIN",
        RequirementStatus::Failed { .. } => "FAIL",
        RequirementStatus::Stale => "STALE",
        RequirementStatus::Unverified => "OPEN",
        RequirementStatus::MethodNotAllowed => "METHOD",
    }
}

fn method_name(method: VerificationMethod) -> &'static str {
    match method {
        VerificationMethod::Analysis => "analysis",
        VerificationMethod::Demonstration => "demonstration",
        VerificationMethod::Test => "field test",
    }
}

/// One displayable row of the verification matrix.
pub(super) struct RowView {
    pub badge: &'static str,
    pub label: String,
    pub rule: String,
    /// Short value such as `5.10 +/-0.15`, or what is missing.
    pub value: String,
    /// Longer explanation including margin and method.
    pub detail: String,
}

pub(super) struct MatrixView {
    pub rows: Vec<RowView>,
    pub ready: bool,
    pub score_percent: u8,
    pub open_items: usize,
}

pub(super) fn matrix_view(active: &ActiveContract, design: &ContentHash) -> MatrixView {
    let requirements = requirements_for(&active.offer);
    let dossier = dossier_for(active, design);
    let mut rows = Vec::new();
    for (requirement, row) in requirements.iter().zip(&dossier.matrix) {
        let comparator = match requirement.cmp {
            Comparator::AtMost => "<=",
            Comparator::AtLeast => ">=",
        };
        let proof = active
            .evidence
            .iter()
            .find(|e| Some(e.run_id) == row.run_id && e.requirement_id == requirement.id);
        let (value, detail) = match (proof, &row.status) {
            (Some(e), RequirementStatus::Stale) => (
                "stale".to_string(),
                format!("evidence is for an older design ({:.2})", e.estimate.value),
            ),
            (Some(e), status) => {
                let margin = match status {
                    RequirementStatus::Verified { margin }
                    | RequirementStatus::Marginal { margin }
                    | RequirementStatus::Failed { margin } => *margin,
                    _ => 0.0,
                };
                (
                    format!("{:.2} +/-{:.2}", e.estimate.value, e.estimate.uncertainty),
                    format!(
                        "{:.2} +/-{:.2}, margin {:+.2} ({})",
                        e.estimate.value,
                        e.estimate.uncertainty,
                        margin,
                        method_name(e.method)
                    ),
                )
            }
            (None, _) => {
                let need = requirement
                    .allowed_methods
                    .first()
                    .map_or("evidence", |m| method_name(*m));
                (format!("needs {need}"), format!("needs {need}"))
            }
        };
        rows.push(RowView {
            badge: badge(&row.status),
            label: requirement.label.clone(),
            rule: format!("{} {:.2}", comparator, requirement.limit),
            value,
            detail,
        });
    }
    let (ready, open_items) = match &dossier.decision {
        DossierDecision::Accepted { .. } => (true, 0),
        DossierDecision::Rejected { reasons } => (false, reasons.len()),
    };
    MatrixView {
        rows,
        ready,
        score_percent: (dossier.score * 100.0).round().clamp(0.0, 100.0) as u8,
        open_items,
    }
}

/// Plain-text rendering of the matrix (used by tests and logs).
pub(super) fn matrix_lines(active: &ActiveContract, design: &ContentHash) -> Vec<String> {
    let view = matrix_view(active, design);
    let mut lines = vec!["    Verification matrix:".to_string()];
    for row in &view.rows {
        lines.push(format!(
            "      [{:<5}] {} {}: {}",
            row.badge, row.label, row.rule, row.detail
        ));
    }
    lines.push(if view.ready {
        format!("    Dossier: READY for delivery (score {}%)", view.score_percent)
    } else {
        format!("    Dossier: NOT READY ({} open item(s))", view.open_items)
    });
    lines
}

/// Margin-too-thin issues for geometry-derived requirements at submission time. Outright failures
/// of mass and cost are already reported by the direct limit checks, so only `Marginal` is added.
pub(super) fn thin_margin_issues(
    offer: &ContractOffer,
    assembly: &Assembly,
    profile: &InstrumentProfile,
    issues: &mut Vec<String>,
) {
    let Ok(design) = assembly_content_hash(assembly) else {
        return;
    };
    let Ok(list) = analysis_evidence(offer, assembly, profile, &design, 1) else {
        return;
    };
    let requirements = requirements_for(offer);
    for ev in list
        .iter()
        .filter(|e| e.requirement_id == "MASS" || e.requirement_id == "COST")
    {
        let Some(req) = requirements.iter().find(|r| r.id == ev.requirement_id) else {
            continue;
        };
        if let RequirementStatus::Marginal { .. } = evaluate_requirement(req, Some(ev), &design, None) {
            issues.push(format!(
                "{} margin is too thin: {:.2} +/-{:.2} against a limit of {:.2}",
                req.label, ev.estimate.value, ev.estimate.uncertainty, req.limit
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loose_offer() -> ContractOffer {
        ContractOffer {
            id: 7,
            domain: ContractDomain::ScientificSounding,
            title: "t".into(),
            client: "c".into(),
            brief: "b".into(),
            requirements: ContractRequirements {
                max_unit_mass_kg: 250,
                max_unit_power_w: 100,
                temperature_min_c: -30,
                temperature_max_c: 60,
                min_uptime_percent: 90,
                min_data_completeness_percent: 90,
                min_storage_gb: 16,
                required_units: 1,
                max_unit_cost_cr: 1_000_000,
                max_cg_offset_ratio: Some(0.9),
            },
            reward: 1,
            deadline_days: 30,
            difficulty: 1,
        }
    }

    fn active(offer: ContractOffer) -> ActiveContract {
        ActiveContract {
            offer,
            phase: ContractPhase::DesignSubmitted,
            snapshot: None,
            analysis_job_id: None,
            test_report: None,
            last_issues: Vec::new(),
            evidence: Vec::new(),
        }
    }

    fn report(uptime: f64, data: f64, design: &ContentHash) -> FieldTestReport {
        FieldTestReport {
            contract_id: 7,
            created_unix_ms: 0,
            design_fingerprint: design.as_str().to_string(),
            random_seed: 1,
            instrument_profile: InstrumentProfile::default(),
            sample_count: 1_000,
            measured_uptime_percent: uptime,
            measured_data_completeness_percent: data,
            thermal_envelope_passed: true,
            passed: true,
            report_path: None,
        }
    }

    #[test]
    fn requirement_ids_are_unique_and_balance_is_optional() {
        let mut offer = loose_offer();
        let with = requirements_for(&offer);
        let mut ids: Vec<_> = with.iter().map(|r| r.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), with.len());
        assert!(with.iter().any(|r| r.id == "BALANCE"));
        offer.requirements.max_cg_offset_ratio = None;
        assert!(!requirements_for(&offer).iter().any(|r| r.id == "BALANCE"));
    }

    #[test]
    fn standard_error_matches_the_binomial_formula() {
        let se = proportion_standard_error(96.0, 1_000);
        assert!((se - 0.619).abs() < 0.001, "{se}");
        assert_eq!(proportion_standard_error(96.0, 0), 0.0);
        assert_eq!(proportion_standard_error(100.0, 1_000), 0.0);
    }

    #[test]
    fn dossier_needs_the_field_test_for_reliability_requirements() {
        let offer = loose_offer();
        let assembly = Assembly::demo();
        let profile = InstrumentProfile::default();
        let design = assembly_content_hash(&assembly).unwrap();
        let mut contract = active(offer.clone());
        contract.evidence = analysis_evidence(&offer, &assembly, &profile, &design, 1).unwrap();

        let before = dossier_for(&contract, &design);
        assert!(
            matches!(&before.decision, DossierDecision::Rejected { reasons } if reasons.len() == 2),
            "{:?}",
            before.decision
        );

        contract
            .evidence
            .extend(field_test_evidence(&report(97.0, 97.0, &design), &design, 2));
        let after = dossier_for(&contract, &design);
        assert!(
            matches!(after.decision, DossierDecision::Accepted { .. }),
            "{:?}",
            after.decision
        );
    }

    #[test]
    fn a_thin_measured_margin_is_not_enough() {
        let offer = loose_offer();
        let assembly = Assembly::demo();
        let design = assembly_content_hash(&assembly).unwrap();
        let mut contract = active(offer.clone());
        contract.evidence = analysis_evidence(&offer, &assembly, &InstrumentProfile::default(), &design, 1).unwrap();
        // 90.2% measured against a 90% limit: inside one standard error (~0.94 points).
        contract
            .evidence
            .extend(field_test_evidence(&report(90.2, 97.0, &design), &design, 2));
        let dossier = dossier_for(&contract, &design);
        assert!(
            matches!(&dossier.decision, DossierDecision::Rejected { reasons } if reasons.len() == 1 && reasons[0].contains("margin"))
        );
    }

    #[test]
    fn changing_the_design_makes_the_evidence_stale() {
        let offer = loose_offer();
        let assembly = Assembly::demo();
        let design = assembly_content_hash(&assembly).unwrap();
        let mut contract = active(offer.clone());
        contract.evidence = analysis_evidence(&offer, &assembly, &InstrumentProfile::default(), &design, 1).unwrap();
        contract
            .evidence
            .extend(field_test_evidence(&report(97.0, 97.0, &design), &design, 2));

        let mut edited = assembly.clone();
        edited.parts[1].position_m[0] -= 0.4;
        let new_hash = assembly_content_hash(&edited).unwrap();
        let dossier = dossier_for(&contract, &new_hash);
        assert!(
            matches!(&dossier.decision, DossierDecision::Rejected { reasons } if reasons.iter().all(|r| r.contains("stale")))
        );
        let lines = matrix_lines(&contract, &new_hash);
        assert!(lines.iter().any(|l| l.contains("[STALE")));
    }

    #[test]
    fn legacy_contract_without_evidence_is_rebuilt() {
        let offer = loose_offer();
        let assembly = Assembly::demo();
        let design = assembly_content_hash(&assembly).unwrap();
        let mut contract = active(offer);
        contract.test_report = Some(report(97.0, 97.0, &design));
        contract.evidence = rebuild_evidence(&contract, &assembly, &InstrumentProfile::default(), &design);
        assert!(matches!(
            dossier_for(&contract, &design).decision,
            DossierDecision::Accepted { .. }
        ));
        assert_eq!(next_run_id(&contract.evidence), 3);
    }

    #[test]
    fn matrix_lines_show_open_items_and_verdict() {
        let offer = loose_offer();
        let assembly = Assembly::demo();
        let design = assembly_content_hash(&assembly).unwrap();
        let mut contract = active(offer.clone());
        contract.evidence = analysis_evidence(&offer, &assembly, &InstrumentProfile::default(), &design, 1).unwrap();
        let lines = matrix_lines(&contract, &design).join("\n");
        assert!(
            lines.contains("[OPEN ] Uptime (%) >= 90.00: needs field test"),
            "{lines}"
        );
        assert!(lines.contains("NOT READY"));
        assert!(lines.contains("[OK   ] Unit mass (kg)"));
    }

    #[test]
    fn thin_cost_margin_blocks_submission_but_clear_margin_does_not() {
        let assembly = Assembly::demo();
        let profile = InstrumentProfile::default();
        let cost = estimated_unit_cost_cr(assembly.estimated_mass_kg(), assembly.parts.len(), &profile);
        let mut offer = loose_offer();
        let mut issues = Vec::new();
        thin_margin_issues(&offer, &assembly, &profile, &mut issues);
        assert!(issues.is_empty(), "{issues:?}");
        // A limit just above the estimate sits inside the cost uncertainty (85 cr/kg of mass error).
        offer.requirements.max_unit_cost_cr = cost + 5;
        thin_margin_issues(&offer, &assembly, &profile, &mut issues);
        assert!(
            issues.iter().any(|i| i.contains("Unit cost")),
            "cost {cost}, {issues:?}"
        );
        // A limit below the estimate is a plain failure, reported by the direct limit check instead.
        issues.clear();
        offer.requirements.max_unit_cost_cr = cost - 1;
        thin_margin_issues(&offer, &assembly, &profile, &mut issues);
        assert!(issues.is_empty(), "{issues:?}");
    }

    #[test]
    fn pre_023_saves_without_evidence_or_dossier_score_still_load() {
        let mut value = serde_json::to_value(active(loose_offer())).unwrap();
        value.as_object_mut().unwrap().remove("evidence");
        let loaded: ActiveContract = serde_json::from_value(value).unwrap();
        assert!(loaded.evidence.is_empty());

        let completed = CompletedContract {
            id: 1,
            title: "t".into(),
            client: "c".into(),
            production_cost_cr: 1,
            payout_cr: 2,
            profit_cr: 1,
            completed_unix_ms: 0,
            dossier_score_percent: Some(100),
        };
        let mut value = serde_json::to_value(completed).unwrap();
        value.as_object_mut().unwrap().remove("dossier_score_percent");
        let loaded: CompletedContract = serde_json::from_value(value).unwrap();
        assert_eq!(loaded.dossier_score_percent, None);
    }
}
