use super::model::{BlockKind, NumericComparison, Program, TemperatureComparison};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::path::Path;

const MAX_INSTRUCTIONS_PER_FRAME: usize = 64;
const MAX_INSTRUCTIONS_PER_RUN: u64 = 10_000;
const MAX_LOG_LINES: usize = 12;
const MAX_TRACE_ROWS: usize = 20_000;
const AMBIENT_TEMPERATURE_C: f64 = 20.0;
const HEATER_GAIN_C_PER_S: f64 = 4.5;
const HEAT_LOSS_PER_S: f64 = 0.08;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeStatus {
    Idle,
    Running,
    Paused,
    Completed,
    Failed,
}

impl RuntimeStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "IDLE",
            Self::Running => "RUNNING",
            Self::Paused => "PAUSED",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraceEntry {
    pub block_id: u32,
    pub simulated_time_s: f64,
    pub chamber_temperature_c: f64,
    pub heater_power: f64,
    pub variables: BTreeMap<String, f64>,
}

pub struct SequenceRuntime {
    pub status: RuntimeStatus,
    pub program_counter: usize,
    pub simulated_time_s: f64,
    pub chamber_temperature_c: f64,
    pub heater_power: f64,
    pub last_temperature_reading_c: Option<f64>,
    pub wait_remaining_s: f64,
    pub instructions_executed: u64,
    pub log: VecDeque<String>,
    pub error: Option<String>,
    pub variables: BTreeMap<String, f64>,
    pub last_executed_block: Option<u32>,
    pub trace: Vec<TraceEntry>,
    breakpoint_hit: Option<u32>,
}

impl Default for SequenceRuntime {
    fn default() -> Self {
        Self {
            status: RuntimeStatus::Idle,
            program_counter: 0,
            simulated_time_s: 0.0,
            chamber_temperature_c: AMBIENT_TEMPERATURE_C,
            heater_power: 0.0,
            last_temperature_reading_c: None,
            wait_remaining_s: 0.0,
            instructions_executed: 0,
            log: VecDeque::new(),
            error: None,
            variables: BTreeMap::new(),
            last_executed_block: None,
            trace: Vec::new(),
            breakpoint_hit: None,
        }
    }
}

impl SequenceRuntime {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn toggle_run(&mut self) {
        match self.status {
            RuntimeStatus::Running => self.status = RuntimeStatus::Paused,
            RuntimeStatus::Idle | RuntimeStatus::Paused => self.status = RuntimeStatus::Running,
            RuntimeStatus::Completed | RuntimeStatus::Failed => {
                self.reset();
                self.status = RuntimeStatus::Running;
            }
        }
    }

    pub fn step_once(&mut self, program: &Program) {
        if self.status == RuntimeStatus::Running || self.status == RuntimeStatus::Completed || self.status == RuntimeStatus::Failed {
            return;
        }
        self.status = RuntimeStatus::Paused;
        if self.wait_remaining_s > 0.0 {
            let remaining = self.wait_remaining_s;
            let wait_block_id = program.blocks.get(self.program_counter).map(|block| block.id);
            self.advance_environment(remaining);
            self.wait_remaining_s = 0.0;
            self.program_counter = self.program_counter.saturating_add(1);
            if let Some(block_id) = wait_block_id {
                self.record_trace(block_id);
            }
            self.count_instruction();
            self.push_log(format!("Paused wait completed by single-step ({remaining:.2} s)"));
            return;
        }
        if self.program_counter >= program.blocks.len() {
            self.status = RuntimeStatus::Completed;
            return;
        }
        let block = program.blocks[self.program_counter].clone();
        if let Err(message) = self.execute_block(program, block, true) {
            self.fail(message);
        }
    }

    pub fn tick(&mut self, program: &Program, delta_seconds: f64) {
        if self.status != RuntimeStatus::Running || !delta_seconds.is_finite() || delta_seconds <= 0.0 {
            return;
        }

        let mut remaining_frame_time = delta_seconds.min(0.25);
        if self.wait_remaining_s > 0.0 {
            let consumed = remaining_frame_time.min(self.wait_remaining_s);
            self.advance_environment(consumed);
            self.wait_remaining_s -= consumed;
            remaining_frame_time -= consumed;
            if self.wait_remaining_s > 1e-9 {
                return;
            }
            self.wait_remaining_s = 0.0;
            let wait_block_id = program.blocks.get(self.program_counter).map(|block| block.id);
            self.program_counter = self.program_counter.saturating_add(1);
            if let Some(block_id) = wait_block_id {
                self.record_trace(block_id);
            }
            self.count_instruction();
            self.push_log("Wait completed".to_string());
        }

        for _ in 0..MAX_INSTRUCTIONS_PER_FRAME {
            if self.status != RuntimeStatus::Running {
                break;
            }
            let Some(block) = program.blocks.get(self.program_counter).cloned() else {
                self.status = RuntimeStatus::Completed;
                break;
            };
            if block.breakpoint && self.breakpoint_hit != Some(block.id) {
                self.breakpoint_hit = Some(block.id);
                self.status = RuntimeStatus::Paused;
                self.push_log(format!("Breakpoint reached at block #{}", block.id));
                break;
            }
            if self.breakpoint_hit == Some(block.id) {
                self.breakpoint_hit = None;
            }
            if matches!(&block.kind, &BlockKind::Wait { .. }) {
                match block.kind {
                    BlockKind::Wait { duration_s } => {
                        if duration_s <= 0.0 {
                            self.program_counter += 1;
                            self.record_trace(block.id);
                            self.count_instruction();
                            continue;
                        }
                        self.wait_remaining_s = duration_s;
                        if remaining_frame_time > 0.0 {
                            let consumed = remaining_frame_time.min(self.wait_remaining_s);
                            self.advance_environment(consumed);
                            self.wait_remaining_s -= consumed;
                            remaining_frame_time -= consumed;
                            if self.wait_remaining_s <= 1e-9 {
                                self.wait_remaining_s = 0.0;
                                self.program_counter += 1;
                                self.record_trace(block.id);
                                self.push_log("Wait completed".to_string());
                                self.count_instruction();
                                continue;
                            }
                        }
                        break;
                    }
                    _ => unreachable!(),
                }
            }
            if let Err(message) = self.execute_block(program, block, false) {
                self.fail(message);
                break;
            }
        }
    }

    fn execute_block(&mut self, program: &Program, block: super::model::ProgramBlock, instant_wait: bool) -> Result<(), String> {
        if self.instructions_executed >= MAX_INSTRUCTIONS_PER_RUN {
            return Err(format!("Execution safety limit of {MAX_INSTRUCTIONS_PER_RUN} blocks reached"));
        }
        match block.kind {
            BlockKind::Start => {
                self.push_log("Sequence started".to_string());
                self.program_counter += 1;
            }
            BlockKind::Wait { duration_s } => {
                if instant_wait {
                    self.advance_environment(duration_s);
                    self.program_counter += 1;
                    self.push_log(format!("Wait advanced by {duration_s:.2} simulated seconds"));
                } else {
                    self.wait_remaining_s = duration_s;
                    if duration_s <= 0.0 {
                        self.program_counter += 1;
                    }
                }
            }
            BlockKind::SetHeaterPower { fraction } => {
                self.heater_power = fraction.clamp(0.0, 1.0);
                self.push_log(format!("Chamber heater set to {:.0}%", self.heater_power * 100.0));
                self.program_counter += 1;
            }
            BlockKind::ReadTemperature { sensor_id } => {
                self.last_temperature_reading_c = Some(self.chamber_temperature_c);
                self.push_log(format!("{sensor_id}: {:.2} C", self.chamber_temperature_c));
                self.program_counter += 1;
            }
            BlockKind::BranchTemperature { comparison, threshold_c, target_block_id } => {
                let reading = self.last_temperature_reading_c.ok_or_else(|| {
                    format!("Condition block #{} has no preceding temperature reading", block.id)
                })?;
                let condition = match comparison {
                    TemperatureComparison::GreaterOrEqual => reading >= threshold_c,
                    TemperatureComparison::LessThan => reading < threshold_c,
                };
                if condition {
                    self.program_counter = program.index_of(target_block_id)
                        .ok_or_else(|| format!("Condition target block #{target_block_id} no longer exists"))?;
                    self.push_log(format!("Condition true: jumping to block #{target_block_id}"));
                } else {
                    self.program_counter += 1;
                    self.push_log("Condition false: continuing to next block".to_string());
                }
            }
            BlockKind::SetVariable { name, value } => {
                self.variables.insert(name.clone(), value);
                self.push_log(format!("Variable {name} = {value:.3}"));
                self.program_counter += 1;
            }
            BlockKind::SetVariableExpression { name, expression } => {
                let value = super::expression::evaluate(&expression, &self.variables)?;
                self.variables.insert(name.clone(), value);
                self.push_log(format!("Variable {name} = {expression} = {value:.3}"));
                self.program_counter += 1;
            }
            BlockKind::AddVariable { name, delta } => {
                let previous = self.variables.get(&name).copied().unwrap_or(0.0);
                let updated = previous + delta;
                if !updated.is_finite() {
                    return Err(format!("Variable '{name}' became non-finite after increment"));
                }
                self.variables.insert(name.clone(), updated);
                self.push_log(format!("Variable {name} += {delta:.3} (now {updated:.3})"));
                self.program_counter += 1;
            }
            BlockKind::BranchVariable { name, comparison, threshold, target_block_id } => {
                let value = self.variables.get(&name).copied().ok_or_else(|| {
                    format!("Condition block #{} reads undefined variable '{name}'", block.id)
                })?;
                let condition = match comparison {
                    NumericComparison::GreaterOrEqual => value >= threshold,
                    NumericComparison::LessThan => value < threshold,
                    NumericComparison::Equal => (value - threshold).abs() <= 1e-9,
                };
                if condition {
                    self.program_counter = program.index_of(target_block_id)
                        .ok_or_else(|| format!("Condition target block #{target_block_id} no longer exists"))?;
                    self.push_log(format!("Variable condition true: {name}={value:.3}, jumping to #{target_block_id}"));
                } else {
                    self.program_counter += 1;
                    self.push_log(format!("Variable condition false: {name}={value:.3}"));
                }
            }
            BlockKind::Record { message } => {
                self.push_log(format!("Record: {message}"));
                self.program_counter += 1;
            }
            BlockKind::End => {
                self.heater_power = 0.0;
                self.status = RuntimeStatus::Completed;
                self.push_log("Sequence completed".to_string());
            }
        }
        self.record_trace(block.id);
        self.count_instruction();
        Ok(())
    }

    fn record_trace(&mut self, block_id: u32) {
        self.last_executed_block = Some(block_id);
        self.trace.push(TraceEntry {
            block_id,
            simulated_time_s: self.simulated_time_s,
            chamber_temperature_c: self.chamber_temperature_c,
            heater_power: self.heater_power,
            variables: self.variables.clone(),
        });
        if self.trace.len() > MAX_TRACE_ROWS {
            self.trace.remove(0);
        }
    }

    pub fn save_trace_to(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let json = serde_json::to_vec_pretty(&self.trace).map_err(|error| error.to_string())?;
        fs::write(path, json).map_err(|error| error.to_string())
    }

    fn count_instruction(&mut self) {
        self.instructions_executed = self.instructions_executed.saturating_add(1);
    }

    fn advance_environment(&mut self, delta_seconds: f64) {
        if delta_seconds <= 0.0 {
            return;
        }
        // Exact solution of a first-order lumped thermal model. This is a gameplay
        // demonstrator, not a validated thermal-design solver.
        let equilibrium = AMBIENT_TEMPERATURE_C
            + (HEATER_GAIN_C_PER_S * self.heater_power / HEAT_LOSS_PER_S);
        let decay = (-HEAT_LOSS_PER_S * delta_seconds).exp();
        self.chamber_temperature_c = equilibrium
            + (self.chamber_temperature_c - equilibrium) * decay;
        self.simulated_time_s += delta_seconds;
    }

    fn push_log(&mut self, message: String) {
        self.log.push_back(message);
        while self.log.len() > MAX_LOG_LINES {
            self.log.pop_front();
        }
    }

    fn fail(&mut self, message: String) {
        self.status = RuntimeStatus::Failed;
        self.heater_power = 0.0;
        self.error = Some(message.clone());
        self.push_log(format!("ERROR: {message}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_step_executes_one_block() {
        let program = Program::demo();
        let mut runtime = SequenceRuntime::default();
        runtime.step_once(&program);
        assert_eq!(runtime.program_counter, 1);
        assert_eq!(runtime.status, RuntimeStatus::Paused);
    }

    #[test]
    fn running_sequence_finishes_and_turns_heater_off() {
        let program = Program::demo();
        let mut runtime = SequenceRuntime::default();
        runtime.toggle_run();
        for _ in 0..2_000 {
            runtime.tick(&program, 0.05);
            if matches!(runtime.status, RuntimeStatus::Completed | RuntimeStatus::Failed) {
                break;
            }
        }
        assert_eq!(runtime.status, RuntimeStatus::Completed, "{:?}", runtime.error);
        assert_eq!(runtime.heater_power, 0.0);
        assert!(runtime.chamber_temperature_c >= 24.0);
    }

    #[test]
    fn completed_wait_is_recorded_once_in_the_trace() {
        let program = Program::demo();
        let mut runtime = SequenceRuntime::default();
        runtime.toggle_run();
        for _ in 0..60 {
            runtime.tick(&program, 0.05);
            if runtime.trace.iter().any(|entry| entry.block_id == 3) {
                break;
            }
        }
        let wait_rows = runtime.trace.iter().filter(|entry| entry.block_id == 3).count();
        assert_eq!(wait_rows, 1);
    }

    #[test]
    fn run_can_be_paused_and_resumed() {
        let program = Program::demo();
        let mut runtime = SequenceRuntime::default();
        runtime.toggle_run();
        runtime.tick(&program, 0.05);
        runtime.toggle_run();
        assert_eq!(runtime.status, RuntimeStatus::Paused);
        runtime.toggle_run();
        assert_eq!(runtime.status, RuntimeStatus::Running);
    }

    #[test]
    fn single_step_finishes_a_wait_that_was_paused_midway() {
        let program = Program::demo();
        let mut runtime = SequenceRuntime::default();
        runtime.toggle_run();
        runtime.tick(&program, 0.05);
        runtime.toggle_run();
        let old_temperature = runtime.chamber_temperature_c;
        assert!(runtime.wait_remaining_s > 0.0);
        runtime.step_once(&program);
        assert_eq!(runtime.wait_remaining_s, 0.0);
        assert_eq!(runtime.program_counter, 3);
        assert!(runtime.chamber_temperature_c > old_temperature);
    }

    #[test]
    fn variables_can_be_assigned_incremented_and_tested() {
        use super::super::model::ProgramBlock;
        use super::super::model::CanvasPosition;
        let program = Program {
            schema_version: super::super::model::CURRENT_PROGRAM_SCHEMA,
            name: "Variable test".to_string(),
            blocks: vec![
                ProgramBlock { id: 1, kind: BlockKind::Start, position: CanvasPosition::grid(0), breakpoint: false },
                ProgramBlock { id: 2, kind: BlockKind::SetVariable { name: "counter".into(), value: 2.0 }, position: CanvasPosition::grid(1), breakpoint: false },
                ProgramBlock { id: 3, kind: BlockKind::AddVariable { name: "counter".into(), delta: 1.0 }, position: CanvasPosition::grid(2), breakpoint: false },
                ProgramBlock { id: 4, kind: BlockKind::SetVariableExpression { name: "derived".into(), expression: "(counter + 1) * 2".into() }, position: CanvasPosition::grid(3), breakpoint: false },
                ProgramBlock { id: 5, kind: BlockKind::BranchVariable { name: "derived".into(), comparison: NumericComparison::GreaterOrEqual, threshold: 8.0, target_block_id: 7 }, position: CanvasPosition::grid(4), breakpoint: false },
                ProgramBlock { id: 6, kind: BlockKind::Record { message: "not reached".into() }, position: CanvasPosition::grid(5), breakpoint: false },
                ProgramBlock { id: 7, kind: BlockKind::End, position: CanvasPosition::grid(6), breakpoint: false },
            ],
        };
        assert!(program.validate().is_ok());
        let mut runtime = SequenceRuntime::default();
        runtime.toggle_run();
        for _ in 0..10 {
            runtime.tick(&program, 0.05);
            if runtime.status == RuntimeStatus::Completed || runtime.status == RuntimeStatus::Failed { break; }
        }
        assert_eq!(runtime.status, RuntimeStatus::Completed);
        assert_eq!(runtime.variables.get("counter"), Some(&3.0));
        assert_eq!(runtime.variables.get("derived"), Some(&8.0));
    }

    #[test]
    fn variable_overflow_fails_cleanly() {
        use super::super::model::{CanvasPosition, ProgramBlock};
        let program = Program {
            schema_version: super::super::model::CURRENT_PROGRAM_SCHEMA,
            name: "Overflow test".to_string(),
            blocks: vec![
                ProgramBlock { id: 1, kind: BlockKind::Start, position: CanvasPosition::grid(0), breakpoint: false },
                ProgramBlock { id: 2, kind: BlockKind::SetVariable { name: "large".into(), value: f64::MAX }, position: CanvasPosition::grid(1), breakpoint: false },
                ProgramBlock { id: 3, kind: BlockKind::AddVariable { name: "large".into(), delta: f64::MAX }, position: CanvasPosition::grid(2), breakpoint: false },
                ProgramBlock { id: 4, kind: BlockKind::End, position: CanvasPosition::grid(3), breakpoint: false },
            ],
        };
        let mut runtime = SequenceRuntime::default();
        runtime.toggle_run();
        runtime.tick(&program, 0.05);
        assert_eq!(runtime.status, RuntimeStatus::Failed);
        assert!(runtime.error.as_deref().unwrap_or_default().contains("non-finite"));
        assert_eq!(runtime.heater_power, 0.0);
    }

    #[test]
    fn breakpoint_pauses_before_executing_target_block() {
        let mut program = Program::demo();
        program.blocks[1].breakpoint = true;
        let mut runtime = SequenceRuntime::default();
        runtime.toggle_run();
        runtime.tick(&program, 0.05);
        assert_eq!(runtime.status, RuntimeStatus::Paused);
        assert_eq!(runtime.program_counter, 1);
        runtime.toggle_run();
        runtime.tick(&program, 0.05);
        assert_eq!(runtime.last_executed_block, Some(2));
    }

    #[test]
    fn runtime_rejects_condition_without_sensor_reading() {
        let mut program = Program::demo();
        program.blocks[1].kind = BlockKind::BranchTemperature {
            comparison: TemperatureComparison::GreaterOrEqual,
            threshold_c: 24.0,
            target_block_id: 10,
        };
        program.blocks[2].kind = BlockKind::End;
        program.blocks.pop();
        let mut runtime = SequenceRuntime::default();
        runtime.step_once(&program);
        runtime.step_once(&program);
        assert_eq!(runtime.status, RuntimeStatus::Failed);
    }
}
