//! Abstract flight program: parallel threads of generic blocks. Inputs read the fitted sensors,
//! conditions and logic share four boolean flags and a mode number, timers pause, outputs act on
//! the vehicle, and diagnostic blocks only leave a named mark in the trace.
//!
//! Everything is generic and fictional: `LockTarget` only asks the flight test for an abstract
//! success roll, and no block carries guidance, tracking or targeting logic.

use crate::assembly::{IssueSeverity, Vehicle};
use serde::{Deserialize, Serialize};

/// Flags a program can set and test. Kept small so a design stays readable on the board.
pub const FLAGS: usize = 4;
/// Mode values a program can select.
pub const MODES: u8 = 4;
/// Diagnostic marks, shown in the debugger trace only.
pub const MARKS: u8 = 8;
/// Longest a condition may wait before it times out and is reported.
pub const MAX_CONDITION_TIMEOUT_S: f64 = 600.0;
/// Program library size, so the file stays a short list of reusable programs.
pub const MAX_SAVED_PROGRAMS: usize = 12;
/// Hard safety bound for a `repeat while`; the timeout is the player-facing limit, this only
/// guarantees a finite run if the simulation clock is paused or a condition is always true.
pub const MAX_LOOP_ITERATIONS: u32 = 1_024;
/// Nested subroutine calls allowed at run time. Also the cycle guard for validation.
pub const MAX_CALL_DEPTH: usize = 8;

/// A condition used by the control structures. It reads only the same abstract flight state as
/// the original wait blocks: airspeed, pressure altitude, a flag or the mode register.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "condition", rename_all = "snake_case")]
pub enum Condition {
    SpeedAtLeast { speed_ms: f64 },
    PressureAtMost { hpa: f64 },
    FlagIs { flag: u8, value: bool },
    ModeIs { mode: u8 },
}

impl Condition {
    pub fn label(self) -> String {
        match self {
            Self::SpeedAtLeast { speed_ms } => format!("speed >= {speed_ms:.0} m/s"),
            Self::PressureAtMost { hpa } => format!("pressure <= {hpa:.0} hPa"),
            Self::FlagIs { flag, value } => format!("F{flag} = {value}"),
            Self::ModeIs { mode } => format!("mode = M{mode}"),
        }
    }

    /// Hardware that this condition needs to be observable.
    pub fn required_capability(self) -> Option<&'static str> {
        match self {
            Self::SpeedAtLeast { .. } => Some("pitot"),
            Self::PressureAtMost { .. } => Some("barometer"),
            Self::FlagIs { .. } | Self::ModeIs { .. } => None,
        }
    }

    pub fn is_met(self, speed_ms: f64, altitude_m: f64, flags: &[bool; FLAGS], mode: u8) -> bool {
        match self {
            Self::SpeedAtLeast { speed_ms: wanted } => speed_ms >= wanted,
            Self::PressureAtMost { hpa } => altitude_m >= pressure_altitude_m(hpa),
            Self::FlagIs { flag, value } => flags.get(usize::from(flag)).copied() == Some(value),
            Self::ModeIs { mode: wanted } => mode == wanted,
        }
    }
}

/// Pressure altitude used by both the legacy wait blocks and the new conditions.
pub fn pressure_altitude_m(hpa: f64) -> f64 {
    -8_000.0 * (hpa / 1_013.25).ln()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "block", rename_all = "snake_case")]
pub enum Block {
    /// Pause this thread for some seconds.
    Wait {
        seconds: f64,
    },
    IgniteStage {
        stage: usize,
    },
    SeparateStage {
        stage: usize,
    },
    /// Throttle for liquid and air-breathing engines (solid motors cannot throttle).
    SetThrottle {
        fraction: f64,
    },
    /// Needs a pitot tube.
    HoldSpeed {
        speed_ms: f64,
    },
    /// Needs a barometer: hold the altitude where the air pressure equals `hpa`.
    HoldPressure {
        hpa: f64,
    },
    /// Needs a radar. Must be the last block of its thread (final phase).
    LockTarget,
    /// Needs a pitot tube: continue once the vehicle reaches `speed_ms`, or after the timeout.
    WaitSpeed {
        speed_ms: f64,
        timeout_s: f64,
    },
    /// Needs a barometer: continue once the altitude reaches the `hpa` level, or after the timeout.
    WaitPressure {
        hpa: f64,
        timeout_s: f64,
    },
    /// Continue once the flag has `value`, or after the timeout.
    WaitFlag {
        flag: u8,
        value: bool,
        timeout_s: f64,
    },
    /// Flip a flag; the only logic operation the abstract computer offers.
    ToggleFlag {
        flag: u8,
    },
    /// Set a flag to a value.
    SetFlag {
        flag: u8,
        value: bool,
    },
    /// Select the program mode other blocks can test.
    SetMode {
        mode: u8,
    },
    /// Continue once the mode is `mode`, or after the timeout.
    WaitMode {
        mode: u8,
        timeout_s: f64,
    },
    /// Leave a numbered diagnostic mark in the timeline; no effect on the flight.
    Mark {
        mark: u8,
    },
    /// Run one of two branches once. The condition is evaluated immediately.
    If {
        condition: Condition,
        #[serde(default)]
        then_blocks: Vec<Block>,
        #[serde(default)]
        else_blocks: Vec<Block>,
    },
    /// Gate the thread until a condition is true, or report a timeout.
    WaitUntil {
        condition: Condition,
        timeout_s: f64,
    },
    /// Repeat a body a fixed number of times.
    Repeat {
        times: u8,
        #[serde(default)]
        body: Vec<Block>,
    },
    /// Repeat a body while a condition stays true, with a deterministic timeout.
    RepeatWhile {
        condition: Condition,
        timeout_s: f64,
        #[serde(default)]
        body: Vec<Block>,
    },
    /// Leave the nearest enclosing loop.
    Break,
    /// Run a reusable named routine.
    Call {
        routine: String,
    },
}

/// The eight palette sections. Presentation metadata owned by the rules so the editor cannot
/// invent a section the validation rules do not know.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BlockCategory {
    Inputs,
    Conditions,
    Logic,
    States,
    Timers,
    Outputs,
    Modes,
    Diagnostics,
}

impl BlockCategory {
    pub const ALL: [Self; 8] = [
        Self::Inputs,
        Self::Conditions,
        Self::Logic,
        Self::States,
        Self::Timers,
        Self::Outputs,
        Self::Modes,
        Self::Diagnostics,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Inputs => "Inputs",
            Self::Conditions => "Conditions",
            Self::Logic => "Logic",
            Self::States => "States",
            Self::Timers => "Timers",
            Self::Outputs => "Outputs",
            Self::Modes => "Modes",
            Self::Diagnostics => "Diagnostics",
        }
    }
}

impl Block {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Wait { .. } => "wait",
            Self::IgniteStage { .. } => "ignite_stage",
            Self::SeparateStage { .. } => "separate_stage",
            Self::SetThrottle { .. } => "set_throttle",
            Self::HoldSpeed { .. } => "hold_speed",
            Self::HoldPressure { .. } => "hold_pressure",
            Self::LockTarget => "lock_target",
            Self::WaitSpeed { .. } => "wait_speed",
            Self::WaitPressure { .. } => "wait_pressure",
            Self::WaitFlag { .. } => "wait_flag",
            Self::ToggleFlag { .. } => "toggle_flag",
            Self::SetFlag { .. } => "set_flag",
            Self::SetMode { .. } => "set_mode",
            Self::WaitMode { .. } => "wait_mode",
            Self::Mark { .. } => "mark",
            Self::If { .. } => "if",
            Self::WaitUntil { .. } => "wait_until",
            Self::Repeat { .. } => "repeat",
            Self::RepeatWhile { .. } => "repeat_while",
            Self::Break => "break",
            Self::Call { .. } => "call",
        }
    }

    pub fn category(&self) -> BlockCategory {
        match self {
            Self::WaitSpeed { .. } | Self::WaitPressure { .. } => BlockCategory::Inputs,
            Self::WaitFlag { .. } => BlockCategory::Conditions,
            Self::ToggleFlag { .. } => BlockCategory::Logic,
            Self::SetFlag { .. } => BlockCategory::States,
            Self::Wait { .. } => BlockCategory::Timers,
            Self::SetMode { .. } | Self::WaitMode { .. } => BlockCategory::Modes,
            Self::Mark { .. } => BlockCategory::Diagnostics,
            Self::If { .. }
            | Self::WaitUntil { .. }
            | Self::Repeat { .. }
            | Self::RepeatWhile { .. }
            | Self::Break
            | Self::Call { .. } => BlockCategory::Logic,
            Self::IgniteStage { .. }
            | Self::SeparateStage { .. }
            | Self::SetThrottle { .. }
            | Self::HoldSpeed { .. }
            | Self::HoldPressure { .. }
            | Self::LockTarget => BlockCategory::Outputs,
        }
    }

    /// One line the editor shows for the selected palette entry.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Wait { .. } => "Pause this thread for a fixed time.",
            Self::IgniteStage { .. } => "Ignite a stage that is still attached.",
            Self::SeparateStage { .. } => "Drop a spent stage (all lower stages must be gone).",
            Self::SetThrottle { .. } => "Set the throttle of liquid and air-breathing engines.",
            Self::HoldSpeed { .. } => "Hold an airspeed; needs a pitot tube.",
            Self::HoldPressure { .. } => "Hold an altitude by pressure; needs a barometer.",
            Self::LockTarget => "Ask for the abstract lock roll; closes the thread.",
            Self::WaitSpeed { .. } => "Wait for an airspeed; needs a pitot tube.",
            Self::WaitPressure { .. } => "Wait for an altitude by pressure; needs a barometer.",
            Self::WaitFlag { .. } => "Wait until a flag has a value, then continue.",
            Self::ToggleFlag { .. } => "Flip a flag; the program's only logic gate.",
            Self::SetFlag { .. } => "Set a flag from a state change.",
            Self::SetMode { .. } => "Select a program mode.",
            Self::WaitMode { .. } => "Wait until the program is in a mode.",
            Self::Mark { .. } => "Leave a numbered mark in the debugger trace.",
            Self::If { .. } => "Run one of two branches once.",
            Self::WaitUntil { .. } => "Gate this thread until an abstract condition is true.",
            Self::Repeat { .. } => "Repeat a body a fixed number of times.",
            Self::RepeatWhile { .. } => "Repeat a body while a condition is true, with a timeout.",
            Self::Break => "Leave the nearest enclosing loop.",
            Self::Call { .. } => "Call a reusable subroutine.",
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Wait { seconds } => format!("Wait {seconds:.1} s"),
            Self::IgniteStage { stage } => format!("Ignite stage {}", stage + 1),
            Self::SeparateStage { stage } => format!("Separate stage {}", stage + 1),
            Self::SetThrottle { fraction } => format!("Throttle {:.0}%", fraction * 100.0),
            Self::HoldSpeed { speed_ms } => format!("Hold speed {speed_ms:.0} m/s"),
            Self::HoldPressure { hpa } => format!("Hold {hpa:.0} hPa"),
            Self::LockTarget => "Lock target (final phase)".to_string(),
            Self::WaitSpeed { speed_ms, timeout_s } => {
                format!("Wait {speed_ms:.0} m/s (timeout {timeout_s:.0} s)")
            }
            Self::WaitPressure { hpa, timeout_s } => {
                format!("Wait {hpa:.0} hPa (timeout {timeout_s:.0} s)")
            }
            Self::WaitFlag { flag, value, timeout_s } => {
                format!("Wait flag F{flag} = {value} (timeout {timeout_s:.0} s)")
            }
            Self::ToggleFlag { flag } => format!("Toggle flag F{flag}"),
            Self::SetFlag { flag, value } => format!("Set flag F{flag} = {value}"),
            Self::SetMode { mode } => format!("Set mode M{mode}"),
            Self::WaitMode { mode, timeout_s } => {
                format!("Wait mode M{mode} (timeout {timeout_s:.0} s)")
            }
            Self::Mark { mark } => format!("Mark #{mark}"),
            Self::If { condition, .. } => format!("If {}", condition.label()),
            Self::WaitUntil { condition, timeout_s } => {
                format!("Wait until {} (timeout {timeout_s:.0} s)", condition.label())
            }
            Self::Repeat { times, .. } => format!("Repeat {times}x"),
            Self::RepeatWhile {
                condition, timeout_s, ..
            } => {
                format!("Repeat while {} (timeout {timeout_s:.0} s)", condition.label())
            }
            Self::Break => "Break loop".to_string(),
            Self::Call { routine } => format!("Call {routine}"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub threads: Vec<Vec<Block>>,
    /// Reusable routines, stored once and callable from any thread.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routines: Vec<Routine>,
    /// Editor-only spatial layout. It has no effect on execution.
    #[serde(default, skip_serializing_if = "ProgramLayout::is_empty")]
    pub layout: ProgramLayout,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Routine {
    pub name: String,
    #[serde(default)]
    pub blocks: Vec<Block>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BlockKey {
    pub thread: usize,
    pub block: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockPosition {
    pub thread: usize,
    pub block: usize,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramGroup {
    pub name: String,
    pub members: Vec<BlockKey>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramLayout {
    #[serde(default = "default_zoom")]
    pub zoom: f32,
    #[serde(default)]
    pub pan_x: f32,
    #[serde(default)]
    pub pan_y: f32,
    #[serde(default)]
    pub positions: Vec<BlockPosition>,
    #[serde(default)]
    pub groups: Vec<ProgramGroup>,
}

fn default_zoom() -> f32 {
    1.0
}

impl Default for ProgramLayout {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            positions: Vec::new(),
            groups: Vec::new(),
        }
    }
}

impl ProgramLayout {
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
            && self.groups.is_empty()
            && self.zoom == 1.0
            && self.pan_x == 0.0
            && self.pan_y == 0.0
    }

    pub fn position(&self, key: BlockKey) -> Option<(f32, f32)> {
        self.positions
            .iter()
            .find(|position| position.thread == key.thread && position.block == key.block)
            .map(|position| (position.x, position.y))
    }

    pub fn set_position(&mut self, key: BlockKey, x: f32, y: f32) {
        if let Some(position) = self
            .positions
            .iter_mut()
            .find(|position| position.thread == key.thread && position.block == key.block)
        {
            position.x = x;
            position.y = y;
        } else {
            self.positions.push(BlockPosition {
                thread: key.thread,
                block: key.block,
                x,
                y,
            });
        }
    }

    pub fn group_of(&self, key: BlockKey) -> Option<&ProgramGroup> {
        self.groups.iter().find(|group| group.members.contains(&key))
    }
}

/// A causal link the editor draws: `from` runs immediately before `to` in the same thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramConnection {
    pub thread: usize,
    pub from: usize,
    pub to: usize,
}

/// One error or warning, placed in the program so the editor can select the block.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgramIssue {
    pub severity: IssueSeverity,
    pub thread: usize,
    pub block: Option<usize>,
    pub message: String,
}

/// One executed block, recorded when `FlightConfig::trace` is on.
#[derive(Clone, Debug, PartialEq)]
pub struct BlockTrace {
    pub t_s: f64,
    pub thread: usize,
    pub block: usize,
    pub label: String,
    /// The condition timed out instead of being met.
    pub timed_out: bool,
    /// The block's execution raised an anomaly (timeout or invalid action).
    pub anomaly: bool,
    /// Program state after the block, for the debugger's state inspector.
    pub flags: [bool; FLAGS],
    pub mode: u8,
}

/// A named program the player can reuse on another vehicle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedProgram {
    pub name: String,
    pub program: Program,
}

/// The saved-program library. A name is unique; saving under an existing name replaces it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProgramLibrary {
    pub programs: Vec<SavedProgram>,
}

impl ProgramLibrary {
    /// Save (or replace) a program under `name`, refusing an empty name or a full library.
    pub fn save(&mut self, name: impl Into<String>, program: Program) -> Result<String, String> {
        let name = name.into().trim().to_string();
        if name.is_empty() {
            return Err("A saved program needs a name.".to_string());
        }
        if let Some(existing) = self.programs.iter_mut().find(|saved| saved.name == name) {
            existing.program = program;
            return Ok(format!("Replaced saved program '{name}'"));
        }
        if self.programs.len() >= MAX_SAVED_PROGRAMS {
            return Err(format!(
                "The library holds {MAX_SAVED_PROGRAMS} programs; remove one first."
            ));
        }
        self.programs.push(SavedProgram {
            name: name.clone(),
            program,
        });
        Ok(format!("Saved program '{name}'"))
    }

    /// First free default name, so one button is enough to keep a program.
    pub fn next_name(&self) -> String {
        let mut index = self.programs.len() + 1;
        loop {
            let name = format!("Program {index}");
            if !self.programs.iter().any(|saved| saved.name == name) {
                return name;
            }
            index += 1;
        }
    }

    pub fn get(&self, index: usize) -> Option<&SavedProgram> {
        self.programs.get(index)
    }

    pub fn remove(&mut self, index: usize) -> Result<String, String> {
        if index >= self.programs.len() {
            return Err("No such saved program.".to_string());
        }
        let removed = self.programs.remove(index);
        Ok(format!("Removed saved program '{}'", removed.name))
    }
}

impl Program {
    pub fn block_count(&self) -> usize {
        self.threads.iter().map(|blocks| count_blocks(blocks)).sum::<usize>()
            + self
                .routines
                .iter()
                .map(|routine| count_blocks(&routine.blocks))
                .sum::<usize>()
    }

    /// One thread from a block list, handy for tests and presets.
    pub fn single(blocks: Vec<Block>) -> Self {
        Self {
            threads: vec![blocks],
            ..Self::default()
        }
    }

    /// Copy used for content hashes: editor positions, zoom, pan and groups are presentation only
    /// and must not invalidate trial evidence or contract dossiers.
    pub fn semantic_copy(&self) -> Self {
        let mut program = self.clone();
        program.layout = ProgramLayout::default();
        program
    }

    /// Adjacent links, one per pair of consecutive blocks, drawn by the editor.
    pub fn connections(&self) -> Vec<ProgramConnection> {
        let mut connections = Vec::new();
        for (thread, blocks) in self.threads.iter().enumerate() {
            for from in 0..blocks.len().saturating_sub(1) {
                connections.push(ProgramConnection {
                    thread,
                    from,
                    to: from + 1,
                });
            }
        }
        connections
    }

    pub fn max_flag(&self) -> Option<u8> {
        let mut best = None;
        for blocks in self
            .threads
            .iter()
            .chain(self.routines.iter().map(|routine| &routine.blocks))
        {
            max_flag_in(blocks, &mut best);
        }
        best
    }

    pub fn max_mode(&self) -> Option<u8> {
        let mut best = None;
        for blocks in self
            .threads
            .iter()
            .chain(self.routines.iter().map(|routine| &routine.blocks))
        {
            max_mode_in(blocks, &mut best);
        }
        best
    }
}

fn count_blocks(blocks: &[Block]) -> usize {
    blocks
        .iter()
        .map(|block| {
            1 + match block {
                Block::If {
                    then_blocks,
                    else_blocks,
                    ..
                } => count_blocks(then_blocks) + count_blocks(else_blocks),
                Block::Repeat { body, .. } | Block::RepeatWhile { body, .. } => count_blocks(body),
                _ => 0,
            }
        })
        .sum()
}

fn max_flag_in(blocks: &[Block], best: &mut Option<u8>) {
    for block in blocks {
        match block {
            Block::WaitFlag { flag, .. } | Block::ToggleFlag { flag } | Block::SetFlag { flag, .. } => {
                *best = Some(best.map_or(*flag, |old| old.max(*flag)));
            }
            Block::If {
                then_blocks,
                else_blocks,
                ..
            } => {
                max_flag_in(then_blocks, best);
                max_flag_in(else_blocks, best);
            }
            Block::Repeat { body, .. } | Block::RepeatWhile { body, .. } => max_flag_in(body, best),
            Block::WaitUntil {
                condition: Condition::FlagIs { flag, .. },
                ..
            } => *best = Some(best.map_or(*flag, |old| old.max(*flag))),
            _ => {}
        }
    }
}

fn max_mode_in(blocks: &[Block], best: &mut Option<u8>) {
    for block in blocks {
        match block {
            Block::SetMode { mode } | Block::WaitMode { mode, .. } => {
                *best = Some(best.map_or(*mode, |old| old.max(*mode)));
            }
            Block::If {
                then_blocks,
                else_blocks,
                ..
            } => {
                max_mode_in(then_blocks, best);
                max_mode_in(else_blocks, best);
            }
            Block::Repeat { body, .. } | Block::RepeatWhile { body, .. } => max_mode_in(body, best),
            Block::WaitUntil {
                condition: Condition::ModeIs { mode },
                ..
            } => *best = Some(best.map_or(*mode, |old| old.max(*mode))),
            _ => {}
        }
    }
}

fn severity_issue(severity: IssueSeverity, thread: usize, block: Option<usize>, message: String) -> ProgramIssue {
    ProgramIssue {
        severity,
        thread,
        block,
        message,
    }
}

fn condition_issues(
    condition: Condition,
    thread: usize,
    block: Option<usize>,
    place: &str,
    available: &std::collections::BTreeSet<&'static str>,
    issues: &mut Vec<ProgramIssue>,
) {
    match condition {
        Condition::SpeedAtLeast { speed_ms } => {
            if !(10.0..=1_500.0).contains(&speed_ms) || !speed_ms.is_finite() {
                issues.push(severity_issue(
                    IssueSeverity::Error,
                    thread,
                    block,
                    format!("{place}: speed must be between 10 and 1500 m/s"),
                ));
            }
            if !available.contains("wait_speed") {
                issues.push(severity_issue(
                    IssueSeverity::Error,
                    thread,
                    block,
                    format!("{place}: this condition needs a pitot tube"),
                ));
            }
        }
        Condition::PressureAtMost { hpa } => {
            if !(50.0..=1_013.25).contains(&hpa) || !hpa.is_finite() {
                issues.push(severity_issue(
                    IssueSeverity::Error,
                    thread,
                    block,
                    format!("{place}: pressure must be between 50 and 1013 hPa"),
                ));
            }
            if !available.contains("wait_pressure") {
                issues.push(severity_issue(
                    IssueSeverity::Error,
                    thread,
                    block,
                    format!("{place}: this condition needs a barometer"),
                ));
            }
        }
        Condition::FlagIs { flag, .. } => {
            if flag >= FLAGS as u8 {
                issues.push(severity_issue(
                    IssueSeverity::Error,
                    thread,
                    block,
                    format!("{place}: flags are F0 to F{}", FLAGS - 1),
                ));
            }
        }
        Condition::ModeIs { mode } => {
            if mode >= MODES {
                issues.push(severity_issue(
                    IssueSeverity::Error,
                    thread,
                    block,
                    format!("{place}: modes are M0 to M{}", MODES - 1),
                ));
            }
        }
    }
}

fn nested_issues(
    thread: usize,
    blocks: &[Block],
    prefix: &str,
    in_loop: bool,
    available: &std::collections::BTreeSet<&'static str>,
    issues: &mut Vec<ProgramIssue>,
) {
    for (index, block) in blocks.iter().enumerate() {
        let place = format!("{prefix} block {}", index + 1);
        if !available.contains(block.kind()) {
            issues.push(severity_issue(
                IssueSeverity::Error,
                thread,
                None,
                format!("{place}: '{}' needs hardware this vehicle does not have", block.label()),
            ));
        }
        match block {
            Block::Wait { seconds } => {
                if !(0.0..=3_600.0).contains(seconds) || !seconds.is_finite() {
                    issues.push(severity_issue(
                        IssueSeverity::Error,
                        thread,
                        None,
                        format!("{place}: wait must be between 0 and 3600 s"),
                    ));
                } else if *seconds == 0.0 {
                    issues.push(severity_issue(
                        IssueSeverity::Warning,
                        thread,
                        None,
                        format!("{place}: a wait of 0 s does nothing"),
                    ));
                }
            }
            Block::SetThrottle { fraction } if !(0.0..=1.0).contains(fraction) || !fraction.is_finite() => {
                issues.push(severity_issue(
                    IssueSeverity::Error,
                    thread,
                    None,
                    format!("{place}: throttle must be between 0 and 1"),
                ));
            }
            Block::HoldSpeed { speed_ms } | Block::WaitSpeed { speed_ms, .. }
                if !(10.0..=1_500.0).contains(speed_ms) || !speed_ms.is_finite() =>
            {
                issues.push(severity_issue(
                    IssueSeverity::Error,
                    thread,
                    None,
                    format!("{place}: speed must be between 10 and 1500 m/s"),
                ));
            }
            Block::HoldPressure { hpa } | Block::WaitPressure { hpa, .. }
                if !(50.0..=1_013.25).contains(hpa) || !hpa.is_finite() =>
            {
                issues.push(severity_issue(
                    IssueSeverity::Error,
                    thread,
                    None,
                    format!("{place}: pressure must be between 50 and 1013 hPa"),
                ));
            }
            Block::WaitSpeed { timeout_s, .. }
            | Block::WaitPressure { timeout_s, .. }
            | Block::WaitFlag { timeout_s, .. }
            | Block::WaitMode { timeout_s, .. }
                if !(0.0..=MAX_CONDITION_TIMEOUT_S).contains(timeout_s) || !timeout_s.is_finite() =>
            {
                issues.push(severity_issue(
                    IssueSeverity::Error,
                    thread,
                    None,
                    format!("{place}: timeout must be between 0 and {MAX_CONDITION_TIMEOUT_S:.0} s"),
                ));
            }
            Block::WaitFlag { flag, .. } | Block::ToggleFlag { flag } | Block::SetFlag { flag, .. }
                if *flag >= FLAGS as u8 =>
            {
                issues.push(severity_issue(
                    IssueSeverity::Error,
                    thread,
                    None,
                    format!("{place}: flags are F0 to F{}", FLAGS - 1),
                ));
            }
            Block::SetMode { mode } | Block::WaitMode { mode, .. } if *mode >= MODES => {
                issues.push(severity_issue(
                    IssueSeverity::Error,
                    thread,
                    None,
                    format!("{place}: modes are M0 to M{}", MODES - 1),
                ));
            }
            Block::Mark { mark } if *mark >= MARKS => {
                issues.push(severity_issue(
                    IssueSeverity::Error,
                    thread,
                    None,
                    format!("{place}: marks are #0 to #{}", MARKS - 1),
                ));
            }
            Block::If {
                condition,
                then_blocks,
                else_blocks,
            } => {
                condition_issues(*condition, thread, None, &place, available, issues);
                if then_blocks.is_empty() && else_blocks.is_empty() {
                    issues.push(severity_issue(
                        IssueSeverity::Warning,
                        thread,
                        None,
                        format!("{place}: both branches are empty"),
                    ));
                }
                nested_issues(
                    thread,
                    then_blocks,
                    &format!("{place} / then"),
                    in_loop,
                    available,
                    issues,
                );
                nested_issues(
                    thread,
                    else_blocks,
                    &format!("{place} / else"),
                    in_loop,
                    available,
                    issues,
                );
            }
            Block::WaitUntil { condition, timeout_s } => {
                condition_issues(*condition, thread, None, &place, available, issues);
                if !(0.0..=MAX_CONDITION_TIMEOUT_S).contains(timeout_s) || !timeout_s.is_finite() {
                    issues.push(severity_issue(
                        IssueSeverity::Error,
                        thread,
                        None,
                        format!("{place}: timeout must be between 0 and {MAX_CONDITION_TIMEOUT_S:.0} s"),
                    ));
                }
            }
            Block::Repeat { times, body } => {
                if *times == 0 {
                    issues.push(severity_issue(
                        IssueSeverity::Warning,
                        thread,
                        None,
                        format!("{place}: a repeat of 0 does nothing"),
                    ));
                }
                if body.is_empty() {
                    issues.push(severity_issue(
                        IssueSeverity::Warning,
                        thread,
                        None,
                        format!("{place}: repeat body is empty"),
                    ));
                }
                nested_issues(thread, body, &format!("{place} / repeat"), true, available, issues);
            }
            Block::RepeatWhile {
                condition,
                timeout_s,
                body,
            } => {
                condition_issues(*condition, thread, None, &place, available, issues);
                if !(0.0..=MAX_CONDITION_TIMEOUT_S).contains(timeout_s) || !timeout_s.is_finite() {
                    issues.push(severity_issue(
                        IssueSeverity::Error,
                        thread,
                        None,
                        format!("{place}: timeout must be between 0 and {MAX_CONDITION_TIMEOUT_S:.0} s"),
                    ));
                }
                if body.is_empty() {
                    issues.push(severity_issue(
                        IssueSeverity::Warning,
                        thread,
                        None,
                        format!("{place}: repeat-while body is empty"),
                    ));
                }
                nested_issues(
                    thread,
                    body,
                    &format!("{place} / repeat while"),
                    true,
                    available,
                    issues,
                );
            }
            Block::Break if !in_loop => issues.push(severity_issue(
                IssueSeverity::Error,
                thread,
                None,
                format!("{place}: 'break' is only valid inside a repeat body"),
            )),
            Block::Call { routine } => {
                if routine.trim().is_empty() {
                    issues.push(severity_issue(
                        IssueSeverity::Error,
                        thread,
                        None,
                        format!("{place}: a call needs a routine name"),
                    ));
                }
            }
            Block::LockTarget => issues.push(severity_issue(
                IssueSeverity::Error,
                thread,
                None,
                format!("{place}: invalid connection: 'Lock target' can only close a top-level thread"),
            )),
            _ => {}
        }
    }
}

/// Everything wrong with the program for this vehicle, errors and warnings alike.
pub fn program_issues(program: &Program, vehicle: &Vehicle) -> Vec<ProgramIssue> {
    let mut issues = Vec::new();
    let Some(computer) = vehicle.computer() else {
        return vec![severity_issue(
            IssueSeverity::Error,
            0,
            None,
            "the vehicle has no flight computer".to_string(),
        )];
    };
    if program.threads.len() > computer.threads() {
        issues.push(severity_issue(
            IssueSeverity::Error,
            0,
            None,
            format!(
                "{} parallel threads but the computer runs only {}",
                program.threads.len(),
                computer.threads()
            ),
        ));
    }
    if program.block_count() > computer.ram_blocks() {
        issues.push(severity_issue(
            IssueSeverity::Error,
            0,
            None,
            format!(
                "{} blocks but the computer memory holds only {}",
                program.block_count(),
                computer.ram_blocks()
            ),
        ));
    } else if program.block_count() * 10 >= computer.ram_blocks() * 9 {
        issues.push(severity_issue(
            IssueSeverity::Warning,
            0,
            None,
            format!(
                "the program uses {} of {} blocks: little memory is left for edits",
                program.block_count(),
                computer.ram_blocks()
            ),
        ));
    }

    let available = vehicle.available_blocks();
    let mut locks = 0;
    for (t, thread) in program.threads.iter().enumerate() {
        if thread.is_empty() {
            issues.push(severity_issue(
                IssueSeverity::Warning,
                t,
                None,
                format!("thread {} is empty", t + 1),
            ));
        }
        let mut separated: Vec<usize> = Vec::new();
        for (b, block) in thread.iter().enumerate() {
            let place = format!("thread {} block {}", t + 1, b + 1);
            if !available.contains(block.kind()) {
                issues.push(severity_issue(
                    IssueSeverity::Error,
                    t,
                    Some(b),
                    format!("{place}: '{}' needs hardware this vehicle does not have", block.label()),
                ));
            }
            match block.clone() {
                Block::Wait { seconds } => {
                    if !(0.0..=3_600.0).contains(&seconds) || !seconds.is_finite() {
                        issues.push(severity_issue(
                            IssueSeverity::Error,
                            t,
                            Some(b),
                            format!("{place}: wait must be between 0 and 3600 s"),
                        ));
                    } else if seconds == 0.0 {
                        issues.push(severity_issue(
                            IssueSeverity::Warning,
                            t,
                            Some(b),
                            format!("{place}: a wait of 0 s does nothing"),
                        ));
                    }
                }
                Block::IgniteStage { stage } | Block::SeparateStage { stage } if stage >= vehicle.stage_count() => {
                    issues.push(severity_issue(
                        IssueSeverity::Error,
                        t,
                        Some(b),
                        format!("{place}: stage {} does not exist", stage + 1),
                    ));
                }
                Block::SetThrottle { fraction } if !(0.0..=1.0).contains(&fraction) || !fraction.is_finite() => {
                    issues.push(severity_issue(
                        IssueSeverity::Error,
                        t,
                        Some(b),
                        format!("{place}: throttle must be between 0 and 1"),
                    ));
                }
                Block::HoldSpeed { speed_ms } | Block::WaitSpeed { speed_ms, .. }
                    if !(10.0..=1_500.0).contains(&speed_ms) || !speed_ms.is_finite() =>
                {
                    issues.push(severity_issue(
                        IssueSeverity::Error,
                        t,
                        Some(b),
                        format!("{place}: speed must be between 10 and 1500 m/s"),
                    ));
                }
                Block::HoldPressure { hpa } | Block::WaitPressure { hpa, .. }
                    if !(50.0..=1_013.25).contains(&hpa) || !hpa.is_finite() =>
                {
                    issues.push(severity_issue(
                        IssueSeverity::Error,
                        t,
                        Some(b),
                        format!("{place}: pressure must be between 50 and 1013 hPa"),
                    ));
                }
                Block::WaitSpeed { timeout_s, .. }
                | Block::WaitPressure { timeout_s, .. }
                | Block::WaitFlag { timeout_s, .. }
                | Block::WaitMode { timeout_s, .. }
                    if !(0.0..=MAX_CONDITION_TIMEOUT_S).contains(&timeout_s) || !timeout_s.is_finite() =>
                {
                    issues.push(severity_issue(
                        IssueSeverity::Error,
                        t,
                        Some(b),
                        format!("{place}: timeout must be between 0 and {MAX_CONDITION_TIMEOUT_S:.0} s"),
                    ));
                }
                Block::WaitFlag { flag, .. } | Block::ToggleFlag { flag } | Block::SetFlag { flag, .. }
                    if flag >= FLAGS as u8 =>
                {
                    issues.push(severity_issue(
                        IssueSeverity::Error,
                        t,
                        Some(b),
                        format!("{place}: flags are F0 to F{}", FLAGS - 1),
                    ));
                }
                Block::SetMode { mode } | Block::WaitMode { mode, .. } if mode >= MODES => {
                    issues.push(severity_issue(
                        IssueSeverity::Error,
                        t,
                        Some(b),
                        format!("{place}: modes are M0 to M{}", MODES - 1),
                    ));
                }
                Block::Mark { mark } if mark >= MARKS => {
                    issues.push(severity_issue(
                        IssueSeverity::Error,
                        t,
                        Some(b),
                        format!("{place}: marks are #0 to #{}", MARKS - 1),
                    ));
                }
                Block::LockTarget => {
                    locks += 1;
                    if b + 1 != thread.len() {
                        issues.push(severity_issue(
                            IssueSeverity::Error,
                            t,
                            Some(b + 1),
                            format!(
                                "{place}: invalid connection: 'Lock target' must be the last block of its thread (final phase)"
                            ),
                        ));
                    }
                }
                Block::If {
                    condition,
                    then_blocks,
                    else_blocks,
                } => {
                    condition_issues(condition, t, Some(b), &place, &available, &mut issues);
                    if then_blocks.is_empty() && else_blocks.is_empty() {
                        issues.push(severity_issue(
                            IssueSeverity::Warning,
                            t,
                            Some(b),
                            format!("{place}: both branches are empty"),
                        ));
                    }
                    nested_issues(
                        t,
                        &then_blocks,
                        &format!("{place} / then"),
                        false,
                        &available,
                        &mut issues,
                    );
                    nested_issues(
                        t,
                        &else_blocks,
                        &format!("{place} / else"),
                        false,
                        &available,
                        &mut issues,
                    );
                }
                Block::WaitUntil { condition, timeout_s } => {
                    condition_issues(condition, t, Some(b), &place, &available, &mut issues);
                    if !(0.0..=MAX_CONDITION_TIMEOUT_S).contains(&timeout_s) || !timeout_s.is_finite() {
                        issues.push(severity_issue(
                            IssueSeverity::Error,
                            t,
                            Some(b),
                            format!("{place}: timeout must be between 0 and {MAX_CONDITION_TIMEOUT_S:.0} s"),
                        ));
                    } else if timeout_s == 0.0 {
                        issues.push(severity_issue(
                            IssueSeverity::Warning,
                            t,
                            Some(b),
                            format!("{place}: a timeout of 0 s cannot wait for the condition"),
                        ));
                    }
                }
                Block::Repeat { times, body } => {
                    if times == 0 {
                        issues.push(severity_issue(
                            IssueSeverity::Warning,
                            t,
                            Some(b),
                            format!("{place}: a repeat of 0 does nothing"),
                        ));
                    }
                    if body.is_empty() {
                        issues.push(severity_issue(
                            IssueSeverity::Warning,
                            t,
                            Some(b),
                            format!("{place}: repeat body is empty"),
                        ));
                    }
                    nested_issues(t, &body, &format!("{place} / repeat"), true, &available, &mut issues);
                }
                Block::RepeatWhile {
                    condition,
                    timeout_s,
                    body,
                } => {
                    condition_issues(condition, t, Some(b), &place, &available, &mut issues);
                    if !(0.0..=MAX_CONDITION_TIMEOUT_S).contains(&timeout_s) || !timeout_s.is_finite() {
                        issues.push(severity_issue(
                            IssueSeverity::Error,
                            t,
                            Some(b),
                            format!("{place}: timeout must be between 0 and {MAX_CONDITION_TIMEOUT_S:.0} s"),
                        ));
                    } else if timeout_s == 0.0 {
                        issues.push(severity_issue(
                            IssueSeverity::Warning,
                            t,
                            Some(b),
                            format!("{place}: a timeout of 0 s stops the repeat immediately"),
                        ));
                    }
                    if body.is_empty() {
                        issues.push(severity_issue(
                            IssueSeverity::Warning,
                            t,
                            Some(b),
                            format!("{place}: repeat-while body is empty"),
                        ));
                    }
                    nested_issues(
                        t,
                        &body,
                        &format!("{place} / repeat while"),
                        true,
                        &available,
                        &mut issues,
                    );
                }
                Block::Break => issues.push(severity_issue(
                    IssueSeverity::Error,
                    t,
                    Some(b),
                    format!("{place}: 'break' is only valid inside a repeat body"),
                )),
                Block::Call { routine } => {
                    if routine.trim().is_empty() {
                        issues.push(severity_issue(
                            IssueSeverity::Error,
                            t,
                            Some(b),
                            format!("{place}: a call needs a routine name"),
                        ));
                    } else if !program.routines.iter().any(|candidate| candidate.name == routine) {
                        issues.push(severity_issue(
                            IssueSeverity::Error,
                            t,
                            Some(b),
                            format!("{place}: call to unknown routine '{routine}'"),
                        ));
                    }
                }
                Block::SeparateStage { stage } => separated.push(stage),
                Block::IgniteStage { stage } => {
                    if let Some(dropped) = separated.iter().copied().find(|dropped| stage <= *dropped) {
                        issues.push(severity_issue(
                            IssueSeverity::Error,
                            t,
                            Some(b),
                            format!(
                                "{place}: invalid connection: stage {} is ignited after stage {} was separated",
                                stage + 1,
                                dropped + 1
                            ),
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    if locks > 1 {
        issues.push(severity_issue(
            IssueSeverity::Error,
            0,
            None,
            "only one 'Lock target' block is allowed".to_string(),
        ));
    }

    // Conditions the program can never satisfy: a warning, because another thread could still be
    // edited in, and the flight reports the timeout at run time.
    let mut flags_set = Vec::new();
    let mut modes_set = Vec::new();
    for (t, thread) in program.threads.iter().enumerate() {
        collect_writes(t, thread, &mut flags_set, &mut modes_set);
    }
    for routine in &program.routines {
        collect_writes(0, &routine.blocks, &mut flags_set, &mut modes_set);
    }
    for (t, thread) in program.threads.iter().enumerate() {
        for (b, block) in thread.iter().enumerate() {
            match *block {
                Block::WaitFlag { flag, value: true, .. } if !flags_set.contains(&flag) => {
                    issues.push(severity_issue(
                        IssueSeverity::Warning,
                        t,
                        Some(b),
                        format!(
                            "thread {} block {}: nobody sets flag F{flag}, so the condition can only time out",
                            t + 1,
                            b + 1
                        ),
                    ));
                }
                Block::WaitFlag { flag, value: false, .. } if !flags_set.contains(&flag) => {
                    issues.push(severity_issue(
                        IssueSeverity::Warning,
                        t,
                        Some(b),
                        format!(
                            "thread {} block {}: nobody ever sets flag F{flag}; the wait is already true at thread start",
                            t + 1,
                            b + 1
                        ),
                    ));
                }
                Block::WaitMode { mode, .. } if !modes_set.contains(&mode) => {
                    issues.push(severity_issue(
                        IssueSeverity::Warning,
                        t,
                        Some(b),
                        format!(
                            "thread {} block {}: nobody selects mode M{mode}, so the condition can only time out",
                            t + 1,
                            b + 1
                        ),
                    ));
                }
                _ => {}
            }
        }
    }
    for (t, thread) in program.threads.iter().enumerate() {
        dead_condition_issues(t, thread, &flags_set, &modes_set, &mut issues);
        nested_call_issues(t, thread, program, &mut issues);
        nested_stage_issues(t, thread, vehicle.stage_count(), false, &mut issues);
    }
    for routine in &program.routines {
        nested_stage_issues(0, &routine.blocks, vehicle.stage_count(), true, &mut issues);
    }
    validate_routines(program, &available, &mut issues);
    issues
}

fn nested_call_issues(thread: usize, blocks: &[Block], program: &Program, issues: &mut Vec<ProgramIssue>) {
    for block in blocks {
        match block {
            Block::If {
                then_blocks,
                else_blocks,
                ..
            } => {
                check_nested_calls(thread, then_blocks, program, issues, "then");
                check_nested_calls(thread, else_blocks, program, issues, "else");
                nested_call_issues(thread, then_blocks, program, issues);
                nested_call_issues(thread, else_blocks, program, issues);
            }
            Block::Repeat { body, .. } | Block::RepeatWhile { body, .. } => {
                check_nested_calls(thread, body, program, issues, "body");
                nested_call_issues(thread, body, program, issues);
            }
            _ => {}
        }
    }
}

fn check_nested_calls(
    thread: usize,
    blocks: &[Block],
    program: &Program,
    issues: &mut Vec<ProgramIssue>,
    branch: &str,
) {
    for block in blocks {
        if let Block::Call { routine } = block
            && !routine.trim().is_empty()
            && !program.routines.iter().any(|candidate| candidate.name == *routine)
        {
            issues.push(severity_issue(
                IssueSeverity::Error,
                thread,
                None,
                format!("thread {} / {branch}: call to unknown routine '{routine}'", thread + 1),
            ));
        }
    }
}

fn nested_stage_issues(
    thread: usize,
    blocks: &[Block],
    stage_count: usize,
    check_current_level: bool,
    issues: &mut Vec<ProgramIssue>,
) {
    for block in blocks {
        match block {
            Block::IgniteStage { stage } | Block::SeparateStage { stage }
                if check_current_level && *stage >= stage_count =>
            {
                issues.push(severity_issue(
                    IssueSeverity::Error,
                    thread,
                    None,
                    format!("thread {}: stage {} does not exist", thread + 1, stage + 1),
                ));
            }
            Block::If {
                then_blocks,
                else_blocks,
                ..
            } => {
                nested_stage_issues(thread, then_blocks, stage_count, true, issues);
                nested_stage_issues(thread, else_blocks, stage_count, true, issues);
            }
            Block::Repeat { body, .. } | Block::RepeatWhile { body, .. } => {
                nested_stage_issues(thread, body, stage_count, true, issues)
            }
            _ => {}
        }
    }
}

fn collect_writes(thread: usize, blocks: &[Block], flags: &mut Vec<u8>, modes: &mut Vec<u8>) {
    let _ = thread;
    for block in blocks {
        match block {
            Block::SetFlag { flag, .. } | Block::ToggleFlag { flag } => flags.push(*flag),
            Block::SetMode { mode } => modes.push(*mode),
            Block::If {
                then_blocks,
                else_blocks,
                ..
            } => {
                collect_writes(thread, then_blocks, flags, modes);
                collect_writes(thread, else_blocks, flags, modes);
            }
            Block::Repeat { body, .. } | Block::RepeatWhile { body, .. } => collect_writes(thread, body, flags, modes),
            _ => {}
        }
    }
}

fn dead_condition_issues(
    thread: usize,
    blocks: &[Block],
    flags_set: &[u8],
    modes_set: &[u8],
    issues: &mut Vec<ProgramIssue>,
) {
    for block in blocks {
        match block {
            Block::If {
                condition,
                then_blocks,
                else_blocks,
            } => {
                dead_condition(thread, *condition, flags_set, modes_set, issues);
                dead_condition_issues(thread, then_blocks, flags_set, modes_set, issues);
                dead_condition_issues(thread, else_blocks, flags_set, modes_set, issues);
            }
            Block::WaitUntil { condition, .. } | Block::RepeatWhile { condition, .. } => {
                dead_condition(thread, *condition, flags_set, modes_set, issues);
                if let Block::RepeatWhile { body, .. } = block {
                    dead_condition_issues(thread, body, flags_set, modes_set, issues);
                }
            }
            Block::Repeat { body, .. } => dead_condition_issues(thread, body, flags_set, modes_set, issues),
            _ => {}
        }
    }
}

fn dead_condition(
    thread: usize,
    condition: Condition,
    flags_set: &[u8],
    modes_set: &[u8],
    issues: &mut Vec<ProgramIssue>,
) {
    match condition {
        Condition::FlagIs { flag, value: true } if !flags_set.contains(&flag) => {
            issues.push(severity_issue(
                IssueSeverity::Warning,
                thread,
                None,
                format!(
                    "thread {}: nobody sets flag F{flag}, so the condition can only time out",
                    thread + 1
                ),
            ));
        }
        Condition::FlagIs { flag, value: false } if !flags_set.contains(&flag) => {
            issues.push(severity_issue(
                IssueSeverity::Warning,
                thread,
                None,
                format!(
                    "thread {}: F{flag} is never written, so the condition is true before any block runs",
                    thread + 1
                ),
            ));
        }
        Condition::ModeIs { mode } if !modes_set.contains(&mode) => {
            issues.push(severity_issue(
                IssueSeverity::Warning,
                thread,
                None,
                format!(
                    "thread {}: nobody selects mode M{mode}, so the condition can only time out",
                    thread + 1
                ),
            ));
        }
        _ => {}
    }
}

fn validate_routines(
    program: &Program,
    available: &std::collections::BTreeSet<&'static str>,
    issues: &mut Vec<ProgramIssue>,
) {
    let mut names = std::collections::BTreeSet::new();
    for routine in &program.routines {
        if routine.name.trim().is_empty() {
            issues.push(severity_issue(
                IssueSeverity::Error,
                0,
                None,
                "a routine needs a name".to_string(),
            ));
        } else if !names.insert(routine.name.as_str()) {
            issues.push(severity_issue(
                IssueSeverity::Error,
                0,
                None,
                format!("routine '{}' is defined twice", routine.name),
            ));
        }
        if routine.blocks.is_empty() {
            issues.push(severity_issue(
                IssueSeverity::Warning,
                0,
                None,
                format!("routine '{}' is empty", routine.name),
            ));
        }
        nested_issues(
            0,
            &routine.blocks,
            &format!("routine '{}'", routine.name),
            false,
            available,
            issues,
        );
        for called in called_routines(&routine.blocks) {
            if !program.routines.iter().any(|candidate| candidate.name == called) {
                issues.push(severity_issue(
                    IssueSeverity::Error,
                    0,
                    None,
                    format!("routine '{}': call to unknown routine '{called}'", routine.name),
                ));
            }
        }
    }
    // A call graph cycle would recurse forever; report each routine that can re-enter itself.
    for routine in &program.routines {
        let mut stack = Vec::new();
        if routine_reaches(&routine.name, &routine.name, program, &mut stack, 0) {
            issues.push(severity_issue(
                IssueSeverity::Error,
                0,
                None,
                format!("routine '{}' calls itself recursively", routine.name),
            ));
        }
    }
}

fn called_routines(blocks: &[Block]) -> Vec<&str> {
    let mut called = Vec::new();
    for block in blocks {
        match block {
            Block::Call { routine } => called.push(routine.as_str()),
            Block::If {
                then_blocks,
                else_blocks,
                ..
            } => {
                called.extend(called_routines(then_blocks));
                called.extend(called_routines(else_blocks));
            }
            Block::Repeat { body, .. } | Block::RepeatWhile { body, .. } => called.extend(called_routines(body)),
            _ => {}
        }
    }
    called
}

fn routine_reaches(start: &str, wanted: &str, program: &Program, stack: &mut Vec<String>, depth: usize) -> bool {
    if depth > MAX_CALL_DEPTH {
        return true;
    }
    let Some(routine) = program.routines.iter().find(|routine| routine.name == start) else {
        return false;
    };
    stack.push(start.to_string());
    for called in called_routines(&routine.blocks) {
        if called == wanted
            || (stack.iter().all(|name| name != called) && routine_reaches(called, wanted, program, stack, depth + 1))
        {
            stack.pop();
            return true;
        }
    }
    stack.pop();
    false
}

/// Everything that stops the program from flying, in plain language (empty = valid).
pub fn validate_program(program: &Program, vehicle: &Vehicle) -> Vec<String> {
    program_issues(program, vehicle)
        .into_iter()
        .filter(|issue| issue.severity == IssueSeverity::Error)
        .map(|issue| issue.message)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assembly::tests::sounding_rocket;
    use crate::vehicle::{ComputerClass, PartSpec, RadarClass, SensorKind};

    fn with_radar() -> Vehicle {
        let mut v = sounding_rocket();
        v.add_to_top(
            "radar",
            PartSpec::Sensor {
                kind: SensorKind::Radar(RadarClass::Basic),
            },
        )
        .unwrap();
        v
    }

    fn errors(program: &Program, vehicle: &Vehicle) -> Vec<String> {
        validate_program(program, vehicle)
    }

    fn warnings(program: &Program, vehicle: &Vehicle) -> Vec<String> {
        program_issues(program, vehicle)
            .into_iter()
            .filter(|issue| issue.severity == IssueSeverity::Warning)
            .map(|issue| issue.message)
            .collect()
    }

    #[test]
    fn valid_sequence_passes() {
        let program = Program::single(vec![
            Block::IgniteStage { stage: 0 },
            Block::Wait { seconds: 20.0 },
            Block::SeparateStage { stage: 0 },
            Block::IgniteStage { stage: 1 },
            Block::HoldSpeed { speed_ms: 300.0 },
        ]);
        assert!(errors(&program, &sounding_rocket()).is_empty());
    }

    #[test]
    fn a_basic_computer_limits_threads_and_memory() {
        let vehicle = sounding_rocket(); // basic computer: 1 thread, 8 blocks
        let program = Program {
            threads: vec![vec![Block::Wait { seconds: 1.0 }], vec![Block::Wait { seconds: 1.0 }]],
            ..Program::default()
        };
        assert!(errors(&program, &vehicle).join(" ").contains("parallel threads"));
        let long = Program::single(vec![Block::Wait { seconds: 1.0 }; 9]);
        assert!(errors(&long, &vehicle).join(" ").contains("memory"));

        let mut better = sounding_rocket();
        better.replace_part(
            "pc",
            PartSpec::Computer {
                class: ComputerClass::Standard,
            },
        );
        assert!(errors(&program, &better).is_empty());
        assert!(errors(&long, &better).is_empty(), "16 blocks fit a standard computer");
    }

    #[test]
    fn blocks_require_the_matching_sensor() {
        let vehicle = sounding_rocket(); // pitot only
        let program = Program::single(vec![Block::HoldPressure { hpa: 900.0 }, Block::LockTarget]);
        let issues = errors(&program, &vehicle).join(" | ");
        assert!(
            issues.contains("Hold 900 hPa") && issues.contains("Lock target"),
            "{issues}"
        );
        let ok = Program::single(vec![Block::HoldSpeed { speed_ms: 250.0 }, Block::LockTarget]);
        assert!(errors(&ok, &with_radar()).is_empty());
    }

    #[test]
    fn lock_target_must_close_its_thread_and_be_unique() {
        let vehicle = with_radar();
        let early = Program::single(vec![Block::LockTarget, Block::Wait { seconds: 1.0 }]);
        assert!(errors(&early, &vehicle).join(" ").contains("invalid connection"));
        let twice = Program {
            threads: vec![vec![Block::LockTarget], vec![Block::LockTarget]],
            ..Program::default()
        };
        assert!(errors(&twice, &vehicle).join(" ").contains("only one"));
    }

    #[test]
    fn invalid_parameters_and_missing_stages_are_reported() {
        let vehicle = sounding_rocket();
        let program = Program::single(vec![
            Block::IgniteStage { stage: 5 },
            Block::SetThrottle { fraction: 1.5 },
            Block::Wait { seconds: -1.0 },
            Block::HoldSpeed { speed_ms: 5.0 },
        ]);
        let issues = errors(&program, &vehicle);
        assert_eq!(issues.len(), 4, "{issues:?}");
        let mut single = sounding_rocket();
        single.remove_subtree("sustainer");
        assert!(!errors(&Program::single(vec![Block::SeparateStage { stage: 0 }]), &single).is_empty());
    }

    #[test]
    fn no_computer_means_no_program() {
        let mut vehicle = sounding_rocket();
        vehicle.remove_subtree("pc");
        assert!(errors(&Program::default(), &vehicle)[0].contains("no flight computer"));
    }

    #[test]
    fn programs_round_trip_through_json() {
        let program = Program::single(vec![
            Block::HoldPressure { hpa: 1013.0 },
            Block::Wait { seconds: 2.5 },
            Block::LockTarget,
        ]);
        let json = serde_json::to_string(&program).unwrap();
        assert_eq!(serde_json::from_str::<Program>(&json).unwrap(), program);
    }

    #[test]
    fn every_category_owns_at_least_one_block() {
        let mut found = std::collections::BTreeSet::new();
        for block in [
            Block::Wait { seconds: 1.0 },
            Block::IgniteStage { stage: 0 },
            Block::SeparateStage { stage: 0 },
            Block::SetThrottle { fraction: 1.0 },
            Block::HoldSpeed { speed_ms: 250.0 },
            Block::HoldPressure { hpa: 900.0 },
            Block::LockTarget,
            Block::WaitSpeed {
                speed_ms: 250.0,
                timeout_s: 20.0,
            },
            Block::WaitPressure {
                hpa: 500.0,
                timeout_s: 20.0,
            },
            Block::WaitFlag {
                flag: 0,
                value: true,
                timeout_s: 5.0,
            },
            Block::ToggleFlag { flag: 0 },
            Block::SetFlag { flag: 0, value: true },
            Block::SetMode { mode: 1 },
            Block::WaitMode {
                mode: 1,
                timeout_s: 5.0,
            },
            Block::Mark { mark: 1 },
        ] {
            found.insert(block.category());
        }
        for category in BlockCategory::ALL {
            assert!(found.contains(&category), "{category:?}");
        }
        assert_eq!(found.len(), BlockCategory::ALL.len());
    }

    #[test]
    fn conditions_modes_and_marks_are_range_checked() {
        let vehicle = sounding_rocket();
        let program = Program::single(vec![
            Block::WaitFlag {
                flag: 9,
                value: true,
                timeout_s: 5.0,
            },
            Block::SetMode { mode: 9 },
            Block::Mark { mark: 99 },
            Block::WaitSpeed {
                speed_ms: 250.0,
                timeout_s: 5_000.0,
            },
        ]);
        let issues = errors(&program, &vehicle).join(" | ");
        assert!(issues.contains("F0 to F3"), "{issues}");
        assert!(issues.contains("M0 to M3"), "{issues}");
        assert!(issues.contains("#0 to #7"), "{issues}");
        assert!(issues.contains("timeout"), "{issues}");
    }

    #[test]
    fn a_separation_before_an_ignition_is_an_invalid_connection() {
        let vehicle = sounding_rocket();
        let program = Program::single(vec![Block::SeparateStage { stage: 0 }, Block::IgniteStage { stage: 0 }]);
        let issues = errors(&program, &vehicle).join(" | ");
        assert!(issues.contains("invalid connection"), "{issues}");
        assert!(issues.contains("separated"), "{issues}");
    }

    #[test]
    fn dead_conditions_and_thin_memory_are_warnings() {
        let mut vehicle = sounding_rocket();
        vehicle.replace_part(
            "pc",
            PartSpec::Computer {
                class: ComputerClass::Standard,
            },
        );
        let program = Program::single(vec![Block::WaitFlag {
            flag: 2,
            value: true,
            timeout_s: 5.0,
        }]);
        let found = warnings(&program, &vehicle).join(" | ");
        assert!(found.contains("nobody sets flag F2"), "{found}");
        assert!(warnings(&Program::single(Vec::new()), &vehicle).contains(&"thread 1 is empty".to_string()));

        let full = Program::single(vec![Block::Wait { seconds: 1.0 }; 15]);
        assert!(warnings(&full, &vehicle).join(" ").contains("little memory"));
    }

    #[test]
    fn connections_link_adjacent_blocks_only() {
        let program = Program {
            threads: vec![
                vec![Block::Wait { seconds: 1.0 }, Block::Mark { mark: 0 }],
                vec![Block::IgniteStage { stage: 0 }],
            ],
            ..Program::default()
        };
        let connections = program.connections();
        assert_eq!(connections.len(), 1);
        assert_eq!(connections[0].thread, 0);
        assert_eq!((connections[0].from, connections[0].to), (0, 1));
        assert_eq!(program.max_flag(), None);
    }

    #[test]
    fn the_library_saves_replaces_lists_and_removes() {
        let mut library = ProgramLibrary::default();
        assert_eq!(library.next_name(), "Program 1");
        let first = Program::single(vec![Block::Wait { seconds: 1.0 }]);
        library.save("Launch", first.clone()).unwrap();
        assert_eq!(library.next_name(), "Program 2");
        library.save("Launch", Program::single(Vec::new())).unwrap();
        assert_eq!(library.programs.len(), 1);
        assert_eq!(library.get(0).unwrap().program, Program::single(Vec::new()));
        assert_eq!(library.remove(0).unwrap(), "Removed saved program 'Launch'");
        assert!(library.remove(0).is_err());
        assert_eq!(library.next_name(), "Program 1");
        assert!(library.save("  ", first).is_err());
    }

    #[test]
    fn the_library_round_trips_and_is_capped() {
        let mut library = ProgramLibrary::default();
        for index in 0..MAX_SAVED_PROGRAMS {
            library
                .save(format!("P{index}"), Program::single(vec![Block::Mark { mark: 0 }]))
                .unwrap();
        }
        let error = library.save("One too many", Program::single(Vec::new())).unwrap_err();
        assert!(error.contains("remove one first"), "{error}");
        let json = serde_json::to_string(&library).unwrap();
        assert_eq!(serde_json::from_str::<ProgramLibrary>(&json).unwrap(), library);
    }

    #[test]
    fn nested_blocks_count_toward_memory_and_validate_their_calls() {
        let vehicle = sounding_rocket();
        let mut program = Program::single(vec![Block::If {
            condition: Condition::FlagIs { flag: 0, value: true },
            then_blocks: vec![Block::Repeat {
                times: 2,
                body: vec![Block::Call {
                    routine: "missing".to_string(),
                }],
            }],
            else_blocks: Vec::new(),
        }]);
        assert_eq!(program.block_count(), 3);
        let found = errors(&program, &vehicle);
        assert!(
            found.iter().any(|issue| issue.contains("unknown routine 'missing'")),
            "{found:?}"
        );

        program.threads[0].push(Block::Break);
        let found = errors(&program, &vehicle);
        assert!(
            found.iter().any(|issue| issue.contains("only valid inside a repeat")),
            "{found:?}"
        );

        program.threads[0] = vec![Block::If {
            condition: Condition::FlagIs { flag: 0, value: true },
            then_blocks: vec![Block::IgniteStage { stage: 5 }],
            else_blocks: Vec::new(),
        }];
        let found = errors(&program, &vehicle);
        assert!(
            found.iter().any(|issue| issue.contains("stage 6 does not exist")),
            "{found:?}"
        );
    }

    #[test]
    fn program_layout_and_routines_round_trip_without_changing_old_threads() {
        let mut program = Program::single(vec![Block::Wait { seconds: 1.0 }]);
        program.layout.zoom = 1.4;
        program.layout.pan_x = -20.0;
        program
            .layout
            .set_position(BlockKey { thread: 0, block: 0 }, 18.0, 42.0);
        program.routines.push(Routine {
            name: "check".to_string(),
            blocks: vec![Block::Mark { mark: 0 }],
        });
        let json = serde_json::to_string(&program).unwrap();
        let loaded: Program = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, program);
        assert_eq!(loaded.layout.zoom, 1.4);
        assert_eq!(loaded.routines.len(), 1);
        let mut moved = program.clone();
        moved.layout.zoom = 0.7;
        moved.layout.set_position(BlockKey { thread: 0, block: 0 }, 500.0, 20.0);
        assert_eq!(program.semantic_copy(), moved.semantic_copy());
    }
}
