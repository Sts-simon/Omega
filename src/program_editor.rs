//! Missile program editor: parallel threads of generic blocks (`omega-rules::program`), limited by
//! the flight computer installed in the vehicle and by the hardware the vehicle actually carries.
//!
//! The editor groups the block library by the rules' eight categories, draws the connections
//! between consecutive blocks, keeps a named library of reusable programs, and can run the seeded
//! flight with a block trace to debug the program.
//!
//! Everything here is fictional and abstract: `Lock target` only asks the flight test for a seeded
//! success roll, it is not guidance, tracking or a seeker model.

use crate::save::{self, SaveFile};
use bevy::prelude::*;
use omega_rules::Vehicle;
use omega_rules::flight::{FlightConfig, FlightReport, simulate};
use omega_rules::program::{
    Block, BlockCategory, BlockKey, Condition, MAX_CONDITION_TIMEOUT_S, Program, ProgramGroup, ProgramIssue,
    ProgramLibrary, Routine, program_issues,
};
use std::collections::BTreeSet;

/// A block the editor can append, independent of the parameter values in the program itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlockKind {
    WaitSpeed,
    WaitPressure,
    WaitFlag,
    ToggleFlag,
    SetFlag,
    Wait,
    Ignite,
    Separate,
    Throttle,
    HoldSpeed,
    HoldPressure,
    LockTarget,
    SetMode,
    WaitMode,
    Mark,
    If,
    WaitUntil,
    Repeat,
    RepeatWhile,
    Break,
    Call,
}

impl BlockKind {
    /// Library order, one run per category.
    pub const ALL: [Self; 21] = [
        Self::WaitSpeed,
        Self::WaitPressure,
        Self::WaitFlag,
        Self::ToggleFlag,
        Self::SetFlag,
        Self::Wait,
        Self::Ignite,
        Self::Separate,
        Self::Throttle,
        Self::HoldSpeed,
        Self::HoldPressure,
        Self::LockTarget,
        Self::SetMode,
        Self::WaitMode,
        Self::Mark,
        Self::If,
        Self::WaitUntil,
        Self::Repeat,
        Self::RepeatWhile,
        Self::Break,
        Self::Call,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::WaitSpeed => "WAIT SPEED",
            Self::WaitPressure => "WAIT PRESSURE",
            Self::WaitFlag => "WAIT FLAG",
            Self::ToggleFlag => "TOGGLE FLAG",
            Self::SetFlag => "SET FLAG",
            Self::Wait => "WAIT",
            Self::Ignite => "IGNITE STAGE",
            Self::Separate => "SEPARATE STAGE",
            Self::Throttle => "THROTTLE",
            Self::HoldSpeed => "HOLD SPEED",
            Self::HoldPressure => "HOLD PRESSURE",
            Self::LockTarget => "LOCK TARGET",
            Self::SetMode => "SET MODE",
            Self::WaitMode => "WAIT MODE",
            Self::Mark => "MARK",
            Self::If => "IF / ELSE",
            Self::WaitUntil => "WAIT UNTIL",
            Self::Repeat => "REPEAT N",
            Self::RepeatWhile => "REPEAT WHILE",
            Self::Break => "BREAK",
            Self::Call => "CALL ROUTINE",
        }
    }

    pub fn category(self) -> BlockCategory {
        self.default_block().category()
    }

    /// What the palette inserts; every value is valid and can then be tuned in the editor.
    pub fn default_block(self) -> Block {
        match self {
            Self::WaitSpeed => Block::WaitSpeed {
                speed_ms: 250.0,
                timeout_s: 20.0,
            },
            Self::WaitPressure => Block::WaitPressure {
                hpa: 900.0,
                timeout_s: 20.0,
            },
            Self::WaitFlag => Block::WaitFlag {
                flag: 0,
                value: true,
                timeout_s: 10.0,
            },
            Self::ToggleFlag => Block::ToggleFlag { flag: 0 },
            Self::SetFlag => Block::SetFlag { flag: 0, value: true },
            Self::Wait => Block::Wait { seconds: 5.0 },
            Self::Ignite => Block::IgniteStage { stage: 0 },
            Self::Separate => Block::SeparateStage { stage: 0 },
            Self::Throttle => Block::SetThrottle { fraction: 1.0 },
            Self::HoldSpeed => Block::HoldSpeed { speed_ms: 250.0 },
            Self::HoldPressure => Block::HoldPressure { hpa: 900.0 },
            Self::LockTarget => Block::LockTarget,
            Self::SetMode => Block::SetMode { mode: 1 },
            Self::WaitMode => Block::WaitMode {
                mode: 1,
                timeout_s: 10.0,
            },
            Self::Mark => Block::Mark { mark: 0 },
            Self::If => Block::If {
                condition: Condition::SpeedAtLeast { speed_ms: 250.0 },
                then_blocks: Vec::new(),
                else_blocks: Vec::new(),
            },
            Self::WaitUntil => Block::WaitUntil {
                condition: Condition::SpeedAtLeast { speed_ms: 250.0 },
                timeout_s: 20.0,
            },
            Self::Repeat => Block::Repeat {
                times: 3,
                body: Vec::new(),
            },
            Self::RepeatWhile => Block::RepeatWhile {
                condition: Condition::FlagIs { flag: 0, value: true },
                timeout_s: 30.0,
                body: Vec::new(),
            },
            Self::Break => Block::Break,
            Self::Call => Block::Call { routine: String::new() },
        }
    }

    pub fn available(self, vehicle: &Vehicle) -> bool {
        vehicle.available_blocks().contains(self.default_block().kind())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Branch {
    Then,
    Else,
    Body,
}

impl Branch {
    pub fn label(self) -> &'static str {
        match self {
            Self::Then => "THEN",
            Self::Else => "ELSE",
            Self::Body => "BODY",
        }
    }
}

/// One step from a thread down into a nested control body. `branch` is the branch of the parent
/// that contains this block; the first step always uses `Body`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PathStep {
    pub(crate) index: usize,
    pub(crate) branch: Branch,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ProgramAction {
    Append(BlockKind),
    /// Insert after the selected block, which is how a connection is created in the middle of a
    /// thread.
    Insert(BlockKind),
    RemoveSelected,
    SelectThread(usize),
    AddThread,
    RemoveThread,
    MoveUp,
    MoveDown,
    /// `-1` or `+1` on the selected block's main value.
    Adjust(i8),
    AdjustTimeout(i8),
    /// `-1` or `+1` on the selected block's boolean value (flags only).
    Flip,
    Clear,
    /// Save the current program under a free default name.
    SaveCopy,
    LoadSaved(usize),
    DeleteSaved(usize),
    /// Fly the current program with a block trace and keep the report for the debugger panel.
    Debug,
    SelectPath {
        thread: usize,
        path: Vec<PathStep>,
    },
    ToggleSelection {
        thread: usize,
        block: usize,
    },
    CopySelection,
    PasteSelection,
    DuplicateSelection,
    RemoveSelection,
    Align,
    GroupSelection,
    ClearGroups,
    FocusBranch(Branch),
    CycleCondition,
    CreateRoutine,
    DeleteRoutine(usize),
    AppendCall(usize),
    CycleCall,
    DebugStep(i8),
    ToggleDebugPlay,
    CycleDebugSpeed,
    ResetDebug,
    ZoomCanvas(i8),
    ResetCanvas,
    ToggleMultiSelect,
}

#[derive(Resource)]
pub(crate) struct MissileProgramSession {
    pub program: Program,
    pub thread: usize,
    pub block: Option<usize>,
    /// Path to the selected block; empty when nothing is selected, otherwise at least one step.
    pub(crate) path: Vec<PathStep>,
    /// Top-level multi-selection used by the canvas and bulk actions.
    pub(crate) selection: BTreeSet<BlockKey>,
    pub(crate) clipboard: Vec<Block>,
    /// Where a palette insert goes when a control block is selected.
    pub(crate) branch_focus: Option<Branch>,
    pub(crate) multi_select: bool,
    /// Vertical scroll of the block list, kept across board rebuilds so a long program does not
    /// jump back to its first block after every edit.
    pub(crate) scroll: f32,
    pub(crate) library: ProgramLibrary,
    /// Last debug run; cleared by the next edit so the panel cannot show a stale timeline.
    pub(crate) debug: Option<FlightReport>,
    /// Set when the library changed, so the board system writes the library file once.
    pub(crate) library_dirty: bool,
    /// Debug playback cursor (index into the traced blocks) and playback state.
    pub(crate) debug_cursor: usize,
    pub(crate) debug_playing: bool,
    /// 0 = 0.5x, 1 = 1x, 2 = 2x, 3 = 4x.
    pub(crate) debug_speed: u8,
}

impl Default for MissileProgramSession {
    fn default() -> Self {
        Self {
            program: Program {
                threads: vec![Vec::new()],
                ..Program::default()
            },
            thread: 0,
            block: None,
            path: Vec::new(),
            selection: BTreeSet::new(),
            clipboard: Vec::new(),
            branch_focus: None,
            multi_select: false,
            scroll: 0.0,
            library: ProgramLibrary::default(),
            debug: None,
            library_dirty: false,
            debug_cursor: 0,
            debug_playing: false,
            debug_speed: 1,
        }
    }
}

fn round(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn is_control_block(block: &Block) -> bool {
    matches!(
        block,
        Block::If { .. } | Block::Repeat { .. } | Block::RepeatWhile { .. }
    )
}

fn branch_body(block: &Block, branch: Branch) -> Option<&[Block]> {
    match (block, branch) {
        (Block::If { then_blocks, .. }, Branch::Then) => Some(then_blocks),
        (Block::If { else_blocks, .. }, Branch::Else) => Some(else_blocks),
        (Block::Repeat { body, .. }, Branch::Body) => Some(body),
        (Block::RepeatWhile { body, .. }, Branch::Body) => Some(body),
        _ => None,
    }
}

fn branch_body_mut(block: &mut Block, branch: Branch) -> Option<&mut Vec<Block>> {
    match (block, branch) {
        (Block::If { then_blocks, .. }, Branch::Then) => Some(then_blocks),
        (Block::If { else_blocks, .. }, Branch::Else) => Some(else_blocks),
        (Block::Repeat { body, .. }, Branch::Body) => Some(body),
        (Block::RepeatWhile { body, .. }, Branch::Body) => Some(body),
        _ => None,
    }
}

fn selected_in<'a>(blocks: &'a [Block], path: &[PathStep]) -> Option<&'a Block> {
    let mut body = blocks;
    let mut selected = None;
    for (depth, step) in path.iter().enumerate() {
        if depth > 0 {
            body = branch_body(selected?, step.branch)?;
        }
        selected = body.get(step.index);
    }
    selected
}

fn body_mut<'a>(body: &'a mut Vec<Block>, path: &[PathStep]) -> Option<&'a mut Vec<Block>> {
    if path.len() <= 1 {
        return Some(body);
    }
    let parent = body.get_mut(path[0].index)?;
    let child = branch_body_mut(parent, path[1].branch)?;
    body_mut(child, &path[1..])
}

fn cycle(value: u8, direction: i8, count: u8) -> u8 {
    if direction < 0 {
        (value + count - 1) % count
    } else {
        (value + 1) % count
    }
}

fn adjusted_condition(condition: Condition, direction: i8) -> Condition {
    let step = f64::from(direction);
    match condition {
        Condition::SpeedAtLeast { speed_ms } => Condition::SpeedAtLeast {
            speed_ms: round((speed_ms + 25.0 * step).clamp(10.0, 1_500.0)),
        },
        Condition::PressureAtMost { hpa } => Condition::PressureAtMost {
            hpa: round((hpa + 25.0 * step).clamp(50.0, 1_013.25)),
        },
        Condition::FlagIs { flag, value } => Condition::FlagIs {
            flag: cycle(flag, direction, omega_rules::FLAGS as u8),
            value,
        },
        Condition::ModeIs { mode } => Condition::ModeIs {
            mode: cycle(mode, direction, omega_rules::MODES),
        },
    }
}

fn cycled_condition(condition: Condition) -> Condition {
    match condition {
        Condition::SpeedAtLeast { .. } => Condition::PressureAtMost { hpa: 900.0 },
        Condition::PressureAtMost { .. } => Condition::FlagIs { flag: 0, value: true },
        Condition::FlagIs { .. } => Condition::ModeIs { mode: 1 },
        Condition::ModeIs { .. } => Condition::SpeedAtLeast { speed_ms: 250.0 },
    }
}

/// Adjust one value of a block, clamped to the range `validate_program` accepts.
fn adjusted(block: Block, direction: i8, stages: usize) -> Option<Block> {
    let step = f64::from(direction);
    let stage = |value: usize| -> usize {
        let last = stages.saturating_sub(1);
        if direction < 0 {
            value.saturating_sub(1).min(last)
        } else {
            (value + 1).min(last)
        }
    };
    match block {
        Block::Wait { seconds } => Some(Block::Wait {
            seconds: round((seconds + 5.0 * step).clamp(0.0, 3_600.0)),
        }),
        Block::IgniteStage { stage: current } => Some(Block::IgniteStage { stage: stage(current) }),
        Block::SeparateStage { stage: current } => Some(Block::SeparateStage { stage: stage(current) }),
        Block::SetThrottle { fraction } => Some(Block::SetThrottle {
            fraction: round((fraction + 0.1 * step).clamp(0.0, 1.0)),
        }),
        Block::HoldSpeed { speed_ms } => Some(Block::HoldSpeed {
            speed_ms: round((speed_ms + 25.0 * step).clamp(10.0, 1_500.0)),
        }),
        Block::HoldPressure { hpa } => Some(Block::HoldPressure {
            hpa: round((hpa + 25.0 * step).clamp(50.0, 1_013.25)),
        }),
        Block::WaitSpeed { speed_ms, timeout_s } => Some(Block::WaitSpeed {
            speed_ms: round((speed_ms + 25.0 * step).clamp(10.0, 1_500.0)),
            timeout_s,
        }),
        Block::WaitPressure { hpa, timeout_s } => Some(Block::WaitPressure {
            hpa: round((hpa + 25.0 * step).clamp(50.0, 1_013.25)),
            timeout_s,
        }),
        Block::WaitFlag { flag, value, timeout_s } => Some(Block::WaitFlag {
            flag: cycle(flag, direction, omega_rules::FLAGS as u8),
            value,
            timeout_s,
        }),
        Block::ToggleFlag { flag } => Some(Block::ToggleFlag {
            flag: cycle(flag, direction, omega_rules::FLAGS as u8),
        }),
        Block::SetFlag { flag, value } => Some(Block::SetFlag {
            flag: cycle(flag, direction, omega_rules::FLAGS as u8),
            value,
        }),
        Block::SetMode { mode } => Some(Block::SetMode {
            mode: cycle(mode, direction, omega_rules::MODES),
        }),
        Block::WaitMode { mode, timeout_s } => Some(Block::WaitMode {
            mode: cycle(mode, direction, omega_rules::MODES),
            timeout_s,
        }),
        Block::Mark { mark } => Some(Block::Mark {
            mark: cycle(mark, direction, omega_rules::MARKS),
        }),
        Block::If {
            condition,
            then_blocks,
            else_blocks,
        } => Some(Block::If {
            condition: adjusted_condition(condition, direction),
            then_blocks,
            else_blocks,
        }),
        Block::WaitUntil { condition, timeout_s } => Some(Block::WaitUntil {
            condition: adjusted_condition(condition, direction),
            timeout_s,
        }),
        Block::Repeat { times, body } => Some(Block::Repeat {
            times: if direction < 0 {
                times.saturating_sub(1)
            } else {
                times.saturating_add(1)
            },
            body,
        }),
        Block::RepeatWhile {
            condition,
            timeout_s,
            body,
        } => Some(Block::RepeatWhile {
            condition: adjusted_condition(condition, direction),
            timeout_s,
            body,
        }),
        Block::Break => None,
        Block::Call { routine } => Some(Block::Call { routine }),
        Block::LockTarget => None,
    }
}

/// Flip the boolean value of the selected block, for the two flag blocks that carry one.
fn flipped(block: Block) -> Option<Block> {
    match block {
        Block::WaitFlag { flag, value, timeout_s } => Some(Block::WaitFlag {
            flag,
            value: !value,
            timeout_s,
        }),
        Block::SetFlag { flag, value } => Some(Block::SetFlag { flag, value: !value }),
        Block::If {
            condition: Condition::FlagIs { flag, value },
            then_blocks,
            else_blocks,
        } => Some(Block::If {
            condition: Condition::FlagIs { flag, value: !value },
            then_blocks,
            else_blocks,
        }),
        Block::WaitUntil {
            condition: Condition::FlagIs { flag, value },
            timeout_s,
        } => Some(Block::WaitUntil {
            condition: Condition::FlagIs { flag, value: !value },
            timeout_s,
        }),
        Block::RepeatWhile {
            condition: Condition::FlagIs { flag, value },
            timeout_s,
            body,
        } => Some(Block::RepeatWhile {
            condition: Condition::FlagIs { flag, value: !value },
            timeout_s,
            body,
        }),
        _ => None,
    }
}

/// `false` for blocks whose only parameter is their position in the thread.
pub fn adjustable(block: &Block) -> bool {
    !matches!(block, Block::LockTarget | Block::Break | Block::Call { .. })
}

impl MissileProgramSession {
    pub fn threads_used(&self) -> usize {
        self.program.threads.len().max(1)
    }

    pub fn blocks_used(&self) -> usize {
        self.program.block_count()
    }

    pub fn limits(&self, vehicle: &Vehicle) -> Option<(usize, usize)> {
        vehicle.computer().map(|class| (class.threads(), class.ram_blocks()))
    }

    /// Errors and warnings for the whole program, each placed in its thread and block.
    pub fn issues(&self, vehicle: &Vehicle) -> Vec<ProgramIssue> {
        if self.program.threads.iter().all(Vec::is_empty) {
            return vec![ProgramIssue {
                severity: omega_rules::IssueSeverity::Error,
                thread: 0,
                block: None,
                message: "The program is empty: add blocks to a thread.".to_string(),
            }];
        }
        program_issues(&self.program, vehicle)
    }

    pub fn selected_block(&self) -> Option<&Block> {
        if self.path.is_empty() {
            return self.program.threads.get(self.thread)?.get(self.block?);
        }
        let blocks = self.program.threads.get(self.thread)?;
        selected_in(blocks, &self.path)
    }

    fn select_path(&mut self, thread: usize, path: Vec<PathStep>) -> Result<(), String> {
        if path.is_empty() {
            self.block = None;
            self.path.clear();
            return Ok(());
        }
        if thread >= self.program.threads.len() {
            return Err("No such thread".to_string());
        }
        self.thread = thread;
        let blocks = self
            .program
            .threads
            .get(thread)
            .ok_or_else(|| "No such thread".to_string())?;
        if selected_in(blocks, &path).is_none() {
            return Err("No such block".to_string());
        }
        self.block = Some(path[0].index);
        self.path = path;
        self.branch_focus = None;
        Ok(())
    }

    /// Mutable body containing the selected block. `None` when the selection is stale.
    fn selected_container_mut(&mut self) -> Option<(&mut Vec<Block>, usize)> {
        let thread = self.thread;
        let path = self.path.clone();
        if path.is_empty() {
            return None;
        }
        let blocks = self.program.threads.get_mut(thread)?;
        let index = path.last()?.index;
        let container = body_mut(blocks, &path)?;
        Some((container, index))
    }

    fn current_thread_mut(&mut self) -> &mut Vec<Block> {
        // A program loaded from disk may carry no thread at all; keep the editor usable instead of
        // indexing an empty list.
        if self.program.threads.is_empty() {
            self.program.threads.push(Vec::new());
        }
        if self.thread >= self.program.threads.len() {
            self.thread = 0;
        }
        &mut self.program.threads[self.thread]
    }

    fn insert_at(&mut self, kind: BlockKind, index: Option<usize>, vehicle: &Vehicle) -> Result<String, String> {
        let Some((_, block_limit)) = self.limits(vehicle) else {
            return Err("This vehicle has no flight computer. Add one in the workshop first.".to_string());
        };
        if !kind.available(vehicle) {
            return Err(format!(
                "'{}' needs hardware this vehicle does not carry.",
                kind.label()
            ));
        }
        if self.blocks_used() + 1 > block_limit {
            return Err(format!(
                "The computer holds {block_limit} blocks and {} are already used.",
                self.blocks_used()
            ));
        }
        if self.program.threads.is_empty() {
            self.program.threads.push(Vec::new());
        }
        if self.thread >= self.program.threads.len() {
            self.thread = 0;
        }
        let mut block = kind.default_block();
        if let Block::Call { routine } = &mut block
            && routine.is_empty()
            && let Some(first) = self.program.routines.first()
        {
            *routine = first.name.clone();
        }
        let path = self.path.clone();
        let branch_target = self
            .branch_focus
            .filter(|_| self.selected_block().is_some_and(is_control_block));
        let (body_path, selected_branch) = if let Some(branch) = branch_target {
            let mut body_path = path.clone();
            body_path.push(PathStep { index: 0, branch });
            (body_path, branch)
        } else if path.len() > 1 {
            (path.clone(), path.last().map_or(Branch::Body, |step| step.branch))
        } else {
            (vec![], Branch::Body)
        };
        let thread_index = self.thread;
        let top_level = body_path.is_empty();
        let blocks = self
            .program
            .threads
            .get_mut(thread_index)
            .ok_or_else(|| "No such thread".to_string())?;
        let container = if body_path.is_empty() {
            blocks
        } else {
            body_mut(blocks, &body_path).ok_or_else(|| "The selected branch no longer exists.".to_string())?
        };
        let index = if branch_target.is_some() {
            container.len()
        } else {
            index.map_or(container.len(), |index| {
                if index >= container.len() {
                    container.len()
                } else {
                    index + 1
                }
            })
        };
        container.insert(index, block);
        if top_level {
            self.shift_layout(thread_index, index, 1);
        }
        // Keep the new block selected, pointing into the body that now contains it.
        let mut new_path = if branch_target.is_some() {
            let mut path = path.clone();
            path.push(PathStep {
                index,
                branch: selected_branch,
            });
            path
        } else if path.len() > 1 {
            let mut path = path.clone();
            if let Some(last) = path.last_mut() {
                last.index = index;
            }
            path
        } else {
            vec![PathStep {
                index,
                branch: Branch::Body,
            }]
        };
        if new_path.is_empty() {
            new_path.push(PathStep {
                index,
                branch: Branch::Body,
            });
        }
        self.block = Some(new_path[0].index);
        self.path = new_path;
        self.selection = [BlockKey {
            thread: thread_index,
            block: self.block.unwrap_or(index),
        }]
        .into_iter()
        .collect();
        Ok(format!(
            "Added '{}' to thread {} position {}",
            kind.label(),
            self.thread + 1,
            index + 1
        ))
    }

    /// Apply an editor action. `Ok` means the program changed and must be saved; `Err` is a refusal
    /// shown to the player.
    pub fn apply(&mut self, action: ProgramAction, vehicle: &Vehicle) -> Result<String, String> {
        if !matches!(
            &action,
            ProgramAction::Debug
                | ProgramAction::DebugStep(_)
                | ProgramAction::ToggleDebugPlay
                | ProgramAction::CycleDebugSpeed
                | ProgramAction::ResetDebug
        ) {
            self.debug = None;
        }
        let Some((thread_limit, _)) = self.limits(vehicle) else {
            return Err("This vehicle has no flight computer. Add one in the workshop first.".to_string());
        };
        match action {
            ProgramAction::Append(kind) => self.insert_at(kind, None, vehicle),
            ProgramAction::Insert(kind) => {
                let after = self.path.last().map(|step| step.index).or(self.block);
                self.insert_at(kind, after, vehicle)
            }
            ProgramAction::AddThread => {
                if self.program.threads.len() + 1 > thread_limit {
                    return Err(format!("The computer runs {thread_limit} thread(s) in parallel."));
                }
                self.program.threads.push(Vec::new());
                self.thread = self.program.threads.len() - 1;
                self.block = None;
                Ok(format!("Thread {} added", self.thread + 1))
            }
            ProgramAction::RemoveThread => {
                if self.program.threads.len() <= 1 {
                    return Err("A program keeps at least one thread.".to_string());
                }
                let index = self.thread.min(self.program.threads.len() - 1);
                self.program.threads.remove(index);
                self.thread = index.min(self.program.threads.len() - 1);
                self.block = None;
                self.path.clear();
                self.selection.clear();
                self.branch_focus = None;
                self.program
                    .layout
                    .positions
                    .retain(|position| position.thread != index);
                for position in &mut self.program.layout.positions {
                    if position.thread > index {
                        position.thread -= 1;
                    }
                }
                for group in &mut self.program.layout.groups {
                    group.members.retain(|member| member.thread != index);
                    for member in &mut group.members {
                        if member.thread > index {
                            member.thread -= 1;
                        }
                    }
                }
                self.program.layout.groups.retain(|group| !group.members.is_empty());
                Ok(format!("Thread {} removed", index + 1))
            }
            ProgramAction::SelectThread(index) => {
                if index >= self.program.threads.len() {
                    return Err("No such thread".to_string());
                }
                self.scroll = 0.0;
                self.thread = index;
                self.block = None;
                self.path.clear();
                self.selection.clear();
                self.branch_focus = None;
                Ok(String::new())
            }
            ProgramAction::SelectPath { thread, path } => {
                self.selection.clear();
                if let Some(step) = path.first() {
                    self.selection.insert(BlockKey {
                        thread,
                        block: step.index,
                    });
                }
                self.select_path(thread, path)?;
                Ok(String::new())
            }
            ProgramAction::ToggleSelection { thread, block } => {
                if self.program.threads.get(thread).is_none_or(|rows| block >= rows.len()) {
                    return Err("No such block".to_string());
                }
                self.thread = thread;
                let key = BlockKey { thread, block };
                if !self.selection.remove(&key) {
                    self.selection.insert(key);
                }
                self.block = self.selection.iter().next_back().map(|key| key.block);
                if let Some(key) = self.selection.iter().next_back() {
                    if key.thread == self.thread {
                        self.path = vec![PathStep {
                            index: key.block,
                            branch: Branch::Body,
                        }];
                    }
                } else {
                    self.path.clear();
                }
                Ok(String::new())
            }
            ProgramAction::RemoveSelected => {
                let Some((container, index)) = self.selected_container_mut() else {
                    return Err("Select a block first.".to_string());
                };
                if index >= container.len() {
                    return Err("Select a block first.".to_string());
                }
                container.remove(index);
                self.after_removal(index);
                Ok("Block removed".to_string())
            }
            ProgramAction::MoveUp | ProgramAction::MoveDown => {
                let Some(index) = self.block else {
                    return Err("Select a block first.".to_string());
                };
                let Some((container, current)) = self.selected_container_mut() else {
                    return Err("Select a block first.".to_string());
                };
                let target = if action == ProgramAction::MoveUp {
                    current.checked_sub(1)
                } else {
                    Some(current + 1)
                };
                let Some(target) = target.filter(|t| *t < container.len()) else {
                    return Ok(String::new());
                };
                container.swap(current, target);
                if self.path.len() == 1 {
                    self.swap_layout(self.thread, current, target);
                }
                if let Some(last) = self.path.last_mut() {
                    last.index = target;
                }
                self.block = Some(self.path.first().map_or(index, |step| step.index));
                Ok(String::new())
            }
            ProgramAction::Adjust(direction) => {
                let stages = vehicle.stage_count();
                let Some((container, index)) = self.selected_container_mut() else {
                    return Err("Select a block first.".to_string());
                };
                let Some(block) = container.get(index).cloned() else {
                    return Err("Select a block first.".to_string());
                };
                match adjusted(block, direction, stages) {
                    Some(changed) => {
                        container[index] = changed;
                        Ok(String::new())
                    }
                    None => Err("This block has no adjustable value.".to_string()),
                }
            }
            ProgramAction::AdjustTimeout(direction) => {
                let Some((container, index)) = self.selected_container_mut() else {
                    return Err("Select a condition block first.".to_string());
                };
                let Some(block) = container.get_mut(index) else {
                    return Err("Select a condition block first.".to_string());
                };
                let step = 5.0 * f64::from(direction);
                match block {
                    Block::WaitUntil { timeout_s, .. } => {
                        *timeout_s = round((*timeout_s + step).clamp(0.0, MAX_CONDITION_TIMEOUT_S));
                        Ok(format!("Timeout {timeout_s:.0} s"))
                    }
                    Block::RepeatWhile { timeout_s, .. } => {
                        *timeout_s = round((*timeout_s + step).clamp(0.0, MAX_CONDITION_TIMEOUT_S));
                        Ok(format!("Timeout {timeout_s:.0} s"))
                    }
                    _ => Err("This block has no timeout to tune.".to_string()),
                }
            }
            ProgramAction::Flip => {
                let Some((container, index)) = self.selected_container_mut() else {
                    return Err("Select a block first.".to_string());
                };
                let Some(block) = container.get(index).cloned() else {
                    return Err("Select a block first.".to_string());
                };
                match flipped(block) {
                    Some(changed) => {
                        container[index] = changed;
                        Ok(String::new())
                    }
                    None => Err("Only flag conditions carry a boolean value.".to_string()),
                }
            }
            ProgramAction::Clear => {
                self.program = Program {
                    threads: vec![Vec::new()],
                    ..Program::default()
                };
                self.thread = 0;
                self.block = None;
                self.path.clear();
                self.selection.clear();
                self.branch_focus = None;
                self.multi_select = false;
                self.scroll = 0.0;
                self.debug = None;
                self.debug_cursor = 0;
                self.debug_playing = false;
                Ok("Program cleared".to_string())
            }
            ProgramAction::SaveCopy => {
                let name = self.library.next_name();
                let message = self.library.save(name, self.program.clone())?;
                self.library_dirty = true;
                Ok(message)
            }
            ProgramAction::LoadSaved(index) => {
                let Some(saved) = self.library.get(index).cloned() else {
                    return Err("No such saved program.".to_string());
                };
                self.program = saved.program;
                self.thread = 0;
                self.block = None;
                self.path.clear();
                self.selection.clear();
                self.branch_focus = None;
                self.scroll = 0.0;
                self.debug = None;
                Ok(format!("Loaded saved program '{}'", saved.name))
            }
            ProgramAction::DeleteSaved(index) => {
                let message = self.library.remove(index)?;
                self.library_dirty = true;
                Ok(message)
            }
            ProgramAction::CopySelection => {
                self.clipboard.clear();
                let keys: Vec<BlockKey> = if self.selection.is_empty() {
                    self.block
                        .map(|block| BlockKey {
                            thread: self.thread,
                            block,
                        })
                        .into_iter()
                        .collect()
                } else {
                    self.selection.iter().copied().collect()
                };
                for key in &keys {
                    if let Some(block) = self
                        .program
                        .threads
                        .get(key.thread)
                        .and_then(|rows| rows.get(key.block))
                    {
                        self.clipboard.push(block.clone());
                    }
                }
                if self.clipboard.is_empty() {
                    return Err("Select a block first.".to_string());
                }
                Ok(format!("{} block(s) copied", self.clipboard.len()))
            }
            ProgramAction::PasteSelection => {
                if self.clipboard.is_empty() {
                    return Err("Clipboard is empty.".to_string());
                }
                if self.blocks_used() + self.clipboard.len() > vehicle.computer().map_or(0, |c| c.ram_blocks()) {
                    return Err("Not enough computer memory for the clipboard.".to_string());
                }
                let count = self.clipboard.len();
                let thread = self.thread;
                let clipboard = self.clipboard.clone();
                let rows = self.current_thread_mut();
                let start = rows.len();
                rows.extend(clipboard);
                self.selection = (start..start + count).map(|block| BlockKey { thread, block }).collect();
                self.block = Some(start);
                self.path = vec![PathStep {
                    index: start,
                    branch: Branch::Body,
                }];
                Ok(format!("{count} block(s) pasted"))
            }
            ProgramAction::DuplicateSelection => {
                if self.path.len() > 1 {
                    if self.blocks_used() + 1 > vehicle.computer().map_or(0, |c| c.ram_blocks()) {
                        return Err("Not enough computer memory.".to_string());
                    }
                    let Some((container, index)) = self.selected_container_mut() else {
                        return Err("Select a block first.".to_string());
                    };
                    let Some(block) = container.get(index).cloned() else {
                        return Err("Select a block first.".to_string());
                    };
                    container.insert(index + 1, block);
                    if let Some(last) = self.path.last_mut() {
                        last.index = index + 1;
                    }
                    return Ok("Block duplicated".to_string());
                }
                let keys: Vec<BlockKey> = if self.selection.is_empty() {
                    self.block
                        .map(|block| BlockKey {
                            thread: self.thread,
                            block,
                        })
                        .into_iter()
                        .collect()
                } else {
                    self.selection.iter().copied().collect()
                };
                if keys.is_empty() {
                    return Err("Select a block first.".to_string());
                }
                if self.blocks_used() + keys.len() > vehicle.computer().map_or(0, |c| c.ram_blocks()) {
                    return Err("Not enough computer memory to duplicate the selection.".to_string());
                }
                let thread = self.thread;
                let mut created: Vec<BlockKey> = Vec::new();
                for key in keys.iter().rev() {
                    if key.thread != thread {
                        continue;
                    }
                    let Some(block) = self
                        .program
                        .threads
                        .get(thread)
                        .and_then(|rows| rows.get(key.block))
                        .cloned()
                    else {
                        continue;
                    };
                    let rows = self.current_thread_mut();
                    rows.insert(key.block + 1, block);
                    for created_key in &mut created {
                        if created_key.block > key.block {
                            created_key.block += 1;
                        }
                    }
                    self.shift_layout(thread, key.block + 1, 1);
                    created.push(BlockKey {
                        thread,
                        block: key.block + 1,
                    });
                }
                self.selection = created.into_iter().collect();
                self.block = self.selection.iter().next().map(|key| key.block);
                if let Some(block) = self.block {
                    self.path = vec![PathStep {
                        index: block,
                        branch: Branch::Body,
                    }];
                }
                Ok(format!("{} block(s) duplicated", self.selection.len()))
            }
            ProgramAction::RemoveSelection => {
                if self.selection.len() <= 1 {
                    return self.remove_selected();
                }
                let keys: Vec<BlockKey> = self.selection.iter().copied().collect();
                let mut ordered = keys.clone();
                ordered.sort_by(|a, b| b.thread.cmp(&a.thread).then(b.block.cmp(&a.block)));
                for key in &ordered {
                    if let Some(rows) = self.program.threads.get_mut(key.thread)
                        && key.block < rows.len()
                    {
                        rows.remove(key.block);
                    }
                }
                // Removal shifts the following blocks; move their saved positions with them so the
                // canvas does not leave a hole where the deleted card used to be.
                self.program.layout.positions.retain_mut(|position| {
                    let removed = keys
                        .iter()
                        .filter(|key| key.thread == position.thread)
                        .collect::<Vec<_>>();
                    if removed.iter().any(|key| key.block == position.block) {
                        return false;
                    }
                    position.block -= removed.iter().filter(|key| key.block < position.block).count();
                    true
                });
                for group in &mut self.program.layout.groups {
                    group.members.retain_mut(|member| {
                        if keys.contains(member) {
                            return false;
                        }
                        member.block -= keys
                            .iter()
                            .filter(|key| key.thread == member.thread && key.block < member.block)
                            .count();
                        true
                    });
                }
                self.program.layout.groups.retain(|group| !group.members.is_empty());
                self.selection.clear();
                self.block = None;
                self.path.clear();
                Ok(format!("{} block(s) removed", keys.len()))
            }
            ProgramAction::Align => {
                for thread in 0..self.program.threads.len() {
                    self.arrange_thread(thread);
                }
                Ok("Blocks aligned".to_string())
            }
            ProgramAction::GroupSelection => {
                let members: Vec<BlockKey> = if self.selection.is_empty() {
                    self.block
                        .map(|block| BlockKey {
                            thread: self.thread,
                            block,
                        })
                        .into_iter()
                        .collect()
                } else {
                    self.selection.iter().copied().collect()
                };
                if members.is_empty() {
                    return Err("Select a block first.".to_string());
                }
                let index = self.program.layout.groups.len() + 1;
                self.program.layout.groups.push(ProgramGroup {
                    name: format!("Group {index}"),
                    members,
                });
                Ok(format!("Group {index} created"))
            }
            ProgramAction::ClearGroups => {
                self.program.layout.groups.clear();
                Ok("Groups cleared".to_string())
            }
            ProgramAction::FocusBranch(branch) => {
                if !self.selected_block().is_some_and(is_control_block) {
                    return Err("Select an IF, REPEAT or REPEAT WHILE block first.".to_string());
                }
                self.branch_focus = Some(branch);
                Ok(format!("Palette now appends to {} ", branch.label()))
            }
            ProgramAction::CycleCondition => {
                let Some((container, index)) = self.selected_container_mut() else {
                    return Err("Select a control block first.".to_string());
                };
                let Some(block) = container.get_mut(index) else {
                    return Err("Select a control block first.".to_string());
                };
                match block {
                    Block::If { condition, .. }
                    | Block::WaitUntil { condition, .. }
                    | Block::RepeatWhile { condition, .. } => {
                        *condition = cycled_condition(*condition);
                        Ok("Condition type changed".to_string())
                    }
                    _ => Err("This block has no condition.".to_string()),
                }
            }
            ProgramAction::CreateRoutine => {
                let members: Vec<BlockKey> = if self.selection.is_empty() {
                    self.block
                        .map(|block| BlockKey {
                            thread: self.thread,
                            block,
                        })
                        .into_iter()
                        .collect()
                } else {
                    self.selection.iter().copied().collect()
                };
                let blocks: Vec<Block> = members
                    .iter()
                    .filter_map(|key| {
                        self.program
                            .threads
                            .get(key.thread)
                            .and_then(|rows| rows.get(key.block))
                            .cloned()
                    })
                    .collect();
                if blocks.is_empty() {
                    return Err("Select blocks to capture as a routine.".to_string());
                }
                if self.blocks_used() + blocks.len() > vehicle.computer().map_or(0, |computer| computer.ram_blocks()) {
                    return Err("Not enough computer memory for this routine.".to_string());
                }
                let index = self.program.routines.len() + 1;
                let mut name = format!("Routine {index}");
                let mut suffix = 1;
                while self.program.routines.iter().any(|routine| routine.name == name) {
                    name = format!("Routine {index}-{suffix}");
                    suffix += 1;
                }
                self.program.routines.push(Routine {
                    name: name.clone(),
                    blocks,
                });
                Ok(format!("Routine '{name}' created"))
            }
            ProgramAction::DeleteRoutine(index) => {
                if index >= self.program.routines.len() {
                    return Err("No such routine".to_string());
                }
                let removed = self.program.routines.remove(index);
                Ok(format!("Routine '{}' removed", removed.name))
            }
            ProgramAction::AppendCall(index) => {
                let Some(routine) = self.program.routines.get(index).map(|routine| routine.name.clone()) else {
                    return Err("No such routine".to_string());
                };
                let message = self.insert_at(BlockKind::Call, None, vehicle)?;
                let Some((container, block_index)) = self.selected_container_mut() else {
                    return Err(message);
                };
                if let Some(Block::Call { routine: called }) = container.get_mut(block_index) {
                    *called = routine;
                }
                Ok(message)
            }
            ProgramAction::CycleCall => {
                if self.program.routines.is_empty() {
                    return Err("Create a routine first.".to_string());
                }
                let names: Vec<String> = self
                    .program
                    .routines
                    .iter()
                    .map(|routine| routine.name.clone())
                    .collect();
                let Some((container, index)) = self.selected_container_mut() else {
                    return Err("Select a CALL block first.".to_string());
                };
                let Some(Block::Call { routine }) = container.get_mut(index) else {
                    return Err("Select a CALL block first.".to_string());
                };
                let current = names.iter().position(|name| name == routine).unwrap_or(0);
                *routine = names[(current + 1) % names.len()].clone();
                Ok(format!("Call routine '{routine}'"))
            }
            ProgramAction::DebugStep(direction) => {
                let Some(report) = &self.debug else {
                    return Err("Run a trace first.".to_string());
                };
                self.debug_playing = false;
                if direction < 0 {
                    self.debug_cursor = self.debug_cursor.saturating_sub(1);
                } else {
                    self.debug_cursor = (self.debug_cursor + 1).min(report.trace.len().saturating_sub(1));
                }
                Ok(format!("Step {} / {}", self.debug_cursor + 1, report.trace.len()))
            }
            ProgramAction::ToggleDebugPlay => {
                if self.debug.is_none() {
                    return Err("Run a trace first.".to_string());
                }
                self.debug_playing = !self.debug_playing;
                Ok(if self.debug_playing {
                    "Playback running"
                } else {
                    "Playback paused"
                }
                .to_string())
            }
            ProgramAction::CycleDebugSpeed => {
                self.debug_speed = (self.debug_speed + 1) % 4;
                Ok(format!("Playback speed {}", self.debug_speed_multiplier()))
            }
            ProgramAction::ResetDebug => {
                self.debug_cursor = 0;
                self.debug_playing = false;
                Ok("Playback reset".to_string())
            }
            ProgramAction::ZoomCanvas(direction) => {
                let zoom = (self.program.layout.zoom + 0.1 * f32::from(direction)).clamp(0.5, 2.0);
                self.program.layout.zoom = zoom;
                Ok(format!("Canvas zoom {:.0}%", zoom * 100.0))
            }
            ProgramAction::ResetCanvas => {
                self.program.layout.zoom = 1.0;
                self.program.layout.pan_x = 0.0;
                self.program.layout.pan_y = 0.0;
                Ok("Canvas view reset".to_string())
            }
            ProgramAction::ToggleMultiSelect => {
                self.multi_select = !self.multi_select;
                if !self.multi_select {
                    self.selection.clear();
                }
                Ok(if self.multi_select {
                    "Multi-selection on: click blocks to add or remove them".to_string()
                } else {
                    "Multi-selection off".to_string()
                })
            }
            ProgramAction::Debug => self.debug(vehicle),
        }
    }

    /// Fly the current program once with a block trace. Failures are the normal validation issues,
    /// so the panel explains exactly what to fix before a trial.
    fn remove_selected(&mut self) -> Result<String, String> {
        let Some((container, index)) = self.selected_container_mut() else {
            return Err("Select a block first.".to_string());
        };
        if index >= container.len() {
            return Err("Select a block first.".to_string());
        }
        container.remove(index);
        self.after_removal(index);
        Ok("Block removed".to_string())
    }

    fn after_removal(&mut self, index: usize) {
        if self.path.len() == 1 {
            let len = self.program.threads.get(self.thread).map_or(0, Vec::len);
            if len == 0 {
                self.block = None;
                self.path.clear();
            } else {
                let next = index.min(len - 1);
                self.block = Some(next);
                self.path[0].index = next;
            }
            for position in &mut self.program.layout.positions {
                if position.thread == self.thread && position.block > index {
                    position.block -= 1;
                }
            }
            for group in &mut self.program.layout.groups {
                group
                    .members
                    .retain(|member| !(member.thread == self.thread && member.block == index));
                for member in &mut group.members {
                    if member.thread == self.thread && member.block > index {
                        member.block -= 1;
                    }
                }
            }
            self.program.layout.groups.retain(|group| !group.members.is_empty());
        } else {
            while !self.path.is_empty() {
                let Some(rows) = self.program.threads.get(self.thread) else {
                    break;
                };
                if selected_in(rows, &self.path).is_some() {
                    break;
                }
                self.path.pop();
            }
            self.block = self.path.first().map(|step| step.index);
        }
        self.selection.clear();
        if let Some(block) = self.block {
            self.selection.insert(BlockKey {
                thread: self.thread,
                block,
            });
        }
    }

    /// Move saved top-level positions when a block is inserted or removed before them.
    fn shift_layout(&mut self, thread: usize, from: usize, delta: isize) {
        for position in &mut self.program.layout.positions {
            if position.thread == thread && position.block >= from {
                position.block = position.block.saturating_add_signed(delta);
            }
        }
        for group in &mut self.program.layout.groups {
            for member in &mut group.members {
                if member.thread == thread && member.block >= from {
                    member.block = member.block.saturating_add_signed(delta);
                }
            }
        }
    }

    fn swap_layout(&mut self, thread: usize, a: usize, b: usize) {
        let left = self.program.layout.position(BlockKey { thread, block: a });
        let right = self.program.layout.position(BlockKey { thread, block: b });
        if let Some((x, y)) = right {
            self.program.layout.set_position(BlockKey { thread, block: a }, x, y);
        }
        if let Some((x, y)) = left {
            self.program.layout.set_position(BlockKey { thread, block: b }, x, y);
        }
    }

    /// Lay the selected thread's top-level blocks on a regular vertical rail.
    fn arrange_thread(&mut self, thread: usize) {
        let len = self.program.threads.get(thread).map_or(0, Vec::len);
        for block in 0..len {
            self.program.layout.set_position(
                BlockKey { thread, block },
                60.0 + thread as f32 * 380.0,
                110.0 + block as f32 * 92.0,
            );
        }
        self.program
            .layout
            .positions
            .retain(|position| position.thread != thread || position.block < len);
    }

    pub fn debug_speed_multiplier(&self) -> f32 {
        [0.5, 1.0, 2.0, 4.0][self.debug_speed.min(3) as usize]
    }

    fn debug(&mut self, vehicle: &Vehicle) -> Result<String, String> {
        if self.program.threads.iter().all(Vec::is_empty) {
            return Err("The program is empty: add blocks to a thread.".to_string());
        }
        let config = FlightConfig {
            trace: true,
            ..FlightConfig::default()
        };
        let report = simulate(vehicle, &self.program, config).map_err(|issues| issues.join(" | "))?;
        let executed = report.trace.len();
        self.debug = Some(report);
        self.debug_cursor = 0;
        self.debug_playing = true;
        Ok(format!("Debug run: {executed} block(s) executed"))
    }

    /// Stable content hash, ready to bind trial evidence to this exact program.
    pub fn content_hash(&self) -> Result<omega_data::ContentHash, String> {
        omega_data::hash_serializable(&self.program.semantic_copy())
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(&self.program).map_err(|error| error.to_string())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let program: Program = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        let mut session = Self {
            program,
            thread: 0,
            block: None,
            scroll: 0.0,
            ..Self::default()
        };
        session.debug = None;
        Ok(session)
    }
}

pub(crate) fn library_bytes(session: &MissileProgramSession) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(&session.library).map_err(|error| error.to_string())
}

pub(crate) fn persist(session: &MissileProgramSession) -> Result<(), String> {
    session
        .to_bytes()
        .and_then(|bytes| save::write(SaveFile::MissileProgram, &bytes))
}

pub(crate) fn persist_library(session: &MissileProgramSession) -> Result<(), String> {
    library_bytes(session).and_then(|bytes| save::write(SaveFile::ProgramLibrary, &bytes))
}

impl MissileProgramSession {
    pub(crate) fn load_active(&mut self) {
        *self = Self::default();
        match save::read(SaveFile::MissileProgram) {
            Ok(Some(bytes)) => match Self::from_bytes(&bytes) {
                Ok(loaded) => *self = loaded,
                Err(error) => warn!("Could not read the saved missile program: {error}"),
            },
            Ok(None) => {}
            Err(error) => warn!("Could not read the saved missile program: {error}"),
        }
        match save::read(SaveFile::ProgramLibrary) {
            Ok(Some(bytes)) => match serde_json::from_slice::<ProgramLibrary>(&bytes) {
                Ok(library) => self.library = library,
                Err(error) => warn!("Could not read the saved program library: {error}"),
            },
            Ok(None) => {}
            Err(error) => warn!("Could not read the saved program library: {error}"),
        }
    }
}

pub(crate) fn load_program(mut session: ResMut<MissileProgramSession>) {
    session.load_active();
}

/// The highest severity found in a list, for the panel chip.
pub(crate) fn issue_counts(issues: &[ProgramIssue]) -> (usize, usize) {
    let errors = issues
        .iter()
        .filter(|issue| issue.severity == omega_rules::IssueSeverity::Error)
        .count();
    (errors, issues.len() - errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_rules::program::validate_program;
    use omega_rules::{ComputerClass, Part, PartSpec, SensorKind, Stage};

    fn vehicle(computer: ComputerClass, sensor: Option<SensorKind>) -> Vehicle {
        let mut parts = vec![Part {
            id: "computer".into(),
            spec: PartSpec::Computer { class: computer },
        }];
        if let Some(kind) = sensor {
            parts.push(Part {
                id: "sensor".into(),
                spec: PartSpec::Sensor { kind },
            });
        }
        Vehicle::from_stages("test", vec![Stage { parts }])
    }

    fn basic() -> Vehicle {
        vehicle(ComputerClass::Basic, Some(SensorKind::Pitot))
    }

    #[test]
    fn appending_blocks_respects_the_computer_memory() {
        let vehicle = basic(); // 8 blocks
        let mut session = MissileProgramSession::default();
        for _ in 0..8 {
            assert!(session.apply(ProgramAction::Append(BlockKind::Wait), &vehicle).is_ok());
        }
        let error = session
            .apply(ProgramAction::Append(BlockKind::Wait), &vehicle)
            .unwrap_err();
        assert!(error.contains("8 blocks"), "{error}");
        assert_eq!(session.blocks_used(), 8);
    }

    #[test]
    fn blocks_need_the_matching_hardware() {
        let vehicle = basic(); // pitot only, no radar
        let mut session = MissileProgramSession::default();
        let error = session
            .apply(ProgramAction::Append(BlockKind::LockTarget), &vehicle)
            .unwrap_err();
        assert!(error.contains("hardware"), "{error}");
        assert!(
            session
                .apply(ProgramAction::Append(BlockKind::HoldSpeed), &vehicle)
                .is_ok()
        );
        // A vehicle with no sensor cannot read an input at all.
        let blind = vehicle_of(ComputerClass::Advanced);
        let error = session
            .apply(ProgramAction::Append(BlockKind::WaitSpeed), &blind)
            .unwrap_err();
        assert!(error.contains("hardware"), "{error}");
        assert!(session.apply(ProgramAction::Append(BlockKind::Mark), &blind).is_ok());
    }

    #[test]
    fn a_vehicle_without_a_computer_cannot_be_programmed() {
        let vehicle = Vehicle {
            name: "empty".into(),
            parts: Vec::new(),
        };
        let mut session = MissileProgramSession::default();
        let error = session.apply(ProgramAction::AddThread, &vehicle).unwrap_err();
        assert!(error.contains("no flight computer"), "{error}");
    }

    #[test]
    fn thread_count_is_limited_and_never_empty() {
        let vehicle = basic(); // basic computer: one thread
        let mut session = MissileProgramSession::default();
        let error = session.apply(ProgramAction::AddThread, &vehicle).unwrap_err();
        assert!(error.contains("1 thread"), "{error}");
        let advanced = vehicle_of(ComputerClass::Advanced);
        assert!(session.apply(ProgramAction::AddThread, &advanced).is_ok());
        assert_eq!(session.program.threads.len(), 2);
        assert!(session.apply(ProgramAction::RemoveThread, &advanced).is_ok());
        let error = session.apply(ProgramAction::RemoveThread, &advanced).unwrap_err();
        assert!(error.contains("at least one"), "{error}");
    }

    fn vehicle_of(class: ComputerClass) -> Vehicle {
        vehicle(class, None)
    }

    #[test]
    fn adjusting_a_value_clamps_and_moving_reorders() {
        let vehicle = basic();
        let mut session = MissileProgramSession::default();
        session
            .apply(ProgramAction::Append(BlockKind::Throttle), &vehicle)
            .unwrap();
        session.apply(ProgramAction::Append(BlockKind::Wait), &vehicle).unwrap();
        session
            .apply(
                ProgramAction::SelectPath {
                    thread: 0,
                    path: vec![PathStep {
                        index: 0,
                        branch: Branch::Body,
                    }],
                },
                &vehicle,
            )
            .unwrap();
        for _ in 0..20 {
            session.apply(ProgramAction::Adjust(1), &vehicle).unwrap();
        }
        assert_eq!(
            session.selected_block(),
            Some(&Block::SetThrottle { fraction: 1.0 }),
            "throttle is clamped at 100%"
        );
        session.apply(ProgramAction::MoveDown, &vehicle).unwrap();
        assert_eq!(session.block, Some(1));
        assert!(matches!(session.program.threads[0][1], Block::SetThrottle { .. }));
    }

    #[test]
    fn ignition_stage_stays_inside_the_vehicle() {
        let vehicle = basic();
        let mut session = MissileProgramSession::default();
        session
            .apply(ProgramAction::Append(BlockKind::Ignite), &vehicle)
            .unwrap();
        session.apply(ProgramAction::Adjust(-1), &vehicle).unwrap();
        assert_eq!(session.selected_block(), Some(&Block::IgniteStage { stage: 0 }));
        session.apply(ProgramAction::Adjust(1), &vehicle).unwrap();
        assert_eq!(
            session.selected_block(),
            Some(&Block::IgniteStage { stage: 0 }),
            "no second stage on a single-stage vehicle"
        );
    }

    #[test]
    fn flags_modes_and_marks_cycle_and_flip() {
        let vehicle = vehicle_of(ComputerClass::Advanced);
        let mut session = MissileProgramSession::default();
        session
            .apply(ProgramAction::Append(BlockKind::SetFlag), &vehicle)
            .unwrap();
        assert_eq!(session.selected_block(), Some(&Block::SetFlag { flag: 0, value: true }));
        session.apply(ProgramAction::Adjust(1), &vehicle).unwrap();
        assert_eq!(session.selected_block(), Some(&Block::SetFlag { flag: 1, value: true }));
        session.apply(ProgramAction::Flip, &vehicle).unwrap();
        assert_eq!(
            session.selected_block(),
            Some(&Block::SetFlag { flag: 1, value: false })
        );

        session
            .apply(ProgramAction::Append(BlockKind::SetMode), &vehicle)
            .unwrap();
        session.apply(ProgramAction::Adjust(1), &vehicle).unwrap();
        assert_eq!(session.selected_block(), Some(&Block::SetMode { mode: 2 }));

        session.apply(ProgramAction::Append(BlockKind::Mark), &vehicle).unwrap();
        session.apply(ProgramAction::Adjust(1), &vehicle).unwrap();
        assert_eq!(session.selected_block(), Some(&Block::Mark { mark: 1 }));
        assert!(session.apply(ProgramAction::Flip, &vehicle).is_err());
    }

    #[test]
    fn inserting_and_duplicating_create_connections_in_place() {
        let vehicle = basic();
        let mut session = MissileProgramSession::default();
        session.apply(ProgramAction::Append(BlockKind::Wait), &vehicle).unwrap();
        session.apply(ProgramAction::Append(BlockKind::Wait), &vehicle).unwrap();
        session
            .apply(
                ProgramAction::SelectPath {
                    thread: 0,
                    path: vec![PathStep {
                        index: 0,
                        branch: Branch::Body,
                    }],
                },
                &vehicle,
            )
            .unwrap();
        session.apply(ProgramAction::Insert(BlockKind::Mark), &vehicle).unwrap();
        assert_eq!(session.blocks_used(), 3);
        assert!(matches!(session.program.threads[0][1], Block::Mark { .. }));
        session.apply(ProgramAction::DuplicateSelection, &vehicle).unwrap();
        assert_eq!(session.blocks_used(), 4);
        assert!(matches!(session.program.threads[0][2], Block::Mark { .. }));
    }

    #[test]
    fn an_empty_program_says_so_and_json_round_trips() {
        let vehicle = basic();
        let session = MissileProgramSession::default();
        assert!(session.issues(&vehicle)[0].message.contains("empty"));
        let mut session = session;
        session.apply(ProgramAction::Append(BlockKind::Wait), &vehicle).unwrap();
        let bytes = session.to_bytes().unwrap();
        let loaded = MissileProgramSession::from_bytes(&bytes).unwrap();
        assert_eq!(loaded.program, session.program);
        assert!(MissileProgramSession::from_bytes(b"not json").is_err());
    }

    #[test]
    fn a_program_loaded_without_threads_still_accepts_blocks() {
        let vehicle = basic();
        let mut session = MissileProgramSession::from_bytes(br#"{"threads":[]}"#).unwrap();
        assert!(session.issues(&vehicle)[0].message.contains("empty"));
        session.apply(ProgramAction::Append(BlockKind::Wait), &vehicle).unwrap();
        assert_eq!(session.program.threads.len(), 1);
        assert_eq!(session.blocks_used(), 1);
    }

    #[test]
    fn clearing_keeps_one_empty_thread() {
        let vehicle = basic();
        let mut session = MissileProgramSession::default();
        session.apply(ProgramAction::Append(BlockKind::Wait), &vehicle).unwrap();
        session.apply(ProgramAction::Clear, &vehicle).unwrap();
        assert_eq!(session.program.threads.len(), 1);
        assert!(session.program.threads[0].is_empty());
        assert_eq!(session.block, None);
    }

    #[test]
    fn the_library_saves_loads_and_deletes_through_the_session() {
        let vehicle = basic();
        let mut session = MissileProgramSession::default();
        session.apply(ProgramAction::Append(BlockKind::Wait), &vehicle).unwrap();
        session.apply(ProgramAction::SaveCopy, &vehicle).unwrap();
        assert!(session.library_dirty);
        assert_eq!(session.library.programs.len(), 1);
        assert_eq!(session.library.programs[0].name, "Program 1");

        session.apply(ProgramAction::Clear, &vehicle).unwrap();
        assert!(session.program.threads[0].is_empty());
        session.apply(ProgramAction::LoadSaved(0), &vehicle).unwrap();
        assert_eq!(session.blocks_used(), 1);

        session.apply(ProgramAction::DeleteSaved(0), &vehicle).unwrap();
        assert!(session.library.programs.is_empty());
        assert!(session.apply(ProgramAction::LoadSaved(0), &vehicle).is_err());
        let bytes = library_bytes(&session).unwrap();
        assert_eq!(
            serde_json::from_slice::<ProgramLibrary>(&bytes).unwrap(),
            session.library
        );
    }

    #[test]
    fn the_debugger_flies_the_program_and_keeps_a_trace() {
        let vehicle = basic();
        let mut session = MissileProgramSession::default();
        session
            .apply(ProgramAction::Append(BlockKind::Ignite), &vehicle)
            .unwrap();
        session.apply(ProgramAction::Append(BlockKind::Mark), &vehicle).unwrap();
        session.apply(ProgramAction::Append(BlockKind::Wait), &vehicle).unwrap();
        let message = session.apply(ProgramAction::Debug, &vehicle).unwrap();
        assert!(message.contains("block(s) executed"), "{message}");
        let report = session.debug.as_ref().unwrap();
        assert!(report.trace.iter().any(|entry| entry.label == "Mark #0"));
        // Any edit clears the stale timeline.
        session.apply(ProgramAction::Append(BlockKind::Mark), &vehicle).unwrap();
        assert!(session.debug.is_none());
    }

    #[test]
    fn warnings_are_separated_from_errors_in_the_editor() {
        let vehicle = basic();
        let mut session = MissileProgramSession::default();
        session
            .apply(ProgramAction::Append(BlockKind::WaitFlag), &vehicle)
            .unwrap();
        let issues = session.issues(&vehicle);
        let (errors, warnings) = issue_counts(&issues);
        assert_eq!(errors, 0);
        assert_eq!(warnings, 1, "{issues:?}");
        session.apply(ProgramAction::Append(BlockKind::Wait), &vehicle).unwrap();
        assert!(validate_program(&session.program, &vehicle).is_empty());
    }

    fn select_path(session: &mut MissileProgramSession, thread: usize, path: Vec<PathStep>, vehicle: &Vehicle) {
        session
            .apply(ProgramAction::SelectPath { thread, path }, vehicle)
            .unwrap();
    }

    #[test]
    fn branches_receive_palette_inserts_and_stay_editable() {
        let vehicle = vehicle_of(ComputerClass::Advanced);
        let mut session = MissileProgramSession::default();
        session.apply(ProgramAction::Append(BlockKind::If), &vehicle).unwrap();
        session
            .apply(ProgramAction::FocusBranch(Branch::Then), &vehicle)
            .unwrap();
        session.apply(ProgramAction::Append(BlockKind::Mark), &vehicle).unwrap();
        assert!(matches!(
            &session.program.threads[0][0],
            Block::If { then_blocks, .. } if then_blocks.len() == 1
        ));
        select_path(
            &mut session,
            0,
            vec![PathStep {
                index: 0,
                branch: Branch::Body,
            }],
            &vehicle,
        );
        session
            .apply(ProgramAction::FocusBranch(Branch::Else), &vehicle)
            .unwrap();
        session.apply(ProgramAction::Append(BlockKind::Wait), &vehicle).unwrap();
        assert!(matches!(
            &session.program.threads[0][0],
            Block::If { else_blocks, .. } if else_blocks.len() == 1
        ));
        // A nested control can focus its own body without returning to the card first.
        session
            .apply(ProgramAction::Append(BlockKind::Repeat), &vehicle)
            .unwrap();
        assert!(matches!(
            &session.program.threads[0][0],
            Block::If { else_blocks, .. }
                if else_blocks.len() == 2 && matches!(else_blocks[1], Block::Repeat { body: ref b, .. } if b.is_empty())
        ));
        select_path(
            &mut session,
            0,
            vec![
                PathStep {
                    index: 0,
                    branch: Branch::Body,
                },
                PathStep {
                    index: 1,
                    branch: Branch::Else,
                },
            ],
            &vehicle,
        );
        session
            .apply(ProgramAction::FocusBranch(Branch::Body), &vehicle)
            .unwrap();
        session.apply(ProgramAction::Append(BlockKind::Mark), &vehicle).unwrap();
        assert!(matches!(
            &session.program.threads[0][0],
            Block::If { else_blocks, .. }
                if matches!(else_blocks[1], Block::Repeat { body: ref b, .. } if b.len() == 1)
        ));
        assert_eq!(session.blocks_used(), 5);
    }

    #[test]
    fn selection_can_be_copied_duplicated_and_captured_as_a_routine() {
        let vehicle = vehicle_of(ComputerClass::Advanced);
        let mut session = MissileProgramSession::default();
        session.apply(ProgramAction::Append(BlockKind::Wait), &vehicle).unwrap();
        session.apply(ProgramAction::Append(BlockKind::Mark), &vehicle).unwrap();
        session.selection = [BlockKey { thread: 0, block: 0 }, BlockKey { thread: 0, block: 1 }]
            .into_iter()
            .collect();
        session.apply(ProgramAction::CreateRoutine, &vehicle).unwrap();
        assert_eq!(session.program.routines.len(), 1);
        assert_eq!(session.program.routines[0].blocks.len(), 2);

        session.apply(ProgramAction::AppendCall(0), &vehicle).unwrap();
        assert!(matches!(
            session.program.threads[0].last(),
            Some(Block::Call { routine }) if routine == "Routine 1"
        ));

        select_path(
            &mut session,
            0,
            vec![PathStep {
                index: 0,
                branch: Branch::Body,
            }],
            &vehicle,
        );
        session.apply(ProgramAction::CopySelection, &vehicle).unwrap();
        session.apply(ProgramAction::PasteSelection, &vehicle).unwrap();
        assert_eq!(session.blocks_used(), 6, "routines count toward computer memory");
        assert_eq!(session.clipboard.len(), 1);

        session.apply(ProgramAction::Align, &vehicle).unwrap();
        assert_eq!(
            session.program.layout.position(BlockKey { thread: 0, block: 0 }),
            Some((60.0, 110.0))
        );
    }

    #[test]
    fn debug_playback_can_pause_step_and_replay_from_the_start() {
        let vehicle = basic();
        let mut session = MissileProgramSession::default();
        session.apply(ProgramAction::Append(BlockKind::Wait), &vehicle).unwrap();
        session.apply(ProgramAction::Debug, &vehicle).unwrap();
        assert!(session.debug_playing);
        session.apply(ProgramAction::ToggleDebugPlay, &vehicle).unwrap();
        assert!(!session.debug_playing);
        session.apply(ProgramAction::DebugStep(1), &vehicle).unwrap();
        assert!(session.debug_cursor <= session.debug.as_ref().unwrap().trace.len());
        session.apply(ProgramAction::CycleDebugSpeed, &vehicle).unwrap();
        assert_eq!(session.debug_speed, 2);
        session.apply(ProgramAction::ResetDebug, &vehicle).unwrap();
        assert_eq!(session.debug_cursor, 0);
    }
}
