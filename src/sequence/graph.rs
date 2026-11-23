use super::model::{BlockKind, Program, ProgramBlock};
use super::SequenceWorkspace;
use crate::campus::{CampusState, CampusView};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::HashSet;

pub const GRAPH_LEFT: f32 = 470.0;
pub const GRAPH_TOP: f32 = 42.0;
pub const GRAPH_WIDTH: f32 = 1_110.0;
pub const GRAPH_HEIGHT: f32 = 850.0;
pub const NODE_WIDTH: f32 = 220.0;
pub const NODE_HEIGHT: f32 = 88.0;

#[derive(Component)]
pub struct GraphPanel;

#[derive(Component)]
pub struct GraphHeader;

#[derive(Component)]
pub struct GraphInspector;

#[derive(Component)]
pub struct GraphNodeVisual {
    block_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum EdgeKind {
    Flow,
    BranchTrue,
}

#[derive(Component, Clone, Copy)]
pub struct GraphEdgeVisual {
    from_id: u32,
    to_id: u32,
    kind: EdgeKind,
    segment: u8,
}

#[derive(Resource)]
pub struct GraphPanelEntity(pub Entity);

#[derive(Clone, Copy)]
struct EdgeDescriptor {
    from_id: u32,
    to_id: u32,
    kind: EdgeKind,
}

pub fn setup(mut commands: Commands, workspace: Res<SequenceWorkspace>) {
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(GRAPH_LEFT),
                top: Val::Px(GRAPH_TOP),
                width: Val::Px(GRAPH_WIDTH),
                height: Val::Px(GRAPH_HEIGHT),
                display: Display::None,
                padding: UiRect::all(Val::Px(12.0)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.018, 0.030, 0.043, 0.97)),
            GraphPanel,
        ))
        .id();

    commands.entity(panel).with_children(|parent| {
        parent.spawn((
            Text::new("OMEGA | VISUAL SEQUENCE GRAPH"),
            TextFont { font_size: FontSize::Px(18.0), ..default() },
            TextColor(Color::srgb(0.44, 0.88, 0.98)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(18.0),
                top: Val::Px(10.0),
                ..default()
            },
            GraphHeader,
        ));
        parent.spawn((
            Text::new("F6 close | drag nodes | Space run/pause | N step | B breakpoint | F1-F5/F7-F9 add blocks | Ctrl+F8 save"),
            TextFont { font_size: FontSize::Px(11.0), ..default() },
            TextColor(Color::srgb(0.72, 0.78, 0.83)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(18.0),
                top: Val::Px(38.0),
                ..default()
            },
        ));
        parent.spawn((
            Text::new("Select a node by clicking it. Conditional nodes have a visible TRUE jump and a FALSE sequential path."),
            TextFont { font_size: FontSize::Px(11.0), ..default() },
            TextColor(Color::srgb(0.55, 0.65, 0.72)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(18.0),
                top: Val::Px(68.0),
                ..default()
            },
        ));
        parent.spawn((
            Text::new("Inspector loading..."),
            TextFont { font_size: FontSize::Px(12.0), ..default() },
            TextColor(Color::srgb(0.90, 0.92, 0.94)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(18.0),
                top: Val::Px(768.0),
                width: Val::Px(1_050.0),
                height: Val::Px(65.0),
                ..default()
            },
            GraphInspector,
        ));

        // Connections are spawned first so node cards render over their endpoints.
        for edge in edge_descriptors(&workspace.program) {
            for segment in 0..3 {
                parent.spawn(edge_bundle(edge, segment));
            }
        }
        for block in &workspace.program.blocks {
            parent.spawn(node_bundle(block, &workspace.program, workspace.selected_block_id()));
        }
    });

    commands.insert_resource(GraphPanelEntity(panel));
}

pub fn handle_mouse(
    window: Single<&Window, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut workspace: ResMut<SequenceWorkspace>,
) {
    if !workspace.graph_open {
        workspace.dragging_block = None;
        return;
    }
    if workspace.runtime.status == super::runtime::RuntimeStatus::Running {
        workspace.dragging_block = None;
        return;
    }
    let Some(cursor) = window.cursor_position() else {
        if mouse.just_released(MouseButton::Left) {
            workspace.dragging_block = None;
        }
        return;
    };

    let local = cursor - Vec2::new(GRAPH_LEFT + 12.0, GRAPH_TOP + 12.0);
    if mouse.just_pressed(MouseButton::Left) {
        // Copy the hit data out of the iterator before mutating the workspace.
        // Keeping a reference to `block` alive here conflicts with the resource
        // mutations below under Rust's aliasing rules.
        let hit = workspace.program.blocks.iter().rev().find_map(|block| {
            let position = Vec2::new(block.position.x, block.position.y);
            let contains_cursor = local.x >= position.x
                && local.x <= position.x + NODE_WIDTH
                && local.y >= position.y
                && local.y <= position.y + NODE_HEIGHT;
            contains_cursor.then_some((block.id, position))
        });
        if let Some((block_id, block_position)) = hit {
            workspace.selected_block_id = block_id;
            workspace.dragging_block = Some(block_id);
            workspace.drag_offset = local - block_position;
        }
    }

    if mouse.pressed(MouseButton::Left) {
        // Vec2 is Copy. Snapshot the offset before taking a mutable borrow of
        // the selected block in `workspace.program.blocks`.
        let drag_offset = workspace.drag_offset;
        if let Some(id) = workspace.dragging_block {
            if let Some(block) = workspace.program.blocks.iter_mut().find(|block| block.id == id) {
                block.position.x = (local.x - drag_offset.x)
                    .clamp(8.0, GRAPH_WIDTH - NODE_WIDTH - 20.0);
                block.position.y = (local.y - drag_offset.y)
                    .clamp(104.0, GRAPH_HEIGHT - NODE_HEIGHT - 95.0);
            }
        }
    }
    if mouse.just_released(MouseButton::Left) {
        workspace.dragging_block = None;
    }
}

pub fn sync(
    mut commands: Commands,
    workspace: Res<SequenceWorkspace>,
    campus: Res<CampusState>,
    panel_entity: Res<GraphPanelEntity>,
    mut panel_query: Query<&mut Node, With<GraphPanel>>,
    mut node_query: Query<
        (Entity, &GraphNodeVisual, &mut Node, &mut Text, &mut BackgroundColor),
        (Without<GraphPanel>, Without<GraphEdgeVisual>),
    >,
    mut edge_query: Query<
        (Entity, &GraphEdgeVisual, &mut Node, &mut BackgroundColor),
        (Without<GraphPanel>, Without<GraphNodeVisual>),
    >,
    mut header_query: Query<&mut Text, (With<GraphHeader>, Without<GraphInspector>, Without<GraphNodeVisual>)>,
    mut inspector_query: Query<&mut Text, (With<GraphInspector>, Without<GraphHeader>, Without<GraphNodeVisual>)>,
) {
    if let Ok(mut panel) = panel_query.single_mut() {
        panel.display = if workspace.graph_open && campus.view == CampusView::Programming {
            Display::Flex
        } else {
            Display::None
        };
    }

    let mut existing_node_ids = HashSet::new();
    for (entity, visual, mut node, mut text, mut background) in &mut node_query {
        existing_node_ids.insert(visual.block_id);
        if let Some(block) = workspace.program.blocks.iter().find(|block| block.id == visual.block_id) {
            node.left = Val::Px(block.position.x);
            node.top = Val::Px(block.position.y);
            text.0 = format_node(block, &workspace.program);
            background.0 = node_color(block, &workspace);
        } else {
            commands.entity(entity).despawn();
        }
    }

    let mut missing_nodes = Vec::new();
    for block in &workspace.program.blocks {
        if !existing_node_ids.contains(&block.id) {
            missing_nodes.push(block.clone());
        }
    }

    let desired_edges = edge_descriptors(&workspace.program);
    let mut existing_edge_keys = HashSet::new();
    for (entity, visual, mut node, mut background) in &mut edge_query {
        existing_edge_keys.insert((visual.from_id, visual.to_id, visual.kind, visual.segment));
        let descriptor_exists = desired_edges.iter().any(|edge| {
            edge.from_id == visual.from_id && edge.to_id == visual.to_id && edge.kind == visual.kind
        });
        if !descriptor_exists {
            commands.entity(entity).despawn();
            continue;
        }
        if let (Some(from), Some(to)) = (
            workspace.program.blocks.iter().find(|block| block.id == visual.from_id),
            workspace.program.blocks.iter().find(|block| block.id == visual.to_id),
        ) {
            let (left, top, width, height) = edge_rect(from, to, visual.segment);
            node.left = Val::Px(left);
            node.top = Val::Px(top);
            node.width = Val::Px(width);
            node.height = Val::Px(height);
            background.0 = match visual.kind {
                EdgeKind::Flow => Color::srgb(0.20, 0.60, 0.72),
                EdgeKind::BranchTrue => Color::srgb(0.95, 0.58, 0.22),
            };
        }
    }

    let mut missing_edges = Vec::new();
    for edge in desired_edges {
        for segment in 0..3 {
            if !existing_edge_keys.contains(&(edge.from_id, edge.to_id, edge.kind, segment)) {
                missing_edges.push(edge_bundle(edge, segment));
            }
        }
    }

    if !missing_edges.is_empty() || !missing_nodes.is_empty() {
        commands.entity(panel_entity.0).with_children(|parent| {
            for edge in missing_edges {
                parent.spawn(edge);
            }
            for block in missing_nodes {
                parent.spawn(node_bundle(&block, &workspace.program, workspace.selected_block_id()));
            }
        });
    }

    if let Ok(mut header) = header_query.single_mut() {
        header.0 = format!(
            "OMEGA | VISUAL SEQUENCE GRAPH   [{}]   {} blocks",
            workspace.runtime.status.label(),
            workspace.program.blocks.len()
        );
    }
    if let Ok(mut inspector) = inspector_query.single_mut() {
        let selected = workspace.program.blocks.iter().find(|block| block.id == workspace.selected_block_id());
        let selected_text = selected.map(|block| format!("Selected #{}: {}", block.id, block_summary(&block.kind)))
            .unwrap_or_else(|| "No node selected".to_string());
        let variables = if workspace.runtime.variables.is_empty() {
            "Variables: (none)".to_string()
        } else {
            format!("Variables: {}", workspace.runtime.variables.iter()
                .map(|(name, value)| format!("{name}={value:.3}"))
                .collect::<Vec<_>>().join(" | "))
        };
        inspector.0 = format!(
            "{selected_text}   |   PC: {}   |   T: {:.2} C   |   heater: {:.0}%\n{variables}",
            workspace.runtime.program_counter + 1,
            workspace.runtime.chamber_temperature_c,
            workspace.runtime.heater_power * 100.0,
        );
    }
}

fn node_bundle(block: &ProgramBlock, program: &Program, selected_id: u32) -> impl Bundle {
    let selected = block.id == selected_id;
    (
        Text::new(format_node(block, program)),
        TextFont { font_size: FontSize::Px(12.0), ..default() },
        TextColor(Color::srgb(0.93, 0.95, 0.97)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(block.position.x),
            top: Val::Px(block.position.y),
            width: Val::Px(NODE_WIDTH),
            height: Val::Px(NODE_HEIGHT),
            padding: UiRect::all(Val::Px(8.0)),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(if selected { Color::srgb(0.10, 0.30, 0.35) } else { block_color(&block.kind) }),
        GraphNodeVisual { block_id: block.id },
    )
}

fn edge_bundle(edge: EdgeDescriptor, segment: u8) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(3.0),
            height: Val::Px(3.0),
            ..default()
        },
        BackgroundColor(match edge.kind {
            EdgeKind::Flow => Color::srgb(0.20, 0.60, 0.72),
            EdgeKind::BranchTrue => Color::srgb(0.95, 0.58, 0.22),
        }),
        GraphEdgeVisual { from_id: edge.from_id, to_id: edge.to_id, kind: edge.kind, segment },
    )
}

fn edge_descriptors(program: &Program) -> Vec<EdgeDescriptor> {
    let mut edges = Vec::new();
    for pair in program.blocks.windows(2) {
        edges.push(EdgeDescriptor { from_id: pair[0].id, to_id: pair[1].id, kind: EdgeKind::Flow });
    }
    for block in &program.blocks {
        let target = match &block.kind {
            BlockKind::BranchTemperature { target_block_id, .. }
            | BlockKind::BranchVariable { target_block_id, .. } => Some(*target_block_id),
            _ => None,
        };
        if let Some(target_id) = target {
            if program.index_of(target_id).is_some() {
                edges.push(EdgeDescriptor { from_id: block.id, to_id: target_id, kind: EdgeKind::BranchTrue });
            }
        }
    }
    edges
}

fn edge_rect(from: &ProgramBlock, to: &ProgramBlock, segment: u8) -> (f32, f32, f32, f32) {
    let start = Vec2::new(from.position.x + NODE_WIDTH, from.position.y + NODE_HEIGHT * 0.5);
    let end = Vec2::new(to.position.x, to.position.y + NODE_HEIGHT * 0.5);
    let middle_x = (start.x + end.x) * 0.5;
    match segment {
        0 => (start.x.min(middle_x), start.y - 1.5, (middle_x - start.x).abs().max(3.0), 3.0),
        1 => (middle_x - 1.5, start.y.min(end.y), 3.0, (end.y - start.y).abs().max(3.0)),
        _ => (middle_x.min(end.x), end.y - 1.5, (end.x - middle_x).abs().max(3.0), 3.0),
    }
}

fn format_node(block: &ProgramBlock, program: &Program) -> String {
    let breakpoint = if block.breakpoint { "  [BREAKPOINT]" } else { "" };
    let title = block_summary(&block.kind);
    let outputs = match &block.kind {
        BlockKind::BranchTemperature { target_block_id, .. }
        | BlockKind::BranchVariable { target_block_id, .. } => {
            let false_target = program.index_of(block.id).and_then(|index| program.blocks.get(index + 1)).map(|next| next.id);
            format!("TRUE -> #{target_block_id:03}   FALSE -> {}", false_target.map(|id| format!("#{id:03}")).unwrap_or_else(|| "END".to_string()))
        }
        BlockKind::End => "OUTPUT: none".to_string(),
        _ => program.index_of(block.id).and_then(|index| program.blocks.get(index + 1))
            .map(|next| format!("NEXT -> #{:03}", next.id)).unwrap_or_else(|| "NEXT -> END".to_string()),
    };
    format!("#{:03}  {}{}\n{}", block.id, title, breakpoint, outputs)
}

fn block_summary(kind: &BlockKind) -> String {
    match kind {
        BlockKind::Start => "START".to_string(),
        BlockKind::Wait { duration_s } => format!("WAIT  {duration_s:.2} s"),
        BlockKind::SetHeaterPower { fraction } => format!("SET HEATER  {:.0}%", fraction * 100.0),
        BlockKind::ReadTemperature { sensor_id } => format!("READ TEMP  {sensor_id}"),
        BlockKind::BranchTemperature { comparison, threshold_c, target_block_id } => {
            let op = match comparison { super::model::TemperatureComparison::GreaterOrEqual => ">=", super::model::TemperatureComparison::LessThan => "<" };
            format!("IF TEMP {op} {threshold_c:.1} C  -> #{target_block_id:03}")
        }
        BlockKind::SetVariable { name, value } => format!("SET {name} = {value:.2}"),
        BlockKind::SetVariableExpression { name, expression } => format!("SET {name} = {expression}"),
        BlockKind::AddVariable { name, delta } => format!("ADD {name} += {delta:.2}"),
        BlockKind::BranchVariable { name, comparison, threshold, target_block_id } => {
            let op = match comparison {
                super::model::NumericComparison::GreaterOrEqual => ">=",
                super::model::NumericComparison::LessThan => "<",
                super::model::NumericComparison::Equal => "==",
            };
            format!("IF {name} {op} {threshold:.2} -> #{target_block_id:03}")
        }
        BlockKind::Record { message } => format!("RECORD  {}", message.chars().take(20).collect::<String>()),
        BlockKind::End => "END".to_string(),
    }
}

fn block_color(kind: &BlockKind) -> Color {
    match kind {
        BlockKind::Start | BlockKind::End => Color::srgb(0.12, 0.25, 0.29),
        BlockKind::Wait { .. } => Color::srgb(0.20, 0.22, 0.34),
        BlockKind::SetHeaterPower { .. }
        | BlockKind::SetVariable { .. }
        | BlockKind::SetVariableExpression { .. }
        | BlockKind::AddVariable { .. } => Color::srgb(0.12, 0.30, 0.24),
        BlockKind::ReadTemperature { .. } => Color::srgb(0.12, 0.28, 0.35),
        BlockKind::BranchTemperature { .. } | BlockKind::BranchVariable { .. } => Color::srgb(0.37, 0.25, 0.12),
        BlockKind::Record { .. } => Color::srgb(0.30, 0.22, 0.30),
    }
}

fn node_color(block: &ProgramBlock, workspace: &SequenceWorkspace) -> Color {
    if block.id == workspace.selected_block_id() {
        Color::srgb(0.10, 0.34, 0.39)
    } else if block.breakpoint {
        Color::srgb(0.42, 0.16, 0.16)
    } else if workspace.runtime.last_executed_block == Some(block.id) {
        Color::srgb(0.18, 0.36, 0.20)
    } else {
        block_color(&block.kind)
    }
}
