use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

pub const CURRENT_PROGRAM_SCHEMA: u32 = 2;
pub const MAX_PROGRAM_BLOCKS: usize = 128;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct CanvasPosition {
    pub x: f32,
    pub y: f32,
}

impl CanvasPosition {
    pub fn grid(index: usize) -> Self {
        let column = index % 4;
        let row = index / 4;
        Self {
            x: 20.0 + column as f32 * 258.0,
            y: 104.0 + row as f32 * 142.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Program {
    pub schema_version: u32,
    pub name: String,
    pub blocks: Vec<ProgramBlock>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ProgramBlock {
    pub id: u32,
    pub kind: BlockKind,
    #[serde(default)]
    pub position: CanvasPosition,
    #[serde(default)]
    pub breakpoint: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockKind {
    Start,
    Wait { duration_s: f64 },
    SetHeaterPower { fraction: f64 },
    ReadTemperature { sensor_id: String },
    BranchTemperature {
        comparison: TemperatureComparison,
        threshold_c: f64,
        target_block_id: u32,
    },
    SetVariable { name: String, value: f64 },
    SetVariableExpression { name: String, expression: String },
    AddVariable { name: String, delta: f64 },
    BranchVariable {
        name: String,
        comparison: NumericComparison,
        threshold: f64,
        target_block_id: u32,
    },
    Record { message: String },
    End,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TemperatureComparison {
    GreaterOrEqual,
    LessThan,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NumericComparison {
    GreaterOrEqual,
    LessThan,
    Equal,
}

impl Program {
    pub fn demo() -> Self {
        let kinds = vec![
            BlockKind::Start,
            BlockKind::SetHeaterPower { fraction: 0.65 },
            BlockKind::Wait { duration_s: 2.0 },
            BlockKind::ReadTemperature { sensor_id: "chamber_01".to_string() },
            BlockKind::BranchTemperature {
                comparison: TemperatureComparison::GreaterOrEqual,
                threshold_c: 24.0,
                target_block_id: 10,
            },
            BlockKind::Record { message: "Target not reached; continuing the test".to_string() },
            BlockKind::Wait { duration_s: 2.0 },
            BlockKind::ReadTemperature { sensor_id: "chamber_01".to_string() },
            BlockKind::BranchTemperature {
                comparison: TemperatureComparison::LessThan,
                threshold_c: 24.0,
                target_block_id: 3,
            },
            BlockKind::SetHeaterPower { fraction: 0.0 },
            BlockKind::SetVariable { name: "cycles_done".to_string(), value: 1.0 },
            BlockKind::Record { message: "Thermal acceptance cycle completed".to_string() },
            BlockKind::End,
        ];
        let blocks = kinds
            .into_iter()
            .enumerate()
            .map(|(index, kind)| ProgramBlock {
                id: (index + 1) as u32,
                kind,
                position: CanvasPosition::grid(index),
                breakpoint: false,
            })
            .collect();
        Self {
            schema_version: CURRENT_PROGRAM_SCHEMA,
            name: "Chamber thermal acceptance test".to_string(),
            blocks,
        }
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut issues = Vec::new();
        if self.schema_version != CURRENT_PROGRAM_SCHEMA {
            issues.push(format!("Unsupported program schema {}", self.schema_version));
        }
        if self.name.trim().is_empty() || self.name.len() > 100 {
            issues.push("Program name must contain 1 to 100 characters".to_string());
        }
        if self.blocks.len() < 2 {
            issues.push("A program needs at least a start and an end block".to_string());
        }
        if self.blocks.len() > MAX_PROGRAM_BLOCKS {
            issues.push(format!("Program exceeds the {MAX_PROGRAM_BLOCKS}-block limit"));
        }

        let mut ids = HashSet::new();
        for block in &self.blocks {
            if block.id == 0 || !ids.insert(block.id) {
                issues.push(format!("Block identifier {} is empty or duplicated", block.id));
            }
            if !block.position.x.is_finite()
                || !block.position.y.is_finite()
                || block.position.x.abs() > 20_000.0
                || block.position.y.abs() > 20_000.0
            {
                issues.push(format!("Block #{} has an invalid canvas position", block.id));
            }
        }

        if let Some(first) = self.blocks.first() {
            if !matches!(&first.kind, &BlockKind::Start) {
                issues.push("The first block must be Start".to_string());
            }
        }
        if let Some(last) = self.blocks.last() {
            if !matches!(&last.kind, &BlockKind::End) {
                issues.push("The last block must be End".to_string());
            }
        }
        if self.blocks.iter().skip(1).any(|block| matches!(&block.kind, &BlockKind::Start)) {
            issues.push("Start can only appear as the first block".to_string());
        }
        if self.blocks.iter().take(self.blocks.len().saturating_sub(1)).any(|block| matches!(&block.kind, &BlockKind::End)) {
            issues.push("End can only appear as the last block".to_string());
        }

        for block in &self.blocks {
            match &block.kind {
                BlockKind::Wait { duration_s } => {
                    if !duration_s.is_finite() || *duration_s < 0.0 || *duration_s > 86_400.0 {
                        issues.push(format!("Block #{} has an invalid wait duration", block.id));
                    }
                }
                BlockKind::SetHeaterPower { fraction } => {
                    if !fraction.is_finite() || !(0.0..=1.0).contains(fraction) {
                        issues.push(format!("Block #{} heater power must be between 0 and 1", block.id));
                    }
                }
                BlockKind::ReadTemperature { sensor_id } => {
                    if sensor_id.trim().is_empty() || sensor_id.len() > 64 {
                        issues.push(format!("Block #{} has an invalid sensor identifier", block.id));
                    }
                }
                BlockKind::BranchTemperature { threshold_c, target_block_id, .. } => {
                    if !threshold_c.is_finite() || !(-100.0..=1_000.0).contains(threshold_c) {
                        issues.push(format!("Block #{} has an invalid temperature threshold", block.id));
                    }
                    if !self.blocks.iter().any(|candidate| candidate.id == *target_block_id) {
                        issues.push(format!("Block #{} targets missing block #{}", block.id, target_block_id));
                    }
                }
                BlockKind::SetVariable { name, value } => {
                    if !valid_variable_name(name) || !value.is_finite() {
                        issues.push(format!("Block #{} has an invalid variable assignment", block.id));
                    }
                }
                BlockKind::SetVariableExpression { name, expression } => {
                    if !valid_variable_name(name)
                        || expression.trim().is_empty()
                        || expression.len() > 128
                        || expression.chars().any(char::is_control)
                    {
                        issues.push(format!("Block #{} has an invalid variable expression", block.id));
                    }
                }
                BlockKind::AddVariable { name, delta } => {
                    if !valid_variable_name(name) || !delta.is_finite() {
                        issues.push(format!("Block #{} has an invalid variable increment", block.id));
                    }
                }
                BlockKind::BranchVariable { name, threshold, target_block_id, .. } => {
                    if !valid_variable_name(name) || !threshold.is_finite() {
                        issues.push(format!("Block #{} has an invalid variable condition", block.id));
                    }
                    if !self.blocks.iter().any(|candidate| candidate.id == *target_block_id) {
                        issues.push(format!("Block #{} targets missing block #{}", block.id, target_block_id));
                    }
                }
                BlockKind::Record { message } => {
                    if message.trim().is_empty() || message.len() > 160 {
                        issues.push(format!("Block #{} has an invalid record message", block.id));
                    }
                }
                BlockKind::Start | BlockKind::End => {}
            }
        }

        if issues.is_empty() { Ok(()) } else { Err(issues) }
    }

    pub fn next_id(&self) -> u32 {
        self.blocks.iter().map(|block| block.id).max().unwrap_or(0).saturating_add(1)
    }

    pub fn index_of(&self, id: u32) -> Option<usize> {
        self.blocks.iter().position(|block| block.id == id)
    }

    pub fn insert_after(&mut self, selected_id: u32, kind: BlockKind) -> Result<u32, String> {
        if self.blocks.len() >= MAX_PROGRAM_BLOCKS {
            return Err(format!("Program is limited to {MAX_PROGRAM_BLOCKS} blocks"));
        }
        let selected_index = self.index_of(selected_id).ok_or_else(|| "Selected block no longer exists".to_string())?;
        let selected_position = self.blocks[selected_index].position;
        let insertion_index = if matches!(&self.blocks[selected_index].kind, &BlockKind::End) {
            selected_index
        } else {
            selected_index + 1
        };
        let id = self.next_id();
        let mut candidate = self.clone();
        candidate.blocks.insert(insertion_index, ProgramBlock {
            id,
            kind,
            position: CanvasPosition {
                x: selected_position.x,
                y: selected_position.y + 118.0,
            },
            breakpoint: false,
        });
        candidate.validate().map_err(|issues| issues.join("; "))?;
        self.blocks = candidate.blocks;
        Ok(id)
    }

    pub fn remove(&mut self, block_id: u32) -> Result<(), String> {
        let index = self.index_of(block_id).ok_or_else(|| "Selected block no longer exists".to_string())?;
        if index == 0 || index + 1 == self.blocks.len() {
            return Err("Start and End blocks cannot be removed".to_string());
        }
        if self.blocks.iter().any(|block| matches!(
            &block.kind,
            BlockKind::BranchTemperature { target_block_id, .. }
                | BlockKind::BranchVariable { target_block_id, .. } if *target_block_id == block_id
        )) {
            return Err("Another condition jumps to this block; retarget it before removal".to_string());
        }
        self.blocks.remove(index);
        Ok(())
    }

    pub fn move_block(&mut self, block_id: u32, direction: isize) -> bool {
        let Some(index) = self.index_of(block_id) else { return false };
        let target = index as isize + direction;
        if index == 0 || index + 1 == self.blocks.len() || target <= 0 || target >= self.blocks.len() as isize - 1 {
            return false;
        }
        self.blocks.swap(index, target as usize);
        true
    }

    pub fn toggle_breakpoint(&mut self, block_id: u32) -> Result<bool, String> {
        let block = self.blocks.iter_mut().find(|block| block.id == block_id)
            .ok_or_else(|| "Selected block no longer exists".to_string())?;
        block.breakpoint = !block.breakpoint;
        Ok(block.breakpoint)
    }

    /// Validated JSON form used by every save path.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate().map_err(|issues| issues.join("; "))?;
        serde_json::to_vec_pretty(self).map_err(|error| error.to_string())
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let json = self.to_bytes()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(path, json).map_err(|error| error.to_string())
    }

    pub fn load_from(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|error| error.to_string())?;
        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut program: Self = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        if program.schema_version == 1 {
            for (index, block) in program.blocks.iter_mut().enumerate() {
                block.position = CanvasPosition::grid(index);
                block.breakpoint = false;
            }
            program.schema_version = CURRENT_PROGRAM_SCHEMA;
        }
        program.validate().map_err(|issues| issues.join("; "))?;
        Ok(program)
    }
}

fn valid_variable_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name.chars().enumerate().all(|(index, character)| {
            character == '_' || character.is_ascii_alphanumeric() && (index > 0 || !character.is_ascii_digit())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demonstration_program_is_valid() {
        assert!(Program::demo().validate().is_ok());
    }

    #[test]
    fn missing_branch_target_is_rejected() {
        let mut program = Program::demo();
        let branch = program.blocks.iter_mut().find(|block| matches!(&block.kind, &BlockKind::BranchTemperature { .. })).unwrap();
        if let BlockKind::BranchTemperature { target_block_id, .. } = &mut branch.kind {
            *target_block_id = 999;
        }
        assert!(program.validate().is_err());
    }

    #[test]
    fn block_order_changes_do_not_break_id_based_targets() {
        let mut program = Program::demo();
        let target = match &program.blocks[4].kind {
            BlockKind::BranchTemperature { target_block_id, .. } => *target_block_id,
            _ => unreachable!(),
        };
        assert!(program.move_block(6, 1));
        assert!(matches!(&program.blocks[4].kind, BlockKind::BranchTemperature { target_block_id, .. } if *target_block_id == target));
    }

    #[test]
    fn referenced_block_cannot_be_removed() {
        let mut program = Program::demo();
        assert!(program.remove(10).is_err());
    }

    #[test]
    fn insertion_keeps_start_and_end_boundaries() {
        let mut program = Program::demo();
        let inserted = program.insert_after(1, BlockKind::Wait { duration_s: 1.0 }).unwrap();
        assert!(matches!(&program.blocks.first().unwrap().kind, &BlockKind::Start));
        assert!(matches!(&program.blocks.last().unwrap().kind, &BlockKind::End));
        assert!(program.index_of(inserted).is_some());
        assert!(program.validate().is_ok());
    }

    #[test]
    fn invalid_canvas_coordinates_are_rejected() {
        let mut program = Program::demo();
        program.blocks[1].position.x = f32::NAN;
        assert!(program.validate().is_err());
    }

    #[test]
    fn variable_names_are_validated() {
        let mut program = Program::demo();
        program.blocks[1].kind = BlockKind::SetVariable { name: "1bad".into(), value: 2.0 };
        assert!(program.validate().is_err());
    }

    #[test]
    fn version_one_programs_migrate_canvas_layout() {
        let old = r#"{
            "schema_version": 1,
            "name": "Old sequence",
            "blocks": [
                {"id": 1, "kind": {"type": "start"}},
                {"id": 2, "kind": {"type": "end"}}
            ]
        }"#;
        let path = std::env::temp_dir().join(format!("omega-sequence-migration-{}.json", std::process::id()));
        fs::write(&path, old).unwrap();
        let program = Program::load_from(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert_eq!(program.schema_version, CURRENT_PROGRAM_SCHEMA);
        assert!(program.validate().is_ok());
        assert_eq!(program.blocks[1].position, CanvasPosition::grid(1));
        assert!(!program.blocks[1].breakpoint);
    }
}
