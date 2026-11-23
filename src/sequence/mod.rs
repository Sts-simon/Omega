mod expression;
mod graph;
mod model;
mod runtime;

pub use graph::{handle_mouse as graph_input, setup as setup_graph_ui, sync as sync_graph_ui};

use bevy::prelude::*;
use crate::campus::{CampusState, CampusView};
use model::{BlockKind, Program, ProgramBlock, TemperatureComparison};
use runtime::{RuntimeStatus, SequenceRuntime};
use std::path::Path;

#[derive(Resource)]
pub struct SequenceWorkspace {
    pub program: Program,
    pub runtime: SequenceRuntime,
    selected_block_id: u32,
    message: String,
    graph_open: bool,
    dragging_block: Option<u32>,
    drag_offset: Vec2,
}

impl Default for SequenceWorkspace {
    fn default() -> Self {
        let program = Program::demo();
        Self {
            selected_block_id: 2,
            program,
            runtime: SequenceRuntime::default(),
            message: "Demo sequence loaded".to_string(),
            graph_open: false,
            dragging_block: None,
            drag_offset: Vec2::ZERO,
        }
    }
}

#[derive(Component)]
pub struct SequenceHud;

impl SequenceWorkspace {
    fn selected_index(&self) -> Option<usize> {
        self.program.index_of(self.selected_block_id)
    }

    pub fn selected_block_id(&self) -> u32 {
        self.selected_block_id
    }

    pub fn graph_is_open(&self) -> bool {
        self.graph_open
    }

    pub fn toggle_graph_from_ui(&mut self) {
        self.graph_open = !self.graph_open;
        self.dragging_block = None;
        self.message = if self.graph_open {
            "Visual graph editor opened".to_string()
        } else {
            "Visual graph editor closed".to_string()
        };
    }

    fn select_relative(&mut self, offset: isize) {
        if self.program.blocks.is_empty() {
            return;
        }
        let current = self.selected_index().unwrap_or(0) as isize;
        let next = (current + offset).clamp(0, self.program.blocks.len() as isize - 1) as usize;
        self.selected_block_id = self.program.blocks[next].id;
    }

    fn ensure_editable(&mut self) -> bool {
        if self.runtime.status == RuntimeStatus::Running {
            self.message = "Pause the sequence before editing it".to_string();
            return false;
        }
        if self.runtime.status != RuntimeStatus::Idle {
            self.runtime.reset();
        }
        true
    }

    fn add_block(&mut self, kind: BlockKind) {
        if !self.ensure_editable() {
            return;
        }
        match self.program.insert_after(self.selected_block_id, kind) {
            Ok(id) => {
                self.selected_block_id = id;
                self.message = format!("Added block #{id}");
            }
            Err(error) => self.message = error,
        }
    }

    fn add_condition(&mut self, comparison: TemperatureComparison) {
        let target = self.program.blocks.last().map(|block| block.id).unwrap_or(0);
        self.add_block(BlockKind::BranchTemperature {
            comparison,
            threshold_c: 24.0,
            target_block_id: target,
        });
    }

    fn cycle_branch_target(&mut self, direction: isize) {
        if !self.ensure_editable() {
            return;
        }
        let Some(selected_index) = self.selected_index() else { return };
        let current_target = match &self.program.blocks[selected_index].kind {
            BlockKind::BranchTemperature { target_block_id, .. }
            | BlockKind::BranchVariable { target_block_id, .. } => *target_block_id,
            _ => {
                self.message = "Select a condition block to change its jump target".to_string();
                return;
            }
        };
        let Some(target_index) = self.program.index_of(current_target) else {
            self.message = "Current branch target is missing".to_string();
            return;
        };
        let last = self.program.blocks.len() as isize - 1;
        let next_index = (target_index as isize + direction).clamp(0, last) as usize;
        let next_id = self.program.blocks[next_index].id;
        match &mut self.program.blocks[selected_index].kind {
            BlockKind::BranchTemperature { target_block_id, .. }
            | BlockKind::BranchVariable { target_block_id, .. } => {
                *target_block_id = next_id;
                self.message = format!("Condition now jumps to block #{next_id}");
            }
            _ => {}
        }
    }

    fn remove_selected(&mut self) {
        if !self.ensure_editable() {
            return;
        }
        let old_index = self.selected_index().unwrap_or(0);
        match self.program.remove(self.selected_block_id) {
            Ok(()) => {
                let next_index = old_index.saturating_sub(1).min(self.program.blocks.len() - 1);
                self.selected_block_id = self.program.blocks[next_index].id;
                self.message = "Selected block removed".to_string();
            }
            Err(error) => self.message = error,
        }
    }

    fn move_selected(&mut self, direction: isize) {
        if !self.ensure_editable() {
            return;
        }
        if self.program.move_block(self.selected_block_id, direction) {
            self.message = "Block order changed".to_string();
        } else {
            self.message = "Start and End stay fixed; block cannot move farther".to_string();
        }
    }

    fn adjust_selected(&mut self, delta: f64) {
        if !self.ensure_editable() || !delta.is_finite() {
            return;
        }
        let Some(block) = self.program.blocks.iter_mut().find(|block| block.id == self.selected_block_id) else {
            return;
        };
        match &mut block.kind {
            BlockKind::Wait { duration_s } => {
                *duration_s = (*duration_s + delta).clamp(0.0, 3_600.0);
                self.message = format!("Wait duration: {:.2} s", *duration_s);
            }
            BlockKind::SetHeaterPower { fraction } => {
                *fraction = (*fraction + delta * 0.1).clamp(0.0, 1.0);
                self.message = format!("Heater power: {:.0}%", *fraction * 100.0);
            }
            BlockKind::BranchTemperature { threshold_c, .. } => {
                *threshold_c = (*threshold_c + delta).clamp(-100.0, 1_000.0);
                self.message = format!("Temperature threshold: {:.1} C", *threshold_c);
            }
            BlockKind::SetVariable { value, .. } => {
                *value += delta;
                self.message = format!("Variable value: {:.3}", *value);
            }
            BlockKind::AddVariable { delta: amount, .. } => {
                *amount += delta;
                self.message = format!("Variable increment: {:.3}", *amount);
            }
            BlockKind::BranchVariable { threshold, .. } => {
                *threshold += delta;
                self.message = format!("Variable threshold: {:.3}", *threshold);
            }
            _ => self.message = "This block has no editable numeric parameter".to_string(),
        }
    }

    fn toggle_breakpoint(&mut self) {
        if !self.ensure_editable() {
            return;
        }
        match self.program.toggle_breakpoint(self.selected_block_id) {
            Ok(enabled) => self.message = if enabled {
                format!("Breakpoint enabled on block #{}", self.selected_block_id)
            } else {
                format!("Breakpoint removed from block #{}", self.selected_block_id)
            },
            Err(error) => self.message = error,
        }
    }

    fn save_trace(&mut self) {
        match self.runtime.save_trace_to(Path::new("results/sequence_trace.json")) {
            Ok(()) => self.message = "Execution trace saved to results/sequence_trace.json".to_string(),
            Err(error) => self.message = format!("Trace export failed: {error}"),
        }
    }

    fn save(&mut self) {
        match self.program.to_bytes().and_then(|bytes| crate::save::write(crate::save::SaveFile::Program, &bytes)) {
            Ok(()) => self.message = "Program saved to the save slot".to_string(),
            Err(error) => self.message = format!("Save failed: {error}"),
        }
    }

    fn load(&mut self) {
        let loaded = crate::save::read(crate::save::SaveFile::Program)
            .and_then(|bytes| bytes.ok_or_else(|| "no saved program found".to_string()))
            .and_then(|bytes| Program::from_bytes(&bytes));
        match loaded {
            Ok(program) => {
                self.selected_block_id = program.blocks.first().map(|block| block.id).unwrap_or(0);
                self.program = program;
                self.runtime.reset();
                self.dragging_block = None;
                self.message = "Program loaded; runtime reset".to_string();
            }
            Err(error) => self.message = format!("Load failed: {error}"),
        }
    }

    fn toggle_run(&mut self) {
        if self.runtime.status != RuntimeStatus::Running {
            if let Err(issues) = self.program.validate() {
                self.runtime.status = RuntimeStatus::Failed;
                self.runtime.error = Some(issues.join("; "));
                self.message = "Program validation failed; see runtime error".to_string();
                return;
            }
        }
        self.runtime.toggle_run();
        self.message = format!("Sequence {}", self.runtime.status.label().to_lowercase());
    }

    fn step_once(&mut self) {
        if let Err(issues) = self.program.validate() {
            self.message = format!("Cannot step: {}", issues.join("; "));
            return;
        }
        self.runtime.step_once(&self.program);
        self.message = format!("Executed one block; runtime {}", self.runtime.status.label());
    }
}

pub fn setup_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("AEROLAB | Lab sequence editor\nLoading program..."),
        TextFont {
            font_size: FontSize::Px(13.0),
            ..default()
        },
        TextColor(Color::srgb(0.93, 0.96, 0.98)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(14.0),
            left: Val::Px(14.0),
            max_width: Val::Px(660.0),
            max_height: Val::Px(430.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        },
        Interaction::None,
        BackgroundColor(Color::srgba(0.025, 0.04, 0.055, 0.91)),
        SequenceHud,
    ));
}

pub fn handle_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    campus: Res<CampusState>,
    sketch: Res<crate::design::PolygonSketchSession>,
    mut workspace: ResMut<SequenceWorkspace>,
) {
    if campus.view != CampusView::Programming || campus.profile_editor_open || sketch.active {
        return;
    }
    let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    // F6 is the context switch. The sequencer's editing shortcuts are active only
    // while its graph is open, avoiding collisions with the 3D assembly editor.
    if keys.just_pressed(KeyCode::F6) && !control {
        workspace.graph_open = !workspace.graph_open;
        workspace.dragging_block = None;
        workspace.message = (if workspace.graph_open {
            "Visual graph editor opened"
        } else {
            "Visual graph editor closed"
        })
        .to_string();
        return;
    }
    if keys.just_pressed(KeyCode::Escape) && workspace.graph_open {
        workspace.graph_open = false;
        workspace.dragging_block = None;
        workspace.message = "Visual graph editor closed".to_string();
        return;
    }
    if !workspace.graph_open {
        return;
    }

    if keys.just_pressed(KeyCode::Space) && !control {
        workspace.toggle_run();
    }
    if keys.just_pressed(KeyCode::KeyN) && !control {
        workspace.step_once();
    }

    if keys.just_pressed(KeyCode::F1) && !control {
        workspace.add_block(BlockKind::Wait { duration_s: 2.0 });
    }
    if keys.just_pressed(KeyCode::F2) && !control {
        workspace.add_block(BlockKind::ReadTemperature {
            sensor_id: "chamber_01".to_string(),
        });
    }
    if keys.just_pressed(KeyCode::F3) && !shift && !control {
        workspace.add_condition(TemperatureComparison::GreaterOrEqual);
    }
    if keys.just_pressed(KeyCode::F3) && shift && !control {
        workspace.add_condition(TemperatureComparison::LessThan);
    }
    if keys.just_pressed(KeyCode::F4) && !control {
        workspace.add_block(BlockKind::SetHeaterPower { fraction: 0.5 });
    }
    if keys.just_pressed(KeyCode::F5) && !control {
        workspace.add_block(BlockKind::SetVariable {
            name: "sample_count".to_string(),
            value: 0.0,
        });
    }
    if keys.just_pressed(KeyCode::F7) && !control {
        workspace.add_block(BlockKind::AddVariable {
            name: "sample_count".to_string(),
            delta: 1.0,
        });
    }
    if keys.just_pressed(KeyCode::F8) && !control {
        let target = workspace.program.blocks.last().map(|block| block.id).unwrap_or(0);
        workspace.add_block(BlockKind::BranchVariable {
            name: "sample_count".to_string(),
            comparison: model::NumericComparison::GreaterOrEqual,
            threshold: 3.0,
            target_block_id: target,
        });
    }
    if keys.just_pressed(KeyCode::F9) && !control {
        workspace.add_block(BlockKind::SetVariableExpression {
            name: "derived_value".to_string(),
            expression: "(sample_count + 1) * 2".to_string(),
        });
    }
    if keys.just_pressed(KeyCode::KeyB) && !control {
        workspace.toggle_breakpoint();
    }
    if keys.just_pressed(KeyCode::F11) && !control {
        workspace.add_block(BlockKind::Record {
            message: "Operator checkpoint".to_string(),
        });
    }
    if keys.just_pressed(KeyCode::F12) && shift && !control {
        workspace.remove_selected();
    }

    if keys.just_pressed(KeyCode::PageUp) && !control {
        if shift {
            workspace.move_selected(-1);
        } else {
            workspace.select_relative(-1);
        }
    }
    if keys.just_pressed(KeyCode::PageDown) && !control {
        if shift {
            workspace.move_selected(1);
        } else {
            workspace.select_relative(1);
        }
    }
    if keys.just_pressed(KeyCode::KeyT) && !control {
        workspace.cycle_branch_target(if shift { -1 } else { 1 });
    }
    if (keys.just_pressed(KeyCode::Equal) || keys.just_pressed(KeyCode::NumpadAdd)) && !control {
        workspace.adjust_selected(1.0);
    }
    if (keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::NumpadSubtract)) && !control {
        workspace.adjust_selected(-1.0);
    }

    if control && keys.just_pressed(KeyCode::F7) {
        workspace.save_trace();
    }
    if control && keys.just_pressed(KeyCode::F8) {
        workspace.save();
    }
    if control && keys.just_pressed(KeyCode::F9) {
        workspace.load();
    }
    if control && keys.just_pressed(KeyCode::F10) {
        workspace.runtime.reset();
        workspace.message = "Runtime reset".to_string();
    }
}

pub fn tick(
    time: Res<Time>,
    campus: Res<CampusState>,
    mut workspace: ResMut<SequenceWorkspace>,
) {
    if campus.view != CampusView::Programming || campus.profile_editor_open {
        return;
    }
    let delta = time.delta_secs_f64();
    let SequenceWorkspace { program, runtime, .. } = &mut *workspace;
    runtime.tick(program, delta);
}

pub fn update_hud(
    workspace: Res<SequenceWorkspace>,
    campus: Res<CampusState>,
    mut hud: Query<(&mut Text, &mut Node), With<SequenceHud>>,
) {
    let Ok((mut text, mut node)) = hud.single_mut() else { return };
    node.display = if campus.view == CampusView::Programming && !campus.profile_editor_open && !workspace.graph_is_open() {
        Display::Flex
    } else {
        Display::None
    };
    if campus.view != CampusView::Programming || workspace.graph_is_open() {
        return;
    }
    let mut lines = vec![format!(
        "AEROLAB 0.26.0 | LAB SEQUENCER   [{}]",
        workspace.runtime.status.label()
    )];
    lines.push("F6 open graph | Space run/pause | N step | B breakpoint | F1 wait | F2 read | F3 temp condition".to_string());
    lines.push("F4 heater | F5 set variable | F7 increment | F8 variable condition | F11 record".to_string());
    lines.push("Shift+F12 delete | PgUp/PgDn select | Ctrl+F7 export trace".to_string());
    lines.push("Shift+PgUp/PgDn reorder | T / Shift+T change branch target | +/- edit value".to_string());
    lines.push("Ctrl+F8 save | Ctrl+F9 load | Ctrl+F10 reset".to_string());
    lines.push(format!("\nPROGRAM: {}  |  {} blocks", workspace.program.name, workspace.program.blocks.len()));

    let selected_index = workspace.program.index_of(workspace.selected_block_id).unwrap_or(0);
    let visible_start = selected_index.saturating_sub(5);
    let visible_blocks = workspace.program.blocks.iter().skip(visible_start).take(12);
    if visible_start > 0 {
        lines.push("... earlier blocks ...".to_string());
    }
    for block in visible_blocks {
        let marker = if block.id == workspace.selected_block_id { ">" } else { " " };
        lines.push(format!("{marker} {}", format_block(block)));
    }
    if visible_start + 12 < workspace.program.blocks.len() {
        lines.push("... later blocks ...".to_string());
    }

    lines.push(format!(
        "\nRuntime: T={:.2} C | heater={:.0}% | sim={:.1} s | PC={} | instructions={}",
        workspace.runtime.chamber_temperature_c,
        workspace.runtime.heater_power * 100.0,
        workspace.runtime.simulated_time_s,
        workspace.runtime.program_counter + 1,
        workspace.runtime.instructions_executed,
    ));
    if workspace.runtime.wait_remaining_s > 0.0 {
        lines.push(format!("Waiting: {:.2} s remaining", workspace.runtime.wait_remaining_s));
    }
    if !workspace.runtime.variables.is_empty() {
        lines.push(format!("Variables: {}", workspace.runtime.variables.iter()
            .map(|(name, value)| format!("{name}={value:.3}"))
            .collect::<Vec<_>>().join(" | ")));
    }
    if let Some(error) = &workspace.runtime.error {
        lines.push(format!("Runtime error: {error}"));
    }
    for entry in workspace.runtime.log.iter().rev().take(4).rev() {
        lines.push(format!("  {entry}"));
    }
    lines.push(format!("\n{}", workspace.message));
    text.0 = lines.join("\n");
}

fn format_block(block: &ProgramBlock) -> String {
    match &block.kind {
        BlockKind::Start => format!("[{:03}] START", block.id),
        BlockKind::Wait { duration_s } => format!("[{:03}] WAIT  {duration_s:.2} s", block.id),
        BlockKind::SetHeaterPower { fraction } => format!("[{:03}] SET CHAMBER HEATER  {:.0}%", block.id, fraction * 100.0),
        BlockKind::ReadTemperature { sensor_id } => format!("[{:03}] READ TEMPERATURE  {sensor_id}", block.id),
        BlockKind::BranchTemperature { comparison, threshold_c, target_block_id } => {
            let operator = match comparison {
                TemperatureComparison::GreaterOrEqual => ">=",
                TemperatureComparison::LessThan => "<",
            };
            format!("[{:03}] IF TEMP {operator} {threshold_c:.1} C  GOTO #{target_block_id:03}", block.id)
        }
        BlockKind::SetVariable { name, value } => format!("[{:03}] SET {name} = {value:.3}", block.id),
        BlockKind::SetVariableExpression { name, expression } => format!("[{:03}] SET {name} = {expression}", block.id),
        BlockKind::AddVariable { name, delta } => format!("[{:03}] ADD {name} += {delta:.3}", block.id),
        BlockKind::BranchVariable { name, comparison, threshold, target_block_id } => {
            let operator = match comparison {
                model::NumericComparison::GreaterOrEqual => ">=",
                model::NumericComparison::LessThan => "<",
                model::NumericComparison::Equal => "==",
            };
            format!("[{:03}] IF {name} {operator} {threshold:.3}  GOTO #{target_block_id:03}", block.id)
        }
        BlockKind::Record { message } => format!("[{:03}] RECORD  {message}", block.id),
        BlockKind::End => format!("[{:03}] END", block.id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_workspace_has_valid_demo_program() {
        assert!(SequenceWorkspace::default().program.validate().is_ok());
    }
}
