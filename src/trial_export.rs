//! Exports for the trial laboratory: a self-contained HTML report, a CSV campaign sheet and a
//! printable PDF technical dossier. No network, no external binary: the files are written into
//! the active slot's `reports/` directory.
//!
//! The PDF writer is deliberately small: text, rules and vector trajectory charts, using only the
//! PDF base-14 Helvetica font. It is a game document, not a typographic engine.

use crate::lab::{self, AnomalyInsight, ComplianceSnapshot, DesignMetrics, PhaseSummary, TrialStats};
use crate::procurement::Procurement;
use crate::revisions::RevisionLog;
use crate::trials::{TrialSession, TrialTab};
use omega_rules::assembly::Vehicle;
use omega_rules::contracts::ContractOffer;
use omega_rules::dossier::Dossier;
use omega_rules::flight::Sample;
use omega_rules::program::Program;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// A flat snapshot shared by all three exports, so HTML, CSV and PDF never disagree.
struct Snapshot {
    label: String,
    design: String,
    seed: u64,
    air_launch: bool,
    runs: usize,
    input_hash: String,
    program_version: String,
    revision: String,
    change_summary: String,
    metrics: DesignMetrics,
    stats: TrialStats,
    phases: Vec<PhaseSummary>,
    anomalies: Vec<AnomalyInsight>,
    events: Vec<(f64, String)>,
    compliance: Option<ComplianceSnapshot>,
    samples: Vec<Sample>,
    stale: bool,
}

fn snapshot(
    session: &TrialSession,
    vehicle: &Vehicle,
    program: &Program,
    procurement: &Procurement,
) -> Result<Snapshot, String> {
    if let Some(run) = &session.report {
        let stale = session.report_stale(vehicle, program);
        let compliance = (!stale).then(|| procurement.selected()).flatten().and_then(|active| {
            lab::compliance_for_report(Some(&active.offer), vehicle, program, run.seed, &run.report)
        });
        return Ok(Snapshot {
            label: format!("Essai {:.0} en cours", run.seed),
            design: vehicle.name.clone(),
            seed: run.seed,
            air_launch: run.air_launch,
            runs: 1,
            input_hash: run.input_hash.clone(),
            program_version: format!("P-{}", lab::short_hash(program)),
            revision: session
                .context
                .revision_id
                .map_or_else(|| "non révisée".to_string(), |id| format!("révision {id}")),
            change_summary: session.context.change_summary.clone(),
            metrics: DesignMetrics::of(vehicle),
            stats: TrialStats::from_report(&run.report),
            phases: lab::phases(&run.report),
            anomalies: lab::analyze_report_anomalies(&run.report, vehicle, program),
            events: run.report.events.clone(),
            compliance,
            samples: run.report.samples.clone(),
            stale,
        });
    }
    let entry = session
        .selected_history
        .and_then(|id| session.history.get(id))
        .or_else(|| session.history.entries().last())
        .ok_or_else(|| "Aucun essai à exporter : lancez un essai ou choisissez une campagne.".to_string())?;
    Ok(Snapshot {
        label: entry.label.clone(),
        design: entry.vehicle.name.clone(),
        seed: entry.seed,
        air_launch: entry.air_launch,
        runs: entry.runs,
        input_hash: entry.input_hash.clone(),
        program_version: entry.program_version.clone(),
        revision: entry
            .revision_id
            .map_or_else(|| "non révisée".to_string(), |id| format!("révision {id}")),
        change_summary: entry.change_summary.clone(),
        metrics: entry.metrics,
        stats: entry.stats.clone(),
        phases: entry.phases.clone(),
        anomalies: entry.anomalies.clone(),
        events: entry.events.clone(),
        compliance: entry.compliance.clone(),
        samples: Vec::new(),
        stale: entry.stale(vehicle, program),
    })
}

fn reports_dir() -> Result<PathBuf, String> {
    let slot = crate::save::active_slot().ok_or_else(|| "Choisissez d'abord une campagne.".to_string())?;
    let dir = crate::save::slot_dir(slot).join("reports");
    fs::create_dir_all(&dir).map_err(|error| format!("Impossible de créer {} : {error}", dir.display()))?;
    Ok(dir)
}

fn report_path(ext: &str, snapshot: &Snapshot) -> Result<PathBuf, String> {
    let dir = reports_dir()?;
    let prefix: String = snapshot.input_hash.chars().take(10).collect();
    Ok(dir.join(format!("omega-trial-{prefix}.{ext}")))
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn csv(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

fn rate(value: Option<f64>) -> String {
    value.map_or_else(|| "—".to_string(), |value| format!("{:.0} %", value * 100.0))
}

fn stale_label(entry: &lab::TrialHistoryEntry, vehicle: &Vehicle, program: &Program) -> String {
    entry
        .stale_reason(vehicle, program)
        .map_or_else(|| "à jour".to_string(), |reason| format!("obsolète ({reason})"))
}

/// Write the HTML dashboard report and return its path.
pub(crate) fn export_html(
    session: &TrialSession,
    vehicle: &Vehicle,
    program: &Program,
    procurement: &Procurement,
    revisions: &RevisionLog,
) -> Result<PathBuf, String> {
    let snapshot = snapshot(session, vehicle, program, procurement)?;
    let path = report_path("html", &snapshot)?;
    let mut html = String::from(
        "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\"><title>Omega — rapport d'essais</title>\
         <style>\
         :root{color-scheme:dark}body{margin:0;background:#08131b;color:#dce9f1;font:14px/1.45 system-ui,sans-serif}\
         main{max-width:1100px;margin:auto;padding:32px}h1{color:#52d6c9;font-size:28px;margin:0 0 4px}\
         h2{color:#52d6c9;font-size:17px;border-bottom:1px solid #24404d;padding-bottom:4px;margin-top:28px}\
         .muted{color:#8fa9b7}.cards{display:flex;gap:10px;flex-wrap:wrap}.card{background:#102531;border:1px solid #24404d;border-radius:8px;padding:10px 14px;min-width:150px}\
         .card b{display:block;font-size:20px;color:#f0c96a}.ok{color:#62d68b}.bad{color:#ff6b6b}.warn{color:#f0c96a}\
         table{width:100%;border-collapse:collapse;margin:8px 0}th,td{text-align:left;padding:6px 8px;border-bottom:1px solid #1c3541}\
         th{color:#8fa9b7;font-weight:600}svg{background:#0c1d27;border:1px solid #24404d;border-radius:6px;max-width:100%}\
         code{color:#b6d4e2}.small{font-size:12px}footer{margin-top:36px;color:#617e8d;font-size:12px}\
         </style></head><body><main>",
    );
    let _ = write!(
        html,
        "<h1>Rapport d'essais Omega</h1><div class=\"muted\">{} · {} · graine {} · {} · {}</div>",
        escape_html(&snapshot.label),
        escape_html(&snapshot.design),
        snapshot.seed,
        if snapshot.air_launch {
            "lancement aérien"
        } else {
            "lancement vertical"
        },
        snapshot.runs
    );
    let _ = write!(
        html,
        "<h2>Résumé</h2><div class=\"cards\">\
         <div class=\"card\">Portée max<b>{:.1} km</b></div>\
         <div class=\"card\">Vitesse max<b>{:.0} m/s</b></div>\
         <div class=\"card\">Apogée max<b>{:.1} km</b></div>\
         <div class=\"card\">Réussite<b>{}</b></div>\
         <div class=\"card\">Anomalies<b class=\"{}\">{}</b></div>\
         <div class=\"card\">Preuve<b>{}</b></div></div>",
        snapshot.stats.max_path_m / 1_000.0,
        snapshot.stats.max_speed_ms,
        snapshot.stats.max_altitude_m / 1_000.0,
        rate(snapshot.stats.success_rate()),
        if snapshot.anomalies.is_empty() { "ok" } else { "bad" },
        snapshot.anomalies.len(),
        if snapshot.stale {
            "obsolète"
        } else if snapshot.stats.runs > 0 {
            "à jour"
        } else {
            "—"
        }
    );
    let _ = write!(
        html,
        "<p class=\"small muted\">Hash d'entrée <code>{}</code> · version programme <code>{}</code> · {}</p>",
        escape_html(&snapshot.input_hash),
        escape_html(&snapshot.program_version),
        escape_html(&format!("{} · vue {}", snapshot.revision, export_tab(session.tab)))
    );
    html.push_str("<h2>Conception</h2><table><tr><th>Masse</th><th>Coût fictif</th><th>Stabilité</th><th>Autorité</th><th>Complexité</th><th>Fiabilité</th></tr><tr>");
    for (_, value) in snapshot.metrics.rows() {
        let _ = write!(html, "<td>{value:.1}</td>");
    }
    html.push_str("</tr></table>");
    if !snapshot.samples.is_empty() {
        html.push_str("<h2>Trajectoire</h2>");
        html.push_str(&svg_chart(
            &snapshot.samples,
            |sample| sample.speed_ms,
            "Vitesse m/s",
            "#52d6c9",
        ));
        html.push_str(&svg_chart(
            &snapshot.samples,
            |sample| sample.altitude_m,
            "Altitude m",
            "#c7a3ff",
        ));
    }
    html.push_str("<h2>Résultats par phase</h2><table><tr><th>Phase</th><th>Durée moyenne</th><th>Vitesse max</th><th>Apogée max</th><th>Masse moyenne</th></tr>");
    for phase in &snapshot.phases {
        let _ = write!(
            html,
            "<tr><td>{}</td><td>{:.1} s</td><td>{:.0} m/s</td><td>{:.0} m</td><td>{:.0} kg</td></tr>",
            escape_html(&phase.name),
            phase.duration_s,
            phase.max_speed_ms,
            phase.max_altitude_m,
            phase.mean_mass_kg
        );
    }
    html.push_str("</table>");
    html.push_str("<h2>Événements importants</h2><table><tr><th>Temps</th><th>Événement</th></tr>");
    for (time, label) in snapshot.events.iter().take(30) {
        let _ = write!(html, "<tr><td>{time:.1} s</td><td>{}</td></tr>", escape_html(label));
    }
    html.push_str("</table>");
    html.push_str("<h2>Anomalies</h2>");
    if snapshot.anomalies.is_empty() {
        html.push_str("<p class=\"ok\">Aucune anomalie signalée.</p>");
    } else {
        html.push_str("<table><tr><th>Type</th><th>Message</th><th>Composant</th><th>Bloc</th></tr>");
        for anomaly in &snapshot.anomalies {
            let _ = write!(
                html,
                "<tr><td class=\"bad\">{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                anomaly.kind.label(),
                escape_html(&anomaly.message),
                escape_html(anomaly.component.as_deref().unwrap_or("—")),
                escape_html(anomaly.block.as_ref().map_or("—", |block| block.label.as_str()))
            );
        }
        html.push_str("</table>");
    }
    if let Some(compliance) = &snapshot.compliance {
        let _ = write!(
            html,
            "<h2>Conformité au contrat</h2><p>{} — score {:.0} % — <span class=\"{}\">{}</span></p>\
             <table><tr><th>Exigence</th><th>Statut</th><th>Marge</th></tr>",
            escape_html(&compliance.title),
            compliance.score_percent,
            if compliance.accepted { "ok" } else { "bad" },
            if compliance.accepted { "accepté" } else { "refusé" },
        );
        for row in &compliance.rows {
            let _ = write!(
                html,
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                escape_html(&row.label),
                escape_html(&row.status),
                row.margin
                    .map_or_else(|| "—".to_string(), |margin| format!("{margin:+.2}"))
            );
        }
        html.push_str("</table>");
    }
    if let Some(comparison) = &session.comparison {
        let a = &comparison.baseline;
        let b = &comparison.current;
        html.push_str("<h2>Comparaison A/B</h2><table><tr><th>Indicateur</th><th>A</th><th>B</th><th>Écart</th></tr>");
        for (label, left, right) in [
            ("Portée moyenne", a.mean_path_m, b.mean_path_m),
            ("Vitesse moyenne max", a.mean_max_speed_ms, b.mean_max_speed_ms),
            ("Temps moyen", a.mean_flight_time_s, b.mean_flight_time_s),
        ] {
            let _ = write!(
                html,
                "<tr><td>{label}</td><td>{left:.2}</td><td>{right:.2}</td><td>{:+.2} ({})</td></tr>",
                right - left,
                lab::percent_delta(left, right).map_or_else(|| "—".to_string(), |value| format!("{value:+.1} %"))
            );
        }
        html.push_str("</table>");
    }
    html.push_str("<h2>Historique des campagnes</h2><table><tr><th>Campagne</th><th>Révision</th><th>Programme</th><th>Graines</th><th>Empreinte</th><th>État</th></tr>");
    for entry in session.history.entries().iter().rev().take(20) {
        let _ = write!(
            html,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            escape_html(&entry.label),
            entry
                .revision_id
                .map_or_else(|| "—".to_string(), |id| format!("rev {id}")),
            escape_html(&entry.program_version),
            escape_html(&entry.seeds_label()),
            escape_html(&lab::entry_hash(entry)),
            escape_html(&stale_label(entry, vehicle, program))
        );
    }
    html.push_str("</table>");
    let current = revisions
        .head()
        .map_or("aucune révision", |revision| revision.label.as_str());
    let _ = write!(
        html,
        "<h2>Modifications depuis la révision</h2><p>{}</p>",
        escape_html(&snapshot.change_summary)
    );
    let _ = write!(
        html,
        "<footer>Omega {} · Laboratoire d'essais · révision courante : {} · document fictif et abstrait.</footer></main></body></html>",
        env!("CARGO_PKG_VERSION"),
        escape_html(current)
    );
    fs::write(&path, html).map_err(|error| format!("Impossible d'écrire le rapport HTML : {error}"))?;
    Ok(path)
}

fn svg_chart(samples: &[Sample], value: impl Fn(&Sample) -> f64, label: &str, color: &str) -> String {
    if samples.is_empty() {
        return String::new();
    }
    let max = samples.iter().map(&value).fold(0.0, f64::max).max(1.0);
    let width = 920.0;
    let height = 170.0;
    let points = samples
        .iter()
        .enumerate()
        .map(|(index, sample)| {
            let x = index as f64 / (samples.len().max(2) - 1) as f64 * width;
            let y = height - (value(sample) / max).clamp(0.0, 1.0) * height;
            format!("{x:.1},{y:.1}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "<h3 style=\"color:#8fa9b7;font-size:13px\">{label} (max {max:.0})</h3>\
         <svg viewBox=\"0 0 {width} {height}\" width=\"100%\" height=\"170\" role=\"img\">\
         <polyline fill=\"none\" stroke=\"{color}\" stroke-width=\"2\" points=\"{points}\"/></svg>"
    )
}

/// Write one CSV row per campaign, with the current design hashes for the obsolete marker.
pub(crate) fn export_csv(session: &TrialSession, vehicle: &Vehicle, program: &Program) -> Result<PathBuf, String> {
    if session.history.entries().is_empty() {
        return Err("Aucune campagne à exporter.".to_string());
    }
    let first = session.history.entries().last().expect("checked non-empty");
    let path = reports_dir()?.join(format!(
        "omega-campaigns-{}.csv",
        first.input_hash.chars().take(10).collect::<String>()
    ));
    let mut out = String::from(
        "campagne;label;type;jour;revision;version_programme;graines;lancements;profil;hash_entree;hash_vehicule;hash_programme;masse_kg;cout_fictif;stabilite;autorite;complexite;fiabilite;portee_m;vitesse_max_ms;apogee_m;taux_reussite;anomalies;contrat;score_contrat;accepte;etat\n",
    );
    for entry in session.history.entries() {
        let compliance = entry.compliance.as_ref();
        let values = [
            entry.id.to_string(),
            entry.label.clone(),
            entry.kind.clone(),
            entry.day.to_string(),
            entry.revision_id.map_or_else(|| "—".to_string(), |id| id.to_string()),
            entry.program_version.clone(),
            entry.seeds_label(),
            entry.runs.to_string(),
            lab::flight_label(entry.air_launch).to_string(),
            entry.input_hash.clone(),
            entry.vehicle_hash.clone(),
            entry.program_hash.clone(),
            format!("{:.2}", entry.metrics.mass_kg),
            format!("{:.0}", entry.metrics.cost_cr),
            format!("{:.1}", entry.metrics.stability),
            format!("{:.1}", entry.metrics.authority),
            format!("{:.1}", entry.metrics.complexity),
            format!("{:.1}", entry.metrics.reliability),
            format!("{:.1}", entry.stats.mean_path_m),
            format!("{:.1}", entry.stats.max_speed_ms),
            format!("{:.1}", entry.stats.max_altitude_m),
            rate(entry.stats.success_rate()),
            entry.anomalies.len().to_string(),
            compliance.map_or_else(|| "—".to_string(), |snapshot| snapshot.title.clone()),
            compliance.map_or_else(|| "—".to_string(), |snapshot| format!("{:.0}", snapshot.score_percent)),
            compliance.map_or_else(|| "—".to_string(), |snapshot| snapshot.accepted.to_string()),
            stale_label(entry, vehicle, program),
        ];
        out.push_str(&values.iter().map(|value| csv(value)).collect::<Vec<_>>().join(";"));
        out.push('\n');
    }
    fs::write(&path, out).map_err(|error| format!("Impossible d'écrire le CSV : {error}"))?;
    Ok(path)
}

/// Write the printable technical dossier (PDF 1.4, base-14 Helvetica).
pub(crate) fn export_pdf(
    session: &TrialSession,
    vehicle: &Vehicle,
    program: &Program,
    procurement: &Procurement,
    revisions: &RevisionLog,
) -> Result<PathBuf, String> {
    let snapshot = snapshot(session, vehicle, program, procurement)?;
    let path = report_path("pdf", &snapshot)?;
    let mut lines = vec![
        "OMEGA - DOSSIER TECHNIQUE D'ESSAIS".to_string(),
        format!("Document fictif et abstrait - version {}", env!("CARGO_PKG_VERSION")),
        format!("Campagne : {}", snapshot.label),
        format!("Conception : {}", snapshot.design),
        format!(
            "Profil : {} - graine {} - {} lancement(s)",
            lab::flight_label(snapshot.air_launch),
            snapshot.seed,
            snapshot.runs
        ),
        format!("Version programme : {}", snapshot.program_version),
        format!("Revision : {}", snapshot.revision),
        format!("Hash d'entree : {}", snapshot.input_hash),
        String::new(),
        "CONCEPTION".to_string(),
        format!(
            "Masse {:.1} kg | cout fictif {:.0} | stabilite {:.1} | autorite {:.1} | complexite {:.1} | fiabilite {:.1}",
            snapshot.metrics.mass_kg,
            snapshot.metrics.cost_cr,
            snapshot.metrics.stability,
            snapshot.metrics.authority,
            snapshot.metrics.complexity,
            snapshot.metrics.reliability
        ),
        String::new(),
        "RESULTATS".to_string(),
        format!(
            "Portee moyenne {:.1} m | vitesse max {:.0} m/s | apogee max {:.0} m | reussite {}",
            snapshot.stats.mean_path_m,
            snapshot.stats.max_speed_ms,
            snapshot.stats.max_altitude_m,
            rate(snapshot.stats.success_rate())
        ),
        format!(
            "Anomalies {} | series avec anomalie {} | verrouillage {}",
            snapshot.anomalies.len(),
            snapshot.stats.anomaly_runs,
            rate(snapshot.stats.lock_rate())
        ),
        format!("Preuve : {}", if snapshot.stale { "obsolète" } else { "à jour" }),
        String::new(),
        "PHASES".to_string(),
    ];
    for phase in &snapshot.phases {
        lines.push(format!(
            "{} : duree {:.1} s, vitesse max {:.0} m/s, apogee {:.0} m, masse moyenne {:.0} kg",
            phase.name, phase.duration_s, phase.max_speed_ms, phase.max_altitude_m, phase.mean_mass_kg
        ));
    }
    lines.push(String::new());
    lines.push("ANOMALIES".to_string());
    if snapshot.anomalies.is_empty() {
        lines.push("Aucune anomalie signalée.".to_string());
    } else {
        for anomaly in &snapshot.anomalies {
            lines.push(format!(
                "[{}] {}{}{}",
                anomaly.kind.label(),
                anomaly.message,
                anomaly
                    .component
                    .as_ref()
                    .map_or(String::new(), |id| format!(" (composant {id})")),
                anomaly
                    .block
                    .as_ref()
                    .map_or(String::new(), |block| format!(" (bloc {})", block.label))
            ));
        }
    }
    if let Some(compliance) = &snapshot.compliance {
        lines.push(String::new());
        lines.push("CONFORMITE AU CONTRAT".to_string());
        lines.push(format!(
            "{} - score {:.0} % - {}",
            compliance.title,
            compliance.score_percent,
            if compliance.accepted { "accepte" } else { "refuse" }
        ));
        for row in &compliance.rows {
            lines.push(format!("{} : {}", row.label, row.status));
        }
    }
    lines.push(String::new());
    lines.push("MODIFICATIONS DEPUIS LA REVISION".to_string());
    lines.push(snapshot.change_summary.clone());
    lines.push(String::new());
    lines.push("HISTORIQUE RECENT".to_string());
    for entry in session.history.entries().iter().rev().take(8) {
        lines.push(format!(
            "{} - rev {} - {} - {} - {}",
            entry.label,
            entry.revision_id.map_or_else(|| "—".to_string(), |id| id.to_string()),
            entry.program_version,
            entry.seeds_label(),
            stale_label(entry, vehicle, program)
        ));
    }
    let chart = snapshot
        .samples
        .iter()
        .map(|sample| (sample.t_s, sample.altitude_m))
        .collect::<Vec<_>>();
    let bytes = pdf_document(
        &lines,
        &chart,
        revisions.head().map_or("aucune", |revision| revision.label.as_str()),
    );
    fs::write(&path, bytes).map_err(|error| format!("Impossible d'écrire le PDF : {error}"))?;
    Ok(path)
}

fn pdf_text(text: &str) -> String {
    let mut out = String::new();
    for character in text.chars() {
        let replacement = match character {
            'é' | 'è' | 'ê' | 'ë' => "e",
            'à' | 'â' | 'ä' => "a",
            'î' | 'ï' => "i",
            'ô' | 'ö' => "o",
            'ù' | 'û' | 'ü' => "u",
            'ç' => "c",
            'É' | 'È' | 'Ê' => "E",
            'À' | 'Â' => "A",
            'Ô' => "O",
            '—' => "-",
            '’' => "'",
            '“' | '”' => "\"",
            '×' => "x",
            '≤' => "<=",
            '≥' => ">=",
            _ if character.is_ascii() => {
                out.push(character);
                continue;
            }
            _ => "?",
        };
        out.push_str(replacement);
    }
    out.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)")
}

fn pdf_document(lines: &[String], chart: &[(f64, f64)], revision: &str) -> Vec<u8> {
    let per_page = 42;
    let mut pages: Vec<String> = Vec::new();
    for chunk in lines.chunks(per_page) {
        let mut content = String::new();
        content.push_str("BT /F1 10 Tf 42 806 Td 14 TL\n");
        for line in chunk {
            content.push_str(&format!("({}) Tj T*\n", pdf_text(line)));
        }
        content.push_str("ET\n");
        pages.push(content);
    }
    if pages.is_empty() {
        pages.push(String::new());
    }
    // The trajectory chart is appended to the last page, below the text.
    if chart.len() >= 2 {
        let max = chart.iter().map(|(_, value)| *value).fold(0.0, f64::max).max(1.0);
        let mut content = pages.pop().unwrap_or_default();
        content.push_str("q 0.32 0.84 0.79 RG 1.5 w\n");
        for (index, (_, value)) in chart.iter().enumerate() {
            let x = 48.0 + index as f64 / (chart.len().max(2) - 1) as f64 * 500.0;
            let y = 80.0 + (value / max).clamp(0.0, 1.0) * 180.0;
            if index == 0 {
                content.push_str(&format!("{x:.1} {y:.1} m\n"));
            } else {
                content.push_str(&format!("{x:.1} {y:.1} l\n"));
            }
        }
        content.push_str("S Q\n");
        content.push_str(&format!(
            "BT /F1 8 Tf 48 64 Td (Altitude (m) - revision {}) Tj ET\n",
            pdf_text(revision)
        ));
        pages.push(content);
    }

    let page_count = pages.len();
    let font_obj = (3 + page_count * 2) as u32;
    let mut objects: Vec<(u32, Vec<u8>)> = Vec::new();
    objects.push((1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()));
    let kids = (0..page_count)
        .map(|index| format!("{} 0 R", 3 + index * 2))
        .collect::<Vec<_>>()
        .join(" ");
    objects.push((
        2,
        format!("<< /Type /Pages /Kids [{kids}] /Count {page_count} >>").into_bytes(),
    ));
    for (index, content) in pages.iter().enumerate() {
        let page_obj = 3 + index as u32 * 2;
        let content_obj = page_obj + 1;
        objects.push((
            page_obj,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 {font_obj} 0 R >> >> /Contents {content_obj} 0 R >>"
            )
            .into_bytes(),
        ));
        objects.push((
            content_obj,
            format!("<< /Length {} >>\nstream\n{}\nendstream", content.len(), content).into_bytes(),
        ));
    }
    objects.push((
        font_obj,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
    ));

    let mut pdf = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (number, body) in &objects {
        offsets.push((*number, pdf.len()));
        pdf.extend_from_slice(format!("{number} 0 obj\n").as_bytes());
        pdf.extend_from_slice(body);
        pdf.extend_from_slice(b"\nendobj\n");
    }
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for index in 1..=objects.len() {
        let offset = offsets
            .iter()
            .find(|(number, _)| *number == index as u32)
            .map_or(0, |(_, offset)| *offset);
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    pdf
}

/// All export paths in a short message.
pub(crate) fn export_message(path: &Path) -> String {
    format!("Exporté vers {}", path.display())
}

/// The tab a report was requested from, kept in the export metadata.
pub(crate) fn export_tab(tab: TrialTab) -> &'static str {
    match tab {
        TrialTab::Dashboard => "dashboard",
        TrialTab::Compare => "comparison",
        TrialTab::History => "history",
    }
}

/// Technical dossier handed over when a contract is delivered. Unlike the trial report, this is
/// tied to the accepted offer and the submitted dossier: it includes the airframe tree, equipment,
/// design parameters, embedded program, trial evidence and the validated requirement matrix.
pub(crate) fn contract_dossier_lines(
    offer: &ContractOffer,
    dossier: &Dossier,
    vehicle: &Vehicle,
    program: &Program,
    history: &lab::TrialHistory,
    revision_id: Option<u32>,
) -> Vec<String> {
    let metrics = DesignMetrics::of(vehicle);
    let (vehicle_hash, program_hash) = crate::procurement::hashes(vehicle, program);
    let mut lines = vec![
        "OMEGA - DOSSIER TECHNIQUE DE LIVRAISON".to_string(),
        format!("Contrat #{} - {}", offer.id, offer.title),
        format!("Client : {}", offer.client),
        format!("Contexte : {}", offer.context),
        format!(
            "Mission {} - difficulte {} - budget {} cr - echeance {} jours",
            offer.mission_family.label(),
            offer.difficulty,
            offer.design_budget_cr,
            offer.deadline_days
        ),
        format!(
            "Version soumise : {} - hash vehicule {} - hash programme {}",
            revision_id.map_or_else(|| "non révisée".to_string(), |id| format!("révision {id}")),
            vehicle_hash.as_str(),
            program_hash.as_str()
        ),
        String::new(),
        "VUE GENERALE DU VEHICULE".to_string(),
        format!(
            "{} - {} etage(s) - masse {} kg - diametre max {} m",
            vehicle.name,
            vehicle.stage_count(),
            vehicle.total_wet_mass_kg().round(),
            vehicle.max_diameter_m()
        ),
        format!(
            "Cout fictif {:.0} cr | stabilite {:.0}/100 | autorite {:.0}/100 | complexite {:.0}/100 | fiabilite {:.0}/100",
            metrics.cost_cr, metrics.stability, metrics.authority, metrics.complexity, metrics.reliability
        ),
        String::new(),
        "ARBRE D'ASSEMBLAGE".to_string(),
    ];
    for entry in vehicle.tree_entries() {
        if let Some(part) = vehicle.part(&entry.id) {
            let parent = entry.parent.as_deref().unwrap_or("-");
            lines.push(format!(
                "{}- [etage {}] {} (parent {}) : {}",
                "  ".repeat(entry.depth),
                entry.stage + 1,
                part.id,
                parent,
                crate::campus::spec_summary(&part.spec)
            ));
        }
    }
    lines.push(String::new());
    lines.push("EQUIPEMENTS".to_string());
    for part in &vehicle.parts {
        lines.push(format!(
            "- [{}] {} : {}",
            part.spec.category().label(),
            part.id,
            crate::campus::spec_summary(&part.spec)
        ));
    }
    lines.push(String::new());
    lines.push("PARAMETRES DE CONCEPTION".to_string());
    for (label, value) in metrics.rows() {
        lines.push(format!("{label} : {value:.1}"));
    }
    lines.push(format!("Budget autorise : {} cr", offer.design_budget_cr));
    for goal in lab::goal_statuses(&offer.primary_goals, &metrics) {
        lines.push(format!(
            "Objectif principal {} : {}",
            goal.label,
            if goal.met { "satisfait" } else { "non satisfait" }
        ));
    }
    for goal in lab::goal_statuses(&offer.secondary_goals, &metrics) {
        lines.push(format!(
            "Objectif secondaire {} : {}",
            goal.label,
            if goal.met { "satisfait" } else { "non satisfait" }
        ));
    }
    lines.push(String::new());
    lines.push("PROGRAMME EMBARQUE".to_string());
    lines.push(format!(
        "{} bloc(s) - {} tache(s) - {} routine(s)",
        program.block_count(),
        program.threads.len(),
        program.routines.len()
    ));
    for (index, thread) in program.threads.iter().enumerate() {
        lines.push(format!("TACHE {}", index + 1));
        for block in thread {
            lines.push(format!("  - {}", block.label()));
        }
    }
    for routine in &program.routines {
        lines.push(format!("ROUTINE {} ({} bloc(s))", routine.name, routine.blocks.len()));
    }
    lines.push(String::new());
    lines.push("RESULTATS DES ESSAIS".to_string());
    let matching: Vec<&lab::TrialHistoryEntry> = history
        .entries()
        .iter()
        .filter(|entry| entry.vehicle_hash == vehicle_hash.as_str() && entry.program_hash == program_hash.as_str())
        .collect();
    if matching.is_empty() {
        lines.push("Aucune campagne enregistree pour ces empreintes.".to_string());
    } else {
        for entry in matching.iter().rev().take(6) {
            lines.push(format!(
                "{} - {} - revision {} - portee {:.1} km - vitesse {:.0} m/s - apogee {:.1} km - reussite {}",
                entry.label,
                entry.seeds_label(),
                entry.revision_id.map_or_else(|| "-".to_string(), |id| id.to_string()),
                entry.stats.mean_path_m / 1_000.0,
                entry.stats.max_speed_ms,
                entry.stats.max_altitude_m / 1_000.0,
                rate(entry.stats.success_rate())
            ));
        }
    }
    if let Some(entry) = matching.last() {
        lines.push(String::new());
        lines.push("ANOMALIES EVENTUELLES".to_string());
        if entry.anomalies.is_empty() {
            lines.push("Aucune anomalie enregistree.".to_string());
        } else {
            for anomaly in &entry.anomalies {
                lines.push(format!("[{}] {}", anomaly.kind.label(), anomaly.message));
            }
        }
    } else {
        lines.push(String::new());
        lines.push("ANOMALIES EVENTUELLES".to_string());
        lines.push("Aucune campagne compatible : aucune anomalie a documenter.".to_string());
    }
    lines.push(String::new());
    lines.push("EXIGENCES VALIDEES".to_string());
    for row in &dossier.matrix {
        lines.push(format!("{} : {:?}", row.label, row.status));
    }
    lines.push(format!("Score du dossier : {:.0} %", dossier.score * 100.0));
    lines.push(String::new());
    lines.push("SECTIONS DEMANDEES PAR LE CLIENT".to_string());
    for section in &offer.requested_dossier {
        lines.push(format!("- {section}"));
    }
    lines.push(String::new());
    lines.push(format!(
        "Document fictif et abstrait - Omega {} - revision {} - hash {}.",
        env!("CARGO_PKG_VERSION"),
        revision_id.map_or_else(|| "-".to_string(), |id| id.to_string()),
        vehicle_hash.as_str()
    ));
    lines
}

pub(crate) fn export_contract_dossier(
    offer: &ContractOffer,
    dossier: &Dossier,
    vehicle: &Vehicle,
    program: &Program,
    history: &lab::TrialHistory,
    revision_id: Option<u32>,
) -> Result<PathBuf, String> {
    let lines = contract_dossier_lines(offer, dossier, vehicle, program, history, revision_id);
    let (vehicle_hash, _) = crate::procurement::hashes(vehicle, program);
    let path = reports_dir()?.join(format!(
        "contrat-{}-dossier-{}.pdf",
        offer.id,
        vehicle_hash.as_str().chars().take(8).collect::<String>()
    ));
    let bytes = pdf_document(&lines, &[], &format!("contrat {}", offer.id));
    fs::write(&path, bytes).map_err(|error| format!("Impossible d'écrire {} : {error}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escapes_user_content_and_csv_quotes_separators() {
        assert_eq!(escape_html("<A&B>"), "&lt;A&amp;B&gt;");
        assert_eq!(csv("a;b"), "a;b");
        assert_eq!(csv("a,b\"c"), "\"a,b\"\"c\"");
    }

    #[test]
    fn pdf_document_has_a_complete_trailer_and_xref() {
        let bytes = pdf_document(&["Test".to_string()], &[(0.0, 0.0), (1.0, 10.0)], "rev 1");
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("/Type /Catalog"));
        assert!(text.contains("startxref"));
        assert!(text.ends_with("%%EOF\n"));
    }

    #[test]
    fn a_contract_dossier_lists_the_requested_technical_sections() {
        use omega_rules::dossier::{Dossier, DossierDecision};
        let offer = omega_rules::contracts::generate_contract_board(4, 1).remove(0);
        let vehicle = Vehicle::from_stages(
            "Dossier rocket",
            vec![omega_rules::Stage {
                parts: vec![omega_rules::Part {
                    id: "motor".into(),
                    spec: omega_rules::PartSpec::SolidMotor {
                        length_m: 2.0,
                        diameter_m: 0.4,
                        burn_time_s: 20.0,
                    },
                }],
            }],
        );
        let program = Program::single(vec![omega_rules::Block::IgniteStage { stage: 0 }]);
        let hash = omega_data::hash_serializable(&vehicle).unwrap();
        let dossier = Dossier {
            contract_id: offer.id,
            design: hash.clone(),
            program: Some(hash),
            matrix: Vec::new(),
            score: 0.0,
            decision: DossierDecision::Rejected { reasons: Vec::new() },
        };
        let lines = contract_dossier_lines(
            &offer,
            &dossier,
            &vehicle,
            &program,
            &lab::TrialHistory::default(),
            Some(7),
        );
        let text = lines.join("\n");
        assert!(text.contains("VUE GENERALE DU VEHICULE"), "{text}");
        assert!(text.contains("ARBRE D'ASSEMBLAGE"), "{text}");
        assert!(text.contains("PROGRAMME EMBARQUE"), "{text}");
        assert!(text.contains("EXIGENCES VALIDEES"), "{text}");
        assert!(text.contains("revision 7"), "{text}");
        assert!(text.contains("Dossier rocket"), "{text}");
    }
}
