//! Card-based boards: the island facility cards, the missile Contract Office (offers, active
//! contracts, dossier), the Research Centre, the workshop, the program editor and the trial range.
//! Boards are rebuilt only when what they show changes.

use super::*;
use crate::inspector::{self, InspectorState};
use crate::interface::{UiAction, UiActionButton};
use crate::lab::TrialContext;
use crate::lab::{DesignMetrics, GoalStatus, budget_ok, goal_statuses};
use crate::missile::{self, MissileAction, MissileSession, PartTemplate};
use crate::procurement::{self, EvidenceStatus, Procurement};
use crate::program_editor::{self, BlockKind, Branch, MissileProgramSession, PathStep, ProgramAction, issue_counts};
use crate::revisions::RevisionLog;
use crate::scene::{CameraAction, CameraView};
use crate::subassembly::SubassemblyKind;
use crate::trial_export;
use crate::trials::{self, TrialAction, TrialSession};
use crate::ui_kit::{self, ButtonTone, NavStatus};
use bevy::ecs::system::SystemParam;
use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::ui::ScrollPosition;
use std::collections::HashMap;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum BoardKind {
    Island,
    Contracts,
    Research,
    Missile,
    Program,
    Trials,
}

#[derive(Component)]
pub(crate) struct ToastText;

/// Marks the scrolling container that holds the missile program block list.
#[derive(Component)]
pub(crate) struct ProgramListScroll;

/// Marks the scrolling container that holds the research departments.
#[derive(Component)]
pub(crate) struct ResearchGrid;

/// Marks the scrolling offer list. The dossier stays in its own column, so scrolling the eight
/// offers never hides the contract being worked on.
#[derive(Component)]
pub(crate) struct ContractOfferScroll;

/// Scroll offset of the offer list, kept across board rebuilds.
#[derive(Resource, Default)]
pub(crate) struct ContractScroll(pub(crate) f32);

/// Scroll offset of the research grid, kept across board rebuilds.
#[derive(Resource, Default)]
pub(crate) struct ResearchScroll(pub(crate) f32);

/// Marks the scrolling container that holds the workshop stage list.
#[derive(Component)]
pub(crate) struct PartListScroll;

/// Marks the scrolling selected-part inspector.
#[derive(Component)]
pub(crate) struct InspectorScrollPanel;

/// Scroll offset of the selected-part inspector, kept across board rebuilds.
#[derive(Resource, Default)]
pub(crate) struct InspectorScrollOffset(pub(crate) f32);

/// Marks the scrolling assembly-control panel.
#[derive(Component)]
pub(crate) struct AssemblyControlPanel;

/// Scroll offset of the assembly-control panel, kept across board rebuilds.
#[derive(Resource, Default)]
pub(crate) struct AssemblyControlScroll(pub(crate) f32);

/// Bundles the two inspector resources so `sync_boards` stays below Bevy's system-parameter limit.
#[derive(SystemParam)]
pub(crate) struct InspectorPanel<'w> {
    pub(crate) state: Res<'w, InspectorState>,
    pub(crate) scroll: Res<'w, InspectorScrollOffset>,
}

/// The board scroll offsets, grouped for the same reason.
#[derive(SystemParam)]
pub(crate) struct BoardScrolls<'w> {
    contract: Res<'w, ContractScroll>,
    research: Res<'w, ResearchScroll>,
    workshop: Res<'w, WorkshopScroll>,
    workshop_palette: Res<'w, WorkshopPaletteScroll>,
    program_palette: Res<'w, ProgramPaletteScroll>,
    assembly: Res<'w, AssemblyControlScroll>,
}

/// Scroll offset of the workshop stage list, kept across board rebuilds.
#[derive(Resource, Default)]
pub(crate) struct WorkshopScroll(pub(crate) f32);

/// Marks the scrolling container that holds the workshop palette.
#[derive(Component)]
pub(crate) struct WorkshopPaletteList;

/// Scroll offset of the workshop palette, kept across board rebuilds.
#[derive(Resource, Default)]
pub(crate) struct WorkshopPaletteScroll(pub(crate) f32);

/// Marks the scrolling container that holds the program block library.
#[derive(Component)]
pub(crate) struct ProgramPaletteList;

/// Scroll offset of the program palette, kept across board rebuilds.
#[derive(Resource, Default)]
pub(crate) struct ProgramPaletteScroll(pub(crate) f32);

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
    }
    node
}

pub(crate) fn setup_boards(mut commands: Commands) {
    for kind in [
        BoardKind::Island,
        BoardKind::Contracts,
        BoardKind::Research,
        BoardKind::Missile,
        BoardKind::Program,
        BoardKind::Trials,
    ] {
        commands.spawn((root_node(kind), kind));
    }
}

pub(crate) fn heading(parent: &mut ChildSpawnerCommands<'_>, text: impl Into<String>) {
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

fn build_island(root: &mut ChildSpawnerCommands<'_>, campus: &CampusState, procurement: &Procurement) {
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(|hero| {
        ui_kit::label(hero, "RESEARCH ISLAND", 30.0, ui_kit::ACCENT);
        ui_kit::label(
            hero,
            "A fictional missile programme: take a contract, design the vehicle, fly its trial, deliver the plan.",
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
                    "{}/{} active contracts",
                    procurement.active().len(),
                    campus.max_active_projects()
                ),
                ui_kit::WARN,
            );
            ui_kit::chip(
                stats,
                format!("{} delivered", procurement.completed().len()),
                ui_kit::NEUTRAL,
            );
        });
    });

    let facilities: [(&str, &str, &str, CampusView, Color); 5] = [
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
            "F5",
            "Fly the vehicle and its program: four plots, events, anomalies, seeded batches and A/B comparison.",
            CampusView::Trials,
            Color::srgb(0.90, 0.55, 0.35),
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
// Contract Office
// ----------------------------------------------------------------------------------------------

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
    let label = ui_kit::fr(requirement.label.clone());
    match requirement.metric.as_str() {
        "path_length_m" => format!("{label} ≥ {:.0} km", requirement.limit / 1_000.0),
        "max_speed_ms" => format!("{label} ≥ {:.0} m/s", requirement.limit),
        "launch_mass_kg" => format!("{label} ≤ {:.0} kg", requirement.limit),
        "payload_kg" => format!("{label} ≥ {:.0} kg", requirement.limit),
        "payload_yield_kg" => format!("{label} ≥ {:.0} kg", requirement.limit),
        "discretion" => format!("{label} ≥ {:.0}/100", requirement.limit),
        "max_altitude_m" => format!("{label} ≥ {:.1} km", requirement.limit / 1_000.0),
        _ => format!("{label} requis"),
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

/// What the office is still missing from research, written for a player rather than a log.
fn locked_family_hints(campus: &CampusState) -> Vec<String> {
    use omega_rules::contracts::MissionFamily;
    use omega_rules::research::capability_label;
    let owned = campus.owned_capabilities();
    let mut hints = Vec::new();
    for family in [MissionFamily::SurfaceToAir, MissionFamily::AirToAir] {
        let capability = family.capability(1);
        if !owned.contains(capability) {
            hints.push(format!(
                "{} offers need \"{}\"",
                family.label(),
                capability_label(capability)
            ));
        }
    }
    if !owned.contains("family:cruise") {
        hints.push("CRUISE vehicles need \"Cruise family\"".to_string());
    }
    if !owned.contains("family:anti_ship") {
        hints.push("difficulty 4-5 SURFACE-TO-SURFACE work needs \"Anti-ship family\"".to_string());
    }
    hints
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
                difficulty_bar(top, offer.difficulty);
            });
            card.spawn(Node {
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|meta| {
                ui_kit::chip(meta, offer.profile_label(), color);
                ui_kit::chip(meta, offer.vehicle_family.label(), ui_kit::NEUTRAL);
            });
            card.spawn(Node {
                min_height: Val::Px(34.0),
                ..default()
            })
            .with_children(|t| ui_kit::label(t, offer.title.clone(), 14.0, ui_kit::TEXT));
            ui_kit::label(card, offer.client.clone(), 10.0, ui_kit::TEXT_DIM);
            ui_kit::label(card, offer.brief.clone(), 9.5, ui_kit::TEXT_DIM);
            ui_kit::label(card, offer.context.clone(), 8.5, ui_kit::TEXT_DIM);
            ui_kit::label(
                card,
                format!(
                    "Budget {} cr | dossier : {} sections",
                    ui_kit::thousands(offer.design_budget_cr),
                    offer.requested_dossier.len()
                ),
                9.0,
                ui_kit::ACCENT,
            );
            for goal in &offer.primary_goals {
                ui_kit::label(card, format!("PRIMAIRE  {}", goal.text()), 9.0, ui_kit::TEXT);
            }
            if !offer.secondary_goals.is_empty() {
                for goal in &offer.secondary_goals {
                    ui_kit::label(card, format!("SECONDAIRE  {}", goal.text()), 8.5, ui_kit::TEXT_DIM);
                }
                ui_kit::label(
                    card,
                    format!("Bonus secondaire +{} cr", ui_kit::thousands(offer.secondary_bonus_cr)),
                    9.0,
                    ui_kit::GOOD,
                );
            }
            let requirements = offer.missile_requirements();
            let mut shown: Vec<&omega_rules::Requirement> = Vec::new();
            for id in ["PAYLOAD", "YIELD", "DISCRETION", "CEILING", "RANGE", "SPEED"] {
                if let Some(requirement) = requirements.iter().find(|requirement| requirement.id == id) {
                    shown.push(requirement);
                }
            }
            for requirement in &requirements {
                if shown.len() >= 4 {
                    break;
                }
                if !shown.iter().any(|known| known.id == requirement.id) {
                    shown.push(requirement);
                }
            }
            for requirement in shown.into_iter().take(4) {
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
                        ui_kit::label(pay, format!("{} jours", offer.deadline_days), 9.5, ui_kit::TEXT_DIM);
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
    active: &procurement::ActiveContract,
    status: EvidenceStatus,
    selected: bool,
    deadline_passed: bool,
    score: Option<f64>,
    ready: bool,
    metrics: DesignMetrics,
    primary: &[GoalStatus],
    secondary: &[GoalStatus],
) {
    let color = contract_color(active.offer.mission_family);
    let (mut node, mut bg, border) = ui_kit::card_node(Val::Px(560.0), color);
    if selected {
        node.border = UiRect::all(Val::Px(2.0));
        bg = BackgroundColor(ui_kit::CARD_SELECTED);
    }
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
                ui_kit::chip(top, format!("#{:04}", active.offer.id), ui_kit::NEUTRAL);
                ui_kit::chip(top, active.offer.mission_family.label(), color);
                ui_kit::chip(top, active.offer.profile_label(), ui_kit::ACCENT);
                if selected {
                    ui_kit::chip(top, "SELECTED", ui_kit::ACCENT);
                }
                if deadline_passed {
                    ui_kit::chip(top, "DEADLINE PASSED", ui_kit::BAD);
                }
            });
            card.spawn(Node {
                min_height: Val::Px(30.0),
                ..default()
            })
            .with_children(|t| ui_kit::label(t, active.offer.title.clone(), 15.0, ui_kit::TEXT));
            ui_kit::label(
                card,
                format!(
                    "{} | due day {} | {} trial(s)",
                    active.offer.client,
                    active.due_day(),
                    active.trials.len()
                ),
                10.0,
                ui_kit::TEXT_DIM,
            );
            if let Some(score) = score {
                ui_kit::label(
                    card,
                    format!(
                        "Dossier score {:.0}% | target {}%",
                        score * 100.0,
                        active.offer.requirements.min_trial_score
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
            ui_kit::label(
                card,
                format!(
                    "Budget {} / {} cr | masse {:.0} kg | fiabilité {:.0}/100 | complexité {:.0}/100",
                    metrics.cost_cr.round(),
                    active.offer.design_budget_cr,
                    metrics.mass_kg,
                    metrics.reliability,
                    metrics.complexity
                ),
                9.5,
                if budget_ok(&active.offer, &metrics) {
                    ui_kit::TEXT_DIM
                } else {
                    ui_kit::BAD
                },
            );
            for goal in primary {
                ui_kit::label(
                    card,
                    format!("{}  {}", if goal.met { "OK" } else { "KO" }, goal.text()),
                    9.5,
                    if goal.met { ui_kit::GOOD } else { ui_kit::BAD },
                );
            }
            if !secondary.is_empty() {
                let bonus_ready = secondary.iter().all(|goal| goal.met);
                ui_kit::label(
                    card,
                    format!(
                        "Bonus secondaire {} | +{} cr",
                        if bonus_ready { "acquis" } else { "non acquis" },
                        ui_kit::thousands(active.offer.secondary_bonus_cr)
                    ),
                    9.0,
                    if bonus_ready { ui_kit::GOOD } else { ui_kit::TEXT_DIM },
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
                if !selected {
                    ui_kit::button_row_item(
                        buttons,
                        "SELECT",
                        UiAction::SelectMissileContract(active.offer.id),
                        ButtonTone::Secondary,
                    );
                }
                ui_kit::button_row_item(
                    buttons,
                    "GO TO TRIALS",
                    UiAction::Navigate(CampusView::Trials),
                    ButtonTone::Secondary,
                );
                // A stale or missing dossier is one click from fresh evidence: the same seeded run
                // the trial board would make, recorded on every active contract.
                if status != EvidenceStatus::Current {
                    ui_kit::button_row_item(buttons, "RE-FLY", UiAction::ReflyMissileContract, ButtonTone::Secondary);
                }
                ui_kit::button_row_item(
                    buttons,
                    "DELIVER",
                    UiAction::DeliverMissile(active.offer.id),
                    if ready && !deadline_passed {
                        ButtonTone::Primary
                    } else {
                        ButtonTone::Disabled
                    },
                );
            });
        });
}

fn missile_dossier_panel(
    panel: &mut ChildSpawnerCommands<'_>,
    procurement: &Procurement,
    active: &procurement::ActiveContract,
    vehicle: &omega_rules::Vehicle,
    program: &omega_rules::Program,
    log: &RevisionLog,
) {
    ui_kit::label(panel, active.offer.title.clone(), 11.5, ui_kit::TEXT);
    ui_kit::label(panel, active.offer.profile_label(), 9.5, ui_kit::ACCENT);
    ui_kit::label(
        panel,
        format!("Accepted day {} | due day {}", active.accepted_day, active.due_day()),
        9.5,
        ui_kit::TEXT_DIM,
    );
    ui_kit::label(panel, active.offer.brief.clone(), 9.0, ui_kit::TEXT_DIM);
    ui_kit::label(panel, active.offer.context.clone(), 8.5, ui_kit::TEXT_DIM);
    let flying = omega_rules::VehicleFamily::of_vehicle(vehicle);
    if active.offer.profile.is_some() && flying != active.offer.vehicle_family {
        ui_kit::label(
            panel,
            format!(
                "PROFIL INCOMPATIBLE : cet appareil vole en {} ; le contrat demande {}.",
                flying.label(),
                active.offer.vehicle_family.label()
            ),
            9.0,
            ui_kit::BAD,
        );
    }
    let dossier = procurement.dossier_for(active, vehicle, program);
    let metrics = DesignMetrics::of(vehicle);
    let primary = goal_statuses(&active.offer.primary_goals, &metrics);
    let secondary = goal_statuses(&active.offer.secondary_goals, &metrics);
    ui_kit::label(
        panel,
        format!(
            "Budget {} cr | coût {:.0} cr | masse {:.0} kg",
            ui_kit::thousands(active.offer.design_budget_cr),
            metrics.cost_cr,
            metrics.mass_kg
        ),
        9.0,
        if budget_ok(&active.offer, &metrics) {
            ui_kit::GOOD
        } else {
            ui_kit::BAD
        },
    );
    for goal in &primary {
        ui_kit::label(
            panel,
            format!("Principal : {}", goal.text()),
            9.0,
            if goal.met { ui_kit::GOOD } else { ui_kit::BAD },
        );
    }
    for goal in &secondary {
        ui_kit::label(
            panel,
            format!("Secondaire : {}", goal.text()),
            8.5,
            if goal.met { ui_kit::GOOD } else { ui_kit::TEXT_DIM },
        );
    }
    ui_kit::label(
        panel,
        if log.head().is_some() {
            format!(
                "Version à soumettre : revision {} ({})",
                log.head().map_or(0, |revision| revision.id),
                log.head().map_or("", |revision| revision.label.as_str())
            )
        } else {
            "Aucune version enregistrée : la livraison utilisera l'état courant.".to_string()
        },
        8.5,
        ui_kit::ACCENT,
    );
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
    ui_kit::label(
        panel,
        format!("Dossier demandé : {}", active.offer.requested_dossier.join(" · ")),
        8.0,
        ui_kit::TEXT_DIM,
    );
}

fn build_missile_contracts(
    root: &mut ChildSpawnerCommands<'_>,
    campus: &CampusState,
    procurement: &Procurement,
    vehicle: &omega_rules::Vehicle,
    program: &omega_rules::Program,
    log: &RevisionLog,
    scroll: f32,
) {
    let max_active = campus.max_active_projects();
    let can_accept = procurement.active().len() < max_active;
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        flex_grow: 2.0,
        flex_basis: Val::Px(0.0),
        height: Val::Percent(100.0),
        min_height: Val::Px(0.0),
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
                    "Accept a family contract, fly its trial, then deliver the hash-bound dossier.",
                    10.5,
                    ui_kit::TEXT_DIM,
                );
            });
            ui_kit::chip(head, format!("{} cr", ui_kit::thousands(campus.credits)), ui_kit::GOOD);
            ui_kit::chip(
                head,
                format!("{}/{} slots", procurement.active().len(), max_active),
                ui_kit::WARN,
            );
            let selected_status = procurement.evidence_status(vehicle, program);
            if selected_status != EvidenceStatus::NoContract {
                ui_kit::chip(head, selected_status.label(), ui_kit::ACCENT);
            }
            ui_kit::chip(
                head,
                format!("{} delivered", procurement.completed().len()),
                ui_kit::NEUTRAL,
            );
        });
        left.spawn(Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            margin: UiRect {
                top: Val::Px(4.0),
                ..default()
            },
            ..default()
        })
        .with_children(|head| {
            ui_kit::label(head, "OFFERS", 11.0, ui_kit::TEXT_DIM);
            ui_kit::label(head, "MOLETTE : PARCOURIR", 9.0, ui_kit::TEXT_DIM);
        });
        left.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                row_gap: Val::Px(10.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition(Vec2::new(0.0, scroll)),
            ContractOfferScroll,
        ))
        .with_children(|scroll| {
            scroll
                .spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(10.0),
                    row_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|offers| {
                    if procurement.offers().is_empty() {
                        ui_kit::label(
                            offers,
                            "No offers from the researched families. Deliver a contract, or research a new family:",
                            11.0,
                            ui_kit::TEXT_DIM,
                        );
                    }
                    for hint in locked_family_hints(campus) {
                        ui_kit::label(offers, format!("- {hint}"), 10.0, ui_kit::TEXT_DIM);
                    }
                    for (index, offer) in procurement.offers().iter().enumerate() {
                        missile_offer_card(offers, index, offer, can_accept);
                    }
                });
            heading(
                scroll,
                format!("ACTIVE CONTRACTS  {}/{}", procurement.active().len(), max_active),
            );
            scroll
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    ..default()
                })
                .with_children(|actives| {
                    if procurement.active().is_empty() {
                        ui_kit::label(
                            actives,
                            "None. Accept an offer above; the trials you fly afterwards become its evidence.",
                            11.0,
                            ui_kit::TEXT_DIM,
                        );
                    }
                    for active in procurement.active() {
                        let status = procurement.status_for(active, vehicle, program);
                        let dossier = procurement.dossier_for(active, vehicle, program);
                        let ready = active.offer.accepts_dossier(&dossier);
                        let metrics = DesignMetrics::of(vehicle);
                        let primary = goal_statuses(&active.offer.primary_goals, &metrics);
                        let secondary = goal_statuses(&active.offer.secondary_goals, &metrics);
                        missile_active_card(
                            actives,
                            active,
                            status,
                            procurement.selected_id() == Some(active.offer.id),
                            campus.day > active.due_day(),
                            Some(dossier.score),
                            ready,
                            metrics,
                            &primary,
                            &secondary,
                        );
                    }
                });
        });
    });
    root.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            align_self: AlignSelf::Stretch,
            min_height: Val::Px(0.0),
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
        match procurement.selected() {
            None => ui_kit::label(
                panel,
                "No active contract. Accept an offer to open a dossier.",
                10.0,
                ui_kit::TEXT_DIM,
            ),
            Some(active) => missile_dossier_panel(panel, procurement, active, vehicle, program, log),
        }
        heading(panel, "DELIVERED");
        if procurement.completed().is_empty() {
            ui_kit::label(panel, "None yet.", 10.0, ui_kit::TEXT_DIM);
        } else {
            for contract in procurement.completed().iter().rev().take(4) {
                ui_kit::label(
                    panel,
                    format!(
                        "#{} {} | +{} cr | {}% | rev {} | bonus {}",
                        contract.id,
                        contract.title,
                        ui_kit::thousands(contract.reward),
                        contract.score_percent,
                        contract
                            .revision_id
                            .map_or_else(|| "-".to_string(), |id| id.to_string()),
                        ui_kit::thousands(contract.secondary_bonus)
                    ),
                    9.0,
                    ui_kit::TEXT_DIM,
                );
            }
        }
    });
}

fn department_color(dept: omega_rules::Department) -> Color {
    use omega_rules::Department::*;
    match dept {
        VehicleFamilies => Color::srgb(0.36, 0.84, 0.94),
        Materials => Color::srgb(0.92, 0.62, 0.30),
        Aerodynamics => Color::srgb(0.55, 0.82, 0.45),
        Sensors => Color::srgb(0.35, 0.75, 0.55),
        Computing => Color::srgb(0.62, 0.50, 0.90),
        DesignTools => Color::srgb(0.36, 0.62, 0.95),
    }
}

fn research_tier(tree: &[omega_rules::TechNode], id: &str, memo: &mut HashMap<String, usize>) -> usize {
    if let Some(tier) = memo.get(id) {
        return *tier;
    }
    let Some(node) = tree.iter().find(|node| node.id == id) else {
        return 0;
    };
    let tier = node
        .prereqs
        .iter()
        .map(|prereq| research_tier(tree, prereq, memo) + 1)
        .max()
        .unwrap_or(0);
    memo.insert(id.to_string(), tier);
    tier
}

/// Parts and possibilities a technology actually unlocks. Empty entries are filtered so the tree
/// never claims a technology is useful when it grants no part or campaign slot.
fn research_unlocks(tech: &omega_rules::TechNode) -> String {
    use omega_rules::{Grant, capability_label};
    let mut names = Vec::new();
    for grant in &tech.grants {
        match grant {
            Grant::Capability(capability) => {
                let parts: Vec<&str> = PartTemplate::ALL
                    .iter()
                    .filter(|template| template.spec().required_capability() == capability)
                    .map(|template| template.label())
                    .collect();
                if parts.is_empty() {
                    names.push(format!("capacité {}", capability_label(capability)));
                } else {
                    names.extend(parts.into_iter().map(str::to_string));
                }
            }
            Grant::ExtraActiveProjects(slots) => {
                names.push(format!("+{slots} projet(s) de contrat actif(s)"));
            }
        }
    }
    if names.is_empty() {
        "aucun déblocage direct".to_string()
    } else {
        names.join(", ")
    }
}

fn build_research(root: &mut ChildSpawnerCommands<'_>, campus: &CampusState, scroll: f32) {
    use omega_rules::{Department, ResearchError, default_tree};
    let tree = default_tree();
    let mut tiers = HashMap::new();
    let completed = campus.research.completed.len();
    let owned = campus.research.owned_capabilities(&tree).len();
    let available: Vec<(usize, &omega_rules::TechNode)> = tree
        .iter()
        .enumerate()
        .filter(|(_, tech)| {
            !campus.research.is_completed(&tech.id)
                && campus
                    .research
                    .active
                    .as_ref()
                    .is_none_or(|active| active.id != tech.id)
                && tech.prereqs.iter().all(|prereq| campus.research.is_completed(prereq))
        })
        .collect();
    let total_cost: u64 = tree.iter().map(|tech| tech.cost_cr).sum();
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        row_gap: Val::Px(10.0),
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
                            .find(|tech| tech.id == active.id)
                            .map_or(active.id.as_str(), |tech| tech.label.as_str());
                        format!(
                            "Researching: {label} - {} day(s) left. One project at a time; press NEXT DAY to advance.",
                            active.days_left
                        )
                    }
                    None => "The lab is free. Start a technology, then press NEXT DAY to let time pass.".to_string(),
                };
                ui_kit::label(title, status, 10.5, ui_kit::TEXT_DIM);
            });
            ui_kit::chip(head, format!("{completed}/{} RESEARCHED", tree.len()), ui_kit::GOOD);
            ui_kit::chip(head, format!("{} CAPABILITIES", owned), ui_kit::PURPLE);
            ui_kit::chip(head, format!("{} AVAILABLE", available.len()), ui_kit::ACCENT);
            ui_kit::chip(head, format!("DAY {}", campus.day), ui_kit::NEUTRAL);
            ui_kit::chip(head, format!("{} cr", ui_kit::thousands(campus.credits)), ui_kit::GOOD);
            ui_kit::button(head, "NEXT DAY", UiAction::NextDay, ButtonTone::Primary, Val::Px(130.0));
        });

        // The comparison strip is deliberately compact: it answers "what can I start now, and what
        // does each choice unlock?" before the player scrolls through six department columns.
        if !available.is_empty() {
            let mut comparable = available.clone();
            comparable.sort_by_key(|(_, tech)| (tech.cost_cr, tech.duration_days));
            col.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            })
            .insert(BackgroundColor(ui_kit::PANEL))
            .with_children(|panel| {
                ui_kit::label(panel, "COMPARER LES TECHNOLOGIES DISPONIBLES", 10.0, ui_kit::ACCENT);
                for (_, tech) in comparable.iter().take(6) {
                    ui_kit::label(
                        panel,
                        format!(
                            "{} — {} cr / {} j — débloque : {}",
                            tech.label,
                            ui_kit::thousands(tech.cost_cr),
                            tech.duration_days,
                            research_unlocks(tech)
                        ),
                        8.8,
                        ui_kit::TEXT_DIM,
                    );
                }
                ui_kit::label(
                    panel,
                    format!(
                        "{} cr pour terminer l'arbre · {} technologie(s) au total · une seule recherche à la fois",
                        ui_kit::thousands(total_cost),
                        tree.len()
                    ),
                    8.2,
                    ui_kit::TEXT_DIM,
                );
            });
        }

        // A department can hold more cards than fit under the window, so the grid scrolls
        // instead of drawing cards nobody can reach at the bottom of the screen.
        col.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                column_gap: Val::Px(10.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition(Vec2::new(0.0, scroll)),
            ResearchGrid,
        ))
        .with_children(|grid| {
            for dept in Department::ALL {
                let color = department_color(dept);
                let dept_techs: Vec<(usize, &omega_rules::TechNode)> = tree
                    .iter()
                    .enumerate()
                    .filter(|(_, tech)| tech.department == dept)
                    .collect();
                let done = dept_techs
                    .iter()
                    .filter(|(_, tech)| campus.research.is_completed(&tech.id))
                    .count();
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
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(2.0),
                            padding: UiRect::bottom(Val::Px(2.0)),
                            ..default()
                        })
                        .with_children(|h| {
                            ui_kit::label(h, format!("{}  {done}/{}", dept.label(), dept_techs.len()), 11.0, color);
                            let mut lane: Vec<&omega_rules::TechNode> =
                                dept_techs.iter().map(|(_, tech)| *tech).collect();
                            lane.sort_by_key(|tech| (research_tier(&tree, &tech.id, &mut tiers), &tech.label));
                            let path: Vec<String> = lane
                                .iter()
                                .map(|tech| {
                                    format!("T{} {}", research_tier(&tree, &tech.id, &mut tiers) + 1, tech.label)
                                })
                                .collect();
                            ui_kit::label(h, format!("PARCOURS : {}", path.join("  →  ")), 8.0, ui_kit::TEXT_DIM);
                        });
                    for (index, tech) in dept_techs {
                        let tier = research_tier(&tree, &tech.id, &mut tiers);
                        let (mut node, bg, border) = ui_kit::card_node(Val::Percent(100.0), color);
                        node.row_gap = Val::Px(4.0);
                        column
                            .spawn((node, bg, border, UiInteraction::None))
                            .with_children(|card| {
                                card.spawn(Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(4.0),
                                    ..default()
                                })
                                .with_children(|top| {
                                    ui_kit::chip(top, format!("PALIER {}", tier + 1), color);
                                    if let Some(prereq) = tech.prereqs.first() {
                                        let name = tree
                                            .iter()
                                            .find(|candidate| &candidate.id == prereq)
                                            .map_or(prereq.as_str(), |candidate| candidate.label.as_str());
                                        ui_kit::chip(top, format!("APRÈS {name}"), ui_kit::NEUTRAL);
                                    }
                                });
                                ui_kit::label(card, tech.label.clone(), 13.0, ui_kit::TEXT);
                                ui_kit::label(card, tech.description.clone(), 9.5, ui_kit::TEXT_DIM);
                                ui_kit::label(
                                    card,
                                    format!("DÉBLOQUE : {}", research_unlocks(tech)),
                                    9.0,
                                    ui_kit::ACCENT,
                                );
                                ui_kit::label(
                                    card,
                                    format!("{} cr | {} jours", ui_kit::thousands(tech.cost_cr), tech.duration_days),
                                    10.0,
                                    Color::srgb(0.78, 0.84, 0.88),
                                );
                                if !tech.prereqs.is_empty() {
                                    let names: Vec<String> = tech
                                        .prereqs
                                        .iter()
                                        .map(|prereq| {
                                            let label = tree
                                                .iter()
                                                .find(|candidate| &candidate.id == prereq)
                                                .map_or(prereq.as_str(), |candidate| candidate.label.as_str());
                                            if campus.research.is_completed(prereq) {
                                                format!("{label} OK")
                                            } else {
                                                format!("{label} requis")
                                            }
                                        })
                                        .collect();
                                    ui_kit::label(
                                        card,
                                        format!("Dépendances : {}", names.join(", ")),
                                        8.5,
                                        ui_kit::TEXT_DIM,
                                    );
                                }
                                if campus.research.is_completed(&tech.id) {
                                    card.spawn(Node::default())
                                        .with_children(|c| ui_kit::chip(c, "RESEARCHED", ui_kit::GOOD));
                                } else if campus
                                    .research
                                    .active
                                    .as_ref()
                                    .is_some_and(|active| active.id == tech.id)
                                {
                                    let left = campus.research.active.as_ref().map_or(0, |active| active.days_left);
                                    card.spawn(Node::default()).with_children(|c| {
                                        ui_kit::chip(c, format!("{left} DAY(S) LEFT"), ui_kit::PURPLE)
                                    });
                                } else {
                                    match campus.research.can_start(&tree, &tech.id, campus.credits) {
                                        Ok(()) => {
                                            if tech.prereqs.is_empty() {
                                                ui_kit::chip(card, "NOUVELLE POSSIBILITÉ", ui_kit::GOOD);
                                            }
                                            ui_kit::button(
                                                card,
                                                "START RESEARCH",
                                                UiAction::StartResearch(index),
                                                ButtonTone::Primary,
                                                Val::Percent(100.0),
                                            );
                                        }
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

fn part_title(spec: &omega_rules::PartSpec) -> &'static str {
    use omega_rules::{ComputerClass, PartSpec, RadarClass, SensorKind};
    match spec {
        PartSpec::SolidMotor { .. } => "Moteur solide",
        PartSpec::FuelTank { .. } => "Réservoir",
        PartSpec::LiquidEngine { .. } => "Moteur liquide",
        PartSpec::AirBreathing { .. } => "Propulseur à air",
        PartSpec::Sensor {
            kind: SensorKind::Pitot,
        } => "Sonde de pression",
        PartSpec::Sensor {
            kind: SensorKind::Barometer,
        } => "Baromètre",
        PartSpec::Sensor {
            kind: SensorKind::Radar(RadarClass::Basic),
        } => "Radar simple",
        PartSpec::Sensor {
            kind: SensorKind::Radar(RadarClass::Active),
        } => "Radar actif",
        PartSpec::Computer {
            class: ComputerClass::Basic,
        } => "Ordinateur de bord basique",
        PartSpec::Computer {
            class: ComputerClass::Standard,
        } => "Ordinateur de bord standard",
        PartSpec::Computer {
            class: ComputerClass::Advanced,
        } => "Ordinateur de bord avancé",
        PartSpec::Decoupler => "Séparateur",
        PartSpec::MissionModule { .. } => "Module de mission",
        PartSpec::Explosive { grade, .. } => match grade {
            omega_rules::ExplosiveGrade::Standard => "Charge explosive",
            omega_rules::ExplosiveGrade::Improved => "Charge explosive améliorée",
            omega_rules::ExplosiveGrade::Advanced => "Charge explosive avancée",
        },
        PartSpec::Adapter { .. } => "Adaptateur",
        PartSpec::NoseCone { .. } => "Coiffe",
        PartSpec::Wing { .. } => "Aile",
        PartSpec::FinSet { .. } => "Jeu d'ailettes",
        PartSpec::ControlSurface { .. } => "Surface de commande",
        PartSpec::Avionics { module } => match module {
            omega_rules::AvionicsModule::ControlAugment => "Module de commande",
            omega_rules::AvionicsModule::InertialNav => "Plateforme inertielle",
            omega_rules::AvionicsModule::Datalink => "Liaison de données",
            omega_rules::AvionicsModule::PowerCell => "Cellule d'alimentation",
            omega_rules::AvionicsModule::Diagnostics => "Module de diagnostic",
        },
    }
}

fn class_fr(class: omega_rules::ComputerClass) -> &'static str {
    use omega_rules::ComputerClass;
    match class {
        ComputerClass::Basic => "basique",
        ComputerClass::Standard => "standard",
        ComputerClass::Advanced => "avancé",
    }
}

/// French name of a programming block kind, as listed by `Vehicle::available_blocks`.
fn block_name_fr(kind: &str) -> &'static str {
    match kind {
        "ignite_stage" => "allumer l'étage",
        "separate_stage" => "séparer l'étage",
        "set_throttle" => "régler les gaz",
        "hold_speed" => "maintenir la vitesse",
        "hold_pressure" => "maintenir la pression",
        "lock_target" => "verrouiller la cible",
        "wait" => "attendre",
        "wait_speed" => "attendre la vitesse",
        "wait_pressure" => "attendre la pression",
        "wait_flag" => "attendre un indicateur",
        "toggle_flag" => "basculer un indicateur",
        "set_flag" => "définir un indicateur",
        "set_mode" => "définir un mode",
        "wait_mode" => "attendre un mode",
        "mark" => "marque",
        _ => "bloc",
    }
}

pub(crate) fn spec_summary(spec: &omega_rules::PartSpec) -> String {
    use omega_rules::PartSpec::*;
    match *spec {
        SolidMotor {
            length_m,
            diameter_m,
            burn_time_s,
        } => format!("Moteur solide · {length_m:.1} m × {diameter_m:.2} m · {burn_time_s:.0} s"),
        FuelTank { length_m, diameter_m } => {
            format!("Réservoir · {length_m:.1} m × {diameter_m:.2} m")
        }
        Avionics { .. } | Sensor { .. } | Computer { .. } | Decoupler => part_title(spec).to_string(),
        LiquidEngine { scale } => format!("Moteur liquide · ×{scale:.1}"),
        AirBreathing { scale } => format!("Propulseur à air · ×{scale:.1}"),
        MissionModule { mass_kg } => format!("Module de mission · {mass_kg:.0} kg"),
        Explosive { mass_kg, grade } => {
            let stats = spec.stats();
            let grade = match grade {
                omega_rules::ExplosiveGrade::Standard => "standard",
                omega_rules::ExplosiveGrade::Improved => "améliorée",
                omega_rules::ExplosiveGrade::Advanced => "avancée",
            };
            format!(
                "Charge {grade} · {mass_kg:.0} kg · rendement {:.0} kg",
                stats.payload_yield_kg
            )
        }
        Adapter {
            length_m,
            diameter_m,
            exit_diameter_m,
            shape,
        } => format!(
            "Adaptateur {} · {length_m:.2} m · Ø {diameter_m:.2} → {exit_diameter_m:.2} m",
            ui_kit::fr(shape.label())
        ),
        NoseCone {
            length_m,
            diameter_m,
            shape,
        } => format!(
            "Coiffe {} · {length_m:.2} m · Ø {diameter_m:.2} m",
            ui_kit::fr(shape.label())
        ),
        Wing {
            span_m,
            chord_m,
            sweep_deg,
            planform,
            profile,
            ..
        } => format!(
            "Aile {} · {span_m:.2} × {chord_m:.2} m · flèche {sweep_deg:.0}° · profil {}",
            ui_kit::fr(planform.label()),
            ui_kit::fr(profile.label())
        ),
        FinSet {
            span_m,
            chord_m,
            role,
            layout,
            ..
        } => format!(
            "Ailettes {} · {span_m:.2} × {chord_m:.2} m · {}",
            ui_kit::fr(role.label()),
            ui_kit::fr(layout.label())
        ),
        ControlSurface {
            span_m, chord_m, kind, ..
        } => format!("Commande {} · {span_m:.2} × {chord_m:.2} m", ui_kit::fr(kind.label())),
    }
}

fn transform_axis_row(
    parent: &mut ChildSpawnerCommands<'_>,
    axis_label: &'static str,
    axis: usize,
    position: f64,
    rotation: f64,
) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            align_items: AlignItems::Center,
            column_gap: Val::Px(3.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                width: Val::Px(14.0),
                ..default()
            })
            .with_children(|label| ui_kit::label(label, axis_label, 10.0, ui_kit::ACCENT));
            ui_kit::label(row, format!("{position:+.2} m"), 9.5, ui_kit::TEXT);
            ui_kit::button(
                row,
                "−",
                UiAction::Missile(MissileAction::AdjustTransform {
                    axis,
                    delta: -0.05,
                    rotate: false,
                }),
                ButtonTone::Secondary,
                Val::Px(24.0),
            );
            ui_kit::button(
                row,
                "+",
                UiAction::Missile(MissileAction::AdjustTransform {
                    axis,
                    delta: 0.05,
                    rotate: false,
                }),
                ButtonTone::Secondary,
                Val::Px(24.0),
            );
            ui_kit::label(row, format!("{rotation:.0}°"), 9.5, ui_kit::TEXT);
            ui_kit::button(
                row,
                "−",
                UiAction::Missile(MissileAction::AdjustTransform {
                    axis,
                    delta: -5.0,
                    rotate: true,
                }),
                ButtonTone::Secondary,
                Val::Px(24.0),
            );
            ui_kit::button(
                row,
                "+",
                UiAction::Missile(MissileAction::AdjustTransform {
                    axis,
                    delta: 5.0,
                    rotate: true,
                }),
                ButtonTone::Secondary,
                Val::Px(24.0),
            );
        });
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

pub(crate) fn score_color(value: f64) -> Color {
    if value < 30.0 {
        ui_kit::BAD
    } else if value < 55.0 {
        ui_kit::WARN
    } else {
        ui_kit::GOOD
    }
}

pub(crate) fn panel_node(width: Val) -> (Node, BackgroundColor, UiInteraction) {
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

/// Small fixed-width star beside a palette entry; a full button's padding would clip it in the
/// 252 px column.
fn palette_favorite(cell: &mut ChildSpawnerCommands<'_>, template: PartTemplate, favorite: bool) {
    let tone = if favorite {
        ButtonTone::Primary
    } else {
        ButtonTone::Ghost
    };
    cell.spawn((
        UiInteraction::default(),
        bevy::ui::FocusPolicy::Block,
        Node {
            width: Val::Px(22.0),
            min_height: Val::Px(28.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(tone.background(UiInteraction::None)),
        tone,
        UiActionButton(UiAction::Missile(MissileAction::ToggleFavorite(template))),
    ))
    .with_children(|b| {
        ui_kit::label(b, if favorite { "*" } else { "+" }, 12.0, ui_kit::TEXT);
    });
}

/// Compact square control used by the assembly tree rows (collapse, group, rename, lock, hide).
fn tree_control(row: &mut ChildSpawnerCommands<'_>, text: &str, action: UiAction, on: bool) {
    let tone = if on { ButtonTone::Secondary } else { ButtonTone::Ghost };
    row.spawn((
        UiInteraction::default(),
        bevy::ui::FocusPolicy::Block,
        Node {
            width: Val::Px(22.0),
            min_height: Val::Px(28.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(tone.background(UiInteraction::None)),
        tone,
        UiActionButton(action),
    ))
    .with_children(|b| {
        ui_kit::label(b, text, 9.0, if on { ui_kit::TEXT } else { ui_kit::TEXT_DIM });
    });
}

/// Two palette entries per row, each with its favourite toggle. The entries keep their drag data
/// through `spawn_palette_button`; the star is a plain action button beside them.
fn palette_grid(
    panel: &mut ChildSpawnerCommands<'_>,
    templates: &[PartTemplate],
    session: &MissileSession,
    owned: &std::collections::BTreeSet<String>,
) {
    for entries in templates.chunks(2) {
        panel
            .spawn(Node {
                width: Val::Percent(100.0),
                column_gap: Val::Px(3.0),
                ..default()
            })
            .with_children(|row| {
                for template in entries {
                    row.spawn(Node {
                        flex_grow: 1.0,
                        flex_basis: Val::Px(0.0),
                        column_gap: Val::Px(2.0),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|cell| {
                        crate::workshop::spawn_palette_button(
                            cell,
                            *template,
                            owned.contains(template.spec().required_capability()),
                        );
                        let favorite = session.favorites.contains(template);
                        palette_favorite(cell, *template, favorite);
                    });
                }
            });
    }
}

fn build_missile(
    root: &mut ChildSpawnerCommands<'_>,
    campus: &CampusState,
    session: &MissileSession,
    log: &RevisionLog,
    trials: &TrialSession,
    procurement: &Procurement,
    inspector: &InspectorState,
    preview_image: &Handle<Image>,
    scroll: f32,
    palette_scroll: f32,
    inspector_scroll: f32,
    assembly_scroll: f32,
) {
    let owned = campus.owned_capabilities();
    let vehicle = &session.vehicle;

    // Left: palette. It scrolls on short windows so the versions panel stays reachable.
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        width: Val::Px(252.0),
        height: Val::Percent(100.0),
        min_height: Val::Px(0.0),
        row_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|left| {
        let (mut node, bg, interaction) = panel_node(Val::Percent(100.0));
        node.flex_grow = 1.0;
        node.min_height = Val::Px(0.0);
        left.spawn((node, bg, interaction))
        .with_children(|panel| {
            ui_kit::label(panel, "MISSILE WORKSHOP", 15.0, ui_kit::ACCENT);
            ui_kit::label(panel, vehicle.name.clone(), 10.5, ui_kit::TEXT_DIM);
            // Only the palette scrolls; the versions footer stays outside the scroll area.
            panel
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        min_height: Val::Px(0.0),
                        row_gap: Val::Px(6.0),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    ScrollPosition(Vec2::new(0.0, palette_scroll)),
                    WorkshopPaletteList,
                ))
                .with_children(|panel| {
            heading(panel, "RECHERCHE");
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    let query = if session.palette_search.is_empty() {
                        "/ : rechercher une piece...".to_string()
                    } else {
                        format!("\"{}\"", session.palette_search)
                    };
                    ui_kit::label(row, query, 9.5, ui_kit::TEXT);
                    if !session.palette_search.is_empty() {
                        ui_kit::button(
                            row,
                            "EFFACER",
                            UiAction::Missile(MissileAction::ClearPaletteSearch),
                            ButtonTone::Ghost,
                            Val::Px(58.0),
                        );
                    }
                });
            // A render-to-texture camera shows the hovered part in the palette itself.
            let mut preview = ImageNode::new(preview_image.clone());
            preview.image_mode = bevy::ui::widget::NodeImageMode::Stretch;
            panel.spawn((
                preview,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(104.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(ui_kit::CARD),
            ));
            panel.spawn((
                Text::new("Survolez une piece"),
                TextFont {
                    font_size: FontSize::Px(9.0),
                    ..default()
                },
                TextColor(ui_kit::TEXT_DIM),
                crate::workshop::PaletteHintText,
            ));
            heading(panel, "CAMERA");
            for row_items in [
                [
                    ("FACE", CameraAction::View(CameraView::Front)),
                    ("COTE", CameraAction::View(CameraView::Side)),
                    ("DESSUS", CameraAction::View(CameraView::Top)),
                ],
                [
                    ("ARRIERE", CameraAction::View(CameraView::Rear)),
                    ("ISO", CameraAction::View(CameraView::Iso)),
                    ("CADRER", CameraAction::FrameVehicle),
                ],
                [
                    ("CENTRER", CameraAction::CenterSelection),
                    ("LIBRE/ORBITE", CameraAction::ToggleMode),
                    ("VITESSE -", CameraAction::Slower),
                ],
            ] {
                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        column_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|row| {
                        for (label, action) in row_items {
                            ui_kit::button_row_item(
                                row,
                                label,
                                UiAction::Camera(action),
                                ButtonTone::Ghost,
                            );
                        }
                    });
            }
            ui_kit::button(
                panel,
                "VITESSE +",
                UiAction::Camera(CameraAction::Faster),
                ButtonTone::Ghost,
                Val::Percent(100.0),
            );
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
                    ui_kit::button_row_item(
                        row,
                        "FINS",
                        UiAction::Missile(MissileAction::Preset(2)),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(
                        row,
                        "WINGS",
                        UiAction::Missile(MissileAction::Preset(3)),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(row, "EMPTY", UiAction::Missile(MissileAction::Clear), ButtonTone::Ghost);
                });
            let favorites: Vec<PartTemplate> = PartTemplate::ALL
                .iter()
                .copied()
                .filter(|template| session.favorites.contains(template))
                .filter(|template| session.palette_matches(*template))
                .collect();
            if !favorites.is_empty() {
                heading(panel, "FAVORIS");
                palette_grid(panel, &favorites, session, &owned);
            }
            let recents: Vec<PartTemplate> = session
                .recent
                .iter()
                .copied()
                .filter(|template| session.palette_matches(*template))
                .take(6)
                .collect();
            if !recents.is_empty() {
                heading(panel, "RÉCENTS");
                palette_grid(panel, &recents, session, &owned);
            }
            heading(panel, "CATÉGORIES DE PIÈCES");
            let category_buttons = [
                (None, "TOUT"),
                (Some(omega_rules::PartCategory::Propulsion), "PROPULSION"),
                (Some(omega_rules::PartCategory::Airframe), "STRUCTURE"),
                (Some(omega_rules::PartCategory::Aerodynamics), "AÉRODYNAMIQUE"),
                (Some(omega_rules::PartCategory::Sensors), "CAPTEURS"),
                (Some(omega_rules::PartCategory::Avionics), "AVIONIQUE"),
            ];
            for row_items in category_buttons.chunks(3) {
                panel.spawn(Node { width: Val::Percent(100.0), column_gap: Val::Px(4.0), ..default() })
                    .with_children(|row| {
                        for (category, label) in row_items {
                            let selected = session.palette_category == *category;
                            ui_kit::button_row_item(
                                row,
                                *label,
                                UiAction::Missile(MissileAction::SetPaletteCategory(*category)),
                                if selected { ButtonTone::Primary } else { ButtonTone::Ghost },
                            );
                        }
                    });
            }
            // The palette is grouped by the rules' own part categories, so a new part family
            // appears in the right section without editing this board.
            for category in omega_rules::PartCategory::ALL {
                let templates: Vec<PartTemplate> = PartTemplate::ALL
                    .iter()
                    .copied()
                    .filter(|template| template.spec().category() == category)
                    .filter(|_| session.palette_category.is_none_or(|selected| selected == category))
                    .filter(|template| session.palette_matches(*template))
                    .collect();
                if templates.is_empty() {
                    continue;
                }
                if session.palette_category.is_none() { heading(panel, category.label()); }
                palette_grid(panel, &templates, session, &owned);
            }
            heading(panel, "COMMANDES");
            ui_kit::label(
                panel,
                "Clic : sélectionner | Ctrl+clic : ajouter/retirer | Échap : désélectionner.\nGlisser une pièce vers un point vert : monter. Glisser une flèche : déplacer le groupe.",
                9.0,
                ui_kit::TEXT_DIM,
            );
            ui_kit::label(
                panel,
                "W/R : déplacer/pivoter | Suppr : retirer | Ctrl+Z/Y : annuler/rétablir | X : symétrie.\nF : centrer | Début : cadrer | 1-5 : vues | V : libre/orbite | [ ] : vitesse.",
                9.0,
                ui_kit::TEXT_DIM,
            );
            heading(panel, "SYMÉTRIE");
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    let count = if session.symmetry_kind == omega_rules::assembly::SymmetryKind::Radial {
                        session.symmetry
                    } else {
                        2
                    };
                    ui_kit::button_row_item(
                        row,
                        format!("{} x{count}", session.symmetry_kind.short_label()),
                        UiAction::Missile(MissileAction::CycleSymmetry),
                        ButtonTone::Secondary,
                    );
                    ui_kit::button_row_item(
                        row,
                        "MODE",
                        UiAction::Missile(MissileAction::CycleSymmetryKind),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(
                        row,
                        "ROMPRE",
                        UiAction::Missile(MissileAction::BreakSymmetry),
                        ButtonTone::Ghost,
                    );
                });
            ui_kit::label(
                panel,
                "X : nombre de copies radiales (2-8) | Maj+X : mode radial/bilatéral/axial.\nCtrl+clic sur deux pièces puis LIER pour les réunir en symétrie ; ROMPRE détache la sélection.",
                9.0,
                ui_kit::TEXT_DIM,
            );
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    ui_kit::button_row_item(
                        row,
                        "LIER LA SÉLECTION",
                        UiAction::Missile(MissileAction::LinkSymmetry),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(
                        row,
                        "DUPLIQUER",
                        UiAction::Missile(MissileAction::DuplicateSelection),
                        ButtonTone::Secondary,
                    );
                    ui_kit::button_row_item(
                        row,
                        "UNDO",
                        UiAction::Missile(MissileAction::Undo),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(
                        row,
                        "REDO",
                        UiAction::Missile(MissileAction::Redo),
                        ButtonTone::Ghost,
                    );
                });
            heading(panel, "BIBLIOTHÈQUE DE SOUS-ENSEMBLES");
            ui_kit::label(
                panel,
                "Sélectionnez un groupe puis enregistrez-le par catégorie. Les fichiers exportés sont relisibles dans une autre conception.",
                9.0,
                ui_kit::TEXT_DIM,
            );
            for kinds in [
                vec![
                    SubassemblyKind::Surfaces,
                    SubassemblyKind::Electronics,
                    SubassemblyKind::Structure,
                ],
                vec![SubassemblyKind::Stage, SubassemblyKind::Configuration],
            ] {
                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        column_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|row| {
                        for kind in kinds {
                            ui_kit::button_row_item(
                                row,
                                format!("ENREG. {}", kind.short_label()),
                                UiAction::Missile(MissileAction::SaveSubassembly(kind)),
                                ButtonTone::Ghost,
                            );
                        }
                    });
            }
            ui_kit::button(
                panel,
                "IMPORTER LES EXPORTS",
                UiAction::Missile(MissileAction::ImportSubassemblies),
                ButtonTone::Ghost,
                Val::Percent(100.0),
            );
            if session.library.is_empty() {
                ui_kit::label(panel, "Bibliothèque vide.", 9.5, ui_kit::TEXT_DIM);
            }
            for (index, entry) in session.library.iter().enumerate().take(4) {
                ui_kit::label(
                    panel,
                    format!(
                        "{} · {} · {} pièce(s)",
                        entry.name,
                        entry.kind.short_label(),
                        entry.parts.len()
                    ),
                    9.5,
                    ui_kit::TEXT,
                );
                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        column_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|row| {
                        ui_kit::button_row_item(
                            row,
                            "INSÉRER",
                            UiAction::Missile(MissileAction::ApplySubassembly(index)),
                            ButtonTone::Secondary,
                        );
                        ui_kit::button_row_item(
                            row,
                            "EXPORTER",
                            UiAction::Missile(MissileAction::ExportSubassembly(index)),
                            ButtonTone::Ghost,
                        );
                        ui_kit::button_row_item(
                            row,
                            "RETIRER",
                            UiAction::Missile(MissileAction::DeleteSubassembly(index)),
                            ButtonTone::Danger,
                        );
                    });
            }
            if session.library.len() > 4 {
                ui_kit::label(
                    panel,
                    format!("+{} autre(s) dans la bibliothèque", session.library.len() - 4),
                    9.0,
                    ui_kit::TEXT_DIM,
                );
            }
                });
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|footer| {
                    heading(footer, "VERSIONS");
                    if let Some(head) = log.head() {
                        ui_kit::label(
                            footer,
                            format!("HEAD rev {} | {} | {}", head.id, head.label, head.hash),
                            8.5,
                            ui_kit::TEXT_DIM,
                        );
                    } else {
                        ui_kit::label(footer, "Aucune version enregistrée.", 9.0, ui_kit::TEXT_DIM);
                    }
                    if let Some(submitted) = log.submitted() {
                        ui_kit::label(
                            footer,
                            format!("SOUMISE : rev {} ({})", submitted.id, submitted.label),
                            8.5,
                            ui_kit::GOOD,
                        );
                    }
                    if log.naming_target.is_some() {
                        ui_kit::label(
                            footer,
                            format!("Nom : {}_  (ENTRÉE pour valider)", log.naming_draft),
                            9.0,
                            ui_kit::ACCENT,
                        );
                        footer
                            .spawn(Node {
                                column_gap: Val::Px(3.0),
                                ..default()
                            })
                            .with_children(|row| {
                                ui_kit::button_row_item(
                                    row,
                                    "VALIDER",
                                    UiAction::CommitRevisionName,
                                    ButtonTone::Primary,
                                );
                                ui_kit::button_row_item(
                                    row,
                                    "ANNULER",
                                    UiAction::CancelRevisionName,
                                    ButtonTone::Ghost,
                                );
                            });
                    }
                    ui_kit::button(
                        footer,
                        "RECORD REVISION",
                        UiAction::RecordMissileRevision,
                        ButtonTone::Secondary,
                        Val::Percent(100.0),
                    );
                    for revision in log.vehicle_revisions().take(2) {
                        let trial_count = trials
                            .history
                            .entries()
                            .iter()
                            .filter(|entry| entry.revision_id == Some(revision.id))
                            .count();
                        ui_kit::label(
                            footer,
                            format!(
                                "rev {} | {} | {} essai(s) | {}",
                                revision.id,
                                revision.label,
                                trial_count,
                                if log.submitted().is_some_and(|submitted| submitted.id == revision.id) {
                                    "SOUMISE"
                                } else {
                                    "locale"
                                }
                            ),
                            8.0,
                            ui_kit::TEXT_DIM,
                        );
                        footer
                            .spawn(Node {
                                column_gap: Val::Px(3.0),
                                ..default()
                            })
                            .with_children(|row| {
                                ui_kit::button_row_item(
                                    row,
                                    "NOMMER",
                                    UiAction::BeginRevisionName(revision.id),
                                    ButtonTone::Ghost,
                                );
                                if log.head().is_some_and(|head| head.id != revision.id) {
                                    ui_kit::button_row_item(
                                        row,
                                        "COMPARER",
                                        UiAction::CompareRevision(revision.id),
                                        ButtonTone::Ghost,
                                    );
                                }
                                ui_kit::button_row_item(
                                    row,
                                    "RESTAURER",
                                    UiAction::RestoreRevision(revision.id),
                                    ButtonTone::Secondary,
                                );
                            });
                    }
                    if let Some((from, to)) = log.compare {
                        let diff = log
                            .revision(from)
                            .zip(log.revision(to))
                            .map_or_else(|| "comparaison indisponible".to_string(), |(a, b)| {
                                crate::missile::vehicle_diff(&a.vehicle, &b.vehicle)
                            });
                        ui_kit::label(
                            footer,
                            format!("DIFF rev {from} → rev {to} : {diff}"),
                            8.0,
                            ui_kit::ACCENT,
                        );
                        ui_kit::button(
                            footer,
                            "FERMER LA COMPARAISON",
                            UiAction::ClearRevisionComparison,
                            ButtonTone::Ghost,
                            Val::Percent(100.0),
                        );
                    }
                });
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
            ui_kit::label(panel, "AÉRODYNAMIQUE", 12.0, ui_kit::ACCENT);
            let aero = vehicle.aero();
            // The snapshot control sits right under the title: it stays inside the panel's first
            // rows whatever height the column gives it, unlike a control at the end of the card.
            match &session.comparison {
                Some(_) => ui_kit::button(
                    panel,
                    "CLEAR COMPARISON",
                    UiAction::Missile(MissileAction::ClearComparison),
                    ButtonTone::Ghost,
                    Val::Percent(100.0),
                ),
                None => ui_kit::button(
                    panel,
                    "SNAPSHOT A",
                    UiAction::Missile(MissileAction::CompareAero),
                    ButtonTone::Ghost,
                    Val::Percent(100.0),
                ),
            }
            if aero.surface_count == 0 {
                ui_kit::label(
                    panel,
                    "No aerodynamic surfaces. Drag a wing, fin set or control surface onto a radial mount.",
                    10.0,
                    ui_kit::TEXT_DIM,
                );
            } else {
                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        column_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|row| {
                        for (label, value, color) in [
                            ("STABILITY", aero.stability, score_color(aero.stability)),
                            ("CONTROL", aero.control_authority, score_color(aero.control_authority)),
                            ("LIFT", aero.lift, ui_kit::ACCENT),
                            ("DRAG", aero.drag_index, score_color(100.0 - aero.drag_index)),
                        ] {
                            let (mut node, bg, border) = ui_kit::card_node(Val::Px(0.0), color);
                            node.flex_grow = 1.0;
                            node.flex_basis = Val::Px(0.0);
                            node.row_gap = Val::Px(1.0);
                            row.spawn((node, bg, border)).with_children(|card| {
                                ui_kit::label(card, label, 8.5, ui_kit::TEXT_DIM);
                                ui_kit::label(card, format!("{value:.0}"), 15.0, color);
                            });
                        }
                    });
                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        column_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|row| {
                        for (label, text, color) in [
                            ("COMPLEXITY", format!("{:.0}", aero.complexity), ui_kit::TEXT_DIM),
                            (
                                "RELIABILITY",
                                format!("{:.0}", aero.reliability),
                                score_color(aero.reliability),
                            ),
                            ("AERO MASS", format!("{:.0} kg", aero.mass_kg), ui_kit::TEXT_DIM),
                        ] {
                            row.spawn(Node {
                                flex_grow: 1.0,
                                flex_basis: Val::Px(0.0),
                                flex_direction: FlexDirection::Column,
                                ..default()
                            })
                            .with_children(|col| {
                                ui_kit::label(col, label, 8.5, ui_kit::TEXT_DIM);
                                ui_kit::label(col, text, 11.0, color);
                            });
                        }
                    });
                let (low, high) = aero.stable_band_m();
                ui_kit::label(
                    panel,
                    format!(
                        "CG {:.2} m | CA {:.2} m | marge {:+.2} m (bande {:.2}-{:.2} m)",
                        aero.cg_y_m, aero.ca_y_m, aero.margin_m, low, high
                    ),
                    9.5,
                    ui_kit::TEXT,
                );
                panel.spawn(Node::default()).with_children(|c| {
                    let (text, color) = if aero.is_stable() {
                        ("BANDE STABLE", ui_kit::GOOD)
                    } else if aero.margin_m < 0.0 {
                        ("INSTABLE : CA DEVANT LE CG", ui_kit::BAD)
                    } else {
                        ("LIMITE", ui_kit::WARN)
                    };
                    ui_kit::chip(c, text, color);
                });
                ui_kit::label(
                    panel,
                    format!(
                        "Aile {:.2} m² | ailettes {:.2} m² | commandes {:.2} m² | flèche {:.0}°",
                        aero.wing_area_m2, aero.fin_area_m2, aero.control_area_m2, aero.mean_sweep_deg
                    ),
                    9.0,
                    ui_kit::TEXT_DIM,
                );
                // Explainability: what the selected surface actually contributes to the totals.
                if let Some(id) = &session.selected
                    && let Some(row) = omega_rules::aero::breakdown(vehicle).iter().find(|row| &row.id == id)
                {
                    ui_kit::label(
                        panel,
                        format!(
                            "{} {} : portance {:.2} | traînée {:.2} | stabilité {:.2} | autorité {:.2} m² | {:.1} kg",
                            row.family, row.id, row.lift, row.drag, row.stability, row.authority, row.mass_kg
                        ),
                        8.5,
                        ui_kit::TEXT_DIM,
                    );
                }
            }
            if let Some(before) = &session.comparison {
                let current = vehicle.aero();
                ui_kit::label(
                    panel,
                    format!(
                        "A vs actuel : stabilité {:+.0} | commande {:+.0} | portance {:+.0} | traînée {:+.0} | masse {:+.0} kg",
                        current.stability - before.stability,
                        current.control_authority - before.control_authority,
                        current.lift - before.lift,
                        current.drag_index - before.drag_index,
                        current.mass_kg - before.mass_kg
                    ),
                    9.5,
                    ui_kit::TEXT,
                );
            }
        });
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        middle.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "CONFORMITÉ AU CONTRAT", 12.0, ui_kit::ACCENT);
            match procurement.selected() {
                None => ui_kit::label(
                    panel,
                    "No active contract. Accept one in the CONTRACT OFFICE to see its requirements here.",
                    9.5,
                    ui_kit::TEXT_DIM,
                ),
                Some(active) => {
                    let left = active.due_day().saturating_sub(campus.day);
                    ui_kit::label(
                        panel,
                        format!(
                            "#{} {} | due day {} ({} left)",
                            active.offer.id,
                            active.offer.title,
                            active.due_day(),
                            left
                        ),
                        9.5,
                        ui_kit::TEXT_DIM,
                    );
                    let checks = omega_rules::contract_preview(vehicle, &active.offer);
                    for check in checks.iter().take(8) {
                        let color = match check.status {
                            omega_rules::CheckStatus::Met => ui_kit::GOOD,
                            omega_rules::CheckStatus::Close => ui_kit::WARN,
                            omega_rules::CheckStatus::Missed => ui_kit::BAD,
                            omega_rules::CheckStatus::Unproven => ui_kit::TEXT_DIM,
                        };
                        let unit = if check.metric == "launch_mass_kg" { " kg" } else { "" };
                        let value = check
                            .value
                            .map_or("trial".to_string(), |value| format!("{value:.0}{unit}"));
                        let comparator = match check.comparator {
                            omega_rules::Comparator::AtLeast => ">=",
                            omega_rules::Comparator::AtMost => "<=",
                        };
                        ui_kit::label(
                            panel,
                            format!(
                                "{}: {value} {comparator} {:.0}{unit}  [{}]",
                                check.label,
                                check.limit,
                                check.status.label()
                            ),
                            9.5,
                            color,
                        );
                    }
                }
            }
        });
        let (node, bg, interaction) = panel_node(Val::Percent(100.0));
        middle.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "BILAN DES ÉTAGES", 12.0, ui_kit::ACCENT);
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
                                    "Masse {:.0} kg (emporte {:.0} kg)",
                                    report.stage_wet_mass_kg, report.ignition_mass_kg
                                ),
                                10.0,
                                ui_kit::TEXT_DIM,
                            );
                            ui_kit::label(
                                card,
                                format!("Poussée {:.1} kN", report.thrust_n / 1000.0),
                                10.0,
                                ui_kit::TEXT_DIM,
                            );
                            let burn = report
                                .burn_time_s
                                .map_or("alimenté par les réservoirs".to_string(), |b| format!("{b:.0} s"));
                            ui_kit::label(
                                card,
                                format!("Combustion {burn} | Δv {:.0} m/s", report.delta_v_ms),
                                10.0,
                                ui_kit::TEXT_DIM,
                            );
                        });
                    }
                });
            let length = vehicle.aero().length_m;
            ui_kit::label(
                panel,
                format!(
                    "Masse totale {:.0} kg | longueur {:.2} m | {} pièces | {} étages",
                    vehicle.total_wet_mass_kg(),
                    length,
                    vehicle.parts.len(),
                    vehicle.stage_count()
                ),
                11.0,
                ui_kit::TEXT,
            );
            let avionics = vehicle.avionics();
            if avionics.devices > 0 {
                let power = if avionics.power_draw_w > 0.0 {
                    format!(
                        "{:.0}/{:.0} W, marge {:.0} W",
                        avionics.power_draw_w, avionics.power_supply_w, avionics.power_margin_w
                    )
                } else {
                    "alimenté par le bus".to_string()
                };
                ui_kit::label(
                    panel,
                    format!(
                        "Électronique : {} équipements | {} | {} crédits",
                        avionics.devices, power, avionics.cost_cr
                    ),
                    10.0,
                    if avionics.power_shortfall() {
                        ui_kit::BAD
                    } else {
                        ui_kit::TEXT_DIM
                    },
                );
            }
            match vehicle.computer() {
                Some(class) => ui_kit::label(
                    panel,
                    format!(
                        "Ordinateur {} : {} tâche(s), {} blocs de mémoire",
                        class_fr(class),
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
            let blocks: Vec<&str> = vehicle
                .available_blocks()
                .into_iter()
                .map(block_name_fr)
                .collect();
            ui_kit::label(
                panel,
                format!("Blocs de programmation débloqués : {}", blocks.join(", ")),
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
        let (mut node, bg, interaction) = panel_node(Val::Percent(100.0));
        // A design with wings, fins and control surfaces has many rows; cap the tree so the
        // selected-part inspector and the validation panel underneath keep their rows on screen.
        node.max_height = Val::Percent(30.0);
        node.overflow = Overflow::clip_y();
        right.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "ASSEMBLY TREE", 12.0, ui_kit::ACCENT);
            ui_kit::label(
                panel,
                "-/+ plier · G groupe · L verrou · H masque · N nommer. Un découpleur ouvre un étage.",
                9.0,
                ui_kit::TEXT_DIM,
            );
            if vehicle.parts.is_empty() {
                ui_kit::label(panel, "No parts yet: add one from the palette.", 10.0, ui_kit::TEXT_DIM);
            }
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
                    ScrollPosition(Vec2::new(0.0, scroll)),
                    PartListScroll,
                ))
                .with_children(|panel| {
                    for entry in session.tree_rows() {
                        let children = vehicle.children(&entry.id).len();
                        let count = vehicle.subtree_ids(&entry.id).len();
                        let collapsed = session.collapsed.contains(&entry.id);
                        let locked = session.locked.contains(&entry.id);
                        let hidden = session.hidden.contains(&entry.id);
                        let primary = session.selected.as_deref() == Some(entry.id.as_str());
                        let selected = session.is_selected(&entry.id);
                        let name = session.group_name(&entry.id).map_or_else(
                            || session.vehicle
                                .part(&entry.id)
                                .map(|part| part_title(&part.spec).to_string())
                                .unwrap_or_else(|| entry.id.clone()),
                            str::to_string,
                        );
                        let indent = if entry.depth == 0 {
                            String::new()
                        } else {
                            format!("{}└ ", "  ".repeat(entry.depth))
                        };
                        let state = match (locked, hidden) {
                            (true, true) => " · verrouillé, masqué",
                            (true, false) => " · verrouillé",
                            (false, true) => " · masqué",
                            (false, false) => "",
                        };
                        let symmetry = vehicle
                            .symmetry_kind_of(&entry.id)
                            .map(|kind| format!(" · sym {}", kind.label()))
                            .unwrap_or_default();
                        let tone = if primary {
                            ButtonTone::Primary
                        } else if selected {
                            ButtonTone::Secondary
                        } else {
                            ButtonTone::Ghost
                        };
                        panel
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                column_gap: Val::Px(2.0),
                                align_items: AlignItems::Center,
                                ..default()
                            })
                            .with_children(|row| {
                                if children > 0 {
                                    tree_control(
                                        row,
                                        if collapsed { "+" } else { "-" },
                                        UiAction::Missile(MissileAction::ToggleCollapsed { id: entry.id.clone() }),
                                        false,
                                    );
                                } else {
                                    row.spawn(Node { width: Val::Px(22.0), ..default() });
                                }
                                ui_kit::button_row_item(
                                    row,
                                    format!("{indent}{name} [{count}] · E{}{state}{symmetry}", entry.stage + 1),
                                    UiAction::Missile(MissileAction::Select { id: entry.id.clone() }),
                                    tone,
                                );
                                if children > 0 {
                                    tree_control(
                                        row,
                                        "G",
                                        UiAction::Missile(MissileAction::SelectGroup { id: entry.id.clone() }),
                                        false,
                                    );
                                    tree_control(
                                        row,
                                        "N",
                                        UiAction::Missile(MissileAction::BeginRename { id: entry.id.clone() }),
                                        false,
                                    );
                                }
                                tree_control(
                                    row,
                                    if locked { "L" } else { "l" },
                                    UiAction::Missile(MissileAction::ToggleLock { id: entry.id.clone() }),
                                    locked,
                                );
                                tree_control(
                                    row,
                                    if hidden { "H" } else { "h" },
                                    UiAction::Missile(MissileAction::ToggleHidden { id: entry.id.clone() }),
                                    hidden,
                                );
                            });
                    }
                });
        });
        let (mut node, bg, interaction) = panel_node(Val::Percent(100.0));
        node.max_height = Val::Percent(52.0);
        node.overflow = Overflow::scroll_y();
        right
            .spawn((
                node,
                bg,
                interaction,
                ScrollPosition(Vec2::new(0.0, inspector_scroll)),
                InspectorScrollPanel,
            ))
            .with_children(|panel| {
                ui_kit::label(panel, "SELECTED PART · MOLETTE : DÉFILER", 12.0, ui_kit::ACCENT);
                if session.selection.len() > 1 {
                    ui_kit::label(
                        panel,
                        format!(
                            "{} pièces sélectionnées : le gizmo déplace le groupe sans changer la hiérarchie. Ctrl+clic pour ajuster, Échap pour tout désélectionner.",
                            session.selection.len()
                        ),
                        9.5,
                        ui_kit::WARN,
                    );
<<<<<<< Updated upstream
                    if let Some(attach) = part.attach.as_ref() {
                        ui_kit::label(panel, "TRANSFORMATIONS LOCALES · pas 0,05 m / 5°", 9.5, ui_kit::ACCENT);
                        transform_axis_row(panel, "X", 0, attach.offset_x_m, attach.rotation_x_deg);
                        transform_axis_row(panel, "Y", 1, attach.offset_y_m, attach.rotation_y_deg);
                        transform_axis_row(panel, "Z", 2, attach.offset_z_m, attach.rotation_z_deg);
                    }
                    let needed: Vec<&str> = std::iter::once(part.spec.required_capability())
                        .chain(part.spec.extra_capabilities())
                        .collect();
                    let missing: Vec<String> = needed
                        .iter()
                        .filter(|capability| !owned.contains(**capability))
                        .map(|capability| omega_rules::research::capability_label(capability))
                        .collect();
                    if !missing.is_empty() {
                        ui_kit::label(
                            panel,
                            format!("Needs research: {}", missing.join(", ")),
                            9.5,
                            ui_kit::WARN,
                        );
                    }
                    if let Some(category) = omega_rules::avionics::category_of(&part.spec) {
                        ui_kit::label(
                            panel,
                            format!("Électronique : {}", ui_kit::fr(category.label())),
                            9.5,
                            ui_kit::TEXT_DIM,
                        );
                    }
                    if let omega_rules::PartSpec::Avionics { module } = part.spec {
                        let spec = module.spec();
                        let power = if spec.power_w < 0.0 {
                            format!("alimente {:.0} W", -spec.power_w)
                        } else {
                            format!("demande {:.0} W", spec.power_w)
                        };
                        ui_kit::label(
                            panel,
                            format!(
                                "{} crédits | {} | complexité {:.0} %",
                                spec.cost_cr,
                                power,
                                spec.complexity * 100.0
                            ),
                            9.5,
                            ui_kit::TEXT_DIM,
                        );
                        let mut effect = Vec::new();
                        if spec.authority_bonus > 0.0 {
                            effect.push(format!("autorité +{:.0}%", spec.authority_bonus * 100.0));
                        }
                        if spec.lock_bonus > 0.0 {
                            effect.push(format!("verrouillage +{:.0} pts", spec.lock_bonus * 100.0));
                        }
                        if spec.fault_reduction > 0.0 {
                            effect.push(format!("pannes -{:.0}%", spec.fault_reduction * 100.0));
                        }
                        if effect.is_empty() {
                            effect.push(format!("aucun effet en vol, fiabilité {:+.2}", spec.reliability));
                        }
                        ui_kit::label(panel, effect.join(" | "), 9.5, ui_kit::TEXT_DIM);
                        let mut needs = Vec::new();
                        if module.needs_standard_computer() {
                            needs.push("un ordinateur standard");
                        }
                        if module.needs_navigation() {
                            needs.push("un module de navigation");
                        }
                        if !needs.is_empty() {
                            ui_kit::label(panel, format!("Nécessite {}", needs.join(" et ")), 9.5, ui_kit::WARN);
                        }
                    }
                    let params = missile::params_of(&part.spec);
                    if params.is_empty() {
                        ui_kit::label(panel, "This part has no adjustable setting.", 10.0, ui_kit::TEXT_DIM);
                    }
                    for (param, value) in params {
                        param_row(panel, param, value);
                    }
                    for choice in missile::choices_of(&part.spec) {
                        ui_kit::button(
                            panel,
                            format!("{}: {}", choice.label(), missile::choice_value_for_part(part, choice)),
                            UiAction::Missile(MissileAction::CycleAero(choice)),
                            ButtonTone::Ghost,
                            Val::Percent(100.0),
                        );
                    }
                    let stats = part.spec.stats();
                    let mut line = format!("Masse sèche {:.1} kg", stats.dry_mass_kg);
                    if stats.propellant_kg > 0.0 {
                        line.push_str(&format!(" | propergol {:.1} kg", stats.propellant_kg));
                    }
                    if stats.thrust_n > 0.0 {
                        line.push_str(&format!(" | poussée {:.1} kN", stats.thrust_n / 1000.0));
                    }
                    ui_kit::label(panel, line, 10.0, ui_kit::TEXT_DIM);
=======
>>>>>>> Stashed changes
                    ui_kit::button(
                        panel,
                        "DÉSELECTIONNER TOUT",
                        UiAction::Missile(MissileAction::ClearSelection),
                        ButtonTone::Ghost,
                        Val::Percent(100.0),
                    );
                }
                match session.selected_part() {
                    Some(part) => {
                        ui_kit::label(panel, spec_summary(&part.spec), 11.0, ui_kit::TEXT);
                        let help = missile::part_help(&part.spec);
                        for (title, line) in [("RÔLE", 0), ("CONSÉQUENCES", 1), ("À SAVOIR", 2)] {
                            if let Some(line) = help.get(line) {
                                ui_kit::label(panel, title, 9.0, ui_kit::ACCENT);
                                ui_kit::label(panel, line, 9.5, ui_kit::TEXT_DIM);
                            }
                        }
                        let balance = omega_rules::balance::part_balance(&part.spec);
                        let (advantage, limitation) = omega_rules::balance::tradeoff(&part.spec);
                        ui_kit::label(panel, "BILAN DE LA PIÈCE", 9.0, ui_kit::ACCENT);
                        ui_kit::label(
                            panel,
                            format!(
                                "Masse {:.1} kg · carburant {:.1} kg · coût ~{:.0} cr",
                                balance.dry_mass_kg, balance.propellant_kg, balance.cost_cr
                            ),
                            9.5,
                            ui_kit::TEXT_DIM,
                        );
                        ui_kit::label(
                            panel,
                            format!(
                                "Puissance {:+.0} W · complexité {:.2} · fiabilité {:+.2}",
                                balance.power_w, balance.complexity, balance.reliability
                            ),
                            9.5,
                            ui_kit::TEXT_DIM,
                        );
                        ui_kit::label(panel, format!("Avantage : {advantage}"), 9.0, ui_kit::GOOD);
                        ui_kit::label(panel, format!("Limite : {limitation}"), 9.0, ui_kit::WARN);
                        let has_children = !vehicle.children(&part.id).is_empty();
                        let locked = session.locked.contains(&part.id);
                        let hidden = session.hidden.contains(&part.id);
                        panel
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                column_gap: Val::Px(4.0),
                                ..default()
                            })
                            .with_children(|row| {
                                ui_kit::button_row_item(
                                    row,
                                    if locked { "DÉVERROUILLER" } else { "VERROUILLER" },
                                    UiAction::Missile(MissileAction::ToggleLock { id: part.id.clone() }),
                                    if locked { ButtonTone::Secondary } else { ButtonTone::Ghost },
                                );
                                ui_kit::button_row_item(
                                    row,
                                    if hidden { "AFFICHER" } else { "MASQUER" },
                                    UiAction::Missile(MissileAction::ToggleHidden { id: part.id.clone() }),
                                    if hidden { ButtonTone::Secondary } else { ButtonTone::Ghost },
                                );
                            });
                        if has_children {
                            panel
                                .spawn(Node {
                                    width: Val::Percent(100.0),
                                    column_gap: Val::Px(4.0),
                                    ..default()
                                })
                                .with_children(|row| {
                                    ui_kit::button_row_item(
                                        row,
                                        "TOUT LE GROUPE",
                                        UiAction::Missile(MissileAction::SelectGroup { id: part.id.clone() }),
                                        ButtonTone::Ghost,
                                    );
                                    ui_kit::button_row_item(
                                        row,
                                        "NOMMER LE GROUPE",
                                        UiAction::Missile(MissileAction::BeginRename { id: part.id.clone() }),
                                        ButtonTone::Ghost,
                                    );
                                });
                        }
                        if session.rename_target.as_deref() == Some(part.id.as_str()) {
                            ui_kit::label(
                                panel,
                                format!("Nom : {}", session.rename_draft),
                                10.0,
                                ui_kit::TEXT,
                            );
                            panel
                                .spawn(Node {
                                    width: Val::Percent(100.0),
                                    column_gap: Val::Px(4.0),
                                    ..default()
                                })
                                .with_children(|row| {
                                    ui_kit::button_row_item(
                                        row,
                                        "OK",
                                        UiAction::Missile(MissileAction::CommitRename),
                                        ButtonTone::Primary,
                                    );
                                    ui_kit::button_row_item(
                                        row,
                                        "ANNULER",
                                        UiAction::Missile(MissileAction::CancelRename),
                                        ButtonTone::Ghost,
                                    );
                                });
                        }
                        ui_kit::label(panel, "PARAMÈTRES", 9.0, ui_kit::ACCENT);
                        let (height, radius) = part.dimensions();
                        let place = part.attach.as_ref().map_or("pièce racine".to_string(), |attach| {
                            if part.spec.is_aero() {
                                format!(
                                    "sur {} via {} | {:.0}°",
                                    attach.parent,
                                    ui_kit::fr(attach.node.label()),
                                    attach.angle_deg
                                )
                            } else {
                                format!("sur {} via {}", attach.parent, ui_kit::fr(attach.node.label()))
                            }
                        });
                        ui_kit::label(
                            panel,
                            format!(
                                "{} | étage {} | {:.2} m × {:.2} m | {}",
                                place,
                                vehicle.stage_of(&part.id).unwrap_or(0) + 1,
                                height,
                                radius * 2.0,
                                ui_kit::fr(part.spec.category().label())
                            ),
                            9.5,
                            ui_kit::TEXT_DIM,
                        );
                        inspector::spawn_editor(panel, part, inspector.precision);
                        let needed: Vec<&str> = std::iter::once(part.spec.required_capability())
                            .chain(part.spec.extra_capabilities())
                            .collect();
                        let missing: Vec<String> = needed
                            .iter()
                            .filter(|capability| !owned.contains(**capability))
                            .map(|capability| omega_rules::research::capability_label(capability))
                            .collect();
                        if !missing.is_empty() {
                            ui_kit::label(
                                panel,
                                format!("Needs research: {}", missing.join(", ")),
                                9.5,
                                ui_kit::WARN,
                            );
                        }
                        if let Some(kind) = vehicle.symmetry_kind_of(&part.id) {
                            let members = vehicle.symmetry_members(&part.id);
                            ui_kit::label(
                                panel,
                                format!(
                                    "Symétrie {} · {} copie(s) liée(s) : les réglages et transformations se propagent.",
                                    kind.label(),
                                    members.len()
                                ),
                                9.5,
                                ui_kit::ACCENT,
                            );
                            panel
                                .spawn(Node {
                                    width: Val::Percent(100.0),
                                    column_gap: Val::Px(4.0),
                                    ..default()
                                })
                                .with_children(|row| {
                                    ui_kit::button_row_item(
                                        row,
                                        "ROMPRE LE LIEN",
                                        UiAction::Missile(MissileAction::BreakSymmetry),
                                        ButtonTone::Ghost,
                                    );
                                    ui_kit::button_row_item(
                                        row,
                                        "LIER LA SÉLECTION",
                                        UiAction::Missile(MissileAction::LinkSymmetry),
                                        ButtonTone::Ghost,
                                    );
                                });
                        }
                        if let Some(category) = omega_rules::avionics::category_of(&part.spec) {
                            ui_kit::label(
                                panel,
                                format!("Électronique : {}", ui_kit::fr(category.label())),
                                9.5,
                                ui_kit::TEXT_DIM,
                            );
                        }
                        if let omega_rules::PartSpec::Avionics { module } = part.spec {
                            let spec = module.spec();
                            let power = if spec.power_w < 0.0 {
                                format!("alimente {:.0} W", -spec.power_w)
                            } else {
                                format!("demande {:.0} W", spec.power_w)
                            };
                            ui_kit::label(
                                panel,
                                format!(
                                    "{} crédits | {} | complexité {:.0} %",
                                    spec.cost_cr,
                                    power,
                                    spec.complexity * 100.0
                                ),
                                9.5,
                                ui_kit::TEXT_DIM,
                            );
                            let mut effect = Vec::new();
                            if spec.authority_bonus > 0.0 {
                                effect.push(format!("autorité +{:.0}%", spec.authority_bonus * 100.0));
                            }
                            if spec.lock_bonus > 0.0 {
                                effect.push(format!("verrouillage +{:.0} pts", spec.lock_bonus * 100.0));
                            }
                            if spec.fault_reduction > 0.0 {
                                effect.push(format!("pannes -{:.0}%", spec.fault_reduction * 100.0));
                            }
                            if effect.is_empty() {
                                effect.push(format!("aucun effet en vol, fiabilité {:+.2}", spec.reliability));
                            }
                            ui_kit::label(panel, effect.join(" | "), 9.5, ui_kit::TEXT_DIM);
                            let mut needs = Vec::new();
                            if module.needs_standard_computer() {
                                needs.push("un ordinateur standard");
                            }
                            if module.needs_navigation() {
                                needs.push("un module de navigation");
                            }
                            if !needs.is_empty() {
                                ui_kit::label(panel, format!("Nécessite {}", needs.join(" et ")), 9.5, ui_kit::WARN);
                            }
                        }
                        let params = missile::params_of(part);
                        if params.is_empty() {
                            ui_kit::label(panel, "This part has no adjustable setting.", 10.0, ui_kit::TEXT_DIM);
                        }
                        for (param, value) in params {
                            param_row(panel, param, value);
                        }
                        for choice in missile::choices_of(&part.spec) {
                            ui_kit::button(
                                panel,
                                format!("{}: {}", choice.label(), missile::choice_value_for_part(part, choice)),
                                UiAction::Missile(MissileAction::CycleAero(choice)),
                                ButtonTone::Ghost,
                                Val::Percent(100.0),
                            );
                        }
                        let stats = part.spec.stats();
                        let mut line = format!("Masse sèche {:.1} kg", stats.dry_mass_kg);
                        if stats.propellant_kg > 0.0 {
                            line.push_str(&format!(" | propergol {:.1} kg", stats.propellant_kg));
                        }
                        if stats.thrust_n > 0.0 {
                            line.push_str(&format!(" | poussée {:.1} kN", stats.thrust_n / 1000.0));
                        }
                        ui_kit::label(panel, line, 10.0, ui_kit::TEXT_DIM);
                        ui_kit::button(
                            panel,
                            "SUPPRIMER LA PIÈCE",
                            UiAction::Missile(MissileAction::RemovePart),
                            ButtonTone::Danger,
                            Val::Percent(100.0),
                        );
                    }
                    None => ui_kit::label(
                        panel,
                        "Click a part in the tree above to inspect, resize or remove it.",
                        10.5,
                        ui_kit::TEXT_DIM,
                    ),
                }
            });
        let (mut node, bg, interaction) = panel_node(Val::Percent(100.0));
        node.flex_grow = 1.0;
        node.min_height = Val::Px(0.0);
        node.overflow = Overflow::scroll_y();
        right
            .spawn((
                node,
                bg,
                interaction,
                ScrollPosition(Vec2::new(0.0, assembly_scroll)),
                AssemblyControlPanel,
            ))
            .with_children(|panel| {
            let report = session.assembly_report();
            let errors = report
                .iter()
                .filter(|issue| issue.severity == omega_rules::IssueSeverity::Error)
                .count();
            let warnings = report.len() - errors;
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
                    align_items: AlignItems::Center,
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    ui_kit::label(row, "CONTRÔLE DE L'ASSEMBLAGE", 12.0, ui_kit::ACCENT);
                    let (text, color) = if errors == 0 {
                        ("ASSEMBLAGE COHÉRENT".to_string(), ui_kit::GOOD)
                    } else if errors == 1 {
                        ("1 ERREUR".to_string(), ui_kit::BAD)
                    } else {
                        (format!("{errors} ERREURS"), ui_kit::BAD)
                    };
                    ui_kit::chip(row, text, color);
                    if warnings > 0 {
                        ui_kit::chip(
                            row,
                            if warnings == 1 {
                                "1 AVERTISSEMENT".to_string()
                            } else {
                                format!("{warnings} AVERTISSEMENTS")
                            },
                            ui_kit::WARN,
                        );
                    }
                });
            if report.is_empty() {
                ui_kit::label(
                    panel,
                    "Aucun problème : connexions, points d'attache, catégories, verrous et sous-ensembles sont cohérents.",
                    9.5,
                    ui_kit::TEXT_DIM,
                );
            }
            // A dedicated panel groups construction problems by cause. Every row that names a
            // part is a shortcut: pressing it selects the part in the tree and in 3D.
            for kind in [
                missile::AssemblyIssueKind::Unconnected,
                missile::AssemblyIssueKind::Occupied,
                missile::AssemblyIssueKind::Incompatible,
                missile::AssemblyIssueKind::Locked,
                missile::AssemblyIssueKind::Subassembly,
            ] {
                let rows: Vec<&missile::AssemblyIssue> =
                    report.iter().filter(|issue| issue.kind == kind).collect();
                if rows.is_empty() {
                    continue;
                }
                let issues_are_errors = rows
                    .iter()
                    .any(|issue| issue.severity == omega_rules::IssueSeverity::Error);
                ui_kit::label(
                    panel,
                    format!("{} ({})", kind.label(), rows.len()),
                    9.0,
                    if issues_are_errors { ui_kit::BAD } else { ui_kit::WARN },
                );
                for issue in &rows {
                    let prefix = match issue.severity {
                        omega_rules::IssueSeverity::Error => "ERREUR",
                        omega_rules::IssueSeverity::Warning => "AVERT.",
                    };
                    match &issue.part {
                        Some(id) => ui_kit::button(
                            panel,
                            format!("{prefix}  {}  [{id}]", issue.message),
                            UiAction::Missile(MissileAction::Select { id: id.clone() }),
                            ButtonTone::Ghost,
                            Val::Percent(100.0),
                        ),
                        None => ui_kit::label(
                            panel,
                            format!("{prefix}  {}", issue.message),
                            9.5,
                            if issue.severity == omega_rules::IssueSeverity::Error {
                                ui_kit::BAD
                            } else {
                                ui_kit::WARN
                            },
                        ),
                    };
                }
            }
        });
    });
}

// ----------------------------------------------------------------------------------------------
// Workshop project card
// ----------------------------------------------------------------------------------------------

fn build_program(
    root: &mut ChildSpawnerCommands<'_>,
    vehicle: &omega_rules::Vehicle,
    session: &MissileProgramSession,
    palette_scroll: f32,
) {
    let issues = session.issues(vehicle);

    // Left: block palette, gated by the hardware fitted to the vehicle. It scrolls on short
    // windows so CLEAR PROGRAM stays reachable.
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        width: Val::Px(252.0),
        height: Val::Percent(100.0),
        min_height: Val::Px(0.0),
        row_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|left| {
        let (mut node, bg, interaction) = panel_node(Val::Percent(100.0));
        node.flex_grow = 1.0;
        node.min_height = Val::Px(0.0);
        left.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "MISSILE PROGRAM", 15.0, ui_kit::ACCENT);
            ui_kit::label(panel, vehicle.name.clone(), 10.5, ui_kit::TEXT_DIM);
            // Only the library scrolls; the clear action stays outside the scroll area.
            panel
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        min_height: Val::Px(0.0),
                        row_gap: Val::Px(6.0),
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    ScrollPosition(Vec2::new(0.0, palette_scroll)),
                    ProgramPaletteList,
                ))
                .with_children(|panel| {
                    heading(panel, "BLOCK LIBRARY");
                    // The palette follows the rules' categories, so a new block family lands in the right
                    // section without editing this board.
                    for category in omega_rules::BlockCategory::ALL {
                        let kinds: Vec<BlockKind> = BlockKind::ALL
                            .iter()
                            .copied()
                            .filter(|kind| kind.category() == category)
                            .collect();
                        if kinds.is_empty() {
                            continue;
                        }
                        heading(panel, category.label());
                        for row in kinds.chunks(2) {
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
                                        // A selected block is the insertion point; with nothing selected
                                        // the library appends to the thread.
                                        let action = if session.block.is_some() {
                                            ProgramAction::Insert(*kind)
                                        } else {
                                            ProgramAction::Append(*kind)
                                        };
                                        ui_kit::button_row_item(r, kind.label(), UiAction::Program(action), tone);
                                    }
                                });
                        }
                    }
                    ui_kit::label(
                        panel,
                        "Greyed blocks need hardware this vehicle does not carry.",
                        9.5,
                        ui_kit::TEXT_DIM,
                    );
                });
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|footer| {
                    heading(footer, "PROGRAM");
                    ui_kit::button(
                        footer,
                        "CLEAR PROGRAM",
                        UiAction::Program(ProgramAction::Clear),
                        ButtonTone::Danger,
                        Val::Percent(100.0),
                    );
                });
        });
    });

    // Middle: the visual canvas and its compact inspector.
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        flex_basis: Val::Px(0.0),
        min_width: Val::Px(0.0),
        min_height: Val::Px(0.0),
        row_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|middle| {
        crate::campus::program_canvas::build_program_canvas_column(middle, vehicle, session);
    });

    // Right: computer limits and validation.
    root.spawn(Node {
        flex_direction: FlexDirection::Column,
        width: Val::Px(330.0),
        row_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(|right| {
        let (mut node, bg, interaction) = panel_node(Val::Percent(100.0));
        node.max_height = Val::Percent(55.0);
        node.overflow = Overflow::clip_y();
        right.spawn((node, bg, interaction)).with_children(|panel| {
            ui_kit::label(panel, "FLIGHT COMPUTER", 12.0, ui_kit::ACCENT);
            match (vehicle.computer(), session.limits(vehicle)) {
                (Some(class), Some((threads, blocks))) => {
                    ui_kit::label(panel, format!("Ordinateur {}", class_fr(class)), 11.0, ui_kit::TEXT);
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
                                format!("{} / {threads} tâches", session.threads_used()),
                                thread_tone,
                            );
                            ui_kit::chip(row, format!("{} / {blocks} blocs", session.blocks_used()), block_tone);
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
            let (errors, warnings) = issue_counts(&issues);
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    if errors == 0 {
                        ui_kit::chip(row, "PROGRAM VALID", ui_kit::GOOD);
                    } else {
                        ui_kit::chip(
                            row,
                            if errors == 1 {
                                "1 ERREUR".to_string()
                            } else {
                                format!("{errors} ERREURS")
                            },
                            ui_kit::BAD,
                        );
                    }
                    if warnings > 0 {
                        ui_kit::chip(
                            row,
                            if warnings == 1 {
                                "1 AVERTISSEMENT".to_string()
                            } else {
                                format!("{warnings} AVERTISSEMENTS")
                            },
                            ui_kit::WARN,
                        );
                    }
                });
            for issue in issues.iter().take(6) {
                let prefix = match issue.severity {
                    omega_rules::IssueSeverity::Error => "ERROR",
                    omega_rules::IssueSeverity::Warning => "WARN",
                };
                let color = match issue.severity {
                    omega_rules::IssueSeverity::Error => ui_kit::BAD,
                    omega_rules::IssueSeverity::Warning => ui_kit::WARN,
                };
                ui_kit::label(panel, format!("{prefix}  {}", issue.message), 9.5, color);
            }
            if issues.len() > 6 {
                ui_kit::label(
                    panel,
                    format!("+{} autre(s) problème(s)", issues.len() - 6),
                    9.5,
                    ui_kit::TEXT_DIM,
                );
            }
            if let Ok(hash) = session.content_hash() {
                let short: String = hash.as_str().chars().take(16).collect();
                ui_kit::label(panel, format!("Empreinte du programme {short}"), 9.5, ui_kit::TEXT_DIM);
            }
            ui_kit::label(
                panel,
                "Saved to programs/missile.json on every change; saved copies live in programs/library.json.",
                9.5,
                ui_kit::TEXT_DIM,
            );
        });
        let (mut node, bg, interaction) = panel_node(Val::Percent(100.0));
        node.flex_grow = 1.0;
        node.min_height = Val::Px(0.0);
        node.height = Val::Auto;
        node.overflow = Overflow::clip_y();
        right.spawn((node, bg, interaction)).with_children(|panel| {
            heading(panel, "SAVED PROGRAMS");
            ui_kit::button(
                panel,
                "SAVE COPY",
                UiAction::Program(ProgramAction::SaveCopy),
                ButtonTone::Secondary,
                Val::Percent(100.0),
            );
            if session.library.programs.is_empty() {
                ui_kit::label(
                    panel,
                    "No saved program yet. A copy can be loaded on any vehicle; the rules check its hardware.",
                    9.5,
                    ui_kit::TEXT_DIM,
                );
            }
            for (index, saved) in session.library.programs.iter().take(4).enumerate() {
                panel
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        column_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|row| {
                        ui_kit::button_row_item(
                            row,
                            format!("CHARGER {}", saved.name),
                            UiAction::Program(ProgramAction::LoadSaved(index)),
                            ButtonTone::Ghost,
                        );
                        ui_kit::button_row_item(
                            row,
                            "X",
                            UiAction::Program(ProgramAction::DeleteSaved(index)),
                            ButtonTone::Danger,
                        );
                    });
            }
            if session.library.programs.len() > 4 {
                ui_kit::label(
                    panel,
                    format!("+{} autre(s) programme(s)", session.library.programs.len() - 4),
                    9.0,
                    ui_kit::TEXT_DIM,
                );
            }
            heading(panel, "DEBUGGER");
            ui_kit::button(
                panel,
                "RUN TRACE",
                UiAction::Program(ProgramAction::Debug),
                ButtonTone::Secondary,
                Val::Percent(100.0),
            );
            match &session.debug {
                Some(report) => {
                    let timeouts = report.trace.iter().filter(|entry| entry.timed_out).count();
                    ui_kit::label(
                        panel,
                        format!(
                            "{} blocs exécutés | {} événements | {} délai(s)",
                            report.trace.len(),
                            report.events.len(),
                            timeouts
                        ),
                        9.5,
                        ui_kit::TEXT,
                    );
                    ui_kit::label(
                        panel,
                        format!(
                            "{:.1} s de vol | {:.0} m/s max | {} anomalie(s)",
                            report.flight_time_s,
                            report.max_speed_ms,
                            report.anomalies.len()
                        ),
                        9.5,
                        ui_kit::TEXT_DIM,
                    );
                    for (index, entry) in report.trace.iter().enumerate().take(6) {
                        let (marker, color) = if index == session.debug_cursor {
                            (">", ui_kit::ACCENT)
                        } else if entry.anomaly {
                            ("!", ui_kit::BAD)
                        } else if index < session.debug_cursor {
                            ("+", ui_kit::GOOD)
                        } else {
                            ("·", ui_kit::WARN)
                        };
                        ui_kit::label(
                            panel,
                            format!(
                                "{marker} {:.1}s T{} #{} {}",
                                entry.t_s,
                                entry.thread + 1,
                                entry.block + 1,
                                entry.label
                            ),
                            9.0,
                            color,
                        );
                    }
                    for anomaly in report.anomalies.iter().take(2) {
                        ui_kit::label(panel, format!("! {anomaly}"), 9.0, ui_kit::BAD);
                    }
                }
                None => ui_kit::label(
                    panel,
                    "Run a trace to fly this program once and list the blocks it executed, with their times.",
                    9.5,
                    ui_kit::TEXT_DIM,
                ),
            }
        });
    });
}

pub(crate) fn rate(value: Option<f64>) -> String {
    value
        .map(|rate| format!("{:.0}%", rate * 100.0))
        .unwrap_or_else(|| "-".to_string())
}

/// One bar chart: samples collapsed to at most 96 bars, scaled to the series maximum.
pub(crate) fn plot(
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
    log: &RevisionLog,
) {
    super::trial_lab::build(root, campus, vehicle, program, session, procurement, log);
}

/// Mouse-wheel scrolling for the workshop stage list, mirroring [`scroll_program_list`].
pub(crate) fn scroll_part_list(
    window: Single<&Window>,
    wheel: Res<AccumulatedMouseScroll>,
    mut scroll: ResMut<WorkshopScroll>,
    mut lists: Query<&mut ScrollPosition, With<PartListScroll>>,
) {
    let Ok(mut position) = lists.single_mut() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if cursor.x < window.width() - 380.0 || cursor.y > window.height() * 0.30 {
        return;
    }
    if wheel.delta.y != 0.0 {
        let step = match wheel.unit {
            MouseScrollUnit::Line => 34.0,
            MouseScrollUnit::Pixel => 1.0,
        } * wheel.delta.y;
        position.y = (position.y - step).max(0.0);
    }
    if scroll.0 != position.y {
        scroll.0 = position.y;
    }
}

/// Mouse-wheel scrolling for the selected-part inspector, the lower half of the right column.
pub(crate) fn scroll_inspector(
    window: Single<&Window>,
    wheel: Res<AccumulatedMouseScroll>,
    mut scroll: ResMut<InspectorScrollOffset>,
    mut panels: Query<&mut ScrollPosition, With<InspectorScrollPanel>>,
) {
    let Ok(mut position) = panels.single_mut() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if cursor.x < window.width() - 380.0 || cursor.y < window.height() * 0.30 || cursor.y > window.height() * 0.84 {
        return;
    }
    if wheel.delta.y != 0.0 {
        let step = match wheel.unit {
            MouseScrollUnit::Line => 34.0,
            MouseScrollUnit::Pixel => 1.0,
        } * wheel.delta.y;
        position.y = (position.y - step).max(0.0);
    }
    if scroll.0 != position.y {
        scroll.0 = position.y;
    }
}

/// Mouse-wheel scrolling for the assembly-control panel at the bottom of the right column.
pub(crate) fn scroll_assembly_control(
    window: Single<&Window>,
    wheel: Res<AccumulatedMouseScroll>,
    mut scroll: ResMut<AssemblyControlScroll>,
    mut panels: Query<&mut ScrollPosition, With<AssemblyControlPanel>>,
) {
    let Ok(mut position) = panels.single_mut() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if cursor.x < window.width() - 380.0 || cursor.y < window.height() * 0.84 {
        return;
    }
    if wheel.delta.y != 0.0 {
        let step = match wheel.unit {
            MouseScrollUnit::Line => 34.0,
            MouseScrollUnit::Pixel => 1.0,
        } * wheel.delta.y;
        position.y = (position.y - step).max(0.0);
    }
    if scroll.0 != position.y {
        scroll.0 = position.y;
    }
}

/// Mouse-wheel scrolling for the workshop palette, only while the cursor is over its column.
pub(crate) fn scroll_workshop_palette(
    window: Single<&Window>,
    wheel: Res<AccumulatedMouseScroll>,
    mut scroll: ResMut<WorkshopPaletteScroll>,
    mut lists: Query<&mut ScrollPosition, With<WorkshopPaletteList>>,
) {
    let Ok(mut position) = lists.single_mut() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if cursor.x >= 280.0 {
        return;
    }
    if wheel.delta.y != 0.0 {
        let step = match wheel.unit {
            MouseScrollUnit::Line => 34.0,
            MouseScrollUnit::Pixel => 1.0,
        } * wheel.delta.y;
        position.y = (position.y - step).max(0.0);
    }
    if scroll.0 != position.y {
        scroll.0 = position.y;
    }
}

/// Mouse-wheel scrolling for the program palette, only while the cursor is over its column.
pub(crate) fn scroll_program_palette(
    window: Single<&Window>,
    wheel: Res<AccumulatedMouseScroll>,
    mut scroll: ResMut<ProgramPaletteScroll>,
    mut lists: Query<&mut ScrollPosition, With<ProgramPaletteList>>,
) {
    let Ok(mut position) = lists.single_mut() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if cursor.x >= 280.0 {
        return;
    }
    if wheel.delta.y != 0.0 {
        let step = match wheel.unit {
            MouseScrollUnit::Line => 34.0,
            MouseScrollUnit::Pixel => 1.0,
        } * wheel.delta.y;
        position.y = (position.y - step).max(0.0);
    }
    if scroll.0 != position.y {
        scroll.0 = position.y;
    }
}

/// Mouse-wheel scrolling for the offer list. The dossier stays in its own column, so scrolling the
/// eight offer cards never hides the contract being worked on.
pub(crate) fn scroll_contracts_board(
    campus: Res<CampusState>,
    window: Single<&Window>,
    wheel: Res<AccumulatedMouseScroll>,
    mut scroll: ResMut<ContractScroll>,
    mut lists: Query<&mut ScrollPosition, With<ContractOfferScroll>>,
) {
    if campus.view != CampusView::Contracts {
        return;
    }
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    // Only the left column scrolls; the dossier column keeps its own content in place.
    if cursor.x > window.width() * 0.68 {
        return;
    }
    let Ok(mut position) = lists.single_mut() else {
        return;
    };
    if wheel.delta.y != 0.0 {
        let step = match wheel.unit {
            MouseScrollUnit::Line => 40.0,
            MouseScrollUnit::Pixel => 1.0,
        } * wheel.delta.y;
        position.y = (position.y - step).max(0.0);
    }
    if scroll.0 != position.y {
        scroll.0 = position.y;
    }
}

/// Mouse-wheel scrolling for the research grid, mirroring [`scroll_program_list`] so a department
/// with more cards than the panel can show stays reachable.
pub(crate) fn scroll_research_grid(
    wheel: Res<AccumulatedMouseScroll>,
    mut scroll: ResMut<ResearchScroll>,
    mut grids: Query<&mut ScrollPosition, With<ResearchGrid>>,
) {
    let Ok(mut position) = grids.single_mut() else {
        return;
    };
    if wheel.delta.y != 0.0 {
        let step = match wheel.unit {
            MouseScrollUnit::Line => 34.0,
            MouseScrollUnit::Pixel => 1.0,
        } * wheel.delta.y;
        position.y = (position.y - step).max(0.0);
    }
    if scroll.0 != position.y {
        scroll.0 = position.y;
    }
}

/// Mouse-wheel scrolling for the missile program block list.
///
/// `bevy_ui` moves a node's children by `ScrollPosition` but nothing turns the wheel into an
/// offset, so the list would stay stuck on its first blocks once a program outgrows the panel. The
/// offset is mirrored in the session because the board is rebuilt after every edit; the wheel only
/// targets the list while the cursor is over the middle column, since the block library also scrolls.
pub(crate) fn scroll_program_list(
    window: Single<&Window>,
    wheel: Res<AccumulatedMouseScroll>,
    mut session: ResMut<MissileProgramSession>,
    mut lists: Query<&mut ScrollPosition, With<ProgramListScroll>>,
) {
    let Ok(mut position) = lists.single_mut() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if cursor.x < 280.0 {
        return;
    }
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

fn visible(kind: BoardKind, campus: &CampusState) -> bool {
    match kind {
        BoardKind::Island => campus.view == CampusView::Island,
        BoardKind::Contracts => campus.view == CampusView::Contracts,
        BoardKind::Research => campus.view == CampusView::Research,
        BoardKind::Missile => campus.view == CampusView::Missile,
        BoardKind::Program => campus.view == CampusView::Program,
        BoardKind::Trials => campus.view == CampusView::Trials,
    }
}

fn signature(
    kind: BoardKind,
    campus: &CampusState,
    missile: &MissileSession,
    program: &MissileProgramSession,
    trials: &TrialSession,
    procurement: &Procurement,
    log: &RevisionLog,
    inspector: &InspectorState,
) -> String {
    match kind {
        BoardKind::Island => format!(
            "{}|{}|{}|{}",
            campus.credits,
            campus.day,
            procurement.active().len(),
            procurement.completed().len()
        ),
        BoardKind::Contracts => format!(
            "{}|{}|{}|{}|{:?}",
            campus.credits,
            campus.day,
            campus.max_active_projects(),
            procurement.signature(&missile.vehicle, &program.program),
            log.head().map(|revision| (revision.id, revision.label.clone()))
        ),
        BoardKind::Research => format!("{:?}|{}|{}", campus.research, campus.credits, campus.day),
        BoardKind::Missile => format!(
            "{:?}|{}|{}|{}|{:?}|{:?}|{:?}|{}|{}|{}|{}|{:?}|{:?}|{:?}|{:?}|{}",
            missile.vehicle,
            missile.ui_signature(),
            missile.symmetry,
            campus.credits,
            campus.research,
            log.head().map(|revision| revision.hash.clone()),
            // Recording or clearing the A side must redraw the panel, or it keeps showing the
            // button that was just pressed.
            missile.comparison,
            campus.day,
            procurement.active().len(),
            // The contract check follows the selected active contract.
            procurement.selected_id().unwrap_or(0),
            // Precision changes the visible step caption and selected button.
            inspector.signature(),
            log.submitted().map(|revision| revision.id),
            log.naming_target,
            log.naming_draft,
            log.compare,
            trials.history.entries().len()
        ),
        BoardKind::Program => format!(
            "{:?}|{:?}|{}|{:?}|{:?}|{:?}|{}|{}|{}|{}|{:?}|{:?}|{:?}",
            missile.vehicle,
            program.program,
            program.thread,
            program.block,
            program.path,
            program.selection,
            program.debug_cursor,
            program.debug_playing,
            program.debug_speed,
            program.multi_select,
            program.branch_focus,
            program.debug.as_ref().map(|report| (
                report.trace.len(),
                report.anomalies.len(),
                report.flight_time_s.to_bits()
            )),
            program
                .library
                .programs
                .iter()
                .map(|saved| &saved.name)
                .collect::<Vec<_>>()
        ),
        BoardKind::Trials => format!(
            "{}|{}|{}|{}|{}|{}|{}|{:?}|{:?}|{}|{:?}|{:?}",
            trials.revision,
            trials.seed,
            trials.air_launch,
            trials.report.is_some(),
            trials.batch.is_some(),
            trials.comparison.is_some(),
            procurement.signature(&missile.vehicle, &program.program),
            trials.tab,
            trials.selected_history,
            trials.history.entries().len(),
            trials.failure,
            trials.context.revision_id
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_boards(
    mut commands: Commands,
    frontend: Res<crate::main_menu::Frontend>,
    campus: Res<CampusState>,
    missile: Res<MissileSession>,
    program: Res<MissileProgramSession>,
    trials: Res<TrialSession>,
    procurement: Res<Procurement>,
    log: Res<RevisionLog>,
    inspector: InspectorPanel,
    palette_preview: Res<crate::workshop::PalettePreview>,
    scrolls: BoardScrolls,
    mut boards: Query<(Entity, &mut Node, &BoardKind)>,
    mut seen: Local<HashMap<BoardKind, String>>,
) {
    for (entity, mut node, kind) in &mut boards {
        let show = *frontend == crate::main_menu::Frontend::Game && visible(*kind, &campus);
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
            &missile,
            &program,
            &trials,
            &procurement,
            &log,
            &inspector.state,
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
        let procurement_ref: &Procurement = &procurement;
        commands.entity(entity).with_children(|root| match kind {
            BoardKind::Island => build_island(root, campus_ref, procurement_ref),
            BoardKind::Contracts => build_missile_contracts(
                root,
                campus_ref,
                procurement_ref,
                &missile_ref.vehicle,
                &program_ref.program,
                &log,
                scrolls.contract.0,
            ),
            BoardKind::Research => build_research(root, campus_ref, scrolls.research.0),
            BoardKind::Missile => build_missile(
                root,
                campus_ref,
                missile_ref,
                &log,
                &trials,
                procurement_ref,
                &inspector.state,
                &palette_preview.image,
                scrolls.workshop.0,
                scrolls.workshop_palette.0,
                inspector.scroll.0,
                scrolls.assembly.0,
            ),
            BoardKind::Program => build_program(root, &missile_ref.vehicle, program_ref, scrolls.program_palette.0),
            BoardKind::Trials => build_trials(
                root,
                campus_ref,
                &missile_ref.vehicle,
                &program_ref.program,
                trials_ref,
                procurement_ref,
                &log,
            ),
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
        if let Err(error) = trials.persist_history() {
            warn!("Could not save the trial history: {error}");
            campus.message = format!("Historique non sauvegardé : {error}");
        }
    }
}

/// Flush the campaign history once the board has handled input, so exports and navigation do not
/// depend on the next click to persist a finished campaign.
pub(crate) fn persist_trial_history(mut trials: ResMut<TrialSession>) {
    if let Err(error) = trials.persist_history() {
        warn!("Could not save the trial history: {error}");
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
            format!("> {}", ui_kit::fr(state.0.clone()))
        } else {
            String::new()
        };
        if text.0 != wanted {
            text.0 = wanted;
        }
    }
}

pub(crate) fn update_nav_status(
    window: Single<&Window>,
    campus: Res<CampusState>,
    procurement: Res<Procurement>,
    mut status: Query<&mut Text, With<NavStatus>>,
) {
    if let Ok(mut text) = status.single_mut() {
        // On a narrow window the full status would wrap under the nav buttons; split it into two
        // short lines instead (the bar is 62 px, room for two 13 px lines).
        let value = if window.width() < 1400.0 {
            format!(
                "JOUR {} · {} crédits\n{}/{} contrats · {} livrés",
                campus.day,
                ui_kit::thousands(campus.credits),
                procurement.active().len(),
                campus.max_active_projects(),
                procurement.completed().len()
            )
        } else {
            format!(
                "JOUR {}   |   {} crédits   |   {}/{} contrats actifs   |   {} livrés",
                campus.day,
                ui_kit::thousands(campus.credits),
                procurement.active().len(),
                campus.max_active_projects(),
                procurement.completed().len()
            )
        };
        if text.0 != value {
            text.0 = value;
        }
    }
}

/// Snapshot of the design context attached to every new campaign.
fn trial_context(campus: &CampusState, log: &RevisionLog, vehicle: &omega_rules::Vehicle) -> TrialContext {
    let head = log.head();
    let change_summary = head.map_or_else(
        || "aucune révision de conception enregistrée".to_string(),
        |revision| {
            let diff = missile::vehicle_diff(&revision.vehicle, vehicle);
            if diff == "no changes" {
                "aucun écart avec la révision".to_string()
            } else {
                diff
            }
        },
    );
    TrialContext {
        day: campus.day,
        revision_id: head.map(|revision| revision.id),
        revision_hash: head.map(|revision| revision.hash.clone()).unwrap_or_default(),
        revision_label: head.map_or_else(|| "non révisée".to_string(), |revision| revision.label.clone()),
        change_summary,
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

/// Actions that need resources `interface::handle_actions` does not have (revisions, workshop,
/// program and trial sessions).
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_extra_actions(
    buttons: Query<(&UiInteraction, &UiActionButton), Changed<UiInteraction>>,
    mut campus: ResMut<CampusState>,
    mut log: ResMut<RevisionLog>,
    mut missile: ResMut<MissileSession>,
    mut program: ResMut<MissileProgramSession>,
    mut trials: ResMut<TrialSession>,
    mut procurement: ResMut<Procurement>,
    mut workshop_input: ResMut<crate::workshop::WorkshopInput>,
) {
    for (interaction, button) in &buttons {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        match button.0.clone() {
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
            UiAction::BeginRevisionName(id) => {
                log.begin_naming(id);
                campus.message = "Type a revision name, then press Enter.".to_string();
            }
            UiAction::CommitRevisionName => {
                if let Some(id) = log.naming_target {
                    let draft = log.naming_draft.clone();
                    campus.message = match log.rename(id, &draft) {
                        Ok(()) => match log
                            .to_bytes()
                            .and_then(|bytes| crate::save::write(crate::save::SaveFile::Revisions, &bytes))
                        {
                            Ok(()) => format!("Renamed revision {id}"),
                            Err(error) => format!("Revision renamed but not saved: {error}"),
                        },
                        Err(error) => format!("Revision not renamed: {error}"),
                    };
                }
                log.cancel_naming();
            }
            UiAction::CancelRevisionName => {
                log.cancel_naming();
                campus.message = "Revision rename cancelled.".to_string();
            }
            UiAction::CompareRevision(id) => {
                let from = log.head().map(|head| head.id);
                campus.message = match from.map(|from| log.compare_revisions(from, id)) {
                    Some(Ok(diff)) => format!("Compared rev {from:?} to rev {id}: {diff}"),
                    Some(Err(error)) => error,
                    None => "No head revision to compare against.".to_string(),
                };
            }
            UiAction::ClearRevisionComparison => {
                log.clear_comparison();
                campus.message = "Revision comparison closed.".to_string();
            }
            UiAction::RestoreRevision(id) => {
                campus.message = match log.restore_target(id) {
                    Ok(revision) => {
                        missile.vehicle = revision.vehicle;
                        missile.selected = None;
                        missile.selection.clear();
                        missile.collapsed.clear();
                        let new_id = log.commit_vehicle(&missile.vehicle).ok();
                        if let Some(new_id) = new_id {
                            let _ = log.rename(new_id, &format!("Restored from {}", revision.label));
                        }
                        let log_saved = log
                            .to_bytes()
                            .and_then(|bytes| crate::save::write(crate::save::SaveFile::Revisions, &bytes));
                        let vehicle_saved = missile::persist_missile(&missile);
                        match (log_saved, vehicle_saved) {
                            (Ok(()), Ok(())) => format!(
                                "Restored revision {id} as revision {}.",
                                new_id.map_or_else(|| "-".to_string(), |id| id.to_string())
                            ),
                            (Err(error), _) => format!("Restored locally, revision log not saved: {error}"),
                            (_, Err(error)) => format!("Restored locally, vehicle not saved: {error}"),
                        }
                    }
                    Err(error) => error,
                };
            }
            UiAction::SelectMissileContract(id) => {
                campus.message = procurement.select(id);
            }
            UiAction::ReflyMissileContract => {
                let monte_carlo = campus.owned_capabilities().contains(trials::MONTE_CARLO);
                campus.message = match trials::apply_action(
                    &mut trials,
                    TrialAction::Run,
                    &missile.vehicle,
                    &program.program,
                    monte_carlo,
                ) {
                    Ok(_) => format!("Re-flew {} for the active contract(s)", missile.vehicle.name),
                    Err(error) => error,
                };
            }
            UiAction::Program(action) => match program.apply(action, &missile.vehicle) {
                Ok(message) => {
                    if !message.is_empty() {
                        campus.message = message;
                    }
                    if let Err(error) = program_editor::persist(&program) {
                        warn!("Could not save the missile program: {error}");
                        campus.message = format!("Could not save the missile program: {error}");
                    }
                    if program.library_dirty {
                        if let Err(error) = program_editor::persist_library(&program) {
                            warn!("Could not save the program library: {error}");
                            campus.message = format!("Could not save the program library: {error}");
                        }
                        program.library_dirty = false;
                    }
                }
                Err(message) => campus.message = message,
            },
            UiAction::Trial(action) => match action {
                TrialAction::ExportHtml => {
                    campus.message = match trial_export::export_html(
                        &trials,
                        &missile.vehicle,
                        &program.program,
                        &procurement,
                        &log,
                    ) {
                        Ok(path) => trial_export::export_message(&path),
                        Err(error) => error,
                    };
                }
                TrialAction::ExportCsv => {
                    campus.message = match trial_export::export_csv(&trials, &missile.vehicle, &program.program) {
                        Ok(path) => trial_export::export_message(&path),
                        Err(error) => error,
                    };
                }
                TrialAction::ExportPdf => {
                    campus.message =
                        match trial_export::export_pdf(&trials, &missile.vehicle, &program.program, &procurement, &log)
                        {
                            Ok(path) => trial_export::export_message(&path),
                            Err(error) => error,
                        };
                }
                TrialAction::InspectPart(id) => {
                    let owned = campus.owned_capabilities();
                    campus.message = friendly_capabilities(&missile.apply(MissileAction::Select { id }, &owned));
                    campus.view = CampusView::Missile;
                }
                TrialAction::InspectBlock { thread, block } => {
                    let path = vec![PathStep {
                        index: block,
                        branch: Branch::Body,
                    }];
                    match program.apply(ProgramAction::SelectPath { thread, path }, &missile.vehicle) {
                        Ok(_) => {
                            campus.message = "Bloc concerné sélectionné dans le programme".to_string();
                            campus.view = CampusView::Program;
                        }
                        Err(error) => campus.message = error,
                    }
                }
                action => {
                    let context = trial_context(&campus, &log, &missile.vehicle);
                    trials.set_context(context);
                    let monte_carlo = campus.owned_capabilities().contains(trials::MONTE_CARLO);
                    let annotate_compliance = matches!(&action, TrialAction::Run | TrialAction::Relaunch(_));
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
                    if let Err(error) = trials.persist_history() {
                        warn!("Could not save the trial history: {error}");
                        campus.message = format!("Historique non sauvegardé : {error}");
                    }
                    if annotate_compliance
                        && let Some(active) = procurement.selected()
                        && let Some(run) = &trials.report
                        && let Some(entry) = trials.history.entries().last()
                        && entry.input_hash == run.input_hash
                        && let Some(snapshot) = crate::lab::compliance_for_report(
                            Some(&active.offer),
                            &entry.vehicle,
                            &entry.program,
                            entry.seed,
                            &run.report,
                        )
                    {
                        trials.history.annotate_latest_compliance(snapshot);
                        trials.history_dirty = true;
                        if let Err(error) = trials.persist_history() {
                            warn!("Could not save the trial compliance: {error}");
                        }
                    }
                }
            },
            UiAction::Missile(action) => {
                if matches!(
                    &action,
                    MissileAction::ClearPaletteSearch | MissileAction::BeginRename { .. }
                ) {
                    workshop_input.search_active = false;
                }
                let owned = campus.owned_capabilities();
                let before = missile.vehicle.clone();
                let message = friendly_capabilities(&missile.apply(action, &owned));
                if !message.is_empty() {
                    campus.message = message;
                }
                if before != missile.vehicle
                    && let Err(error) = missile::persist_missile(&missile)
                {
                    warn!("Could not save the vehicle: {error}");
                }
                if missile.library_dirty {
                    match missile::persist_library(&missile) {
                        Ok(()) => missile.library_dirty = false,
                        Err(error) => warn!("Could not save the subassembly library: {error}"),
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod research_presentation_tests {
    use super::*;

    #[test]
    fn tiers_follow_prerequisites_and_previews_name_real_parts() {
        let tree = omega_rules::default_tree();
        let mut memo = HashMap::new();
        for tech in &tree {
            let tier = research_tier(&tree, &tech.id, &mut memo);
            for prereq in &tech.prereqs {
                assert!(
                    research_tier(&tree, prereq, &mut memo) < tier,
                    "{} must sit above {prereq}",
                    tech.id
                );
            }
            assert!(
                !research_unlocks(tech).is_empty(),
                "{} must advertise a real possibility",
                tech.id
            );
        }
        let wing = tree.iter().find(|tech| tech.id == "aerodynamics").unwrap();
        assert!(research_unlocks(wing).contains("WING"), "{}", research_unlocks(wing));
    }
}
