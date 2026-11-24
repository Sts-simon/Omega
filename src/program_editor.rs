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
use omega_rules::program::{Block, BlockCategory, Program, ProgramIssue, ProgramLibrary, program_issues};

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
}

impl BlockKind {
    /// Library order, one run per category.
    pub const ALL: [Self; 15] = [
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
        }
    }

    pub fn available(self, vehicle: &Vehicle) -> bool {
        vehicle.available_blocks().contains(self.default_block().kind())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProgramAction {
    Append(BlockKind),
    /// Insert after the selected block, which is how a connection is created in the middle of a
    /// thread.
    Insert(BlockKind),
    /// Copy the selected block just after itself.
    Duplicate,
    RemoveSelected,
    SelectThread(usize),
    AddThread,
    RemoveThread,
    SelectBlock(usize),
    MoveUp,
    MoveDown,
    /// `-1` or `+1` on the selected block's main value.
    Adjust(i8),
    /// `-1` or `+1` on the selected block's boolean value (flags only).
    Flip,
    Clear,
    /// Save the current program under a free default name.
    SaveCopy,
    LoadSaved(usize),
    DeleteSaved(usize),
    /// Fly the current program with a block trace and keep the report for the debugger panel.
    Debug,
}

#[derive(Resource)]
pub(crate) struct MissileProgramSession {
    pub program: Program,
    pub thread: usize,
    pub block: Option<usize>,
    /// Vertical scroll of the block list, kept across board rebuilds so a long program does not
    /// jump back to its first block after every edit.
    pub(crate) scroll: f32,
    pub(crate) library: ProgramLibrary,
    /// Last debug run; cleared by the next edit so the panel cannot show a stale timeline.
    pub(crate) debug: Option<FlightReport>,
    /// Set when the library changed, so the board system writes the library file once.
    pub(crate) library_dirty: bool,
}

impl Default for MissileProgramSession {
    fn default() -> Self {
        Self {
            program: Program {
                threads: vec![Vec::new()],
            },
            thread: 0,
            block: None,
            scroll: 0.0,
            library: ProgramLibrary::default(),
            debug: None,
            library_dirty: false,
        }
    }
}

fn round(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn cycle(value: u8, direction: i8, count: u8) -> u8 {
    if direction < 0 {
        (value + count - 1) % count
    } else {
        (value + 1) % count
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
        _ => None,
    }
}

/// `false` for blocks whose only parameter is their position in the thread.
pub fn adjustable(block: &Block) -> bool {
    !matches!(block, Block::LockTarget)
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
        self.program.threads.get(self.thread)?.get(self.block?)
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
        let block = kind.default_block();
        let thread = self.current_thread_mut();
        let index = index.map_or(thread.len(), |index| {
            if index >= thread.len() { thread.len() } else { index + 1 }
        });
        thread.insert(index, block);
        self.block = Some(index);
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
        if !matches!(action, ProgramAction::Debug) {
            self.debug = None;
        }
        let Some((thread_limit, _)) = self.limits(vehicle) else {
            return Err("This vehicle has no flight computer. Add one in the workshop first.".to_string());
        };
        match action {
            ProgramAction::Append(kind) => self.insert_at(kind, None, vehicle),
            ProgramAction::Insert(kind) => {
                let after = self.block;
                self.insert_at(kind, after, vehicle)
            }
            ProgramAction::Duplicate => {
                let Some(index) = self.block else {
                    return Err("Select a block first.".to_string());
                };
                let Some(block) = self
                    .program
                    .threads
                    .get(self.thread)
                    .and_then(|t| t.get(index))
                    .copied()
                else {
                    return Err("Select a block first.".to_string());
                };
                let kind = BlockKind::ALL
                    .into_iter()
                    .find(|kind| {
                        let default = kind.default_block();
                        std::mem::discriminant(&default) == std::mem::discriminant(&block)
                    })
                    .ok_or_else(|| "This block cannot be copied.".to_string())?;
                self.insert_at(kind, Some(index), vehicle)
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
                Ok(format!("Thread {} removed", index + 1))
            }
            ProgramAction::SelectThread(index) => {
                if index >= self.program.threads.len() {
                    return Err("No such thread".to_string());
                }
                self.scroll = 0.0;
                self.thread = index;
                self.block = None;
                Ok(String::new())
            }
            ProgramAction::SelectBlock(index) => {
                if self.program.threads.get(self.thread).is_none_or(|t| index >= t.len()) {
                    return Err("No such block".to_string());
                }
                self.block = Some(index);
                Ok(String::new())
            }
            ProgramAction::RemoveSelected => {
                let Some(index) = self.block else {
                    return Err("Select a block first.".to_string());
                };
                let thread = self.current_thread_mut();
                if index >= thread.len() {
                    return Err("Select a block first.".to_string());
                }
                thread.remove(index);
                self.block = if thread.is_empty() {
                    None
                } else {
                    Some(index.min(thread.len() - 1))
                };
                Ok("Block removed".to_string())
            }
            ProgramAction::MoveUp | ProgramAction::MoveDown => {
                let Some(index) = self.block else {
                    return Err("Select a block first.".to_string());
                };
                let target = if action == ProgramAction::MoveUp {
                    index.checked_sub(1)
                } else {
                    Some(index + 1)
                };
                let thread = self.current_thread_mut();
                let Some(target) = target.filter(|t| *t < thread.len()) else {
                    return Ok(String::new());
                };
                thread.swap(index, target);
                self.block = Some(target);
                Ok(String::new())
            }
            ProgramAction::Adjust(direction) => {
                let Some(index) = self.block else {
                    return Err("Select a block first.".to_string());
                };
                let stages = vehicle.stage_count();
                let thread = self.current_thread_mut();
                let Some(block) = thread.get(index).copied() else {
                    return Err("Select a block first.".to_string());
                };
                match adjusted(block, direction, stages) {
                    Some(changed) => {
                        thread[index] = changed;
                        Ok(String::new())
                    }
                    None => Err("'Lock target' has no adjustable value.".to_string()),
                }
            }
            ProgramAction::Flip => {
                let Some(index) = self.block else {
                    return Err("Select a block first.".to_string());
                };
                let thread = self.current_thread_mut();
                let Some(block) = thread.get(index).copied() else {
                    return Err("Select a block first.".to_string());
                };
                match flipped(block) {
                    Some(changed) => {
                        thread[index] = changed;
                        Ok(String::new())
                    }
                    None => Err("Only the flag blocks carry a boolean value.".to_string()),
                }
            }
            ProgramAction::Clear => {
                self.program = Program {
                    threads: vec![Vec::new()],
                };
                self.thread = 0;
                self.block = None;
                self.scroll = 0.0;
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
                self.scroll = 0.0;
                Ok(format!("Loaded saved program '{}'", saved.name))
            }
            ProgramAction::DeleteSaved(index) => {
                let message = self.library.remove(index)?;
                self.library_dirty = true;
                Ok(message)
            }
            ProgramAction::Debug => self.debug(vehicle),
        }
    }

    /// Fly the current program once with a block trace. Failures are the normal validation issues,
    /// so the panel explains exactly what to fix before a trial.
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
        Ok(format!("Debug run: {executed} block(s) executed"))
    }

    /// Stable content hash, ready to bind trial evidence to this exact program.
    pub fn content_hash(&self) -> Result<omega_data::ContentHash, String> {
        omega_data::hash_serializable(&self.program)
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

pub(crate) fn load_program(mut session: ResMut<MissileProgramSession>) {
    match save::read(SaveFile::MissileProgram) {
        Ok(Some(bytes)) => match MissileProgramSession::from_bytes(&bytes) {
            Ok(loaded) => *session = loaded,
            Err(error) => warn!("Could not read the saved missile program: {error}"),
        },
        Ok(None) => {}
        Err(error) => warn!("Could not read the saved missile program: {error}"),
    }
    match save::read(SaveFile::ProgramLibrary) {
        Ok(Some(bytes)) => match serde_json::from_slice::<ProgramLibrary>(&bytes) {
            Ok(library) => session.library = library,
            Err(error) => warn!("Could not read the saved program library: {error}"),
        },
        Ok(None) => {}
        Err(error) => warn!("Could not read the saved program library: {error}"),
    }
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
        session.apply(ProgramAction::SelectBlock(0), &vehicle).unwrap();
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
        session.apply(ProgramAction::SelectBlock(0), &vehicle).unwrap();
        session.apply(ProgramAction::Insert(BlockKind::Mark), &vehicle).unwrap();
        assert_eq!(session.blocks_used(), 3);
        assert!(matches!(session.program.threads[0][1], Block::Mark { .. }));
        session.apply(ProgramAction::Duplicate, &vehicle).unwrap();
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
}
