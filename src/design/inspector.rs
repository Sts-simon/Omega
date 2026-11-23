//! Numeric component inspector for the workshop.
//! Values are edited as SI coordinates, metre dimensions and degree rotations.
use super::DesignSession;
use crate::campus::{CampusState, CampusView};
use crate::sequence::SequenceWorkspace;
use super::PolygonSketchSession;
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectorField {
    Position(usize),
    Rotation(usize),
    Dimension(usize),
}

impl InspectorField {
    fn label(self) -> &'static str {
        match self {
            Self::Position(0) => "Position X",
            Self::Position(1) => "Position Y",
            Self::Position(_) => "Position Z",
            Self::Rotation(0) => "Rotation X",
            Self::Rotation(1) => "Rotation Y",
            Self::Rotation(_) => "Rotation Z",
            Self::Dimension(0) => "Size X",
            Self::Dimension(1) => "Size Y",
            Self::Dimension(_) => "Size Z",
        }
    }

    fn unit(self) -> &'static str {
        match self {
            Self::Rotation(_) => "deg",
            _ => "m",
        }
    }

    fn step(self) -> f64 {
        match self {
            Self::Position(_) => 0.01,
            Self::Rotation(_) => 1.0,
            Self::Dimension(_) => 0.001,
        }
    }

    fn value(self, part: &super::Part) -> f64 {
        match self {
            Self::Position(axis) => part.position_m[axis],
            Self::Rotation(axis) => part.rotation_rad[axis].to_degrees(),
            Self::Dimension(axis) => part.dimensions_m[axis],
        }
    }

    fn format(self, value: f64) -> String {
        match self {
            Self::Rotation(_) => format!("{value:.2}"),
            _ => format!("{value:.3}"),
        }
    }
}

#[derive(Resource, Default)]
pub struct InspectorState {
    active: Option<InspectorField>,
    editing_part_id: Option<String>,
    draft: String,
    replace_on_first_input: bool,
    message: String,
}

impl InspectorState {
    pub fn is_editing(&self) -> bool {
        self.active.is_some()
    }
}

#[derive(Component)]
pub(crate) struct InspectorPanel;

#[derive(Component)]
pub(crate) struct InspectorSelectionText;

#[derive(Component)]
pub(crate) struct InspectorValueText(InspectorField);

#[derive(Component)]
pub(crate) struct InspectorEditButton(InspectorField);

#[derive(Component)]
pub(crate) struct InspectorStepButton {
    field: InspectorField,
    delta: f64,
}

#[derive(Component)]
pub(crate) struct InspectorStatusText;

pub fn setup_ui(mut commands: Commands) {
    let panel = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(14.0),
            top: Val::Px(78.0),
            width: Val::Px(286.0),
            max_height: Val::Percent(87.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(7.0),
            padding: UiRect::all(Val::Px(12.0)),
            overflow: Overflow::scroll_y(),
            display: Display::None,
            ..default()
        },
        Interaction::None,
        ScrollPosition::default(),
        BackgroundColor(Color::srgba(0.018, 0.032, 0.046, 0.96)),
        InspectorPanel,
    )).id();

    commands.entity(panel).with_children(|parent| {
        parent.spawn((
            Text::new("COMPONENT INSPECTOR"),
            TextFont { font_size: FontSize::Px(15.0), ..default() },
            TextColor(Color::srgb(0.37, 0.84, 0.94)),
        ));
        parent.spawn((
            Text::new("No component selected"),
            TextFont { font_size: FontSize::Px(11.0), ..default() },
            TextColor(Color::srgb(0.78, 0.85, 0.89)),
            InspectorSelectionText,
        ));
        inspector_section(parent, "POSITION  |  METRES");
        inspector_row(parent, "X", InspectorField::Position(0));
        inspector_row(parent, "Y", InspectorField::Position(1));
        inspector_row(parent, "Z", InspectorField::Position(2));
        inspector_section(parent, "ROTATION  |  DEGREES");
        inspector_row(parent, "X", InspectorField::Rotation(0));
        inspector_row(parent, "Y", InspectorField::Rotation(1));
        inspector_row(parent, "Z", InspectorField::Rotation(2));
        inspector_section(parent, "DIMENSIONS  |  METRES");
        inspector_row(parent, "X", InspectorField::Dimension(0));
        inspector_row(parent, "Y", InspectorField::Dimension(1));
        inspector_row(parent, "Z", InspectorField::Dimension(2));
        parent.spawn((
            Text::new("Click a value to edit. First typed key replaces it. Enter applies, Esc cancels.\nPosition step: 0.01 m | Rotation: 1 deg | Size: 0.001 m"),
            TextFont { font_size: FontSize::Px(10.0), ..default() },
            TextColor(Color::srgb(0.58, 0.68, 0.74)),
            Node { margin: UiRect { top: Val::Px(8.0), ..default() }, ..default() },
        ));
        parent.spawn((
            Text::new("Ready"),
            TextFont { font_size: FontSize::Px(10.5), ..default() },
            TextColor(Color::srgb(0.37, 0.84, 0.94)),
            InspectorStatusText,
        ));
    });
}

fn inspector_section(parent: &mut ChildSpawnerCommands<'_>, label: &'static str) {
    parent.spawn((
        Text::new(label),
        TextFont { font_size: FontSize::Px(9.5), ..default() },
        TextColor(Color::srgb(0.35, 0.72, 0.80)),
        Node {
            width: Val::Percent(100.0),
            margin: UiRect { top: Val::Px(8.0), bottom: Val::Px(1.0), ..default() },
            ..default()
        },
    ));
}

fn inspector_row(parent: &mut ChildSpawnerCommands<'_>, axis_label: &'static str, field: InspectorField) {
    parent.spawn(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(4.0),
        ..default()
    }).with_children(|row| {
        row.spawn((
            Text::new(format!("{} {axis_label}", field.label().split_whitespace().next().unwrap_or("Value"))),
            TextFont { font_size: FontSize::Px(10.5), ..default() },
            TextColor(Color::srgb(0.72, 0.79, 0.83)),
            Node { width: Val::Px(78.0), ..default() },
        ));
        row.spawn((
            // Bevy 0.20 deprecated the `Button` marker; recreate the components it required.
            Interaction::default(),
            bevy::ui::FocusPolicy::Block,
            Node {
                width: Val::Px(112.0),
                height: Val::Px(29.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_grow: 1.0,
                ..default()
            },
            BackgroundColor(Color::srgb(0.075, 0.12, 0.16)),
            InspectorEditButton(field),
        )).with_children(|button| {
            button.spawn((
                Text::new("0.000"),
                TextFont { font_size: FontSize::Px(11.0), ..default() },
                TextColor(Color::srgb(0.93, 0.96, 0.98)),
                InspectorValueText(field),
            ));
        });
        inspector_step_button(row, field, -field.step(), "-");
        inspector_step_button(row, field, field.step(), "+");
    });
}

fn inspector_step_button(parent: &mut ChildSpawnerCommands<'_>, field: InspectorField, delta: f64, label: &'static str) {
    parent.spawn((
        // Bevy 0.20 deprecated the `Button` marker; recreate the components it required.
        Interaction::default(),
        bevy::ui::FocusPolicy::Block,
        Node {
            width: Val::Px(24.0),
            height: Val::Px(29.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgb(0.08, 0.15, 0.19)),
        InspectorStepButton { field, delta },
    )).with_children(|button| {
        button.spawn((
            Text::new(label),
            TextFont { font_size: FontSize::Px(14.0), ..default() },
            TextColor(Color::srgb(0.90, 0.94, 0.97)),
        ));
    });
}

pub fn handle_input(
    mut edit_buttons: Query<(&Interaction, &InspectorEditButton), Changed<Interaction>>,
    mut step_buttons: Query<(&Interaction, &InspectorStepButton), Changed<Interaction>>,
    campus: Res<CampusState>,
    sketch: Res<PolygonSketchSession>,
    sequence: Res<SequenceWorkspace>,
    mut session: ResMut<DesignSession>,
    mut state: ResMut<InspectorState>,
) {
    if campus.view != CampusView::Construction || campus.profile_editor_open || sketch.active || sequence.graph_is_open() {
        return;
    }

    for (interaction, button) in &mut edit_buttons {
        if *interaction != Interaction::Pressed { continue; }
        let Some(index) = session.selected_index() else {
            state.active = None;
            state.message = "Select a component first".to_string();
            continue;
        };
        let part = &session.assembly.parts[index];
        state.active = Some(button.0);
        state.editing_part_id = Some(part.id.clone());
        state.draft = button.0.format(button.0.value(part));
        state.replace_on_first_input = true;
        state.message = format!("Editing {} {} ({}) — first typed key replaces the value", part.label, button.0.label(), button.0.unit());
    }

    for (interaction, button) in &mut step_buttons {
        if *interaction != Interaction::Pressed { continue; }
        let Some(index) = session.selected_index() else { continue; };
        let current = button.field.value(&session.assembly.parts[index]);
        let next = current + button.delta;
        let changed = apply_value(button.field, next, &mut session);
        state.active = None;
        state.editing_part_id = None;
        state.replace_on_first_input = false;
        let actual = session.selected_index().map(|index| button.field.value(&session.assembly.parts[index])).unwrap_or(next);
        state.message = if changed {
            format!("Adjusted {} to {} {}", button.field.label(), button.field.format(actual), button.field.unit())
        } else {
            "Value unchanged".to_string()
        };
    }
}

// Keyboard entry is intentionally separate from Bevy's general editor shortcuts.
pub fn handle_text_input(
    keys: Res<ButtonInput<KeyCode>>,
    campus: Res<CampusState>,
    sketch: Res<PolygonSketchSession>,
    sequence: Res<SequenceWorkspace>,
    mut state: ResMut<InspectorState>,
    mut session: ResMut<DesignSession>,
) {
    let Some(field) = state.active else { return; };
    if campus.view != CampusView::Construction || campus.profile_editor_open || sketch.active || sequence.graph_is_open() {
        state.active = None;
        state.editing_part_id = None;
        state.replace_on_first_input = false;
        return;
    }
    if state.editing_part_id.as_deref() != session.selected_part_id.as_deref() {
        state.active = None;
        state.editing_part_id = None;
        state.replace_on_first_input = false;
        state.message = "Selection changed; numeric edit cancelled".to_string();
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        state.active = None;
        state.editing_part_id = None;
        state.replace_on_first_input = false;
        state.message = "Edit cancelled".to_string();
        return;
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        let parsed = state.draft.replace(',', ".").parse::<f64>();
        match parsed {
            Ok(value) if value.is_finite() => {
                if apply_value(field, value, &mut session) {
                    let actual = session.selected_index()
                        .map(|index| field.value(&session.assembly.parts[index]))
                        .unwrap_or(value);
                    state.message = format!("Applied {} = {} {}", field.label(), field.format(actual), field.unit());
                } else {
                    state.message = "Value unchanged or outside allowed limits".to_string();
                }
                state.active = None;
                state.editing_part_id = None;
                state.replace_on_first_input = false;
            }
            _ => state.message = "Invalid number. Enter a finite value.".to_string(),
        }
        return;
    }
    if keys.just_pressed(KeyCode::Backspace) {
        state.replace_on_first_input = false;
        state.draft.pop();
    }
    if keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::NumpadSubtract) {
        if state.replace_on_first_input {
            state.draft.clear();
            state.replace_on_first_input = false;
        }
        if state.draft.is_empty() {
            state.draft.push('-');
        }
    }
    if keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::NumpadDecimal) || keys.just_pressed(KeyCode::Comma) || keys.just_pressed(KeyCode::NumpadComma) {
        if state.replace_on_first_input {
            state.draft.clear();
            state.replace_on_first_input = false;
        }
        if !state.draft.contains('.') && !state.draft.contains(',') {
            if state.draft.is_empty() || state.draft == "-" { state.draft.push('0'); }
            state.draft.push('.');
        }
    }
    for (key, digit) in [
        (KeyCode::Digit0, '0'), (KeyCode::Digit1, '1'), (KeyCode::Digit2, '2'),
        (KeyCode::Digit3, '3'), (KeyCode::Digit4, '4'), (KeyCode::Digit5, '5'),
        (KeyCode::Digit6, '6'), (KeyCode::Digit7, '7'), (KeyCode::Digit8, '8'),
        (KeyCode::Digit9, '9'), (KeyCode::Numpad0, '0'), (KeyCode::Numpad1, '1'),
        (KeyCode::Numpad2, '2'), (KeyCode::Numpad3, '3'), (KeyCode::Numpad4, '4'),
        (KeyCode::Numpad5, '5'), (KeyCode::Numpad6, '6'), (KeyCode::Numpad7, '7'),
        (KeyCode::Numpad8, '8'), (KeyCode::Numpad9, '9'),
    ] {
        if keys.just_pressed(key) && state.draft.len() < 20 {
            if state.replace_on_first_input {
                state.draft.clear();
                state.replace_on_first_input = false;
            }
            state.draft.push(digit);
        }
    }
}

fn apply_value(field: InspectorField, value: f64, session: &mut DesignSession) -> bool {
    match field {
        InspectorField::Position(axis) => session.set_selected_position_axis(axis, value),
        InspectorField::Rotation(axis) => session.set_selected_rotation_axis_degrees(axis, value),
        InspectorField::Dimension(axis) => session.set_selected_dimension_axis(axis, value),
    }
}

pub fn update_ui(
    campus: Res<CampusState>,
    sketch: Res<PolygonSketchSession>,
    session: Res<DesignSession>,
    sequence: Res<SequenceWorkspace>,
    state: Res<InspectorState>,
    mut panel: Query<&mut Node, With<InspectorPanel>>,
    mut selection_text: Query<&mut Text, (With<InspectorSelectionText>, Without<InspectorValueText>)>,
    mut values: Query<(&mut Text, &InspectorValueText)>,
    mut buttons: Query<(
        &Interaction,
        &mut BackgroundColor,
        Option<&InspectorEditButton>,
        Option<&InspectorStepButton>,
    ), Or<(With<InspectorEditButton>, With<InspectorStepButton>)>>,
    mut status: Query<&mut Text, (With<InspectorStatusText>, Without<InspectorSelectionText>, Without<InspectorValueText>)>,
) {
    if let Ok(mut node) = panel.single_mut() {
        node.display = if campus.view == CampusView::Construction && !campus.profile_editor_open && !sketch.active && !sequence.graph_is_open() { Display::Flex } else { Display::None };
    }
    let selected = session.selected_index().map(|index| &session.assembly.parts[index]);
    if let Ok(mut text) = selection_text.single_mut() {
        text.0 = selected.map(|part| format!("{}  |  {:?}  |  {:?}\nMass estimate: {:.3} kg", part.label, part.kind, part.material, part.estimated_mass_kg()))
            .unwrap_or_else(|| "No component selected".to_string());
    }
    for (mut text, value_text) in &mut values {
        if Some(value_text.0) == state.active {
            text.0 = format!("> {} {}", state.draft, value_text.0.unit());
        } else if let Some(part) = selected {
            text.0 = format!("{} {}", value_text.0.format(value_text.0.value(part)), value_text.0.unit());
        } else {
            text.0 = "—".to_string();
        }
    }
    for (interaction, mut background, edit_button, step_button) in &mut buttons {
        if let Some(button) = edit_button {
            background.0 = if Some(button.0) == state.active {
                Color::srgb(0.10, 0.34, 0.38)
            } else {
                match *interaction {
                    Interaction::Pressed => Color::srgb(0.12, 0.40, 0.35),
                    Interaction::Hovered => Color::srgb(0.13, 0.22, 0.27),
                    Interaction::None => Color::srgb(0.075, 0.12, 0.16),
                }
            };
        } else if let Some(button) = step_button {
            background.0 = match *interaction {
                Interaction::Pressed => Color::srgb(0.12, 0.40, 0.35),
                Interaction::Hovered => Color::srgb(0.13, 0.22, 0.27),
                Interaction::None if Some(button.field) == state.active => Color::srgb(0.10, 0.23, 0.27),
                Interaction::None => Color::srgb(0.08, 0.15, 0.19),
            };
        }
    }
    if let Ok(mut text) = status.single_mut() {
        text.0 = state.message.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::PartKind;

    #[test]
    fn inspector_fields_use_metres_and_degrees() {
        let mut session = DesignSession::default();
        let index = session.selected_index().unwrap();
        let original = session.assembly.parts[index].rotation_rad[1];
        assert!(apply_value(InspectorField::Rotation(1), 90.0, &mut session));
        let changed = session.assembly.parts[session.selected_index().unwrap()].rotation_rad[1];
        assert!((changed.to_degrees() - 90.0).abs() < 1.0e-9);
        assert_ne!(original, changed);
    }

    #[test]
    fn numeric_edits_are_undoable_and_clamped() {
        let mut session = DesignSession::default();
        let index = session.selected_index().unwrap();
        let original = session.assembly.parts[index].dimensions_m[0];
        assert!(apply_value(InspectorField::Dimension(0), 0.0001, &mut session));
        let index = session.selected_index().unwrap();
        assert!((session.assembly.parts[index].dimensions_m[0] - 0.002).abs() < 1.0e-12);
        assert!(session.undo());
        let index = session.selected_index().unwrap();
        assert!((session.assembly.parts[index].dimensions_m[0] - original).abs() < 1.0e-12);
    }

    #[test]
    fn selecting_a_component_then_editing_its_position_works() {
        let mut session = DesignSession::default();
        session.add_part(PartKind::Beam);
        assert!(apply_value(InspectorField::Position(0), -12.345, &mut session));
        let index = session.selected_index().unwrap();
        assert!((session.assembly.parts[index].position_m[0] + 12.345).abs() < 1.0e-12);
    }
}
