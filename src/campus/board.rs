//! Card-based boards that replace the old text walls: the island facility cards, the Contract
//! Office (offers, active projects, dossier), the Research Centre and the workshop project card.
//! Boards are rebuilt only when what they show changes.

use super::*;
use crate::design::{InspectorState, RevisionLog, record_revision, restore_revision};
use crate::interface::{UiAction, UiActionButton};
use crate::missile::{self, MissileAction, MissileSession, PartTemplate};
use crate::procurement::{self, EvidenceStatus, Procurement};
use crate::program_editor::{self, BlockKind, MissileProgramSession, ProgramAction, adjustable};
use crate::trials::{self, TrialAction, TrialSession};
use crate::ui_kit::{self, ButtonTone, NavStatus, UiState};
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::ui::ScrollPosition;
use std::collections::HashMap;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum BoardKind {
    Island,
    Contracts,
    Research,
    Project,
    Missile,
    Program,
    Trials,
}

#[derive(Component)]
pub(crate) struct ToastText;

/// Marks the scrolling container that holds the missile program block list.
#[derive(Component)]
pub(crate) struct ProgramListScroll;

fn root_node(kind: BoardKind) -> Node {
    let mut node = Node {
        position_type: PositionType::Absolute,
        display: Display::None,
        ..default()
    };
    match kind {
        BoardKind::Island => {
            node.left = Val::Px(0.0);
            node.right = Val::Px(0.0);
            node.top = Val::Px(62.0);
            node.bottom = Val::Px(0.0);
            node.flex_direction = FlexDirection::Column;
            node.justify_content = JustifyContent::SpaceBetween;
            node.padding = UiRect {
                left: Val::Px(18.0),
                right: Val::Px(18.0),
                top: Val::Px(20.0),
                bottom: Val::Px(18.0),
            };
        }
        BoardKind::Contracts | BoardKind::Research | BoardKind::Missile | BoardKind::Program | BoardKind::Trials => {
            node.left = Val::Px(14.0);
            node.right = Val::Px(14.0);
            node.top = Val::Px(76.0);
            node.bottom = Val::Px(14.0);
            node.flex_direction = FlexDirection::Row;
            node.column_gap = Val::Px(14.0);
        }
        BoardKind::Project => {
            node.left = Val::Px(640.0);
            node.bottom = Val::Px(14.0);
            node.width = Val::Px(540.0);
            node.flex_direction = FlexDirection::Column;
        }
    }
    node
}

pub(crate) fn setup_boards(mut commands: Commands) {
    for kind in [
        BoardKind::Island,
        BoardKind::Contracts,
        BoardKind::Research,
        BoardKind::Project,
        BoardKind::Missile,
        BoardKind::Program,
        BoardKind::Trials,
    ] {
        commands.spawn((root_node(kind), kind));
    }
}

fn phase_chip(phase: ContractPhase) -> (&'static str, Color) {
    match phase {
        ContractPhase::Accepted => ("DESIGN REQUIRED", ui_kit::NEUTRAL),
        ContractPhase::NeedsRevision => ("REVISE DESIGN", ui_kit::BAD),
        ContractPhase::DesignSubmitted => ("ANALYSIS NEEDED", ui_kit::WARN),
        ContractPhase::AnalysisQueued => ("ANALYSIS RUNNING", ui_kit::PURPLE),
        ContractPhase::AnalysisPassed => ("FIELD TEST NEEDED", ui_kit::WARN),
        ContractPhase::AnalysisFailed => ("ANALYSIS FAILED", ui_kit::BAD),
        ContractPhase::TestPassed => ("READY TO DELIVER", ui_kit::GOOD),
        ContractPhase::TestFailed => ("TEST FAILED", ui_kit::BAD),
    }
}

/// Current step (0 design, 1 analysis, 2 field test, 3 delivery) and whether it has failed.
fn step_of(phase: ContractPhase) -> (usize, bool) {
    match phase {
        ContractPhase::Accepted => (0, false),
        ContractPhase::NeedsRevision => (0, true),
        ContractPhase::DesignSubmitted | ContractPhase::AnalysisQueued => (1, false),
        ContractPhase::AnalysisFailed => (1, true),
        ContractPhase::AnalysisPassed => (2, false),
        ContractPhase::TestFailed => (2, true),
        ContractPhase::TestPassed => (3, false),
    }
}

fn domain_color(domain: ContractDomain) -> Color {
    match domain {
        ContractDomain::ScientificSounding => Color::srgb(0.36, 0.62, 0.95),
        ContractDomain::LongDurationObservation => Color::srgb(0.62, 0.50, 0.90),
        ContractDomain::GroundWeatherNetwork => Color::srgb(0.35, 0.75, 0.55),
        ContractDomain::CivilAirSafety => Color::srgb(0.92, 0.62, 0.30),
        ContractDomain::EnvironmentalSurvey => Color::srgb(0.30, 0.78, 0.80),
    }
}

fn badge_color(badge: &str) -> Color {
    match badge {
        "OK" => ui_kit::GOOD,
        "THIN" => ui_kit::WARN,
        "FAIL" => ui_kit::BAD,
        "STALE" => Color::srgb(0.50, 0.56, 0.64),
        "METHOD" => ui_kit::PURPLE,
        _ => ui_kit::NEUTRAL,
    }
}

fn heading(parent: &mut ChildSpawnerCommands<'_>, text: impl Into<String>) {
    parent
        .spawn(Node {
            margin: UiRect {
                top: Val::Px(4.0),
                ..default()
            },
            ..default()
        })
        .with_children(|h| ui_kit::label(h, text, 11.0, ui_kit::TEXT_DIM));
}

fn difficulty_bar(parent: &mut ChildSpawnerCommands<'_>, difficulty: u8) {
    parent
        .spawn(Node {
            column_gap: Val::Px(3.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|bar| {
            for step in 1..=5u8 {
                bar.spawn((
                    Node {
                        width: Val::Px(9.0),
                        height: Val::Px(9.0),
                        border_radius: BorderRadius::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(if step <= difficulty {
                        ui_kit::WARN
                    } else {
                        Color::srgb(0.14, 0.19, 0.24)
                    }),
                ));
            }
        });
}

// ----------------------------------------------------------------------------------------------
// Island
// ----------------------------------------------------------------------------------------------

fn build_island(root: &mut ChildSpawnerCommands<'_>, campus: &CampusState) {
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|hero| {
        ui_kit::label(hero, "RESEARCH ISLAND", 30.0, ui_kit::ACCENT);
        ui_kit::label(
            hero,
            "A fictional civil and scientific engineering campus. Take contracts, design, test and deliver.",
            12.0,
            ui_kit::TEXT_DIM,
        );
        ui_kit::label(
            hero,
            "Camera: right drag to orbit | middle drag to pan | wheel to zoom",
            10.5,
            ui_kit::TEXT_DIM,
        );
        hero.spawn(Node {
            column_gap: Val::Px(8.0),
            margin: UiRect {
                top: Val::Px(4.0),
                ..default()
            },
            ..default()
        })
        .with_children(|stats| {
            ui_kit::chip(stats, format!("{} cr", ui_kit::thousands(campus.credits)), ui_kit::GOOD);
            ui_kit::chip(
                stats,
                format!(
                    "{}/{} active projects",
                    campus.accepted_contracts.len(),
                    MAX_ACTIVE_CONTRACTS
                ),
                ui_kit::WARN,
            );
            ui_kit::chip(
                stats,
                format!("{} completed", campus.completed_contracts.len()),
                ui_kit::NEUTRAL,
            );
        });
    });

    let facilities: [(&str, &str, &str, CampusView, Color); 6] = [
        (
            "CONTRACT OFFICE",
            "F1",
            "Client offers, requirements, deadlines and payment. Deliver a verified dossier.",
            CampusView::Contracts,
            Color::srgb(0.92, 0.62, 0.30),
        ),
        (
            "MISSILE WORKSHOP",
            "F2",
            "Build a staged vehicle from resizable parts: tanks, motors, engines, sensors and a computer.",
            CampusView::Missile,
            Color::srgb(0.36, 0.84, 0.94),
        ),
        (
            "RESEARCH",
            "F3",
            "Spend credits and days to unlock engines, sensors, computers and missile families.",
            CampusView::Research,
            Color::srgb(0.62, 0.50, 0.90),
        ),
        (
            "MISSILE PROGRAM",
            "F4",
            "Compose the onboard program from the blocks the installed hardware unlocks.",
            CampusView::Program,
            Color::srgb(0.35, 0.75, 0.55),
        ),
        (
            "TRIALS",
            "NEW",
            "Fly the vehicle and its program: four plots, events, anomalies, seeded batches and A/B comparison.",
            CampusView::Trials,
            Color::srgb(0.90, 0.55, 0.35),
        ),
        (
            "COMPUTE",
            "F5",
            "Thermal and structural studies, meshing, field tests and reports.",
            CampusView::Computation,
            Color::srgb(0.36, 0.62, 0.95),
        ),
    ];
    root.spawn(Node {
        width: Val::Percent(100.0),
        column_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|row| {
        for (title, key, text, view, color) in facilities {
            let (node, bg, border) = ui_kit::card_node(Val::Px(0.0), color);
            let mut node = node;
            node.flex_grow = 1.0;
            node.flex_basis = Val::Px(0.0);
            row.spawn((node, bg, border, UiInteraction::None))
                .with_children(|card| {
                    card.spawn(Node {
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|top| {
                        ui_kit::label(top, title, 14.0, ui_kit::TEXT);
                        ui_kit::chip(top, key, color);
                    });
                    ui_kit::label(card, text, 10.5, ui_kit::TEXT_DIM);
                    ui_kit::button(
                        card,
                        "OPEN",
                        UiAction::Navigate(view),
                        ButtonTone::Primary,
                        Val::Percent(100.0),
                    );
                });
        }
    });
}

// ----------------------------------------------------------------------------------------------
// Contract Office
// ----------------------------------------------------------------------------------------------

fn offer_card(parent: &mut ChildSpawnerCommands<'_>, index: usize, offer: &ContractOffer, can_accept: bool) {
    let color = domain_color(offer.domain);
    let (node, bg, border) = ui_kit::card_node(Val::Px(298.0), color);
    parent
        .spawn((node, bg, border, UiInteraction::None))
        .with_children(|card| {
            card.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|top| {
                ui_kit::chip(top, offer.domain.label(), color);
                difficulty_bar(top, offer.difficulty);
            });
            card.spawn(Node {
                min_height: Val::Px(38.0),
                ..default()
            })
            .with_children(|t| ui_kit::label(t, offer.title.clone(), 15.0, ui_kit::TEXT));
            ui_kit::label(card, offer.client.clone(), 10.0, ui_kit::TEXT_DIM);
            let r = &offer.requirements;
            let balance = r
                .max_cg_offset_ratio
                .map_or("n/a".to_string(), |ratio| format!("{:.0}%", ratio * 100.0));
            for line in [
                format!("Mass <= {} kg    Power <= {} W", r.max_unit_mass_kg, r.max_unit_power_w),
                format!(
                    "Uptime >= {}%    Data >= {}%",
                    r.min_uptime_percent, r.min_data_completeness_percent
                ),
                format!("Storage >= {} GB    Units: {}", r.min_storage_gb, r.required_units),
                format!(
                    "Balance <= {balance}    Unit budget {} cr",
                    ui_kit::thousands(r.max_unit_cost_cr)
                ),
            ] {
                ui_kit::label(card, line, 10.5, Color::srgb(0.78, 0.84, 0.88));
            }
            card.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                margin: UiRect {
                    top: Val::Px(4.0),
                    ..default()
                },
                ..default()
            })
            .with_children(|bottom| {
                bottom
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        ..default()
                    })
                    .with_children(|pay| {
                        ui_kit::label(
                            pay,
                            format!("{} cr", ui_kit::thousands(offer.reward)),
                            17.0,
                            ui_kit::GOOD,
                        );
                        ui_kit::label(pay, format!("{} days", offer.deadline_days), 10.0, ui_kit::TEXT_DIM);
                    });
                let tone = if can_accept {
                    ButtonTone::Primary
                } else {
                    ButtonTone::Disabled
                };
                ui_kit::button(bottom, "ACCEPT", UiAction::AcceptOffer(index), tone, Val::Px(110.0));
            });
        });
}

fn active_card(parent: &mut ChildSpawnerCommands<'_>, active: &ActiveContract, selected: bool) {
    let (status, status_color) = phase_chip(active.phase);
    let (step, failed) = step_of(active.phase);
    let accent = if selected {
        ui_kit::ACCENT
    } else {
        domain_color(active.offer.domain)
    };
    let (mut node, mut bg, border) = ui_kit::card_node(Val::Px(298.0), accent);
    if selected {
        bg = BackgroundColor(ui_kit::CARD_SELECTED);
    }
    node.min_height = Val::Px(170.0);
    parent
        .spawn((node, bg, border, UiInteraction::None))
        .with_children(|card| {
            card.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|top| {
                ui_kit::chip(top, status, status_color);
                ui_kit::label(top, format!("#{:04}", active.offer.id), 10.0, ui_kit::TEXT_DIM);
            });
            card.spawn(Node {
                min_height: Val::Px(36.0),
                ..default()
            })
            .with_children(|t| ui_kit::label(t, active.offer.title.clone(), 14.0, ui_kit::TEXT));
            ui_kit::label(card, active.offer.client.clone(), 10.0, ui_kit::TEXT_DIM);
            card.spawn(Node {
                width: Val::Percent(100.0),
                column_gap: Val::Px(4.0),
                margin: UiRect {
                    top: Val::Px(2.0),
                    ..default()
                },
                ..default()
            })
            .with_children(|steps| {
                for (index, name) in ["DESIGN", "ANALYSIS", "FIELD TEST", "DELIVERY"].into_iter().enumerate() {
                    steps
                        .spawn(Node {
                            flex_grow: 1.0,
                            flex_basis: Val::Px(0.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(3.0),
                            ..default()
                        })
                        .with_children(|cell| {
                            let color = if index < step {
                                ui_kit::GOOD
                            } else if index == step {
                                if failed {
                                    ui_kit::BAD
                                } else if active.phase == ContractPhase::TestPassed {
                                    ui_kit::GOOD
                                } else {
                                    ui_kit::ACCENT
                                }
                            } else {
                                Color::srgb(0.14, 0.19, 0.24)
                            };
                            cell.spawn((
                                Node {
                                    height: Val::Px(6.0),
                                    border_radius: BorderRadius::all(Val::Px(3.0)),
                                    ..default()
                                },
                                BackgroundColor(color),
                            ));
                            ui_kit::label(
                                cell,
                                name,
                                8.5,
                                if index <= step { ui_kit::TEXT } else { ui_kit::TEXT_DIM },
                            );
                        });
                }
            });
            card.spawn(Node {
                column_gap: Val::Px(6.0),
                margin: UiRect {
                    top: Val::Px(4.0),
                    ..default()
                },
                ..default()
            })
            .with_children(|buttons| {
                if selected {
                    ui_kit::button_row_item(
                        buttons,
                        "LEGACY BENCH",
                        UiAction::Navigate(CampusView::Construction),
                        ButtonTone::Secondary,
                    );
                    ui_kit::button_row_item(
                        buttons,
                        "COMPUTE",
                        UiAction::Navigate(CampusView::Computation),
                        ButtonTone::Secondary,
                    );
                    let tone = if active.phase == ContractPhase::TestPassed {
                        ButtonTone::Primary
                    } else {
                        ButtonTone::Disabled
                    };
                    ui_kit::button_row_item(buttons, "DELIVER", UiAction::DeliverContract, tone);
                } else {
                    ui_kit::button_row_item(
                        buttons,
                        "SELECT PROJECT",
                        UiAction::SelectContract(active.offer.id),
                        ButtonTone::Primary,
                    );
                }
            });
        });
}

fn dossier_rows(parent: &mut ChildSpawnerCommands<'_>, active: &ActiveContract, design: &ContentHash, compact: bool) {
    let view = verification::matrix_view(active, design);
    for row in &view.rows {
        parent
            .spawn(Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|line| {
                line.spawn(Node {
                    width: Val::Px(50.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|cell| {
                    ui_kit::chip(cell, row.badge, badge_color(row.badge));
                });
                if compact {
                    line.spawn(Node {
                        flex_grow: 1.0,
                        flex_basis: Val::Px(0.0),
                        ..default()
                    })
                    .with_children(|c| ui_kit::label(c, format!("{} {}", row.label, row.rule), 10.5, ui_kit::TEXT));
                    ui_kit::label(line, row.value.clone(), 10.0, ui_kit::TEXT_DIM);
                } else {
                    line.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        flex_grow: 1.0,
                        flex_basis: Val::Px(0.0),
                        ..default()
                    })
                    .with_children(|c| {
                        ui_kit::label(c, format!("{} {}", row.label, row.rule), 11.0, ui_kit::TEXT);
                        ui_kit::label(c, row.detail.clone(), 9.5, ui_kit::TEXT_DIM);
                    });
                }
            });
    }
    let (text, color) = if view.ready {
        (format!("DOSSIER READY - score {}%", view.score_percent), ui_kit::GOOD)
    } else {
        (
            format!("DOSSIER NOT READY - {} open item(s)", view.open_items),
            ui_kit::WARN,
        )
    };
    parent
        .spawn(Node {
            margin: UiRect {
                top: Val::Px(4.0),
                ..default()
            },
            ..default()
        })
        .with_children(|v| ui_kit::chip(v, text, color));
}

fn contract_color(family: omega_rules::contracts::MissionFamily) -> Color {
    use omega_rules::contracts::MissionFamily;
    match family {
        MissionFamily::AirToAir => ui_kit::PURPLE,
        MissionFamily::AirToSurface => ui_kit::ACCENT,
        MissionFamily::SurfaceToAir => ui_kit::GOOD,
        MissionFamily::SurfaceToSurface => ui_kit::WARN,
    }
}

fn requirement_text(requirement: &omega_rules::Requirement) -> String {
    match requirement.metric.as_str() {
        "path_length_m" => format!("{} >= {:.0} km", requirement.label, requirement.limit / 1_000.0),
        "max_speed_ms" => format!("{} >= {:.0} m/s", requirement.label, requirement.limit),
        "launch_mass_kg" => format!("{} <= {:.0} kg", requirement.label, requirement.limit),
        _ => format!("{} required", requirement.label),
    }
}

fn requirement_badge(status: &omega_rules::RequirementStatus) -> (&'static str, Color) {
    match status {
        omega_rules::RequirementStatus::Verified { .. } => ("OK", ui_kit::GOOD),
        omega_rules::RequirementStatus::Marginal { .. } => ("THIN", ui_kit::WARN),
        omega_rules::RequirementStatus::Failed { .. } => ("FAIL", ui_kit::BAD),
        omega_rules::RequirementStatus::Stale => ("STALE", Color::srgb(0.50, 0.56, 0.64)),
        omega_rules::RequirementStatus::Unverified => ("MISSING", ui_kit::NEUTRAL),
        omega_rules::RequirementStatus::MethodNotAllowed => ("METHOD", ui_kit::PURPLE),
    }
}

fn missile_offer_card(
    parent: &mut ChildSpawnerCommands<'_>,
    index: usize,
    offer: &omega_rules::contracts::ContractOffer,
    can_accept: bool,
) {
    let color = contract_color(offer.mission_family);
    let (node, bg, border) = ui_kit::card_node(Val::Px(286.0), color);
    parent
        .spawn((node, bg, border, UiInteraction::None))
        .with_children(|card| {
            card.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|top| {
                ui_kit::chip(top, offer.mission_family.label(), color);
                ui_kit::chip(top, offer.vehicle_family.label(), ui_kit::NEUTRAL);
                difficulty_bar(top, offer.difficulty);
            });
            card.spawn(Node {
                min_height: Val::Px(34.0),
                ..default()
            })
            .with_children(|t| ui_kit::label(t, offer.title.clone(), 14.0, ui_kit::TEXT));
            ui_kit::label(card, offer.client.clone(), 10.0, ui_kit::TEXT_DIM);
            for requirement in offer.missile_requirements().iter().take(4) {
                ui_kit::label(card, requirement_text(requirement), 10.0, ui_kit::TEXT);
            }
            card.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                margin: UiRect {
                    top: Val::Px(4.0),
                    ..default()
                },
                ..default()
            })
            .with_children(|bottom| {
                bottom
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        ..default()
                    })
                    .with_children(|pay| {
                        ui_kit::label(
                            pay,
                            format!("{} cr", ui_kit::thousands(offer.reward_credits)),
                            15.0,
                            ui_kit::GOOD,
                        );
                        ui_kit::label(pay, format!("{} days", offer.deadline_days), 9.5, ui_kit::TEXT_DIM);
                    });
                let tone = if can_accept {
                    ButtonTone::Primary
                } else {
                    ButtonTone::Disabled
                };
                ui_kit::button(
                    bottom,
                    "ACCEPT",
                    UiAction::AcceptMissileOffer(index),
                    tone,
                    Val::Px(96.0),
                );
            });
        });
}

fn missile_active_card(
    parent: &mut ChildSpawnerCommands<'_>,
    active: &omega_rules::contracts::ContractOffer,
    status: EvidenceStatus,
    deadline_passed: bool,
    score: Option<f64>,
    ready: bool,
) {
    let color = contract_color(active.mission_family);
    let (node, bg, border) = ui_kit::card_node(Val::Px(560.0), color);
    parent
        .spawn((node, bg, border, UiInteraction::None))
        .with_children(|card| {
            card.spawn(Node {
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|top| {
                let (label, tone) = match status {
                    EvidenceStatus::NoContract => ("NO CONTRACT", ui_kit::NEUTRAL),
                    EvidenceStatus::NoTrial => ("TRIAL REQUIRED", ui_kit::WARN),
                    EvidenceStatus::Current => ("EVIDENCE CURRENT", ui_kit::GOOD),
                    EvidenceStatus::Stale => ("STALE", ui_kit::BAD),
                };
                ui_kit::chip(top, label, tone);
                ui_kit::chip(top, format!("#{:04}", active.id), ui_kit::NEUTRAL);
                if deadline_passed {
                    ui_kit::chip(top, "DEADLINE PASSED", ui_kit::BAD);
                }
            });
            card.spawn(Node {
                min_height: Val::Px(34.0),
                ..default()
            })
            .with_children(|t| ui_kit::label(t, active.title.clone(), 15.0, ui_kit::TEXT));
            ui_kit::label(card, active.client.clone(), 10.0, ui_kit::TEXT_DIM);
            if let Some(score) = score {
                ui_kit::label(
                    card,
                    format!(
                        "Dossier score {:.0}% | target {}%",
                        score * 100.0,
                        active.requirements.min_trial_score
                    ),
                    10.0,
                    if ready { ui_kit::GOOD } else { ui_kit::TEXT_DIM },
                );
            } else {
                ui_kit::label(
                    card,
                    "No trial yet. Fly the vehicle and program in TRIALS to record evidence.",
                    10.0,
                    ui_kit::TEXT_DIM,
                );
            }
            card.spawn(Node {
                column_gap: Val::Px(6.0),
                margin: UiRect {
                    top: Val::Px(6.0),
                    ..default()
                },
                ..default()
            })
            .with_children(|buttons| {
                ui_kit::button_row_item(
                    buttons,
                    "GO TO TRIALS",
                    UiAction::Navigate(CampusView::Trials),
                    ButtonTone::Secondary,
                );
                ui_kit::button_row_item(
                    buttons,
                    "DELIVER",
                    UiAction::DeliverMissile,
                    if ready && !deadline_passed {
                        ButtonTone::Primary
                    } else {
                        ButtonTone::Disabled
                    },
                );
                ui_kit::button_row_item(
                    buttons,
                    "CIVIL (LEGACY)",
                    UiAction::ToggleCivilContracts,
                    ButtonTone::Ghost,
                );
            });
        });
}

fn missile_dossier_panel(
    panel: &mut ChildSpawnerCommands<'_>,
    procurement: &Procurement,
    vehicle: &omega_rules::Vehicle,
    program: &omega_rules::Program,
) {
    let Some(active) = procurement.active() else {
        ui_kit::label(
            panel,
            "No active contract. Accept an offer to open a dossier.",
            10.0,
            ui_kit::TEXT_DIM,
        );
        return;
    };
    ui_kit::label(panel, active.offer.title.clone(), 11.5, ui_kit::TEXT);
    ui_kit::label(
        panel,
        format!("Accepted day {} | due day {}", active.accepted_day, active.due_day()),
        9.5,
        ui_kit::TEXT_DIM,
    );
    let Some(dossier) = procurement.dossier(vehicle, program) else {
        return;
    };
    let requirements = active.offer.missile_requirements();
    for (requirement, row) in requirements.iter().zip(&dossier.matrix) {
        let (badge, color) = requirement_badge(&row.status);
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|line| {
                line.spawn(Node {
                    width: Val::Px(52.0),
                    ..default()
                })
                .with_children(|cell| ui_kit::chip(cell, badge, color));
                line.spawn(Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    ..default()
                })
                .with_children(|text| ui_kit::label(text, requirement_text(requirement), 10.0, ui_kit::TEXT));
            });
    }
    let ready = active.offer.accepts_dossier(&dossier);
    panel
        .spawn(Node {
            margin: UiRect {
                top: Val::Px(4.0),
                ..default()
            },
            ..default()
        })
        .with_children(|row| {
            ui_kit::chip(
                row,
                format!("score {:.0}%", dossier.score * 100.0),
                if ready { ui_kit::GOOD } else { ui_kit::WARN },
            );
            ui_kit::chip(
                row,
                if ready { "DOSSIER READY" } else { "DOSSIER INCOMPLETE" },
                if ready { ui_kit::GOOD } else { ui_kit::WARN },
            );
        });
    if let omega_rules::DossierDecision::Rejected { reasons } = &dossier.decision {
        for reason in reasons.iter().take(3) {
            ui_kit::label(panel, format!("- {reason}"), 9.0, ui_kit::BAD);
        }
    }
    let (vehicle_hash, program_hash) = procurement::hashes(vehicle, program);
    ui_kit::label(
        panel,
        format!("vehicle {} | program {}", vehicle_hash.as_str(), program_hash.as_str()),
        8.5,
        ui_kit::TEXT_DIM,
    );
    if let Some(trial) = active.trials.last() {
        ui_kit::label(
            panel,
            format!(
                "latest trial {} seed {} | {} runs on file",
                trial.vehicle.as_str(),
                trial.seed,
                active.trials.len()
            ),
            8.5,
            ui_kit::TEXT_DIM,
        );
    }
}

fn build_missile_contracts(
    root: &mut ChildSpawnerCommands<'_>,
    campus: &CampusState,
    procurement: &Procurement,
    vehicle: &omega_rules::Vehicle,
    program: &omega_rules::Program,
) {
    let status = procurement.evidence_status(vehicle, program);
    let dossier = procurement.dossier(vehicle, program);
    let ready = dossier.as_ref().is_some_and(|dossier| {
        procurement
            .active()
            .is_some_and(|active| active.offer.accepts_dossier(dossier))
    });
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        flex_grow: 2.0,
        flex_basis: Val::Px(0.0),
        row_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|left| {
        left.spawn(Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|head| {
            head.spawn(Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|title| {
                ui_kit::label(title, "MISSILE PROCUREMENT", 22.0, ui_kit::ACCENT);
                ui_kit::label(
                    title,
                    "Accept a family contract, fly its trial, then deliver the hash-bound dossier. One active contract at a time.",
                    10.5,
                    ui_kit::TEXT_DIM,
                );
            });
            ui_kit::chip(head, format!("{} cr", ui_kit::thousands(campus.credits)), ui_kit::GOOD);
            ui_kit::chip(
                head,
                format!("{} delivered", procurement.completed().len()),
                ui_kit::NEUTRAL,
            );
            ui_kit::button(
                head,
                "CIVIL (LEGACY)",
                UiAction::ToggleCivilContracts,
                ButtonTone::Ghost,
                Val::Px(130.0),
            );
        });
        heading(left, "OFFERS");
        left.spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(10.0),
            row_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|offers| {
            if procurement.offers().is_empty() {
                ui_kit::label(offers, "No offers on the board.", 12.0, ui_kit::TEXT_DIM);
            }
            for (index, offer) in procurement.offers().iter().enumerate() {
                missile_offer_card(offers, index, offer, procurement.active().is_none());
            }
        });
        heading(left, "ACTIVE CONTRACT");
        match procurement.active() {
            None => ui_kit::label(
                left,
                "None. Accept an offer above; the trials you fly afterwards become its evidence.",
                11.0,
                ui_kit::TEXT_DIM,
            ),
            Some(active) => missile_active_card(
                left,
                &active.offer,
                status,
                campus.day > active.due_day(),
                dossier.as_ref().map(|dossier| dossier.score),
                ready,
            ),
        }
    });
    root.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            row_gap: Val::Px(7.0),
            padding: UiRect::all(Val::Px(14.0)),
            border_radius: BorderRadius::all(Val::Px(7.0)),
            ..default()
        },
        BackgroundColor(ui_kit::PANEL),
        UiInteraction::None,
    ))
    .with_children(|panel| {
        ui_kit::label(panel, "DOSSIER", 16.0, ui_kit::ACCENT);
        missile_dossier_panel(panel, procurement, vehicle, program);
        heading(panel, "DELIVERED");
        if procurement.completed().is_empty() {
            ui_kit::label(panel, "None yet.", 10.0, ui_kit::TEXT_DIM);
        } else {
            for contract in procurement.completed().iter().rev().take(4) {
                ui_kit::label(
                    panel,
                    format!(
                        "#{} {} | +{} cr | {}%",
                        contract.id,
                        contract.title,
                        ui_kit::thousands(contract.reward),
                        contract.score_percent
                    ),
                    9.0,
                    ui_kit::TEXT_DIM,
                );
            }
        }
    });
}

fn build_contracts(
    root: &mut ChildSpawnerCommands<'_>,
    campus: &CampusState,
    procurement: &Procurement,
    vehicle: &omega_rules::Vehicle,
    program: &omega_rules::Program,
    design: &ContentHash,
) {
    if campus.civil_legacy_open {
        build_civil_contracts(root, campus, design);
    } else {
        build_missile_contracts(root, campus, procurement, vehicle, program);
    }
}

fn build_civil_contracts(root: &mut ChildSpawnerCommands<'_>, campus: &CampusState, design: &ContentHash) {
    let can_accept = campus.accepted_contracts.len() < MAX_ACTIVE_CONTRACTS;
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        flex_grow: 2.0,
        flex_basis: Val::Px(0.0),
        row_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|left| {
        left.spawn(Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|head| {
            head.spawn(Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|title| {
                ui_kit::label(title, "CONTRACT OFFICE", 22.0, ui_kit::ACCENT);
                ui_kit::label(
                    title,
                    "Accept up to two projects. Payment follows design, analysis, field test and an accepted dossier.",
                    10.5,
                    ui_kit::TEXT_DIM,
                );
            });
            ui_kit::chip(head, format!("{} cr", ui_kit::thousands(campus.credits)), ui_kit::GOOD);
            ui_kit::button(
                head,
                "REFRESH BOARD",
                UiAction::RefreshContracts,
                ButtonTone::Secondary,
                Val::Px(140.0),
            );
            ui_kit::button(
                head,
                "MISSILE CONTRACTS",
                UiAction::ToggleCivilContracts,
                ButtonTone::Secondary,
                Val::Px(150.0),
            );
        });
        heading(left, "OFFERS");
        left.spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(10.0),
            row_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|offers| {
            if campus.available_contracts.is_empty() {
                ui_kit::label(
                    offers,
                    "No offers remain. Refresh the board for new ones.",
                    12.0,
                    ui_kit::TEXT_DIM,
                );
            }
            for (index, offer) in campus.available_contracts.iter().enumerate() {
                offer_card(offers, index, offer, can_accept);
            }
        });
        heading(
            left,
            format!(
                "ACTIVE PROJECTS  {}/{}",
                campus.accepted_contracts.len(),
                MAX_ACTIVE_CONTRACTS
            ),
        );
        left.spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(10.0),
            row_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|active| {
            if campus.accepted_contracts.is_empty() {
                ui_kit::label(
                    active,
                    "None yet. Accept an offer above to start a project.",
                    12.0,
                    ui_kit::TEXT_DIM,
                );
            }
            for contract in &campus.accepted_contracts {
                active_card(active, contract, Some(contract.offer.id) == campus.selected_contract_id);
            }
        });
    });

    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        flex_basis: Val::Px(0.0),
        row_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|right| {
        right
            .spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(7.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    border_radius: BorderRadius::all(Val::Px(7.0)),
                    ..default()
                },
                BackgroundColor(ui_kit::PANEL),
                UiInteraction::None,
            ))
            .with_children(|panel| {
                ui_kit::label(panel, "DOSSIER", 16.0, ui_kit::ACCENT);
                match campus.selected_contract() {
                    Some(active) => {
                        ui_kit::label(panel, active.offer.title.clone(), 12.0, ui_kit::TEXT);
                        if active.snapshot.is_none() {
                            ui_kit::label(
                                panel,
                                "No design submitted yet. Submit one from the workshop to start gathering evidence.",
                                10.0,
                                ui_kit::TEXT_DIM,
                            );
                        }
                        dossier_rows(panel, active, design, false);
                    }
                    None => ui_kit::label(
                        panel,
                        "Select an active project to see its verification matrix.",
                        11.0,
                        ui_kit::TEXT_DIM,
                    ),
                }
            });
        if !campus.completed_contracts.is_empty() {
            right
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(5.0),
                        padding: UiRect::all(Val::Px(14.0)),
                        border_radius: BorderRadius::all(Val::Px(7.0)),
                        ..default()
                    },
                    BackgroundColor(ui_kit::PANEL),
                    UiInteraction::None,
                ))
                .with_children(|panel| {
                    ui_kit::label(panel, "COMPLETED", 12.0, ui_kit::TEXT_DIM);
                    for completed in campus.completed_contracts.iter().rev().take(4) {
                        let dossier = completed
                            .dossier_score_percent
                            .map_or(String::new(), |s| format!("  | dossier {s}%"));
                        ui_kit::label(
                            panel,
                            format!(
                                "#{:04} {}  | net {} cr{}",
                                completed.id, completed.title, completed.profit_cr, dossier
                            ),
                            10.5,
                            ui_kit::TEXT,
                        );
                    }
                });
        }
    });
}

// ----------------------------------------------------------------------------------------------
// Research
// ----------------------------------------------------------------------------------------------

fn department_color(dept: omega_rules::Department) -> Color {
    use omega_rules::Department::*;
    match dept {
        VehicleFamilies => Color::srgb(0.36, 0.84, 0.94),
        Materials => Color::srgb(0.92, 0.62, 0.30),
        Sensors => Color::srgb(0.35, 0.75, 0.55),
        Computing => Color::srgb(0.62, 0.50, 0.90),
        DesignTools => Color::srgb(0.36, 0.62, 0.95),
    }
}

fn build_research(root: &mut ChildSpawnerCommands<'_>, campus: &CampusState) {
    use omega_rules::{Department, ResearchError, default_tree};
    let tree = default_tree();
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        row_gap: Val::Px(12.0),
        ..default()
    })
    .with_children(|col| {
        col.spawn(Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|head| {
            head.spawn(Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|title| {
                ui_kit::label(title, "RESEARCH CENTRE", 22.0, ui_kit::ACCENT);
                let status = match &campus.research.active {
                    Some(active) => {
                        let label = tree
                            .iter()
                            .find(|t| t.id == active.id)
                            .map_or(active.id.as_str(), |t| t.label.as_str());
                        format!(
                            "Researching: {label} - {} day(s) left. One project at a time; press NEXT DAY to advance.",
                            active.days_left
                        )
                    }
                    None => "The lab is free. Start a technology, then press NEXT DAY to let time pass.".to_string(),
                };
                ui_kit::label(title, status, 10.5, ui_kit::TEXT_DIM);
            });
            ui_kit::chip(head, format!("DAY {}", campus.day), ui_kit::NEUTRAL);
            ui_kit::chip(head, format!("{} cr", ui_kit::thousands(campus.credits)), ui_kit::GOOD);
            ui_kit::button(head, "NEXT DAY", UiAction::NextDay, ButtonTone::Primary, Val::Px(130.0));
        });
        col.spawn(Node {
            width: Val::Percent(100.0),
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|grid| {
            for dept in Department::ALL {
                let color = department_color(dept);
                grid.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    row_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|column| {
                    column
                        .spawn(Node {
                            padding: UiRect::bottom(Val::Px(2.0)),
                            ..default()
                        })
                        .with_children(|h| ui_kit::label(h, dept.label(), 11.0, color));
                    for (index, tech) in tree.iter().enumerate().filter(|(_, t)| t.department == dept) {
                        let (mut node, bg, border) = ui_kit::card_node(Val::Percent(100.0), color);
                        node.row_gap = Val::Px(5.0);
                        column
                            .spawn((node, bg, border, UiInteraction::None))
                            .with_children(|card| {
                                ui_kit::label(card, tech.label.clone(), 13.0, ui_kit::TEXT);
                                ui_kit::label(card, tech.description.clone(), 10.0, ui_kit::TEXT_DIM);
                                ui_kit::label(
                                    card,
                                    format!("{} cr | {} days", ui_kit::thousands(tech.cost_cr), tech.duration_days),
                                    10.5,
                                    Color::srgb(0.78, 0.84, 0.88),
                                );
                                if !tech.prereqs.is_empty() {
                                    let names: Vec<&str> = tech
                                        .prereqs
                                        .iter()
                                        .filter_map(|p| tree.iter().find(|t| &t.id == p).map(|t| t.label.as_str()))
                                        .collect();
                                    ui_kit::label(
                                        card,
                                        format!("Requires: {}", names.join(", ")),
                                        9.5,
                                        ui_kit::TEXT_DIM,
                                    );
                                }
                                if campus.research.is_completed(&tech.id) {
                                    card.spawn(Node::default())
                                        .with_children(|c| ui_kit::chip(c, "RESEARCHED", ui_kit::GOOD));
                                } else if campus.research.active.as_ref().is_some_and(|a| a.id == tech.id) {
                                    let left = campus.research.active.as_ref().map_or(0, |a| a.days_left);
                                    card.spawn(Node::default()).with_children(|c| {
                                        ui_kit::chip(c, format!("{left} DAY(S) LEFT"), ui_kit::PURPLE)
                                    });
                                } else {
                                    match campus.research.can_start(&tree, &tech.id, campus.credits) {
                                        Ok(()) => ui_kit::button(
                                            card,
                                            "START RESEARCH",
                                            UiAction::StartResearch(index),
                                            ButtonTone::Primary,
                                            Val::Percent(100.0),
                                        ),
                                        Err(ResearchError::MissingPrerequisite(_)) => {
                                            card.spawn(Node::default())
                                                .with_children(|c| ui_kit::chip(c, "LOCKED", ui_kit::NEUTRAL));
                                        }
                                        Err(ResearchError::InsufficientCredits { needed, .. }) => {
                                            card.spawn(Node::default()).with_children(|c| {
                                                ui_kit::chip(
                                                    c,
                                                    format!("NEED {} cr", ui_kit::thousands(needed)),
                                                    ui_kit::WARN,
                                                )
                                            });
                                        }
                                        Err(_) => {
                                            card.spawn(Node::default())
                                                .with_children(|c| ui_kit::chip(c, "LAB BUSY", ui_kit::NEUTRAL));
                                        }
                                    }
                                }
                            });
                    }
                });
            }
        });
    });
}

// ----------------------------------------------------------------------------------------------
// Missile workshop
// ----------------------------------------------------------------------------------------------

fn spec_summary(spec: &omega_rules::PartSpec) -> String {
    use omega_rules::PartSpec::*;
    match *spec {
        SolidMotor {
            length_m,
            diameter_m,
            burn_time_s,
        } => format!("Solid motor {length_m:.1} m x {diameter_m:.2} m, {burn_time_s:.0} s"),
        FuelTank { length_m, diameter_m } => format!("Fuel tank {length_m:.1} m x {diameter_m:.2} m"),
        LiquidEngine { scale } => format!("Liquid engine x{scale:.1}"),
        AirBreathing { scale } => format!("Turbofan x{scale:.1}"),
        Sensor { kind } => format!("{kind:?}").replace("Radar(", "Radar ").replace(')', ""),
        Computer { class } => format!("Computer {class:?}"),
        Decoupler => "Decoupler".to_string(),
        MissionModule { mass_kg } => format!("Mission module {mass_kg:.0} kg"),
    }
}

fn param_row(parent: &mut ChildSpawnerCommands<'_>, param: missile::Param, value: f64) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                ..default()
            })
            .with_children(|l| ui_kit::label(l, param.label(), 10.5, ui_kit::TEXT_DIM));
            ui_kit::button(
                row,
                "-",
                UiAction::Missile(MissileAction::Adjust(param, -1)),
                ButtonTone::Secondary,
                Val::Px(34.0),
            );
            row.spawn(Node {
                width: Val::Px(56.0),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|v| ui_kit::label(v, format!("{value:.2}"), 12.0, ui_kit::TEXT));
            ui_kit::button(
                row,
                "+",
                UiAction::Missile(MissileAction::Adjust(param, 1)),
                ButtonTone::Secondary,
                Val::Px(34.0),
            );
        });
}

fn panel_node(width: Val) -> (Node, BackgroundColor, UiInteraction) {
    (
        Node {
            width,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(12.0)),
            border_radius: BorderRadius::all(Val::Px(7.0)),
            ..default()
        },
        BackgroundColor(ui_kit::PANEL),
        UiInteraction::None,
    )
}

fn build_missile(
    root: &mut ChildSpawnerCommands<'_>,
    campus: &CampusState,
    session: &MissileSession,
    log: &RevisionLog,
) {
    let owned = campus.owned_capabilities();
    let vehicle = &session.vehicle;

    // Left: palette
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        width: Val::Px(252.0),
        row_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|left| {
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        left.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "MISSILE WORKSHOP", 15.0, ui_kit::ACCENT);
            ui_kit::label(panel, vehicle.name.clone(), 10.5, ui_kit::TEXT_DIM);
            heading(panel, "START FROM");
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    ui_kit::button_row_item(
                        row,
                        "ROCKET",
                        UiAction::Missile(MissileAction::Preset(0)),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(
                        row,
                        "CRUISE",
                        UiAction::Missile(MissileAction::Preset(1)),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(row, "EMPTY", UiAction::Missile(MissileAction::Clear), ButtonTone::Ghost);
                });
            heading(panel, format!("ADD TO STAGE {}", session.active_stage + 1));
            let rows: [&[PartTemplate]; 6] = [
                &[PartTemplate::SolidMotor, PartTemplate::FuelTank],
                &[PartTemplate::LiquidEngine, PartTemplate::AirBreathing],
                &[PartTemplate::Pitot, PartTemplate::Barometer],
                &[PartTemplate::RadarBasic, PartTemplate::RadarActive],
                &[PartTemplate::Decoupler, PartTemplate::MissionModule],
                &[
                    PartTemplate::ComputerBasic,
                    PartTemplate::ComputerStandard,
                    PartTemplate::ComputerAdvanced,
                ],
            ];
            for row in rows {
                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        column_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|r| {
                        for template in row {
                            let unlocked = owned.contains(template.spec().required_capability());
                            let tone = if unlocked {
                                ButtonTone::Secondary
                            } else {
                                ButtonTone::Disabled
                            };
                            ui_kit::button_row_item(
                                r,
                                template.label(),
                                UiAction::Missile(MissileAction::AddPart(*template)),
                                tone,
                            );
                        }
                    });
            }
            ui_kit::label(
                panel,
                "Greyed parts need research. The computer sits in the last stage.",
                9.5,
                ui_kit::TEXT_DIM,
            );
            heading(panel, "STAGES");
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    for index in 0..vehicle.stages.len() {
                        let tone = if index == session.active_stage {
                            ButtonTone::Primary
                        } else {
                            ButtonTone::Secondary
                        };
                        ui_kit::button_row_item(
                            row,
                            format!("{}", index + 1),
                            UiAction::Missile(MissileAction::ActiveStage(index)),
                            tone,
                        );
                    }
                });
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    ui_kit::button_row_item(
                        row,
                        "+ STAGE",
                        UiAction::Missile(MissileAction::NewStage),
                        ButtonTone::Secondary,
                    );
                    ui_kit::button_row_item(
                        row,
                        "- STAGE",
                        UiAction::Missile(MissileAction::RemoveStage),
                        ButtonTone::Danger,
                    );
                });
            heading(panel, "REVISIONS");
            ui_kit::label(panel, log.missile_hud_text(vehicle), 9.0, ui_kit::TEXT_DIM);
            ui_kit::button(
                panel,
                "RECORD REVISION",
                UiAction::RecordMissileRevision,
                ButtonTone::Secondary,
                Val::Percent(100.0),
            );
        });
    });

    // Middle: stage statistics at the bottom
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        flex_basis: Val::Px(0.0),
        justify_content: JustifyContent::FlexEnd,
        ..default()
    })
    .with_children(|middle| {
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        middle.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "STAGE FIGURES", 12.0, ui_kit::ACCENT);
            let reports = vehicle.stage_reports();
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|row| {
                    for report in &reports {
                        let (mut node, bg, border) = ui_kit::card_node(Val::Px(0.0), ui_kit::ACCENT);
                        node.flex_grow = 1.0;
                        node.flex_basis = Val::Px(0.0);
                        node.row_gap = Val::Px(3.0);
                        row.spawn((node, bg, border)).with_children(|card| {
                            card.spawn(Node {
                                justify_content: JustifyContent::SpaceBetween,
                                align_items: AlignItems::Center,
                                ..default()
                            })
                            .with_children(|top| {
                                ui_kit::label(top, format!("STAGE {}", report.index + 1), 11.0, ui_kit::TEXT);
                                let twr_color = if report.thrust_n <= 0.0 {
                                    ui_kit::NEUTRAL
                                } else if report.index == 0 && report.twr_at_ignition < 1.0 {
                                    ui_kit::BAD
                                } else if report.index == 0 && report.twr_at_ignition < 1.3 {
                                    ui_kit::WARN
                                } else {
                                    ui_kit::GOOD
                                };
                                ui_kit::chip(top, format!("TWR {:.2}", report.twr_at_ignition), twr_color);
                            });
                            ui_kit::label(
                                card,
                                format!(
                                    "Mass {:.0} kg (carries {:.0} kg)",
                                    report.stage_wet_mass_kg, report.ignition_mass_kg
                                ),
                                10.0,
                                ui_kit::TEXT_DIM,
                            );
                            ui_kit::label(
                                card,
                                format!("Thrust {:.1} kN", report.thrust_n / 1000.0),
                                10.0,
                                ui_kit::TEXT_DIM,
                            );
                            let burn = report
                                .burn_time_s
                                .map_or("tank-fed".to_string(), |b| format!("{b:.0} s"));
                            ui_kit::label(
                                card,
                                format!("Burn {burn} | dv {:.0} m/s", report.delta_v_ms),
                                10.0,
                                ui_kit::TEXT_DIM,
                            );
                        });
                    }
                });
            ui_kit::label(
                panel,
                format!("Total mass {:.0} kg", vehicle.total_wet_mass_kg()),
                11.0,
                ui_kit::TEXT,
            );
            match vehicle.computer() {
                Some(class) => ui_kit::label(
                    panel,
                    format!(
                        "Computer {:?}: {} thread(s), {} blocks of memory",
                        class,
                        class.threads(),
                        class.ram_blocks()
                    ),
                    10.5,
                    ui_kit::TEXT_DIM,
                ),
                None => ui_kit::label(
                    panel,
                    "No flight computer: add one to be able to program the vehicle.",
                    10.5,
                    ui_kit::WARN,
                ),
            }
            let blocks: Vec<&str> = vehicle.available_blocks().into_iter().collect();
            ui_kit::label(
                panel,
                format!("Programming blocks unlocked: {}", blocks.join(", ").replace('_', " ")),
                10.0,
                ui_kit::TEXT_DIM,
            );
            let missing = vehicle.missing_research(&owned);
            if !missing.is_empty() {
                ui_kit::label(
                    panel,
                    format!("Needs research: {}", missing.join(", ")),
                    10.0,
                    ui_kit::WARN,
                );
            }
            if let Err(issues) = vehicle.validate() {
                for issue in issues.iter().take(4) {
                    ui_kit::label(panel, format!("- {issue}"), 10.0, Color::srgb(0.90, 0.60, 0.55));
                }
            } else {
                panel
                    .spawn(Node::default())
                    .with_children(|c| ui_kit::chip(c, "DESIGN VALID", ui_kit::GOOD));
            }
        });
    });

    // Right: stage tree and selected part
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        width: Val::Px(350.0),
        row_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|right| {
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        right.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "PARTS BY STAGE", 12.0, ui_kit::ACCENT);
            for (s, stage) in vehicle.stages.iter().enumerate() {
                let marker = if s == session.active_stage {
                    "  (receives new parts)"
                } else {
                    ""
                };
                heading(panel, format!("STAGE {}{}", s + 1, marker));
                if stage.parts.is_empty() {
                    ui_kit::label(panel, "empty", 10.0, ui_kit::TEXT_DIM);
                }
                for (p, part) in stage.parts.iter().enumerate() {
                    let tone = if session.selected == Some((s, p)) {
                        ButtonTone::Primary
                    } else {
                        ButtonTone::Ghost
                    };
                    ui_kit::button(
                        panel,
                        spec_summary(&part.spec),
                        UiAction::Missile(MissileAction::Select { stage: s, part: p }),
                        tone,
                        Val::Percent(100.0),
                    );
                }
            }
        });
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        right.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "SELECTED PART", 12.0, ui_kit::ACCENT);
            match session.selected_part() {
                Some(part) => {
                    ui_kit::label(
                        panel,
                        format!("{}  -  {}", part.id, spec_summary(&part.spec)),
                        11.0,
                        ui_kit::TEXT,
                    );
                    let params = missile::params_of(&part.spec);
                    if params.is_empty() {
                        ui_kit::label(panel, "This part has no adjustable setting.", 10.0, ui_kit::TEXT_DIM);
                    }
                    for (param, value) in params {
                        param_row(panel, param, value);
                    }
                    let stats = part.spec.stats();
                    let mut line = format!("Dry {:.1} kg", stats.dry_mass_kg);
                    if stats.propellant_kg > 0.0 {
                        line.push_str(&format!(" | propellant {:.1} kg", stats.propellant_kg));
                    }
                    if stats.thrust_n > 0.0 {
                        line.push_str(&format!(" | thrust {:.1} kN", stats.thrust_n / 1000.0));
                    }
                    ui_kit::label(panel, line, 10.0, ui_kit::TEXT_DIM);
                    ui_kit::button(
                        panel,
                        "REMOVE PART",
                        UiAction::Missile(MissileAction::RemovePart),
                        ButtonTone::Danger,
                        Val::Percent(100.0),
                    );
                }
                None => ui_kit::label(
                    panel,
                    "Click a part in the list above to resize or remove it.",
                    10.5,
                    ui_kit::TEXT_DIM,
                ),
            }
        });
    });
}

// ----------------------------------------------------------------------------------------------
// Workshop project card
// ----------------------------------------------------------------------------------------------

fn build_project(root: &mut ChildSpawnerCommands<'_>, campus: &CampusState, design: &ContentHash) {
    root.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect {
                top: Val::Px(3.0),
                ..default()
            },
            border_radius: BorderRadius::all(Val::Px(7.0)),
            ..default()
        },
        BackgroundColor(ui_kit::PANEL),
        BorderColor::all(ui_kit::ACCENT),
        UiInteraction::None,
    ))
    .with_children(|card| {
        match campus.selected_contract() {
            Some(active) => {
                let (status, color) = phase_chip(active.phase);
                card.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|top| {
                    ui_kit::label(top, "CURRENT PROJECT", 11.0, ui_kit::TEXT_DIM);
                    ui_kit::chip(top, status, color);
                });
                ui_kit::label(
                    card,
                    format!("#{:04}  {}", active.offer.id, active.offer.title),
                    13.0,
                    ui_kit::TEXT,
                );
                if active.snapshot.is_none() {
                    ui_kit::label(
                        card,
                        "Submit your design to start collecting evidence.",
                        10.0,
                        ui_kit::TEXT_DIM,
                    );
                }
                dossier_rows(card, active, design, true);
                for issue in active.last_issues.iter().take(3) {
                    ui_kit::label(card, format!("- {issue}"), 9.5, Color::srgb(0.90, 0.60, 0.55));
                }
            }
            None => {
                ui_kit::label(card, "NO ACTIVE PROJECT", 12.0, ui_kit::TEXT_DIM);
                ui_kit::label(
                    card,
                    "Accept a contract in the Contract Office to give your design a purpose.",
                    10.5,
                    ui_kit::TEXT_DIM,
                );
            }
        }
        let profile = &campus.instrument_profile;
        ui_kit::label(
            card,
            format!(
                "Profile: {} W | {} GB | uptime {}% | data {}% | {}..{} C",
                profile.power_draw_w,
                profile.storage_gb,
                profile.uptime_percent,
                profile.data_completeness_percent,
                profile.operating_min_c,
                profile.operating_max_c
            ),
            9.5,
            ui_kit::TEXT_DIM,
        );
        card.spawn(Node {
            column_gap: Val::Px(6.0),
            margin: UiRect {
                top: Val::Px(2.0),
                ..default()
            },
            ..default()
        })
        .with_children(|buttons| {
            if campus.selected_contract().is_some() {
                ui_kit::button_row_item(buttons, "SUBMIT DESIGN", UiAction::SubmitDesign, ButtonTone::Primary);
                ui_kit::button_row_item(
                    buttons,
                    "COMPUTE",
                    UiAction::Navigate(CampusView::Computation),
                    ButtonTone::Secondary,
                );
            }
            ui_kit::button_row_item(
                buttons,
                "CONTRACT OFFICE",
                UiAction::Navigate(CampusView::Contracts),
                ButtonTone::Secondary,
            );
        });
    });
}

// ----------------------------------------------------------------------------------------------
// Systems
// ----------------------------------------------------------------------------------------------

// ----------------------------------------------------------------------------------------------
// Missile program editor
// ----------------------------------------------------------------------------------------------

fn build_program(root: &mut ChildSpawnerCommands<'_>, vehicle: &omega_rules::Vehicle, session: &MissileProgramSession) {
    let issues = session.issues(vehicle);

    // Left: block palette, gated by the hardware fitted to the vehicle.
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        width: Val::Px(252.0),
        row_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|left| {
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        left.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "MISSILE PROGRAM", 15.0, ui_kit::ACCENT);
            ui_kit::label(panel, vehicle.name.clone(), 10.5, ui_kit::TEXT_DIM);
            heading(panel, "ADD BLOCK");
            let rows: [&[BlockKind]; 4] = [
                &BlockKind::ALL[0..2],
                &BlockKind::ALL[2..4],
                &BlockKind::ALL[4..6],
                &BlockKind::ALL[6..7],
            ];
            for row in rows {
                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        column_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|r| {
                        for kind in row {
                            let tone = if kind.available(vehicle) {
                                ButtonTone::Secondary
                            } else {
                                ButtonTone::Disabled
                            };
                            ui_kit::button_row_item(
                                r,
                                kind.label(),
                                UiAction::Program(ProgramAction::Append(*kind)),
                                tone,
                            );
                        }
                    });
            }
            ui_kit::label(
                panel,
                "Greyed blocks need hardware this vehicle does not carry.",
                9.5,
                ui_kit::TEXT_DIM,
            );
            heading(panel, "PROGRAM");
            ui_kit::button(
                panel,
                "CLEAR PROGRAM",
                UiAction::Program(ProgramAction::Clear),
                ButtonTone::Danger,
                Val::Percent(100.0),
            );
        });
    });

    // Middle: threads and the selected block.
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        flex_basis: Val::Px(0.0),
        row_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|middle| {
        let (mut node, bg, interaction) = panel_node(Val::Percent(100.0));
        // Cap the list panel so the selected-block controls stay on screen even with a 32-block
        // program; the list scrolls instead of pushing them below the window.
        node.max_height = Val::Percent(70.0);
        node.overflow = Overflow::clip_y();
        middle.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "THREADS", 12.0, ui_kit::ACCENT);
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    for (index, blocks) in session.program.threads.iter().enumerate() {
                        let tone = if index == session.thread {
                            ButtonTone::Primary
                        } else {
                            ButtonTone::Secondary
                        };
                        ui_kit::button_row_item(
                            row,
                            format!("T{} ({})", index + 1, blocks.len()),
                            UiAction::Program(ProgramAction::SelectThread(index)),
                            tone,
                        );
                    }
                    ui_kit::button_row_item(
                        row,
                        "+ THREAD",
                        UiAction::Program(ProgramAction::AddThread),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(
                        row,
                        "- THREAD",
                        UiAction::Program(ProgramAction::RemoveThread),
                        ButtonTone::Danger,
                    );
                });
            heading(panel, format!("THREAD {} BLOCKS", session.thread + 1));
            let blocks = session
                .program
                .threads
                .get(session.thread)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            panel
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        flex_grow: 1.0,
                        min_height: Val::Px(0.0),
                        row_gap: Val::Px(4.0),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    ScrollPosition(Vec2::new(0.0, session.scroll)),
                    ProgramListScroll,
                ))
                .with_children(|list| {
                    if blocks.is_empty() {
                        ui_kit::label(
                            list,
                            "Empty thread. Add a block from the palette.",
                            10.5,
                            ui_kit::TEXT_DIM,
                        );
                    }
                    for (index, block) in blocks.iter().enumerate() {
                        let tone = if session.block == Some(index) {
                            ButtonTone::Primary
                        } else {
                            ButtonTone::Ghost
                        };
                        ui_kit::button(
                            list,
                            format!("{}. {}", index + 1, block.label()),
                            UiAction::Program(ProgramAction::SelectBlock(index)),
                            tone,
                            Val::Percent(100.0),
                        );
                    }
                });
        });
        let (mut node, bg, interaction) = panel_node(Val::Percent(100.0));
        node.flex_shrink = 0.0;
        middle.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "SELECTED BLOCK", 12.0, ui_kit::ACCENT);
            match session.selected_block() {
                None => ui_kit::label(
                    panel,
                    "Select a block to tune, reorder or remove it.",
                    10.5,
                    ui_kit::TEXT_DIM,
                ),
                Some(block) => {
                    ui_kit::label(panel, block.label(), 13.0, ui_kit::TEXT);
                    if adjustable(block) {
                        panel
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                column_gap: Val::Px(4.0),
                                align_items: AlignItems::Center,
                                ..default()
                            })
                            .with_children(|row| {
                                ui_kit::button_row_item(
                                    row,
                                    "-",
                                    UiAction::Program(ProgramAction::Adjust(-1)),
                                    ButtonTone::Secondary,
                                );
                                ui_kit::button_row_item(
                                    row,
                                    "+",
                                    UiAction::Program(ProgramAction::Adjust(1)),
                                    ButtonTone::Secondary,
                                );
                            });
                    } else {
                        ui_kit::label(
                            panel,
                            "This block closes the thread and has no value to tune.",
                            10.0,
                            ui_kit::TEXT_DIM,
                        );
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
                                "MOVE UP",
                                UiAction::Program(ProgramAction::MoveUp),
                                ButtonTone::Secondary,
                            );
                            ui_kit::button_row_item(
                                row,
                                "MOVE DOWN",
                                UiAction::Program(ProgramAction::MoveDown),
                                ButtonTone::Secondary,
                            );
                            ui_kit::button_row_item(
                                row,
                                "REMOVE",
                                UiAction::Program(ProgramAction::RemoveSelected),
                                ButtonTone::Danger,
                            );
                        });
                }
            }
        });
    });

    // Right: computer limits and validation.
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        width: Val::Px(330.0),
        row_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|right| {
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        right.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "FLIGHT COMPUTER", 12.0, ui_kit::ACCENT);
            match (vehicle.computer(), session.limits(vehicle)) {
                (Some(class), Some((threads, blocks))) => {
                    ui_kit::label(panel, format!("{class:?} computer"), 11.0, ui_kit::TEXT);
                    let thread_tone = if session.threads_used() > threads {
                        ui_kit::BAD
                    } else {
                        ui_kit::GOOD
                    };
                    let block_tone = if session.blocks_used() > blocks {
                        ui_kit::BAD
                    } else {
                        ui_kit::GOOD
                    };
                    panel
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            column_gap: Val::Px(6.0),
                            ..default()
                        })
                        .with_children(|row| {
                            ui_kit::chip(
                                row,
                                format!("{} / {threads} threads", session.threads_used()),
                                thread_tone,
                            );
                            ui_kit::chip(row, format!("{} / {blocks} blocks", session.blocks_used()), block_tone);
                        });
                }
                _ => ui_kit::label(
                    panel,
                    "No flight computer: add one in the missile workshop to program this vehicle.",
                    10.5,
                    ui_kit::WARN,
                ),
            }
            heading(panel, "VALIDATION");
            if issues.is_empty() {
                panel
                    .spawn(Node::default())
                    .with_children(|c| ui_kit::chip(c, "PROGRAM VALID", ui_kit::GOOD));
            } else {
                for issue in issues.iter().take(6) {
                    ui_kit::label(panel, format!("- {issue}"), 10.0, ui_kit::BAD);
                }
                if issues.len() > 6 {
                    ui_kit::label(panel, format!("+{} more issue(s)", issues.len() - 6), 10.0, ui_kit::BAD);
                }
            }
            if let Ok(hash) = session.content_hash() {
                let short: String = hash.as_str().chars().take(16).collect();
                ui_kit::label(panel, format!("Program hash {short}"), 9.5, ui_kit::TEXT_DIM);
            }
            ui_kit::label(
                panel,
                "Saved to programs/missile.json on every change.",
                9.5,
                ui_kit::TEXT_DIM,
            );
        });
    });
}

fn rate(value: Option<f64>) -> String {
    value
        .map(|rate| format!("{:.0}%", rate * 100.0))
        .unwrap_or_else(|| "-".to_string())
}

/// One bar chart: samples collapsed to at most 96 bars, scaled to the series maximum.
fn plot(
    parent: &mut ChildSpawnerCommands<'_>,
    title: &str,
    unit: &str,
    color: Color,
    samples: &[omega_rules::flight::Sample],
    value: impl Fn(&omega_rules::flight::Sample) -> f64,
) {
    let max = samples
        .iter()
        .map(|sample| value(sample).max(0.0))
        .fold(0.0, f64::max)
        .max(1e-9);
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|row| {
            ui_kit::label(row, title, 10.5, ui_kit::TEXT);
            ui_kit::label(row, format!("max {max:.0} {unit}"), 9.5, ui_kit::TEXT_DIM);
        });
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Px(62.0),
            align_items: AlignItems::End,
            column_gap: Val::Px(1.0),
            ..default()
        })
        .with_children(|chart| {
            let step = (samples.len() / 96).max(1);
            for sample in samples.iter().step_by(step) {
                let ratio = (value(sample).max(0.0) / max).clamp(0.0, 1.0);
                chart.spawn((
                    Node {
                        flex_grow: 1.0,
                        height: Val::Percent((ratio * 100.0) as f32),
                        min_height: Val::Px(1.0),
                        ..default()
                    },
                    BackgroundColor(color),
                ));
            }
        });
}

fn build_trials(
    root: &mut ChildSpawnerCommands<'_>,
    campus: &CampusState,
    vehicle: &omega_rules::Vehicle,
    program: &omega_rules::Program,
    session: &TrialSession,
    procurement: &Procurement,
) {
    let monte_carlo = campus.owned_capabilities().contains(trials::MONTE_CARLO);
    let busy = session.batch_job.is_some();
    let current_hash = trials::input_hash(vehicle, program, &trials::config(session));
    let batch_stale = session.batch_stale(vehicle, program);

    // Left: seed, run, batch and comparison controls.
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        width: Val::Px(252.0),
        row_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|left| {
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        left.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "TRIAL SETUP", 15.0, ui_kit::ACCENT);
            ui_kit::label(panel, vehicle.name.clone(), 10.5, ui_kit::TEXT_DIM);
            heading(panel, "MISSION PROFILE");
            ui_kit::button(
                panel,
                if session.air_launch {
                    "AIR LAUNCH 200 m/s"
                } else {
                    "VERTICAL LAUNCH"
                },
                UiAction::Trial(TrialAction::ToggleLaunch),
                ButtonTone::Secondary,
                Val::Percent(100.0),
            );
            heading(panel, "SEED");
            ui_kit::label(panel, format!("Seed {}", session.seed), 13.0, ui_kit::TEXT);
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
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
            match trials::lock_probe(vehicle, program, session.seed) {
                Some(probe) => {
                    panel
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            column_gap: Val::Px(4.0),
                            ..default()
                        })
                        .with_children(|row| {
                            ui_kit::chip(row, format!("ROLL {:.2}", probe.roll), ui_kit::NEUTRAL);
                            ui_kit::chip(
                                row,
                                if probe.success { "LOCK" } else { "NO LOCK" },
                                if probe.success { ui_kit::GOOD } else { ui_kit::BAD },
                            );
                        });
                    ui_kit::label(
                        panel,
                        format!(
                            "Lock roll for this seed, under {:.0} m/s ({:.0}%).",
                            probe.max_speed_ms,
                            probe.probability * 100.0
                        ),
                        9.5,
                        ui_kit::TEXT_DIM,
                    );
                }
                None => ui_kit::label(
                    panel,
                    "The seed rolls the target lock; fit a radar and a LOCK TARGET block.",
                    9.5,
                    ui_kit::WARN,
                ),
            }
            let spread = omega_rules::flight::dispersion(session.seed);
            ui_kit::label(
                panel,
                format!(
                    "Seed spread  thrust {:+.1}%  burn {:+.1}%  drag {:+.1}%",
                    (spread.thrust - 1.0) * 100.0,
                    (spread.burn - 1.0) * 100.0,
                    (spread.drag - 1.0) * 100.0
                ),
                9.5,
                ui_kit::TEXT_DIM,
            );
            heading(panel, "CONTRACT");
            match procurement.active() {
                None => ui_kit::label(
                    panel,
                    "Accept a contract to bind trials as evidence.",
                    9.5,
                    ui_kit::TEXT_DIM,
                ),
                Some(active) => {
                    let status = procurement.evidence_status(vehicle, program);
                    panel
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            column_gap: Val::Px(4.0),
                            ..default()
                        })
                        .with_children(|row| {
                            ui_kit::chip(row, format!("#{:04}", active.offer.id), ui_kit::NEUTRAL);
                            let tone = match status {
                                EvidenceStatus::Current => ui_kit::GOOD,
                                EvidenceStatus::Stale => ui_kit::BAD,
                                _ => ui_kit::WARN,
                            };
                            ui_kit::chip(row, status.label(), tone);
                        });
                    ui_kit::label(panel, active.offer.title.clone(), 9.0, ui_kit::TEXT_DIM);
                }
            }
            ui_kit::button(
                panel,
                "RUN TRIAL",
                UiAction::Trial(TrialAction::Run),
                ButtonTone::Primary,
                Val::Percent(100.0),
            );
            heading(panel, "MONTE CARLO BATCH");
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    for runs in trials::BATCH_SIZES {
                        let tone = if monte_carlo && !busy {
                            ButtonTone::Secondary
                        } else {
                            ButtonTone::Disabled
                        };
                        ui_kit::button_row_item(
                            row,
                            ui_kit::thousands(runs as u64),
                            UiAction::Trial(TrialAction::Batch(runs)),
                            tone,
                        );
                    }
                });
            if let Some((done, total)) = session.batch_progress() {
                panel.spawn((
                    Text::new(format!("Running {done} / {total} seeds...")),
                    TextFont {
                        font_size: FontSize::Px(10.0),
                        ..default()
                    },
                    TextColor(ui_kit::ACCENT),
                    BatchProgressText,
                ));
            } else if !monte_carlo {
                ui_kit::label(panel, "Locked: Monte Carlo research needed.", 9.5, ui_kit::WARN);
            }
            if batch_stale && !busy {
                ui_kit::label(panel, "Batch stale: inputs changed.", 9.5, ui_kit::WARN);
            }
            heading(panel, "A / B");
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
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
                        "COMPARE",
                        UiAction::Trial(TrialAction::Compare),
                        ButtonTone::Secondary,
                    );
                });
            match &session.baseline {
                Some(baseline) => ui_kit::label(
                    panel,
                    format!("A: {} ({})", baseline.hash, baseline.vehicle.name),
                    9.5,
                    ui_kit::TEXT_DIM,
                ),
                None => ui_kit::label(panel, "A: empty. Snapshot before editing.", 9.5, ui_kit::TEXT_DIM),
            }
            heading(panel, "INPUT HASH");
            ui_kit::label(
                panel,
                current_hash.chars().take(16).collect::<String>(),
                11.0,
                ui_kit::TEXT,
            );
            if session
                .report
                .as_ref()
                .is_some_and(|run| run.input_hash != current_hash)
            {
                ui_kit::label(panel, "Changed since the last run.", 9.5, ui_kit::WARN);
            }
            ui_kit::label(
                panel,
                "Same vehicle, program and seed give the same report.",
                9.5,
                ui_kit::TEXT_DIM,
            );
        });
    });

    // Middle: the four plots and the headline figures.
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        flex_basis: Val::Px(0.0),
        row_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|middle| {
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        middle.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "TRAJECTORY", 12.0, ui_kit::ACCENT);
            match &session.report {
                None => ui_kit::label(
                    panel,
                    "No trial yet. Press RUN TRIAL to fly the current vehicle and program.",
                    10.5,
                    ui_kit::TEXT_DIM,
                ),
                Some(run) => {
                    let samples = &run.report.samples;
                    plot(panel, "SPEED", "m/s", ui_kit::ACCENT, samples, |s| s.speed_ms);
                    plot(panel, "ALTITUDE", "m", ui_kit::PURPLE, samples, |s| s.altitude_m);
                    plot(panel, "MASS", "kg", ui_kit::WARN, samples, |s| s.mass_kg);
                    plot(panel, "THRUST", "kN", Color::srgb(0.36, 0.62, 0.95), samples, |s| {
                        s.thrust_n / 1_000.0
                    });
                }
            }
        });
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        middle.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "METRICS", 12.0, ui_kit::ACCENT);
            match &session.report {
                None => ui_kit::label(panel, "Run a trial to see figures.", 10.5, ui_kit::TEXT_DIM),
                Some(run) => {
                    let report = &run.report;
                    panel
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            column_gap: Val::Px(6.0),
                            ..default()
                        })
                        .with_children(|row| {
                            ui_kit::chip(row, format!("{:.0} m/s max", report.max_speed_ms), ui_kit::GOOD);
                            ui_kit::chip(
                                row,
                                format!("{:.1} km", report.path_length_m / 1_000.0),
                                ui_kit::NEUTRAL,
                            );
                            ui_kit::chip(row, format!("{:.1} s", report.flight_time_s), ui_kit::NEUTRAL);
                        });
                    panel
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            column_gap: Val::Px(6.0),
                            ..default()
                        })
                        .with_children(|row| {
                            let twr = report.first_ignition_twr;
                            ui_kit::chip(
                                row,
                                format!("TWR {:.2}", twr.unwrap_or(0.0)),
                                if twr.is_some_and(|value| value >= 1.0) {
                                    ui_kit::GOOD
                                } else {
                                    ui_kit::BAD
                                },
                            );
                            match report.lock {
                                Some(lock) => ui_kit::chip(
                                    row,
                                    format!(
                                        "lock {:.0}% {}",
                                        lock.probability * 100.0,
                                        if lock.success { "acquired" } else { "failed" }
                                    ),
                                    if lock.success { ui_kit::GOOD } else { ui_kit::BAD },
                                ),
                                None => ui_kit::chip(row, "no lock attempted", ui_kit::NEUTRAL),
                            }
                            ui_kit::chip(
                                row,
                                if report.lifted_off { "lifted off" } else { "no lift-off" },
                                if report.lifted_off { ui_kit::GOOD } else { ui_kit::BAD },
                            );
                        });
                    ui_kit::label(
                        panel,
                        format!(
                            "Run seed {} ({}) | final {:.0} m/s | {} samples",
                            run.seed,
                            if run.air_launch { "air launch" } else { "vertical" },
                            report.final_speed_ms,
                            report.samples.len()
                        ),
                        9.5,
                        ui_kit::TEXT_DIM,
                    );
                }
            }
        });
    });

    // Right: timeline, anomalies and the batch/comparison results.
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        width: Val::Px(330.0),
        row_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|right| {
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        right.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "EVENTS", 12.0, ui_kit::ACCENT);
            match &session.report {
                None => ui_kit::label(panel, "Nothing flown yet.", 10.0, ui_kit::TEXT_DIM),
                Some(run) if run.report.events.is_empty() => {
                    ui_kit::label(panel, "No event was reported.", 10.0, ui_kit::TEXT_DIM)
                }
                Some(run) => {
                    for (t, label) in run.report.events.iter().take(9) {
                        ui_kit::label(panel, format!("{t:6.1}s  {label}"), 10.0, ui_kit::TEXT);
                    }
                    if run.report.events.len() > 9 {
                        ui_kit::label(
                            panel,
                            format!("+{} more events", run.report.events.len() - 9),
                            9.5,
                            ui_kit::TEXT_DIM,
                        );
                    }
                }
            }
            heading(panel, "ANOMALIES");
            match &session.report {
                None => ui_kit::label(panel, "-", 10.0, ui_kit::TEXT_DIM),
                Some(run) if run.report.anomalies.is_empty() => {
                    panel
                        .spawn(Node::default())
                        .with_children(|row| ui_kit::chip(row, "NO ANOMALY", ui_kit::GOOD));
                }
                Some(run) => {
                    for anomaly in run.report.anomalies.iter().take(5) {
                        ui_kit::label(panel, format!("- {anomaly}"), 9.5, ui_kit::BAD);
                    }
                    if run.report.anomalies.len() > 5 {
                        ui_kit::label(
                            panel,
                            format!("+{} more", run.report.anomalies.len() - 5),
                            9.5,
                            ui_kit::BAD,
                        );
                    }
                }
            }
        });
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        right.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "BATCH", 12.0, ui_kit::ACCENT);
            match &session.batch {
                None => ui_kit::label(
                    panel,
                    "Run a seeded batch to see the distribution.",
                    10.0,
                    ui_kit::TEXT_DIM,
                ),
                Some(batch) => {
                    panel
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            column_gap: Val::Px(6.0),
                            ..default()
                        })
                        .with_children(|row| {
                            ui_kit::chip(row, format!("{} runs", batch.runs), ui_kit::NEUTRAL);
                            ui_kit::chip(row, format!("{} no lift-off", batch.no_liftoff), ui_kit::BAD);
                            ui_kit::chip(row, format!("{} no separation", batch.no_separation), ui_kit::WARN);
                            if batch_stale {
                                ui_kit::chip(row, "STALE", ui_kit::BAD);
                            }
                        });
                    if batch_stale {
                        ui_kit::label(
                            panel,
                            "Vehicle, program, profile or seed changed since this batch.",
                            9.5,
                            ui_kit::BAD,
                        );
                    }
                    ui_kit::label(
                        panel,
                        format!(
                            "lock acquired {}/{} ({}) | {} runs with anomalies",
                            batch.lock_success,
                            batch.lock_attempts,
                            rate(batch.lock_rate()),
                            batch.anomalies
                        ),
                        9.5,
                        ui_kit::TEXT_DIM,
                    );
                    ui_kit::label(
                        panel,
                        format!(
                            "mean {:.0} m/s | {:.1} km | {:.1} s",
                            batch.mean_max_speed_ms,
                            batch.mean_path_m / 1_000.0,
                            batch.mean_flight_time_s
                        ),
                        9.5,
                        ui_kit::TEXT_DIM,
                    );
                }
            }
            heading(panel, "A VS B");
            match &session.comparison {
                None => ui_kit::label(panel, "Snapshot A, edit, then COMPARE.", 10.0, ui_kit::TEXT_DIM),
                Some(comparison) => {
                    let a = &comparison.baseline;
                    let b = &comparison.current;
                    ui_kit::label(
                        panel,
                        format!(
                            "A {}  vs  B {}",
                            comparison.baseline_hash,
                            comparison.current_hash.chars().take(8).collect::<String>()
                        ),
                        9.5,
                        ui_kit::TEXT_DIM,
                    );
                    ui_kit::label(
                        panel,
                        format!(
                            "path   {:.1} -> {:.1} km  ({:+.1})",
                            a.mean_path_m / 1_000.0,
                            b.mean_path_m / 1_000.0,
                            (b.mean_path_m - a.mean_path_m) / 1_000.0
                        ),
                        10.0,
                        ui_kit::TEXT,
                    );
                    ui_kit::label(
                        panel,
                        format!(
                            "speed  {:.0} -> {:.0} m/s  ({:+.0})",
                            a.mean_max_speed_ms,
                            b.mean_max_speed_ms,
                            b.mean_max_speed_ms - a.mean_max_speed_ms
                        ),
                        10.0,
                        ui_kit::TEXT,
                    );
                    ui_kit::label(
                        panel,
                        format!(
                            "lock   {} -> {}  ({} vs {})",
                            rate(a.lock_rate()),
                            rate(b.lock_rate()),
                            a.lock_success,
                            b.lock_success
                        ),
                        10.0,
                        ui_kit::TEXT,
                    );
                }
            }
        });
    });
}

/// Mouse-wheel scrolling for the missile program block list.
///
/// `bevy_ui` moves a node's children by `ScrollPosition` but nothing turns the wheel into an
/// offset, so the list would stay stuck on its first blocks once a program outgrows the panel. The
/// offset is mirrored in the session because the board is rebuilt after every edit, and the wheel
/// always targets the list: it is the only scrollable thing in this view, whose orbit camera is off.
pub(crate) fn scroll_program_list(
    wheel: Res<AccumulatedMouseScroll>,
    mut session: ResMut<MissileProgramSession>,
    mut lists: Query<&mut ScrollPosition, With<ProgramListScroll>>,
) {
    let Ok(mut position) = lists.single_mut() else {
        return;
    };
    if wheel.delta.y != 0.0 {
        let step = match wheel.unit {
            MouseScrollUnit::Line => 34.0,
            MouseScrollUnit::Pixel => 1.0,
        } * wheel.delta.y;
        position.y = (position.y - step).max(0.0);
    }
    // Rebuilding the board reads this back; the layout pass clamps it to the content.
    if session.scroll != position.y {
        session.scroll = position.y;
    }
}

fn visible(kind: BoardKind, campus: &CampusState, sketch_active: bool) -> bool {
    match kind {
        BoardKind::Island => campus.view == CampusView::Island,
        BoardKind::Contracts => campus.view == CampusView::Contracts,
        BoardKind::Research => campus.view == CampusView::Research,
        BoardKind::Missile => campus.view == CampusView::Missile,
        BoardKind::Program => campus.view == CampusView::Program,
        BoardKind::Trials => campus.view == CampusView::Trials,
        BoardKind::Project => campus.view == CampusView::Construction && !campus.profile_editor_open && !sketch_active,
    }
}

fn signature(
    kind: BoardKind,
    campus: &CampusState,
    design: &str,
    missile: &MissileSession,
    program: &MissileProgramSession,
    trials: &TrialSession,
    procurement: &Procurement,
    log: &RevisionLog,
) -> String {
    let contracts = || {
        let active: Vec<String> = campus
            .accepted_contracts
            .iter()
            .map(|a| {
                format!(
                    "{}:{:?}:{}:{}:{}",
                    a.offer.id,
                    a.phase,
                    a.evidence.len(),
                    a.last_issues.len(),
                    a.snapshot.is_some()
                )
            })
            .collect();
        let offers: Vec<u32> = campus.available_contracts.iter().map(|o| o.id).collect();
        format!(
            "{}|{:?}|{:?}|{}|{:?}|{}",
            campus.credits,
            campus.selected_contract_id,
            offers,
            campus.completed_contracts.len(),
            active,
            design
        )
    };
    match kind {
        BoardKind::Island => format!(
            "{}|{}|{}",
            campus.credits,
            campus.accepted_contracts.len(),
            campus.completed_contracts.len()
        ),
        BoardKind::Contracts => format!(
            "{}|{}|{}",
            contracts(),
            procurement.signature(&missile.vehicle, &program.program),
            campus.civil_legacy_open
        ),
        BoardKind::Research => format!("{:?}|{}|{}", campus.research, campus.credits, campus.day),
        BoardKind::Missile => format!(
            "{:?}|{:?}|{}|{:?}|{:?}",
            missile.vehicle,
            (missile.selected, missile.active_stage),
            campus.credits,
            campus.research,
            log.head().map(|revision| revision.hash.clone())
        ),
        BoardKind::Program => format!(
            "{:?}|{:?}|{}|{:?}",
            missile.vehicle, program.program, program.thread, program.block
        ),
        BoardKind::Trials => format!(
            "{}|{}|{}|{}|{}|{}|{}",
            trials.revision,
            trials.seed,
            trials.air_launch,
            trials.report.is_some(),
            trials.batch.is_some(),
            trials.comparison.is_some(),
            procurement.signature(&missile.vehicle, &program.program)
        ),
        BoardKind::Project => format!("{}|{:?}", contracts(), campus.instrument_profile),
    }
}

pub(crate) fn sync_boards(
    mut commands: Commands,
    campus: Res<CampusState>,
    design: Res<DesignSession>,
    sketch: Res<PolygonSketchSession>,
    missile: Res<MissileSession>,
    program: Res<MissileProgramSession>,
    trials: Res<TrialSession>,
    procurement: Res<Procurement>,
    log: Res<RevisionLog>,
    mut boards: Query<(Entity, &mut Node, &BoardKind)>,
    mut seen: Local<HashMap<BoardKind, String>>,
) {
    let design_hash = assembly_content_hash(&design.assembly).unwrap_or_else(|_| ContentHash::of_bytes(b""));
    for (entity, mut node, kind) in &mut boards {
        let show = visible(*kind, &campus, sketch.active);
        let wanted = if show { Display::Flex } else { Display::None };
        if node.display != wanted {
            node.display = wanted;
        }
        if !show {
            seen.remove(kind);
            continue;
        }
        let sig = signature(
            *kind,
            &campus,
            design_hash.as_str(),
            &missile,
            &program,
            &trials,
            &procurement,
            &log,
        );
        if seen.get(kind) == Some(&sig) {
            continue;
        }
        seen.insert(*kind, sig);
        commands.entity(entity).despawn_related::<Children>();
        let kind = *kind;
        let campus_ref: &CampusState = &campus;
        let missile_ref: &MissileSession = &missile;
        let program_ref: &MissileProgramSession = &program;
        let trials_ref: &TrialSession = &trials;
        let hash = design_hash.clone();
        commands.entity(entity).with_children(|root| match kind {
            BoardKind::Island => build_island(root, campus_ref),
            BoardKind::Contracts => build_contracts(
                root,
                campus_ref,
                &procurement,
                &missile_ref.vehicle,
                &program_ref.program,
                &hash,
            ),
            BoardKind::Research => build_research(root, campus_ref),
            BoardKind::Missile => build_missile(root, campus_ref, missile_ref, &log),
            BoardKind::Program => build_program(root, &missile_ref.vehicle, program_ref),
            BoardKind::Trials => build_trials(
                root,
                campus_ref,
                &missile_ref.vehicle,
                &program_ref.program,
                trials_ref,
                &procurement,
            ),
            BoardKind::Project => build_project(root, campus_ref, &hash),
        });
    }
}

/// Progress line of a running seed batch, written in place so the board is not rebuilt per seed.
#[derive(Component)]
pub(crate) struct BatchProgressText;

pub(crate) fn poll_trial_batch(mut trials: ResMut<TrialSession>, mut campus: ResMut<CampusState>) {
    if trials.batch_job.is_none() {
        return;
    }
    if let Some(outcome) = trials.take_batch_outcome() {
        match outcome {
            Ok(summary) => {
                trials.batch = Some(summary);
                campus.message = "Batch done".to_string();
            }
            Err(message) => campus.message = friendly_capabilities(&message),
        }
    }
}

pub(crate) fn update_batch_progress(trials: Res<TrialSession>, mut text: Query<&mut Text, With<BatchProgressText>>) {
    let Ok(mut text) = text.single_mut() else {
        return;
    };
    let wanted = match trials.batch_progress() {
        Some((done, total)) => format!("Running {done} / {total} seeds..."),
        None => String::new(),
    };
    if text.0 != wanted {
        text.0 = wanted;
    }
}

pub(crate) fn update_toast(
    time: Res<Time>,
    campus: Res<CampusState>,
    mut text: Query<&mut Text, With<ToastText>>,
    mut state: Local<(String, f32)>,
) {
    if state.0 != campus.message {
        state.0 = campus.message.clone();
        state.1 = if campus.message.is_empty() { 0.0 } else { 7.0 };
    }
    state.1 = (state.1 - time.delta_secs()).max(0.0);
    if let Ok(mut text) = text.single_mut() {
        let wanted = if state.1 > 0.0 {
            format!("> {}", state.0)
        } else {
            String::new()
        };
        if text.0 != wanted {
            text.0 = wanted;
        }
    }
}

pub(crate) fn update_nav_status(campus: Res<CampusState>, mut status: Query<&mut Text, With<NavStatus>>) {
    if let Ok(mut text) = status.single_mut() {
        let value = format!(
            "DAY {}   |   {} cr   |   {}/{} active   |   {} done",
            campus.day,
            ui_kit::thousands(campus.credits),
            campus.accepted_contracts.len(),
            MAX_ACTIVE_CONTRACTS,
            campus.completed_contracts.len()
        );
        if text.0 != value {
            text.0 = value;
        }
    }
}

/// Replace internal capability ids in a message with the name of the technology that grants them.
pub(crate) fn friendly_capabilities(message: &str) -> String {
    let mut text = message.to_string();
    for tech in omega_rules::default_tree() {
        for grant in &tech.grants {
            if let omega_rules::Grant::Capability(capability) = grant {
                text = text.replace(capability.as_str(), &format!("\"{}\" (Research Centre)", tech.label));
            }
        }
    }
    text
}

/// Actions that need resources `interface::handle_actions` does not have (revisions, help toggle).
pub(crate) fn handle_extra_actions(
    buttons: Query<(&UiInteraction, &UiActionButton), Changed<UiInteraction>>,
    keys: Res<ButtonInput<KeyCode>>,
    sketch: Res<PolygonSketchSession>,
    inspector: Res<InspectorState>,
    mut campus: ResMut<CampusState>,
    mut ui: ResMut<UiState>,
    mut session: ResMut<DesignSession>,
    mut log: ResMut<RevisionLog>,
    mut missile: ResMut<MissileSession>,
    mut program: ResMut<MissileProgramSession>,
    mut trials: ResMut<TrialSession>,
    mut procurement: ResMut<Procurement>,
) {
    for (interaction, button) in &buttons {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        match button.0 {
            UiAction::RecordRevision => campus.message = record_revision(&mut log, &session),
            UiAction::RecordMissileRevision => {
                campus.message = match log.commit_vehicle(&missile.vehicle) {
                    Ok(id) => match log
                        .to_bytes()
                        .and_then(|bytes| crate::save::write(crate::save::SaveFile::Revisions, &bytes))
                    {
                        Ok(()) => format!("Recorded missile revision {id}"),
                        Err(error) => format!("Revision {id} recorded but not saved: {error}"),
                    },
                    Err(error) => format!("Revision not recorded: {error}"),
                };
            }
            UiAction::AcceptMissileOffer(index) => accept_missile_offer(&mut campus, &mut procurement, index),
            UiAction::DeliverMissile => {
                deliver_missile_contract(&mut campus, &mut procurement, &missile.vehicle, &program.program)
            }
            UiAction::RestoreRevision => campus.message = restore_revision(&log, &mut session),
            UiAction::ToggleHelp => ui.help_open = !ui.help_open,
            UiAction::Program(action) => match program.apply(action, &missile.vehicle) {
                Ok(message) => {
                    if !message.is_empty() {
                        campus.message = message;
                    }
                    if let Err(error) = program_editor::persist(&program) {
                        warn!("Could not save the missile program: {error}");
                        campus.message = format!("Could not save the missile program: {error}");
                    }
                }
                Err(message) => campus.message = message,
            },
            UiAction::Trial(action) => {
                let monte_carlo = campus.owned_capabilities().contains(trials::MONTE_CARLO);
                let outcome =
                    trials::apply_action(&mut trials, action, &missile.vehicle, &program.program, monte_carlo);
                match outcome {
                    Ok(message) => {
                        if !message.is_empty() {
                            campus.message = message;
                        }
                    }
                    Err(message) => campus.message = friendly_capabilities(&message),
                }
            }
            UiAction::Missile(action) => {
                let owned = campus.owned_capabilities();
                let message = friendly_capabilities(&missile.apply(action, &owned));
                if !message.is_empty() {
                    campus.message = message;
                }
                if let Err(error) = missile::persist_missile(&missile) {
                    warn!("Could not save the vehicle: {error}");
                }
            }
            _ => {}
        }
    }
    let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if keys.just_pressed(KeyCode::KeyH)
        && !control
        && campus.view == CampusView::Construction
        && !campus.profile_editor_open
        && !sketch.active
        && !inspector.is_editing()
    {
        ui.help_open = !ui.help_open;
    }
}
