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

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
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
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub threads: Vec<Vec<Block>>,
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
        self.threads.iter().map(Vec::len).sum()
    }

    /// One thread from a block list, handy for tests and presets.
    pub fn single(blocks: Vec<Block>) -> Self {
        Self { threads: vec![blocks] }
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
        self.threads
            .iter()
            .flatten()
            .filter_map(|block| match block {
                Block::WaitFlag { flag, .. } | Block::ToggleFlag { flag } | Block::SetFlag { flag, .. } => Some(*flag),
                _ => None,
            })
            .max()
    }

    pub fn max_mode(&self) -> Option<u8> {
        self.threads
            .iter()
            .flatten()
            .filter_map(|block| match block {
                Block::SetMode { mode } | Block::WaitMode { mode, .. } => Some(*mode),
                _ => None,
            })
            .max()
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
            match *block {
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
    let flags_set: Vec<u8> = program
        .threads
        .iter()
        .flatten()
        .filter_map(|block| match block {
            Block::SetFlag { flag, .. } | Block::ToggleFlag { flag } => Some(*flag),
            _ => None,
        })
        .collect();
    let modes_set: Vec<u8> = program
        .threads
        .iter()
        .flatten()
        .filter_map(|block| match block {
            Block::SetMode { mode } => Some(*mode),
            _ => None,
        })
        .collect();
    for (t, thread) in program.threads.iter().enumerate() {
        for (b, block) in thread.iter().enumerate() {
            match *block {
                Block::WaitFlag { flag, .. } if !flags_set.contains(&flag) => {
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
    issues
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
}
