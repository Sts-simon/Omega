//! Missile program editor: parallel threads of abstract blocks (`omega-rules::program`), limited by
//! the flight computer installed in the vehicle and by the hardware the vehicle actually carries.
//!
//! Everything here is fictional and abstract: `Lock target` only asks the flight test for a seeded
//! success roll, it is not guidance, tracking or a seeker model.

use crate::save::{self, SaveFile};
use bevy::prelude::*;
use omega_rules::Vehicle;
use omega_rules::program::{Block, Program, validate_program};

/// A block the editor can append, independent of the parameter values in the program itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlockKind {
    Wait,
    Ignite,
    Separate,
    Throttle,
    HoldSpeed,
    HoldPressure,
    LockTarget,
}

impl BlockKind {
    pub const ALL: [Self; 7] = [
        Self::Wait,
        Self::Ignite,
        Self::Separate,
        Self::Throttle,
        Self::HoldSpeed,
        Self::HoldPressure,
        Self::LockTarget,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Wait => "WAIT",
            Self::Ignite => "IGNITE STAGE",
            Self::Separate => "SEPARATE STAGE",
            Self::Throttle => "THROTTLE",
            Self::HoldSpeed => "HOLD SPEED",
            Self::HoldPressure => "HOLD PRESSURE",
            Self::LockTarget => "LOCK TARGET",
        }
    }

    /// Block identifier used by `Vehicle::available_blocks`, so the palette and the validation
    /// rules can never disagree.
    fn hardware_key(self) -> &'static str {
        match self {
            Self::Wait => "wait",
            Self::Ignite => "ignite_stage",
            Self::Separate => "separate_stage",
            Self::Throttle => "set_throttle",
            Self::HoldSpeed => "hold_speed",
            Self::HoldPressure => "hold_pressure",
            Self::LockTarget => "lock_target",
        }
    }

    pub fn available(self, vehicle: &Vehicle) -> bool {
        vehicle.available_blocks().contains(self.hardware_key())
    }

    /// What the palette inserts; every value is valid and can then be tuned in the editor.
    fn default_block(self) -> Block {
        match self {
            Self::Wait => Block::Wait { seconds: 5.0 },
            Self::Ignite => Block::IgniteStage { stage: 0 },
            Self::Separate => Block::SeparateStage { stage: 0 },
            Self::Throttle => Block::SetThrottle { fraction: 1.0 },
            Self::HoldSpeed => Block::HoldSpeed { speed_ms: 250.0 },
            Self::HoldPressure => Block::HoldPressure { hpa: 900.0 },
            Self::LockTarget => Block::LockTarget,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProgramAction {
    Append(BlockKind),
    RemoveSelected,
    SelectThread(usize),
    AddThread,
    RemoveThread,
    SelectBlock(usize),
    MoveUp,
    MoveDown,
    /// `-1` or `+1` on the selected block's only adjustable value.
    Adjust(i8),
    Clear,
}

#[derive(Resource)]
pub(crate) struct MissileProgramSession {
    pub program: Program,
    pub thread: usize,
    pub block: Option<usize>,
    /// Vertical scroll of the block list, kept across board rebuilds so a long program does not
    /// jump back to its first block after every edit.
    pub(crate) scroll: f32,
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
        }
    }
}

fn round(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
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
        Block::LockTarget => None,
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

    /// Everything that stops this program from flying, in plain language.
    pub fn issues(&self, vehicle: &Vehicle) -> Vec<String> {
        if self.program.threads.iter().all(Vec::is_empty) {
            return vec!["The program is empty: add blocks to a thread.".to_string()];
        }
        validate_program(&self.program, vehicle)
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

    /// Apply an editor action. `Ok` means the program changed and must be saved; `Err` is a refusal
    /// shown to the player.
    pub fn apply(&mut self, action: ProgramAction, vehicle: &Vehicle) -> Result<String, String> {
        let Some((thread_limit, block_limit)) = self.limits(vehicle) else {
            return Err("This vehicle has no flight computer. Add one in the workshop first.".to_string());
        };
        match action {
            ProgramAction::Append(kind) => {
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
                thread.push(block);
                self.block = Some(thread.len() - 1);
                Ok(format!("Added '{}' to thread {}", kind.label(), self.thread + 1))
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
            ProgramAction::Clear => {
                self.program = Program {
                    threads: vec![Vec::new()],
                };
                self.thread = 0;
                self.block = None;
                self.scroll = 0.0;
                Ok("Program cleared".to_string())
            }
        }
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
        Ok(Self {
            program,
            thread: 0,
            block: None,
            scroll: 0.0,
        })
    }
}

pub(crate) fn persist(session: &MissileProgramSession) -> Result<(), String> {
    session
        .to_bytes()
        .and_then(|bytes| save::write(SaveFile::MissileProgram, &bytes))
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
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn an_empty_program_says_so_and_json_round_trips() {
        let vehicle = basic();
        let session = MissileProgramSession::default();
        assert!(session.issues(&vehicle)[0].contains("empty"));
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
        assert!(session.issues(&vehicle)[0].contains("empty"));
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
}
