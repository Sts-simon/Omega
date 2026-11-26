//! Visual program editor: a pannable canvas of freely placed block cards, branch rows for the
//! control structures, multi-selection and debug playback.
//!
//! The board is rebuilt from the session whenever its signature changes, so the canvas only writes
//! persistent positions into `ProgramLayout`; the drag system moves the live nodes to keep the
//! gesture smooth between rebuilds.

use super::{CampusState, CampusView};
use crate::interface::{UiAction, UiActionButton};
use crate::program_editor::{self, Branch, MissileProgramSession, PathStep, ProgramAction, adjustable, issue_counts};
use crate::ui_kit::{self, ButtonTone, UiInteraction};
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use omega_rules::Vehicle;
use omega_rules::program::{Block, BlockKey};

const CARD_WIDTH: f32 = 224.0;
const CARD_STEP: f32 = 104.0;
const THREAD_STEP: f32 = 336.0;
const CANVAS_LEFT_GUTTER: f32 = 300.0;
const CANVAS_RIGHT_GUTTER: f32 = 350.0;

#[derive(Component)]
pub(crate) struct ProgramCanvas;

#[derive(Component, Clone, Copy)]
pub(crate) struct ProgramCard {
    key: BlockKey,
}

/// Absolute canvas item (card or connector) that must follow a canvas pan.
#[derive(Component)]
pub(crate) struct ProgramCanvasItem;

#[derive(Default)]
pub(crate) struct CanvasDrag {
    card: Option<BlockKey>,
    last: Vec2,
    panning: bool,
    moved: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlayStatus {
    Idle,
    Current,
    Done,
    Waiting,
    Anomaly,
}

pub(crate) fn build_program_canvas_column(
    parent: &mut ChildSpawnerCommands<'_>,
    vehicle: &Vehicle,
    session: &MissileProgramSession,
) {
    build_toolbar(parent, session);
    build_canvas(parent, session);
    build_inspector(parent, vehicle, session);
}

fn build_toolbar(parent: &mut ChildSpawnerCommands<'_>, session: &MissileProgramSession) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(ui_kit::PANEL),
            BorderColor::all(ui_kit::NEUTRAL),
        ))
        .with_children(|toolbar| {
            toolbar
                .spawn(Node {
                    width: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|title| {
                    ui_kit::label(title, "VISUAL PROGRAM", 12.0, ui_kit::ACCENT);
                    ui_kit::chip(
                        title,
                        format!("{:.0}%", session.program.layout.zoom * 100.0),
                        ui_kit::NEUTRAL,
                    );
                    if session.multi_select {
                        ui_kit::chip(title, format!("{} SELECTED", session.selection.len()), ui_kit::ACCENT);
                    }
                    if let Some(branch) = session.branch_focus {
                        ui_kit::chip(title, format!("INSERT INTO {}", branch.label()), ui_kit::WARN);
                    }
                    if session.debug.is_some() {
                        ui_kit::label(
                            title,
                            "CYAN CURRENT · GREEN DONE · GOLD WAIT · RED ANOMALY",
                            8.5,
                            ui_kit::TEXT_DIM,
                        );
                    }
                });
            toolbar
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(3.0),
                    flex_wrap: FlexWrap::Wrap,
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
                        "+ TÂCHE",
                        UiAction::Program(ProgramAction::AddThread),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(
                        row,
                        "- TÂCHE",
                        UiAction::Program(ProgramAction::RemoveThread),
                        ButtonTone::Danger,
                    );
                });
            toolbar
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(3.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    ui_kit::button_row_item(
                        row,
                        "ZOOM -",
                        UiAction::Program(ProgramAction::ZoomCanvas(-1)),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(
                        row,
                        "ZOOM +",
                        UiAction::Program(ProgramAction::ZoomCanvas(1)),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(
                        row,
                        "RESET VIEW",
                        UiAction::Program(ProgramAction::ResetCanvas),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(
                        row,
                        "ALIGN",
                        UiAction::Program(ProgramAction::Align),
                        ButtonTone::Secondary,
                    );
                    ui_kit::button_row_item(
                        row,
                        "GROUP",
                        UiAction::Program(ProgramAction::GroupSelection),
                        ButtonTone::Secondary,
                    );
                    ui_kit::button_row_item(
                        row,
                        "UNGROUP",
                        UiAction::Program(ProgramAction::ClearGroups),
                        ButtonTone::Ghost,
                    );
                });
            toolbar
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(3.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    ui_kit::button_row_item(
                        row,
                        if session.multi_select {
                            "MULTI: ON"
                        } else {
                            "MULTI: OFF"
                        },
                        UiAction::Program(ProgramAction::ToggleMultiSelect),
                        if session.multi_select {
                            ButtonTone::Primary
                        } else {
                            ButtonTone::Ghost
                        },
                    );
                    ui_kit::button_row_item(
                        row,
                        "COPY",
                        UiAction::Program(ProgramAction::CopySelection),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(
                        row,
                        "PASTE",
                        UiAction::Program(ProgramAction::PasteSelection),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(
                        row,
                        "DUPLICATE",
                        UiAction::Program(ProgramAction::DuplicateSelection),
                        ButtonTone::Secondary,
                    );
                    ui_kit::button_row_item(
                        row,
                        "DELETE",
                        UiAction::Program(if session.multi_select {
                            ProgramAction::RemoveSelection
                        } else {
                            ProgramAction::RemoveSelected
                        }),
                        ButtonTone::Danger,
                    );
                });
        });
}

fn build_canvas(parent: &mut ChildSpawnerCommands<'_>, session: &MissileProgramSession) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                min_height: Val::Px(240.0),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.010, 0.022, 0.032, 0.97)),
            BorderColor::all(ui_kit::NEUTRAL),
            ProgramCanvas,
        ))
        .with_children(|canvas| {
            let zoom = session.program.layout.zoom.clamp(0.5, 2.0);
            let width = (CARD_WIDTH * zoom).clamp(170.0, 300.0);
            let origin = Vec2::new(16.0 + session.program.layout.pan_x, 16.0 + session.program.layout.pan_y);
            let screen = |key: BlockKey| {
                let position = canvas_position(session, key);
                Vec2::new(origin.x + position.x * zoom, origin.y + position.y * zoom)
            };

            // Connections are drawn first so cards always sit above them.
            for (thread, blocks) in session.program.threads.iter().enumerate() {
                for index in 0..blocks.len().saturating_sub(1) {
                    let from = screen(BlockKey { thread, block: index });
                    let to = screen(BlockKey {
                        thread,
                        block: index + 1,
                    });
                    let start = Vec2::new(from.x + width * 0.5, from.y + 76.0);
                    let end = Vec2::new(to.x + width * 0.5, to.y);
                    let mid_y = (start.y + end.y) * 0.5;
                    spawn_line(canvas, start.x, start.y, 1.0, mid_y - start.y, ui_kit::ACCENT);
                    spawn_line(
                        canvas,
                        start.x.min(end.x),
                        mid_y,
                        (end.x - start.x).abs().max(1.0),
                        1.0,
                        ui_kit::ACCENT,
                    );
                    spawn_line(canvas, end.x, mid_y, 1.0, end.y - mid_y, ui_kit::ACCENT);
                }
            }

            // Group membership is visible as a coloured label and a purple card edge.
            for group in &session.program.layout.groups {
                let Some(first) = group.members.first() else {
                    continue;
                };
                if first.thread >= session.program.threads.len()
                    || first.block >= session.program.threads[first.thread].len()
                {
                    continue;
                }
                let position = screen(*first);
                canvas
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(position.x),
                            top: Val::Px((position.y - 20.0).max(2.0)),
                            padding: UiRect {
                                left: Val::Px(6.0),
                                right: Val::Px(6.0),
                                top: Val::Px(1.0),
                                bottom: Val::Px(1.0),
                            },
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            ..default()
                        },
                        BackgroundColor(ui_kit::PURPLE),
                        ProgramCanvasItem,
                    ))
                    .with_children(|chip| ui_kit::label(chip, group.name.clone(), 8.5, ui_kit::TEXT));
            }

            let mut cards = 0;
            for (thread, blocks) in session.program.threads.iter().enumerate() {
                for (index, block) in blocks.iter().enumerate() {
                    let key = BlockKey { thread, block: index };
                    let position = screen(key);
                    let path = vec![PathStep {
                        index,
                        branch: Branch::Body,
                    }];
                    spawn_card(canvas, session, key, position, width, block, path, &mut cards);
                }
            }
            if cards == 0 {
                ui_kit::label(
                    canvas,
                    "Empty canvas. Add blocks from the library; drag them freely, right-drag to pan, wheel to zoom.",
                    10.5,
                    ui_kit::TEXT_DIM,
                );
            }
        });
}

fn spawn_line(canvas: &mut ChildSpawnerCommands<'_>, left: f32, top: f32, width: f32, height: f32, color: Color) {
    canvas.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(left),
            top: Val::Px(top),
            width: Val::Px(width.max(1.0)),
            height: Val::Px(height.max(1.0)),
            ..default()
        },
        BackgroundColor(color),
        ProgramCanvasItem,
    ));
}

#[allow(clippy::too_many_arguments)]
fn spawn_card(
    canvas: &mut ChildSpawnerCommands<'_>,
    session: &MissileProgramSession,
    key: BlockKey,
    position: Vec2,
    width: f32,
    block: &Block,
    path: Vec<PathStep>,
    cards: &mut usize,
) {
    *cards += 1;
    let selected = session.path.first().is_some_and(|step| step.index == key.block) && session.thread == key.thread;
    let multi_selected = session.selection.contains(&key);
    let status = play_status(session, key);
    let group = session.program.layout.group_of(key);
    let (border, thickness) = match status {
        PlayStatus::Anomaly => (ui_kit::BAD, 3.0),
        PlayStatus::Current => (ui_kit::ACCENT, 3.0),
        PlayStatus::Done => (ui_kit::GOOD, 2.0),
        PlayStatus::Waiting => (ui_kit::WARN, 2.0),
        PlayStatus::Idle if selected => (ui_kit::ACCENT, 3.0),
        PlayStatus::Idle if multi_selected => (ui_kit::ACCENT, 2.0),
        PlayStatus::Idle if group.is_some() => (ui_kit::PURPLE, 2.0),
        PlayStatus::Idle => (ui_kit::NEUTRAL, 1.0),
    };
    let background = if selected || multi_selected || status == PlayStatus::Current {
        ui_kit::CARD_SELECTED
    } else {
        ui_kit::CARD
    };
    let action = if session.multi_select {
        ProgramAction::ToggleSelection {
            thread: key.thread,
            block: key.block,
        }
    } else {
        ProgramAction::SelectPath {
            thread: key.thread,
            path: path.clone(),
        }
    };
    canvas
        .spawn((
            UiInteraction::default(),
            FocusPolicy::Block,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(position.x),
                top: Val::Px(position.y),
                width: Val::Px(width),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(7.0)),
                border: UiRect::all(Val::Px(thickness)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(background),
            BorderColor::all(border),
            ProgramCard { key },
            ProgramCanvasItem,
            UiActionButton(UiAction::Program(action)),
        ))
        .with_children(|card| {
            ui_kit::label(
                card,
                format!("T{} #{}  {}", key.thread + 1, key.block + 1, block.label()),
                10.0,
                ui_kit::TEXT,
            );
            if session.multi_select && session.selection.contains(&key) {
                ui_kit::label(card, "SELECTED", 8.0, ui_kit::ACCENT);
            }
            match block {
                Block::If {
                    condition,
                    then_blocks,
                    else_blocks,
                } => {
                    ui_kit::label(card, condition.label(), 9.0, ui_kit::ACCENT);
                    branch_section(card, session, key.thread, &path, Branch::Then, then_blocks);
                    branch_section(card, session, key.thread, &path, Branch::Else, else_blocks);
                }
                Block::WaitUntil { condition, timeout_s } => {
                    ui_kit::label(card, condition.label(), 9.0, ui_kit::ACCENT);
                    ui_kit::label(card, format!("timeout {timeout_s:.0} s"), 8.5, ui_kit::TEXT_DIM);
                }
                Block::Repeat { times, body } => {
                    ui_kit::label(card, format!("{times} pass(es)"), 9.0, ui_kit::ACCENT);
                    branch_section(card, session, key.thread, &path, Branch::Body, body);
                }
                Block::RepeatWhile {
                    condition,
                    timeout_s,
                    body,
                } => {
                    ui_kit::label(card, condition.label(), 9.0, ui_kit::ACCENT);
                    ui_kit::label(card, format!("max {timeout_s:.0} s"), 8.5, ui_kit::TEXT_DIM);
                    branch_section(card, session, key.thread, &path, Branch::Body, body);
                }
                Block::Call { routine } => {
                    ui_kit::label(card, format!("-> {routine}"), 9.0, ui_kit::PURPLE);
                }
                _ => {
                    ui_kit::label(card, block.category().label(), 8.5, ui_kit::TEXT_DIM);
                }
            }
        });
}

fn branch_section(
    card: &mut ChildSpawnerCommands<'_>,
    session: &MissileProgramSession,
    thread: usize,
    parent_path: &[PathStep],
    branch: Branch,
    blocks: &[Block],
) {
    let focused = session.branch_focus == Some(branch);
    let tone = if focused {
        ButtonTone::Primary
    } else {
        ButtonTone::Ghost
    };
    card.spawn(Node {
        width: Val::Percent(100.0),
        column_gap: Val::Px(3.0),
        ..default()
    })
    .with_children(|row| {
        ui_kit::button_row_item(
            row,
            format!("{} ({})", branch.label(), blocks.len()),
            UiAction::Program(ProgramAction::FocusBranch(branch)),
            tone,
        );
    });
    for (index, block) in blocks.iter().enumerate() {
        let mut path = parent_path.to_vec();
        path.push(PathStep { index, branch });
        let selected = session.path == path;
        ui_kit::button(
            card,
            format!("{}. {}", index + 1, block.label()),
            UiAction::Program(ProgramAction::SelectPath { thread, path }),
            if selected {
                ButtonTone::Primary
            } else {
                ButtonTone::Ghost
            },
            Val::Percent(100.0),
        );
    }
}

fn build_inspector(parent: &mut ChildSpawnerCommands<'_>, vehicle: &Vehicle, session: &MissileProgramSession) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                max_height: Val::Px(216.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                overflow: Overflow::clip_y(),
                ..default()
            },
            BackgroundColor(ui_kit::PANEL),
            BorderColor::all(ui_kit::NEUTRAL),
        ))
        .with_children(|panel| {
            let issues = session.issues(vehicle);
            let (errors, warnings) = issue_counts(&issues);
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    ui_kit::label(row, "SELECTED", 11.0, ui_kit::ACCENT);
                    ui_kit::chip(
                        row,
                        if errors == 0 {
                            "VALID".to_string()
                        } else {
                            format!("{errors} ERR")
                        },
                        if errors == 0 { ui_kit::GOOD } else { ui_kit::BAD },
                    );
                    ui_kit::chip(row, format!("{warnings} WARN"), if warnings == 0 { ui_kit::NEUTRAL } else { ui_kit::WARN });
                    if !session.selection.is_empty() {
                        ui_kit::chip(row, format!("{} MULTI", session.selection.len()), ui_kit::ACCENT);
                    }
                });
            match session.selected_block() {
                None => ui_kit::label(
                    panel,
                    "Click a card or a nested branch row to inspect it. Drag a card to move it; drag the background to pan.",
                    9.5,
                    ui_kit::TEXT_DIM,
                ),
                Some(block) => {
                    let place = if session.path.len() > 1 {
                        "nested"
                    } else {
                        "thread"
                    };
                    ui_kit::label(
                        panel,
                        format!("{place} block · {}", block.label()),
                        10.0,
                        ui_kit::TEXT,
                    );
                    ui_kit::label(panel, block.description(), 8.5, ui_kit::TEXT_DIM);
                    panel
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            column_gap: Val::Px(3.0),
                            flex_wrap: FlexWrap::Wrap,
                            ..default()
                        })
                        .with_children(|row| {
                            if adjustable(block) {
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
                            }
                            ui_kit::button_row_item(
                                row,
                                "FLIP",
                                UiAction::Program(ProgramAction::Flip),
                                ButtonTone::Ghost,
                            );
                            ui_kit::button_row_item(
                                row,
                                "CONDITION",
                                UiAction::Program(ProgramAction::CycleCondition),
                                ButtonTone::Ghost,
                            );
                            ui_kit::button_row_item(
                                row,
                                "ROUTINE",
                                UiAction::Program(ProgramAction::CycleCall),
                                ButtonTone::Ghost,
                            );
                        });
                    if matches!(block, Block::WaitUntil { .. } | Block::RepeatWhile { .. }) {
                        panel
                            .spawn(Node {
                                width: Val::Percent(100.0),
                                column_gap: Val::Px(3.0),
                                flex_wrap: FlexWrap::Wrap,
                                ..default()
                            })
                            .with_children(|row| {
                                ui_kit::button_row_item(
                                    row,
                                    "TIMEOUT -",
                                    UiAction::Program(ProgramAction::AdjustTimeout(-1)),
                                    ButtonTone::Ghost,
                                );
                                ui_kit::button_row_item(
                                    row,
                                    "TIMEOUT +",
                                    UiAction::Program(ProgramAction::AdjustTimeout(1)),
                                    ButtonTone::Ghost,
                                );
                            });
                    }
                    panel
                        .spawn(Node {
                            width: Val::Percent(100.0),
                            column_gap: Val::Px(3.0),
                            flex_wrap: FlexWrap::Wrap,
                            ..default()
                        })
                        .with_children(|row| {
                            ui_kit::button_row_item(
                                row,
                                "MOVE UP",
                                UiAction::Program(ProgramAction::MoveUp),
                                ButtonTone::Ghost,
                            );
                            ui_kit::button_row_item(
                                row,
                                "MOVE DOWN",
                                UiAction::Program(ProgramAction::MoveDown),
                                ButtonTone::Ghost,
                            );
                            ui_kit::button_row_item(
                                row,
                                "REMOVE",
                                UiAction::Program(ProgramAction::RemoveSelection),
                                ButtonTone::Danger,
                            );
                        });
                }
            }
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(3.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    ui_kit::label(row, "ROUTINES", 9.5, ui_kit::ACCENT);
                    ui_kit::button_row_item(
                        row,
                        "CAPTURE SELECTION",
                        UiAction::Program(ProgramAction::CreateRoutine),
                        ButtonTone::Secondary,
                    );
                    for (index, routine) in session.program.routines.iter().enumerate() {
                        ui_kit::button_row_item(
                            row,
                            format!("CALL {} ({})", routine.name, routine.blocks.len()),
                            UiAction::Program(ProgramAction::AppendCall(index)),
                            ButtonTone::Ghost,
                        );
                        ui_kit::button_row_item(
                            row,
                            "X",
                            UiAction::Program(ProgramAction::DeleteRoutine(index)),
                            ButtonTone::Danger,
                        );
                    }
                });
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(3.0),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|row| {
                    ui_kit::label(row, "DEBUG", 9.5, ui_kit::ACCENT);
                    ui_kit::button_row_item(
                        row,
                        "RUN",
                        UiAction::Program(ProgramAction::Debug),
                        ButtonTone::Secondary,
                    );
                    ui_kit::button_row_item(
                        row,
                        "< STEP",
                        UiAction::Program(ProgramAction::DebugStep(-1)),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(
                        row,
                        if session.debug_playing { "PAUSE" } else { "PLAY" },
                        UiAction::Program(ProgramAction::ToggleDebugPlay),
                        ButtonTone::Primary,
                    );
                    ui_kit::button_row_item(
                        row,
                        "STEP >",
                        UiAction::Program(ProgramAction::DebugStep(1)),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(
                        row,
                        format!("{}x", session.debug_speed_multiplier()),
                        UiAction::Program(ProgramAction::CycleDebugSpeed),
                        ButtonTone::Ghost,
                    );
                    ui_kit::button_row_item(
                        row,
                        "RESET",
                        UiAction::Program(ProgramAction::ResetDebug),
                        ButtonTone::Ghost,
                    );
                });
            if let Some(report) = &session.debug {
                if let Some(entry) = report.trace.get(session.debug_cursor) {
                    ui_kit::label(
                        panel,
                        format!(
                            "{}  t={:.1}s  T{} #{}  mode M{}  flags {}",
                            if entry.anomaly { "ANOMALY" } else { "CURRENT" },
                            entry.t_s,
                            entry.thread + 1,
                            entry.block + 1,
                            entry.mode,
                            entry
                                .flags
                                .iter()
                                .take(4)
                                .enumerate()
                                .map(|(index, value)| format!("F{index}={}", u8::from(*value)))
                                .collect::<Vec<_>>()
                                .join(" ")
                        ),
                        8.5,
                        if entry.anomaly { ui_kit::BAD } else { ui_kit::ACCENT },
                    );
                }
                ui_kit::label(
                    panel,
                    format!(
                        "step {} / {}  ·  {:.1}s flight  ·  final M{}  ·  {} anomalie(s)",
                        session.debug_cursor + 1,
                        report.trace.len(),
                        report.flight_time_s,
                        report.final_mode,
                        report.anomalies.len()
                    ),
                    8.5,
                    ui_kit::TEXT_DIM,
                );
                ui_kit::label(
                    panel,
                    format!(
                        "final flags {}",
                        report
                            .final_flags
                            .iter()
                            .enumerate()
                            .map(|(index, value)| format!("F{index}={}", u8::from(*value)))
                            .collect::<Vec<_>>()
                            .join(" ")
                    ),
                    8.5,
                    ui_kit::TEXT_DIM,
                );
            } else {
                ui_kit::label(
                    panel,
                    "Run a trace: cards turn green when done, yellow while waiting and red on an anomaly.",
                    8.5,
                    ui_kit::TEXT_DIM,
                );
            }
        });
}

fn canvas_position(session: &MissileProgramSession, key: BlockKey) -> Vec2 {
    session.program.layout.position(key).map_or_else(
        || {
            Vec2::new(
                24.0 + key.thread as f32 * THREAD_STEP,
                28.0 + key.block as f32 * CARD_STEP,
            )
        },
        |(x, y)| Vec2::new(x, y),
    )
}

fn play_status(session: &MissileProgramSession, key: BlockKey) -> PlayStatus {
    let Some(report) = &session.debug else {
        return PlayStatus::Idle;
    };
    let mut found = false;
    let mut status = PlayStatus::Waiting;
    for (index, entry) in report.trace.iter().enumerate() {
        if entry.thread != key.thread || entry.block != key.block {
            continue;
        }
        found = true;
        if entry.anomaly {
            return PlayStatus::Anomaly;
        }
        if index == session.debug_cursor {
            status = PlayStatus::Current;
        } else if index < session.debug_cursor && status != PlayStatus::Current {
            status = PlayStatus::Done;
        }
    }
    if found { status } else { PlayStatus::Waiting }
}

/// Drag cards or pan the canvas; positions are persisted once, on release.
pub(crate) fn drag_program_canvas(
    window: Single<&Window>,
    mouse: Res<ButtonInput<MouseButton>>,
    campus: Res<CampusState>,
    mut session: ResMut<MissileProgramSession>,
    canvases: Query<(), With<ProgramCanvas>>,
    cards: Query<(&UiInteraction, &ProgramCard)>,
    mut nodes: Query<(&ProgramCard, &mut Node)>,
    mut items: Query<&mut Node, (With<ProgramCanvasItem>, Without<ProgramCard>)>,
    mut drag: Local<CanvasDrag>,
) {
    if campus.view != CampusView::Program || canvases.is_empty() {
        return;
    }
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let in_canvas = cursor.x > CANVAS_LEFT_GUTTER
        && cursor.x < window.width() - CANVAS_RIGHT_GUTTER
        && cursor.y > 110.0
        && cursor.y < window.height() - 210.0;
    if mouse.just_pressed(MouseButton::Left) || mouse.just_pressed(MouseButton::Right) {
        let hovered = cards
            .iter()
            .find(|(interaction, _)| !matches!(interaction, UiInteraction::None))
            .map(|(_, card)| card.key);
        if let Some(key) = hovered {
            drag.card = Some(key);
            drag.panning = false;
        } else if in_canvas {
            drag.card = None;
            drag.panning = true;
        }
        drag.last = cursor;
        drag.moved = false;
    }
    let held = if drag.panning {
        mouse.pressed(MouseButton::Left) || mouse.pressed(MouseButton::Right)
    } else {
        mouse.pressed(MouseButton::Left)
    };
    if held {
        let delta = cursor - drag.last;
        if delta.length_squared() > 0.04 {
            drag.moved = true;
            let zoom = session.program.layout.zoom.clamp(0.5, 2.0);
            if let Some(key) = drag.card {
                let position = session.program.layout.position(key).unwrap_or_else(|| {
                    let fallback = canvas_position(&session, key);
                    (fallback.x, fallback.y)
                });
                session
                    .program
                    .layout
                    .set_position(key, position.0 + delta.x / zoom, position.1 + delta.y / zoom);
                for (card, mut node) in &mut nodes {
                    if card.key == key {
                        at_px(&mut node.left, delta.x);
                        at_px(&mut node.top, delta.y);
                        break;
                    }
                }
            } else if drag.panning {
                session.program.layout.pan_x += delta.x;
                session.program.layout.pan_y += delta.y;
                for (_, mut node) in &mut nodes {
                    at_px(&mut node.left, delta.x);
                    at_px(&mut node.top, delta.y);
                }
                for mut node in &mut items {
                    at_px(&mut node.left, delta.x);
                    at_px(&mut node.top, delta.y);
                }
            }
            drag.last = cursor;
        }
    }
    if mouse.just_released(MouseButton::Left) || mouse.just_released(MouseButton::Right) {
        if drag.moved
            && let Err(error) = program_editor::persist(&session)
        {
            warn!("Could not save the canvas layout: {error}");
        }
        drag.card = None;
        drag.panning = false;
        drag.moved = false;
    }
}

fn at_px(value: &mut Val, delta: f32) {
    let current = match *value {
        Val::Px(px) => px,
        _ => 0.0,
    };
    *value = Val::Px(current + delta);
}

/// Wheel over the canvas changes its zoom, leaving palette and library scrolling untouched.
pub(crate) fn zoom_program_canvas(
    window: Single<&Window>,
    wheel: Res<AccumulatedMouseScroll>,
    campus: Res<CampusState>,
    mut session: ResMut<MissileProgramSession>,
) {
    if campus.view != CampusView::Program || wheel.delta.y == 0.0 {
        return;
    }
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if cursor.x < CANVAS_LEFT_GUTTER
        || cursor.x > window.width() - CANVAS_RIGHT_GUTTER
        || cursor.y > window.height() - 210.0
    {
        return;
    }
    let next = (session.program.layout.zoom + wheel.delta.y.signum() * 0.1).clamp(0.5, 2.0);
    if (next - session.program.layout.zoom).abs() > f32::EPSILON {
        session.program.layout.zoom = next;
        if let Err(error) = program_editor::persist(&session) {
            warn!("Could not save the canvas zoom: {error}");
        }
    }
}

/// Animate debug playback at the selected speed; `debug_cursor` drives card colours.
pub(crate) fn advance_program_debug(
    time: Res<Time>,
    campus: Res<CampusState>,
    mut session: ResMut<MissileProgramSession>,
    mut carry: Local<f32>,
) {
    if campus.view != CampusView::Program || !session.debug_playing {
        *carry = 0.0;
        return;
    }
    let Some(report) = &session.debug else {
        session.debug_playing = false;
        return;
    };
    if report.trace.is_empty() {
        session.debug_playing = false;
        return;
    }
    let len = report.trace.len();
    *carry += time.delta_secs() * session.debug_speed_multiplier() * 1.4;
    while *carry >= 1.0 {
        *carry -= 1.0;
        if session.debug_cursor + 1 >= len {
            session.debug_cursor = len - 1;
            session.debug_playing = false;
            break;
        }
        session.debug_cursor += 1;
    }
}
