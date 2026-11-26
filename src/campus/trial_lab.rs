//! The 0.44 trial laboratory: a tabbed dashboard for the current run, the design comparison and
//! the persisted campaign history. All controls route through `TrialAction`; this module only
//! reads the rules and sessions and draws the cards.

use super::CampusState;
use super::board::{heading, panel_node, plot, rate, score_color};
use crate::interface::UiAction;
use crate::lab::{
    self, AnomalyInsight, CompliancePreview, ComplianceSnapshot, DesignMetrics, PhaseSummary, TrialStats,
};
use crate::procurement::Procurement;
use crate::revisions::RevisionLog;
use crate::trials::{self, TrialAction, TrialSession, TrialTab};
use crate::ui_kit::{self, ButtonTone};
use bevy::prelude::*;
use omega_rules::Vehicle;
use omega_rules::program::Program;

const LEFT_WIDTH: f32 = 268.0;
const RIGHT_WIDTH: f32 = 350.0;

pub(crate) fn build(
    root: &mut ChildSpawnerCommands<'_>,
    campus: &CampusState,
    vehicle: &Vehicle,
    program: &Program,
    session: &TrialSession,
    procurement: &Procurement,
    log: &RevisionLog,
) {
    root.spawn(Node {
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|page| {
        build_header(page, campus, session, procurement);
        page.spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(10.0),
            overflow: Overflow::clip(),
            ..default()
        })
        .with_children(|body| match session.tab {
            TrialTab::Dashboard => build_dashboard(body, campus, vehicle, program, session, procurement, log),
            TrialTab::Compare => build_compare(body, campus, vehicle, program, session, procurement),
            TrialTab::History => build_history(body, vehicle, program, session, procurement),
        });
    });
}

fn build_header(
    page: &mut ChildSpawnerCommands<'_>,
    campus: &CampusState,
    session: &TrialSession,
    procurement: &Procurement,
) {
    let (node, bg, interaction) = panel_node(Val::Percent(100.0));
    page.spawn((node, bg, interaction)).with_children(|header| {
        header
            .spawn(Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            })
            .with_children(|row| {
                ui_kit::label(row, "LABORATOIRE D'ESSAIS", 15.0, ui_kit::ACCENT);
                ui_kit::chip(row, format!("JOUR {}", campus.day), ui_kit::NEUTRAL);
                ui_kit::chip(
                    row,
                    format!("{} CAMPAGNES", session.history.entries().len()),
                    ui_kit::NEUTRAL,
                );
                if let Some((done, total)) = session.batch_progress() {
                    ui_kit::chip(row, format!("LOT {done}/{total}"), ui_kit::WARN);
                }
                if procurement.active().is_empty() {
                    ui_kit::chip(row, "HORS CONTRAT", ui_kit::WARN);
                } else {
                    ui_kit::chip(row, format!("{} CONTRAT(S)", procurement.active().len()), ui_kit::GOOD);
                }
            });
        header
            .spawn(Node {
                width: Val::Percent(100.0),
                column_gap: Val::Px(4.0),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            })
            .with_children(|row| {
                for (label, tab) in [
                    ("TABLEAU DE BORD", TrialTab::Dashboard),
                    ("COMPARAISON", TrialTab::Compare),
                    ("HISTORIQUE", TrialTab::History),
                ] {
                    ui_kit::button_row_item(
                        row,
                        label,
                        UiAction::Trial(TrialAction::SetTab(tab)),
                        if session.tab == tab {
                            ButtonTone::Primary
                        } else {
                            ButtonTone::Ghost
                        },
                    );
                }
                ui_kit::button_row_item(
                    row,
                    "HTML",
                    UiAction::Trial(TrialAction::ExportHtml),
                    ButtonTone::Secondary,
                );
                ui_kit::button_row_item(
                    row,
                    "CSV",
                    UiAction::Trial(TrialAction::ExportCsv),
                    ButtonTone::Secondary,
                );
                ui_kit::button_row_item(row, "PDF", UiAction::Trial(TrialAction::ExportPdf), ButtonTone::Primary);
            });
    });
}

fn build_dashboard(
    body: &mut ChildSpawnerCommands<'_>,
    campus: &CampusState,
    vehicle: &Vehicle,
    program: &Program,
    session: &TrialSession,
    procurement: &Procurement,
    log: &RevisionLog,
) {
    body.spawn(Node {
        width: Val::Px(LEFT_WIDTH),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|left| {
        build_setup_panel(left, campus, vehicle, program, session);
        build_recent_panel(left, vehicle, program, session);
    });

    body.spawn(Node {
        flex_grow: 1.0,
        flex_basis: Val::Px(0.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        overflow: Overflow::clip(),
        ..default()
    })
    .with_children(|middle| {
        build_summary_panel(middle, vehicle, program, session, log);
        build_trajectory_panel(middle, session);
        build_phases_panel(middle, session);
    });

    body.spawn(Node {
        width: Val::Px(RIGHT_WIDTH),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        overflow: Overflow::clip(),
        ..default()
    })
    .with_children(|right| {
        build_compliance_panel(right, vehicle, program, session, procurement);
        build_anomaly_panel(right, session, vehicle, program);
        build_event_panel(right, session);
    });
}

fn build_setup_panel(
    left: &mut ChildSpawnerCommands<'_>,
    campus: &CampusState,
    vehicle: &Vehicle,
    program: &Program,
    session: &TrialSession,
) {
    let (node, bg, interaction) = panel_node(Val::Percent(100.0));
    left.spawn((node, bg, interaction)).with_children(|panel| {
        ui_kit::label(panel, "ESSAI", 13.0, ui_kit::ACCENT);
        ui_kit::label(panel, vehicle.name.clone(), 10.0, ui_kit::TEXT);
        let revision = session.context.revision_id.map_or("non révisée".to_string(), |id| {
            format!("rév {id} · {}", session.context.revision_label)
        });
        let program_label = format!("P-{}", lab::short_hash(program));
        ui_kit::label(
            panel,
            format!(
                "{} · {} · {}",
                revision,
                program_label,
                lab::flight_label(session.air_launch)
            ),
            9.0,
            ui_kit::TEXT_DIM,
        );
        heading(panel, "PROFIL");
        ui_kit::button(
            panel,
            if session.air_launch {
                "LARGAGE AÉRIEN 200 m/s"
            } else {
                "LANCEMENT VERTICAL"
            },
            UiAction::Trial(TrialAction::ToggleLaunch),
            ButtonTone::Secondary,
            Val::Percent(100.0),
        );
        heading(panel, "GRAINE");
        ui_kit::label(panel, format!("Graine {}", session.seed), 13.0, ui_kit::TEXT);
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                column_gap: Val::Px(3.0),
                ..default()
            })
            .with_children(|row| {
                for (text, step) in [("-10", -10i64), ("-1", -1), ("+1", 1), ("+10", 10)] {
                    ui_kit::button_row_item(
                        row,
                        text,
                        UiAction::Trial(TrialAction::SeedStep(step)),
                        ButtonTone::Ghost,
                    );
                }
            });
        if let Some(probe) = trials::lock_probe(vehicle, program, session.seed) {
            ui_kit::label(
                panel,
                format!(
                    "Verrouillage : jet {:.0} % · probabilité {:.0} % · {:.0} m/s max · {}",
                    probe.roll * 100.0,
                    probe.probability * 100.0,
                    probe.max_speed_ms,
                    if probe.success { "réussi" } else { "raté" }
                ),
                8.5,
                if probe.success { ui_kit::GOOD } else { ui_kit::WARN },
            );
        }
        ui_kit::button(
            panel,
            "LANCER L'ESSAI",
            UiAction::Trial(TrialAction::Run),
            ButtonTone::Primary,
            Val::Percent(100.0),
        );
        heading(panel, "LOT MONTE-CARLO");
        let monte_carlo = campus.owned_capabilities().contains(trials::MONTE_CARLO);
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                column_gap: Val::Px(3.0),
                ..default()
            })
            .with_children(|row| {
                for runs in trials::BATCH_SIZES {
                    ui_kit::button_row_item(
                        row,
                        ui_kit::thousands(runs as u64),
                        UiAction::Trial(TrialAction::Batch(runs)),
                        if monte_carlo && session.batch_job.is_none() {
                            ButtonTone::Secondary
                        } else {
                            ButtonTone::Disabled
                        },
                    );
                }
            });
        if !monte_carlo {
            ui_kit::label(panel, "Recherche Monte-Carlo requise.", 9.0, ui_kit::WARN);
        }
        heading(panel, "A / B");
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                column_gap: Val::Px(3.0),
                ..default()
            })
            .with_children(|row| {
                ui_kit::button_row_item(
                    row,
                    "SNAPSHOT A",
                    UiAction::Trial(TrialAction::Snapshot),
                    ButtonTone::Secondary,
                );
                ui_kit::button_row_item(
                    row,
                    "COMPARER",
                    UiAction::Trial(TrialAction::Compare),
                    if monte_carlo {
                        ButtonTone::Secondary
                    } else {
                        ButtonTone::Disabled
                    },
                );
            });
        match &session.baseline {
            Some(baseline) => ui_kit::label(
                panel,
                format!("A : {} ({})", baseline.hash, baseline.vehicle.name),
                9.0,
                ui_kit::TEXT_DIM,
            ),
            None => ui_kit::label(
                panel,
                "A vide : faites un instantané avant de modifier.",
                9.0,
                ui_kit::TEXT_DIM,
            ),
        }
    });
}

fn build_recent_panel(
    left: &mut ChildSpawnerCommands<'_>,
    vehicle: &Vehicle,
    program: &Program,
    session: &TrialSession,
) {
    let (node, bg, interaction) = panel_node(Val::Percent(100.0));
    left.spawn((node, bg, interaction)).with_children(|panel| {
        ui_kit::label(panel, "DERNIÈRES CAMPAGNES", 11.0, ui_kit::ACCENT);
        if session.history.entries().is_empty() {
            ui_kit::label(panel, "Aucun essai enregistré.", 9.0, ui_kit::TEXT_DIM);
            return;
        }
        for entry in session.history.entries().iter().rev().take(3) {
            let stale = entry.stale_reason(vehicle, program);
            let tone = if stale.is_some() {
                ButtonTone::Danger
            } else {
                ButtonTone::Ghost
            };
            ui_kit::button(
                panel,
                format!("{} · {}", entry.label, entry.seeds_label()),
                UiAction::Trial(TrialAction::SelectHistory(entry.id)),
                tone,
                Val::Percent(100.0),
            );
            ui_kit::label(
                panel,
                format!(
                    "{} · {} · {}",
                    entry.program_version,
                    entry
                        .revision_id
                        .map_or("sans révision".to_string(), |id| format!("rév {id}")),
                    stale.unwrap_or("à jour")
                ),
                8.5,
                if stale.is_some() { ui_kit::BAD } else { ui_kit::TEXT_DIM },
            );
        }
    });
}

fn build_summary_panel(
    middle: &mut ChildSpawnerCommands<'_>,
    vehicle: &Vehicle,
    program: &Program,
    session: &TrialSession,
    log: &RevisionLog,
) {
    let (node, bg, interaction) = panel_node(Val::Percent(100.0));
    middle.spawn((node, bg, interaction)).with_children(|panel| {
        ui_kit::label(panel, "RÉSUMÉ GÉNÉRAL", 12.0, ui_kit::ACCENT);
        let metrics = DesignMetrics::of(vehicle);
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                column_gap: Val::Px(4.0),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            })
            .with_children(|row| {
                ui_kit::chip(row, format!("{:.0} kg", metrics.mass_kg), ui_kit::NEUTRAL);
                ui_kit::chip(row, format!("{:.0} cr fictifs", metrics.cost_cr), ui_kit::NEUTRAL);
                ui_kit::chip(
                    row,
                    format!("stab {:.0}", metrics.stability),
                    score_color(metrics.stability),
                );
                ui_kit::chip(
                    row,
                    format!("autorité {:.0}", metrics.authority),
                    score_color(metrics.authority),
                );
                ui_kit::chip(
                    row,
                    format!("fiab {:.0}", metrics.reliability),
                    score_color(metrics.reliability),
                );
                ui_kit::chip(row, format!("complexité {:.0}", metrics.complexity), ui_kit::TEXT_DIM);
            });
        if session.batch_stale(vehicle, program) {
            ui_kit::label(
                panel,
                "Le dernier lot a été volé avec d'autres entrées : il est obsolète.",
                9.0,
                ui_kit::WARN,
            );
        }
        match (&session.report, &session.batch, &session.failure) {
            (_, _, Some(failure)) => {
                ui_kit::chip(panel, "ESSAI REFUSÉ", ui_kit::BAD);
                ui_kit::label(panel, failure.message.clone(), 10.0, ui_kit::BAD);
                ui_kit::label(
                    panel,
                    format!(
                        "{} anomalie(s) structurée(s) — voir la colonne de droite.",
                        failure.anomalies.len()
                    ),
                    9.0,
                    ui_kit::TEXT_DIM,
                );
            }
            (Some(run), _, _) => {
                let stats = TrialStats::from_report(&run.report);
                if session.report_stale(vehicle, program) {
                    ui_kit::chip(panel, "ESSAI OBSOLÈTE", ui_kit::BAD);
                }
                ui_kit::label(
                    panel,
                    format!("Essai courant · graine {} · {}", run.seed, run.input_hash),
                    9.5,
                    ui_kit::TEXT_DIM,
                );
                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        column_gap: Val::Px(4.0),
                        flex_wrap: FlexWrap::Wrap,
                        ..default()
                    })
                    .with_children(|row| {
                        ui_kit::chip(row, format!("{:.1} km", stats.max_path_m / 1_000.0), ui_kit::GOOD);
                        ui_kit::chip(row, format!("{:.0} m/s", stats.max_speed_ms), ui_kit::ACCENT);
                        ui_kit::chip(
                            row,
                            format!("{:.1} km apogée", stats.max_altitude_m / 1_000.0),
                            ui_kit::PURPLE,
                        );
                        ui_kit::chip(row, format!("{:.1} s", stats.mean_flight_time_s), ui_kit::NEUTRAL);
                        if run.report.anomalies.is_empty() {
                            ui_kit::chip(row, "SANS ANOMALIE", ui_kit::GOOD);
                        } else {
                            ui_kit::chip(row, format!("{} ANOMALIE(S)", run.report.anomalies.len()), ui_kit::BAD);
                        }
                    });
            }
            (None, Some(batch), _) => {
                ui_kit::label(
                    panel,
                    format!("Dernier lot · {} graines", batch.runs),
                    9.5,
                    ui_kit::TEXT_DIM,
                );
                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        column_gap: Val::Px(4.0),
                        flex_wrap: FlexWrap::Wrap,
                        ..default()
                    })
                    .with_children(|row| {
                        ui_kit::chip(
                            row,
                            format!("{:.0} % réussite", batch.stats.success_rate().unwrap_or(0.0) * 100.0),
                            ui_kit::GOOD,
                        );
                        ui_kit::chip(
                            row,
                            format!("{:.1} km moyen", batch.mean_path_m / 1_000.0),
                            ui_kit::ACCENT,
                        );
                        ui_kit::chip(
                            row,
                            format!("{:.0} m/s moyen", batch.mean_max_speed_ms),
                            ui_kit::NEUTRAL,
                        );
                        ui_kit::chip(
                            row,
                            format!("{} anomalie(s)", batch.anomalies),
                            if batch.anomalies == 0 {
                                ui_kit::GOOD
                            } else {
                                ui_kit::WARN
                            },
                        );
                    });
            }
            (None, None, None) => {
                ui_kit::label(
                    panel,
                    "Aucun essai. Réglez la graine puis lancez un tir ou un lot.",
                    10.0,
                    ui_kit::TEXT_DIM,
                );
            }
        }
        ui_kit::label(
            panel,
            format!(
                "Révision courante : {} · programme P-{}",
                log.head()
                    .map_or("aucune".to_string(), |revision| revision.label.clone()),
                lab::short_hash(program)
            ),
            9.0,
            ui_kit::TEXT_DIM,
        );
    });
}

fn build_trajectory_panel(middle: &mut ChildSpawnerCommands<'_>, session: &TrialSession) {
    let (node, bg, interaction) = panel_node(Val::Percent(100.0));
    middle.spawn((node, bg, interaction)).with_children(|panel| {
        ui_kit::label(panel, "TRAJECTOIRE", 12.0, ui_kit::ACCENT);
        let Some(run) = &session.report else {
            if session.batch.is_some() {
                ui_kit::label(
                    panel,
                    "Un lot n'a pas de courbe unique : ouvrez PHASES ou l'HISTORIQUE.",
                    9.5,
                    ui_kit::TEXT_DIM,
                );
            } else {
                ui_kit::label(
                    panel,
                    "Lancez un essai pour tracer vitesse, altitude, masse et poussée.",
                    9.5,
                    ui_kit::TEXT_DIM,
                );
            }
            return;
        };
        let samples = &run.report.samples;
        plot(panel, "VITESSE", "m/s", ui_kit::ACCENT, samples, |sample| {
            sample.speed_ms
        });
        plot(panel, "ALTITUDE", "m", ui_kit::PURPLE, samples, |sample| {
            sample.altitude_m
        });
        plot(panel, "MASSE", "kg", ui_kit::WARN, samples, |sample| sample.mass_kg);
        plot(
            panel,
            "POUSSÉE",
            "kN",
            Color::srgb(0.36, 0.62, 0.95),
            samples,
            |sample| sample.thrust_n / 1_000.0,
        );
    });
}

fn build_phases_panel(middle: &mut ChildSpawnerCommands<'_>, session: &TrialSession) {
    let phases: Vec<PhaseSummary> = session
        .report
        .as_ref()
        .map(|run| lab::phases(&run.report))
        .or_else(|| session.batch.as_ref().map(|batch| batch.phases.clone()))
        .unwrap_or_default();
    let (node, bg, interaction) = panel_node(Val::Percent(100.0));
    middle.spawn((node, bg, interaction)).with_children(|panel| {
        ui_kit::label(panel, "RÉSULTATS PAR PHASE", 12.0, ui_kit::ACCENT);
        if phases.is_empty() {
            ui_kit::label(panel, "Pas de phase : aucun essai complet.", 9.0, ui_kit::TEXT_DIM);
            return;
        }
        table_header(panel, &["PHASE", "DURÉE", "VITESSE MAX", "APOGÉE", "MASSE"]);
        for phase in phases.iter().take(4) {
            table_row(
                panel,
                &[
                    phase.name.clone(),
                    format!("{:.1} s", phase.duration_s),
                    format!("{:.0} m/s", phase.max_speed_ms),
                    format!("{:.0} m", phase.max_altitude_m),
                    format!("{:.0} kg", phase.mean_mass_kg),
                ],
                ui_kit::TEXT,
            );
        }
    });
}

fn build_compliance_panel(
    right: &mut ChildSpawnerCommands<'_>,
    vehicle: &Vehicle,
    program: &Program,
    session: &TrialSession,
    procurement: &Procurement,
) {
    let (node, bg, interaction) = panel_node(Val::Percent(100.0));
    right.spawn((node, bg, interaction)).with_children(|panel| {
        ui_kit::label(panel, "CONFORMITÉ AU CONTRAT", 12.0, ui_kit::ACCENT);
        let offer = procurement.selected().map(|active| &active.offer);
        let Some(offer) = offer else {
            ui_kit::label(
                panel,
                "Aucun contrat actif : acceptez une offre pour lier cet essai.",
                9.0,
                ui_kit::TEXT_DIM,
            );
            return;
        };
        if session.report_stale(vehicle, program) {
            ui_kit::label(
                panel,
                "Essai obsolète : la conception, le programme, le profil ou la graine a changé depuis le tir. Relancez avant de conclure sur le contrat.",
                9.0,
                ui_kit::WARN,
            );
            return;
        }
        let snapshot = session
            .report
            .as_ref()
            .and_then(|run| lab::compliance_for_report(Some(offer), vehicle, program, run.seed, &run.report));
        ui_kit::label(panel, format!("#{} {}", offer.id, offer.title), 9.5, ui_kit::TEXT);
        match snapshot {
            Some(snapshot) => compliance_snapshot(panel, &snapshot),
            None => {
                let preview = CompliancePreview::of(vehicle, Some(offer));
                ui_kit::label(panel, preview.label(), 9.0, ui_kit::TEXT_DIM);
                ui_kit::label(
                    panel,
                    "Le dossier complet sera calculé après un tir.",
                    9.0,
                    ui_kit::TEXT_DIM,
                );
            }
        }
    });
}

fn compliance_snapshot(panel: &mut ChildSpawnerCommands<'_>, snapshot: &ComplianceSnapshot) {
    panel
        .spawn(Node {
            width: Val::Percent(100.0),
            column_gap: Val::Px(4.0),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|row| {
            ui_kit::chip(
                row,
                if snapshot.accepted { "ACCEPTÉ" } else { "REFUSÉ" },
                if snapshot.accepted { ui_kit::GOOD } else { ui_kit::BAD },
            );
            ui_kit::chip(
                row,
                format!("{:.0} %", snapshot.score_percent),
                if snapshot.accepted { ui_kit::GOOD } else { ui_kit::WARN },
            );
        });
    for row in snapshot.rows.iter().take(6) {
        let color = match row.status.as_str() {
            "VERIFIED" => ui_kit::GOOD,
            "MARGINAL" => ui_kit::WARN,
            _ => ui_kit::BAD,
        };
        ui_kit::label(
            panel,
            format!(
                "{} : {} {}",
                row.label,
                row.status,
                row.margin.map_or(String::new(), |margin| format!("({margin:+.1})"))
            ),
            9.0,
            color,
        );
    }
    if snapshot.rows.len() > 6 {
        ui_kit::label(
            panel,
            format!("+{} autres exigences", snapshot.rows.len() - 6),
            8.5,
            ui_kit::TEXT_DIM,
        );
    }
    for insight in lab::contract_anomalies(snapshot).iter().take(2) {
        ui_kit::label(panel, format!("⚠ {}", insight.message), 8.5, ui_kit::BAD);
    }
}

fn build_anomaly_panel(
    right: &mut ChildSpawnerCommands<'_>,
    session: &TrialSession,
    vehicle: &Vehicle,
    program: &Program,
) {
    let (node, bg, interaction) = panel_node(Val::Percent(100.0));
    right.spawn((node, bg, interaction)).with_children(|panel| {
        ui_kit::label(panel, "ANOMALIES", 12.0, ui_kit::ACCENT);
        let insights: Vec<AnomalyInsight> = match (&session.failure, &session.report) {
            (Some(failure), _) => failure.anomalies.clone(),
            (None, Some(run)) => lab::analyze_report_anomalies(&run.report, vehicle, program),
            _ => Vec::new(),
        };
        if insights.is_empty() {
            ui_kit::chip(panel, "AUCUNE ANOMALIE", ui_kit::GOOD);
            return;
        }
        for insight in insights.iter().take(5) {
            anomaly_card(panel, insight);
        }
        if insights.len() > 5 {
            ui_kit::label(panel, format!("+{} autres", insights.len() - 5), 8.5, ui_kit::BAD);
        }
    });
}

fn anomaly_card(panel: &mut ChildSpawnerCommands<'_>, insight: &AnomalyInsight) {
    panel
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            padding: UiRect::all(Val::Px(5.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        })
        .with_children(|card| {
            ui_kit::label(
                card,
                format!("{} · {}", insight.kind.label(), insight.message),
                9.0,
                ui_kit::BAD,
            );
            if !insight.detail.is_empty() {
                ui_kit::label(card, insight.detail.clone(), 8.5, ui_kit::TEXT_DIM);
            }
            if let Some(component) = &insight.component {
                ui_kit::button(
                    card,
                    format!("ATELIER · {component}"),
                    UiAction::Trial(TrialAction::InspectPart(component.clone())),
                    ButtonTone::Ghost,
                    Val::Percent(100.0),
                );
            }
            if let Some(block) = &insight.block {
                ui_kit::button(
                    card,
                    format!(
                        "PROGRAMME · {} (T{}/{})",
                        block.label,
                        block.thread + 1,
                        block.block + 1
                    ),
                    UiAction::Trial(TrialAction::InspectBlock {
                        thread: block.thread,
                        block: block.block,
                    }),
                    ButtonTone::Ghost,
                    Val::Percent(100.0),
                );
            }
        });
}

fn build_event_panel(right: &mut ChildSpawnerCommands<'_>, session: &TrialSession) {
    let (node, bg, interaction) = panel_node(Val::Percent(100.0));
    right.spawn((node, bg, interaction)).with_children(|panel| {
        ui_kit::label(panel, "ÉVÉNEMENTS IMPORTANTS", 12.0, ui_kit::ACCENT);
        let events = session
            .report
            .as_ref()
            .map(|run| run.report.events.as_slice())
            .unwrap_or(&[]);
        if events.is_empty() {
            ui_kit::label(panel, "Aucun événement de vol enregistré.", 9.0, ui_kit::TEXT_DIM);
            return;
        }
        for (time, label) in events.iter().take(8) {
            ui_kit::label(panel, format!("{time:6.1}s  {label}"), 9.0, ui_kit::TEXT);
        }
        if events.len() > 8 {
            ui_kit::label(
                panel,
                format!("+{} autres événements", events.len() - 8),
                8.5,
                ui_kit::TEXT_DIM,
            );
        }
    });
}

fn build_compare(
    body: &mut ChildSpawnerCommands<'_>,
    campus: &CampusState,
    vehicle: &Vehicle,
    program: &Program,
    session: &TrialSession,
    procurement: &Procurement,
) {
    let offer = procurement.selected().map(|active| &active.offer);
    body.spawn(Node {
        width: Val::Px(300.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|left| {
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        left.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "COMPARAISON", 13.0, ui_kit::ACCENT);
            ui_kit::label(panel, "A · instantané de référence", 10.0, ui_kit::TEXT);
            match &session.baseline {
                Some(baseline) => {
                    ui_kit::label(panel, baseline.vehicle.name.clone(), 10.0, ui_kit::TEXT_DIM);
                    ui_kit::label(panel, format!("hash {}", baseline.hash), 9.0, ui_kit::TEXT_DIM);
                    let metrics = DesignMetrics::of(&baseline.vehicle);
                    metric_lines(panel, &metrics);
                }
                None => ui_kit::label(
                    panel,
                    "Aucun instantané. Faites SNAPSHOT A avant de modifier.",
                    9.0,
                    ui_kit::TEXT_DIM,
                ),
            }
            heading(panel, "B · conception courante");
            ui_kit::label(panel, vehicle.name.clone(), 10.0, ui_kit::TEXT);
            metric_lines(panel, &DesignMetrics::of(vehicle));
            ui_kit::button(
                panel,
                "INSTANTANÉ A",
                UiAction::Trial(TrialAction::Snapshot),
                ButtonTone::Secondary,
                Val::Percent(100.0),
            );
            let monte_carlo = campus.owned_capabilities().contains(trials::MONTE_CARLO);
            ui_kit::button(
                panel,
                "CALCULER A / B",
                UiAction::Trial(TrialAction::Compare),
                if monte_carlo {
                    ButtonTone::Primary
                } else {
                    ButtonTone::Disabled
                },
                Val::Percent(100.0),
            );
            if let Some(comparison) = &session.comparison {
                ui_kit::label(
                    panel,
                    format!(
                        "A {} · B {}",
                        comparison.baseline_hash,
                        comparison.current_hash.chars().take(8).collect::<String>()
                    ),
                    9.0,
                    ui_kit::TEXT_DIM,
                );
            }
        });
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        left.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "CONFORMITÉ SUR PAPIER", 11.0, ui_kit::ACCENT);
            match offer {
                Some(offer) => {
                    if let Some(a) = &session.baseline {
                        preview_lines(panel, "A", &CompliancePreview::of(&a.vehicle, Some(offer)));
                    }
                    preview_lines(panel, "B", &CompliancePreview::of(vehicle, Some(offer)));
                    if let Some(entry) = session.history.entries().last() {
                        preview_lines(panel, "C", &CompliancePreview::of(&entry.vehicle, Some(offer)));
                    }
                }
                None => ui_kit::label(panel, "Aucun contrat actif.", 9.0, ui_kit::TEXT_DIM),
            }
        });
    });

    body.spawn(Node {
        flex_grow: 1.0,
        flex_basis: Val::Px(0.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        overflow: Overflow::clip(),
        ..default()
    })
    .with_children(|middle| {
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        middle.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "A / B / C", 12.0, ui_kit::ACCENT);
            let a_metrics = session
                .baseline
                .as_ref()
                .map(|baseline| DesignMetrics::of(&baseline.vehicle));
            let b_metrics = DesignMetrics::of(vehicle);
            let c_metrics = session.history.entries().last().map(|entry| entry.metrics);
            let mut header = vec!["INDICATEUR".to_string(), "A".to_string(), "B".to_string()];
            if c_metrics.is_some() {
                header.push("C".to_string());
            }
            header.push("B − A".to_string());
            header.push("%".to_string());
            table_header(panel, &header.iter().map(String::as_str).collect::<Vec<_>>());
            let rows = a_metrics.map(|metrics| lab::compare_rows(&metrics, &b_metrics));
            for (index, (label, b)) in b_metrics.rows().iter().enumerate() {
                let a = rows.as_ref().map(|rows| rows[index].a);
                let c = c_metrics.map(|metrics| metrics.rows()[index].1);
                let mut row = vec![
                    (*label).to_string(),
                    a.map_or_else(|| "—".to_string(), |value| format!("{value:.1}")),
                    format!("{b:.1}"),
                ];
                if c.is_some() {
                    row.push(c.map_or_else(|| "—".to_string(), |value| format!("{value:.1}")));
                }
                row.push(
                    rows.as_ref()
                        .map_or_else(|| "—".to_string(), |rows| format!("{:+.1}", rows[index].absolute())),
                );
                row.push(
                    rows.as_ref()
                        .and_then(|rows| rows[index].percent())
                        .map_or_else(|| "—".to_string(), |value| format!("{value:+.1} %")),
                );
                table_row(panel, &row, ui_kit::TEXT);
            }
            heading(panel, "RÉSULTATS DES ESSAIS");
            if let Some(comparison) = &session.comparison {
                let a = &comparison.baseline;
                let b = &comparison.current;
                table_header(panel, &["INDICATEUR", "A", "B", "B − A", "%"]);
                for (label, left, right) in [
                    ("Portée moyenne km", a.mean_path_m / 1_000.0, b.mean_path_m / 1_000.0),
                    ("Vitesse max m/s", a.mean_max_speed_ms, b.mean_max_speed_ms),
                    ("Temps moyen s", a.mean_flight_time_s, b.mean_flight_time_s),
                    ("Sans décollage", a.no_liftoff as f64, b.no_liftoff as f64),
                    ("Sans séparation", a.no_separation as f64, b.no_separation as f64),
                    ("Anomalies", a.anomalies as f64, b.anomalies as f64),
                ] {
                    table_row(
                        panel,
                        &[
                            label.to_string(),
                            format!("{left:.1}"),
                            format!("{right:.1}"),
                            format!("{:+.1}", right - left),
                            lab::percent_delta(left, right)
                                .map_or_else(|| "—".to_string(), |value| format!("{value:+.1} %")),
                        ],
                        ui_kit::TEXT,
                    );
                }
                ui_kit::label(
                    panel,
                    format!("Verrouillage A {} · B {}", rate(a.lock_rate()), rate(b.lock_rate())),
                    9.0,
                    ui_kit::TEXT_DIM,
                );
            } else {
                ui_kit::label(
                    panel,
                    "Aucune comparaison calculée : snapshot A puis CALCULER A/B.",
                    9.0,
                    ui_kit::TEXT_DIM,
                );
            }
        });
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        middle.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "DERNIÈRE CAMPAGNE C", 11.0, ui_kit::ACCENT);
            match session.history.entries().last() {
                Some(entry) => {
                    ui_kit::label(
                        panel,
                        format!("{} · {}", entry.label, entry.seeds_label()),
                        10.0,
                        ui_kit::TEXT,
                    );
                    ui_kit::label(
                        panel,
                        format!(
                            "rév {} · {} · {}",
                            entry.revision_id.map_or_else(|| "—".to_string(), |id| id.to_string()),
                            entry.program_version,
                            entry.stale_reason(vehicle, program).unwrap_or("à jour")
                        ),
                        9.0,
                        ui_kit::TEXT_DIM,
                    );
                    ui_kit::label(
                        panel,
                        format!(
                            "portée {:.1} km · vitesse {:.0} m/s · apogée {:.1} km",
                            entry.stats.mean_path_m / 1_000.0,
                            entry.stats.max_speed_ms,
                            entry.stats.max_altitude_m / 1_000.0
                        ),
                        9.0,
                        ui_kit::TEXT_DIM,
                    );
                    ui_kit::button(
                        panel,
                        "RELANCER CETTE CAMPAGNE",
                        UiAction::Trial(TrialAction::Relaunch(entry.id)),
                        ButtonTone::Secondary,
                        Val::Percent(100.0),
                    );
                }
                None => ui_kit::label(panel, "Aucune campagne enregistrée.", 9.0, ui_kit::TEXT_DIM),
            }
        });
    });

    body.spawn(Node {
        width: Val::Px(RIGHT_WIDTH),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        overflow: Overflow::clip(),
        ..default()
    })
    .with_children(|right| {
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        right.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "LECTURE DU CONTRAT", 12.0, ui_kit::ACCENT);
            if let Some(offer) = offer {
                ui_kit::label(panel, format!("#{} {}", offer.id, offer.title), 10.0, ui_kit::TEXT);
                let checks = CompliancePreview::of(vehicle, Some(offer));
                for check in checks.checks.iter().take(9) {
                    let color = match check.status {
                        omega_rules::CheckStatus::Met => ui_kit::GOOD,
                        omega_rules::CheckStatus::Close => ui_kit::WARN,
                        omega_rules::CheckStatus::Missed => ui_kit::BAD,
                        omega_rules::CheckStatus::Unproven => ui_kit::TEXT_DIM,
                    };
                    ui_kit::label(
                        panel,
                        format!(
                            "{} : {} ({})",
                            check.label,
                            check.status.label(),
                            check.value.map_or("essai".to_string(), |value| format!("{value:.1}"))
                        ),
                        9.0,
                        color,
                    );
                }
            } else {
                ui_kit::label(panel, "Aucun contrat actif.", 9.0, ui_kit::TEXT_DIM);
            }
        });
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        right.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "STATISTIQUES DU LOT", 12.0, ui_kit::ACCENT);
            match &session.batch {
                Some(batch) => {
                    ui_kit::label(
                        panel,
                        format!("{} essais · {} réussites", batch.runs, batch.stats.successes),
                        9.5,
                        ui_kit::TEXT,
                    );
                    ui_kit::label(
                        panel,
                        format!(
                            "portée moyenne {:.1} km · apogée {:.1} km",
                            batch.mean_path_m / 1_000.0,
                            batch.stats.mean_altitude_m / 1_000.0
                        ),
                        9.0,
                        ui_kit::TEXT_DIM,
                    );
                    ui_kit::label(
                        panel,
                        format!(
                            "décollages manqués {} · séparations {} · anomalies {}",
                            batch.no_liftoff, batch.no_separation, batch.anomalies
                        ),
                        9.0,
                        ui_kit::TEXT_DIM,
                    );
                }
                None => ui_kit::label(panel, "Aucun lot disponible.", 9.0, ui_kit::TEXT_DIM),
            }
            if let Some(failure) = &session.failure {
                ui_kit::label(panel, failure.message.clone(), 9.0, ui_kit::BAD);
            }
        });
    });
}

fn build_history(
    body: &mut ChildSpawnerCommands<'_>,
    vehicle: &Vehicle,
    program: &Program,
    session: &TrialSession,
    procurement: &Procurement,
) {
    body.spawn(Node {
        width: Val::Px(360.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(6.0),
        overflow: Overflow::clip(),
        ..default()
    })
    .with_children(|left| {
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        left.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "CAMPAGNES D'ESSAIS", 13.0, ui_kit::ACCENT);
            if session.history.entries().is_empty() {
                ui_kit::label(
                    panel,
                    "Aucune campagne. Lancez un essai pour créer la première.",
                    9.5,
                    ui_kit::TEXT_DIM,
                );
                return;
            }
            for entry in session.history.entries().iter().rev().take(12) {
                let stale = entry.stale_reason(vehicle, program);
                let tone = if stale.is_some() {
                    ButtonTone::Danger
                } else if session.selected_history == Some(entry.id) {
                    ButtonTone::Primary
                } else {
                    ButtonTone::Ghost
                };
                ui_kit::button(
                    panel,
                    format!(
                        "{} · {} · {}",
                        entry.label,
                        entry.seeds_label(),
                        entry
                            .revision_id
                            .map_or("sans rév".to_string(), |id| format!("rév {id}"))
                    ),
                    UiAction::Trial(TrialAction::SelectHistory(entry.id)),
                    tone,
                    Val::Percent(100.0),
                );
                ui_kit::label(
                    panel,
                    format!(
                        "{} · {} · {}",
                        entry.program_version,
                        entry.metrics.mass_kg.round() as u64,
                        stale.unwrap_or("à jour")
                    ),
                    8.0,
                    if stale.is_some() { ui_kit::BAD } else { ui_kit::TEXT_DIM },
                );
            }
        });
    });

    let selected = session
        .selected_history
        .and_then(|id| session.history.get(id))
        .or_else(|| session.history.entries().last());
    let Some(entry) = selected else {
        body.spawn(Node {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|panel| ui_kit::label(panel, "Choisissez une campagne à gauche.", 11.0, ui_kit::TEXT_DIM));
        return;
    };

    body.spawn(Node {
        flex_grow: 1.0,
        flex_basis: Val::Px(0.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        overflow: Overflow::clip(),
        ..default()
    })
    .with_children(|middle| {
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        middle.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, format!("{} · {}", entry.label, entry.kind), 13.0, ui_kit::ACCENT);
            let stale = entry.stale_reason(vehicle, program);
            let is_stale = entry.stale(vehicle, program);
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    ui_kit::chip(
                        row,
                        if is_stale { "OBSOLÈTE" } else { "À JOUR" },
                        if is_stale { ui_kit::BAD } else { ui_kit::GOOD },
                    );
                    ui_kit::chip(row, entry.seeds_label(), ui_kit::NEUTRAL);
                    ui_kit::chip(row, lab::flight_label(entry.air_launch), ui_kit::NEUTRAL);
                    if entry.relaunched_from.is_some() {
                        ui_kit::chip(row, "RELANCÉ", ui_kit::ACCENT);
                    }
                });
            if let Some(reason) = stale {
                ui_kit::label(
                    panel,
                    format!("Preuve obsolète : {reason}. Relancez pour rafraîchir."),
                    9.0,
                    ui_kit::WARN,
                );
            }
            ui_kit::label(
                panel,
                format!(
                    "Révision : {} ({})",
                    entry.revision_label,
                    entry.revision_hash.chars().take(10).collect::<String>()
                ),
                9.0,
                ui_kit::TEXT_DIM,
            );
            ui_kit::label(
                panel,
                format!(
                    "Version programme : {} · hash entrée : {}",
                    entry.program_version, entry.input_hash
                ),
                9.0,
                ui_kit::TEXT_DIM,
            );
            ui_kit::label(
                panel,
                format!("Empreinte rejouable : {}", lab::entry_hash(entry)),
                8.5,
                ui_kit::TEXT_DIM,
            );
            ui_kit::label(
                panel,
                format!("Événements : {}", entry.events.len()),
                9.0,
                ui_kit::TEXT_DIM,
            );
            heading(panel, "FIGURES");
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    ui_kit::chip(row, format!("{:.0} kg", entry.metrics.mass_kg), ui_kit::NEUTRAL);
                    ui_kit::chip(row, format!("{:.0} cr", entry.metrics.cost_cr), ui_kit::NEUTRAL);
                    ui_kit::chip(
                        row,
                        format!("stab {:.0}", entry.metrics.stability),
                        score_color(entry.metrics.stability),
                    );
                    ui_kit::chip(
                        row,
                        format!("autorité {:.0}", entry.metrics.authority),
                        score_color(entry.metrics.authority),
                    );
                    ui_kit::chip(
                        row,
                        format!("fiab {:.0}", entry.metrics.reliability),
                        score_color(entry.metrics.reliability),
                    );
                });
            ui_kit::label(
                panel,
                format!(
                    "Portée moyenne {:.1} km · vitesse max {:.0} m/s · apogée max {:.1} km · réussite {}",
                    entry.stats.mean_path_m / 1_000.0,
                    entry.stats.max_speed_ms,
                    entry.stats.max_altitude_m / 1_000.0,
                    rate(entry.stats.success_rate())
                ),
                9.5,
                ui_kit::TEXT,
            );
            heading(panel, "PHASES");
            for phase in entry.phases.iter().take(4) {
                ui_kit::label(
                    panel,
                    format!(
                        "{} · {:.1} s · {:.0} m/s · {:.0} m",
                        phase.name, phase.duration_s, phase.max_speed_ms, phase.max_altitude_m
                    ),
                    9.0,
                    ui_kit::TEXT_DIM,
                );
            }
            if !entry.events.is_empty() {
                heading(panel, "ÉVÉNEMENTS");
                for (time, event) in entry.events.iter().take(6) {
                    ui_kit::label(panel, format!("{time:6.1}s  {event}"), 8.5, ui_kit::TEXT_DIM);
                }
            }
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    ui_kit::button_row_item(
                        row,
                        "RELANCER",
                        UiAction::Trial(TrialAction::Relaunch(entry.id)),
                        ButtonTone::Primary,
                    );
                    ui_kit::button_row_item(
                        row,
                        "HTML",
                        UiAction::Trial(TrialAction::ExportHtml),
                        ButtonTone::Secondary,
                    );
                    ui_kit::button_row_item(
                        row,
                        "CSV",
                        UiAction::Trial(TrialAction::ExportCsv),
                        ButtonTone::Secondary,
                    );
                    ui_kit::button_row_item(
                        row,
                        "PDF",
                        UiAction::Trial(TrialAction::ExportPdf),
                        ButtonTone::Secondary,
                    );
                });
            if !entry.change_summary.is_empty() {
                ui_kit::label(
                    panel,
                    format!("Changements : {}", entry.change_summary),
                    9.0,
                    ui_kit::TEXT_DIM,
                );
            }
        });
    });

    body.spawn(Node {
        width: Val::Px(RIGHT_WIDTH),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        overflow: Overflow::clip(),
        ..default()
    })
    .with_children(|right| {
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        right.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "ANOMALIES DE LA CAMPAGNE", 12.0, ui_kit::ACCENT);
            if entry.anomalies.is_empty() {
                ui_kit::chip(panel, "AUCUNE ANOMALIE", ui_kit::GOOD);
            } else {
                for insight in entry.anomalies.iter().take(5) {
                    anomaly_card(panel, insight);
                }
            }
        });
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        right.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "CONFORMITÉ HISTORIQUE", 12.0, ui_kit::ACCENT);
            match &entry.compliance {
                Some(snapshot) => compliance_snapshot(panel, snapshot),
                None => {
                    let offer = procurement.selected().map(|active| &active.offer);
                    let preview = CompliancePreview::of(&entry.vehicle, offer);
                    ui_kit::label(panel, preview.label(), 9.0, ui_kit::TEXT_DIM);
                    ui_kit::label(
                        panel,
                        "Le dossier de cette campagne sera figé à la prochaine version du laboratoire.",
                        8.5,
                        ui_kit::TEXT_DIM,
                    );
                }
            }
        });
    });
}

fn metric_lines(panel: &mut ChildSpawnerCommands<'_>, metrics: &DesignMetrics) {
    ui_kit::label(
        panel,
        format!("{:.0} kg · {:.0} cr fictifs", metrics.mass_kg, metrics.cost_cr),
        9.0,
        ui_kit::TEXT_DIM,
    );
    ui_kit::label(
        panel,
        format!(
            "stab {:.0} · autorité {:.0} · complexité {:.0} · fiabilité {:.0}",
            metrics.stability, metrics.authority, metrics.complexity, metrics.reliability
        ),
        9.0,
        ui_kit::TEXT_DIM,
    );
}

fn preview_lines(panel: &mut ChildSpawnerCommands<'_>, label: &str, preview: &CompliancePreview) {
    ui_kit::label(
        panel,
        format!("{label} · {}", preview.label()),
        9.0,
        if preview.missed > 0 {
            ui_kit::BAD
        } else {
            ui_kit::TEXT_DIM
        },
    );
}

fn table_header(panel: &mut ChildSpawnerCommands<'_>, cells: &[&str]) {
    table_row(
        panel,
        &cells.iter().map(|cell| (*cell).to_string()).collect::<Vec<_>>(),
        ui_kit::TEXT_DIM,
    );
}

fn table_row(panel: &mut ChildSpawnerCommands<'_>, cells: &[String], color: Color) {
    panel
        .spawn(Node {
            width: Val::Percent(100.0),
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            for cell in cells {
                row.spawn(Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    ..default()
                })
                .with_children(|cell_node| ui_kit::label(cell_node, cell.clone(), 9.0, color));
            }
        });
}
