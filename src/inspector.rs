//! Numeric transform editor for the selected workshop part.
//!
//! The inspector edits the six local values the 3D view already uses: three attachment offsets in
//! metres and three local Euler rotations in degrees. A field is edited by clicking it, typing a
//! finite number and pressing Enter; Escape restores the displayed value. Every committed entry,
//! step or reset is one undoable operation. Nothing here changes the vehicle schema.

use crate::campus::{CampusState, CampusView};
use crate::main_menu::Frontend;
use crate::missile::{self, MissileSession};
use crate::ui_kit::{self, ButtonTone, UiInteraction};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use omega_rules::PlacedPart;

/// The translation and rotation increments exposed by the inspector. They are independent from
/// the gizmo's grid snapping: this is the precision used by the `-` and `+` buttons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TransformPrecision {
    Grossier,
    #[default]
    Normal,
    Fin,
    TresFin,
}

impl TransformPrecision {
    pub(crate) const ALL: [Self; 4] = [Self::Grossier, Self::Normal, Self::Fin, Self::TresFin];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Grossier => "GROSSIER",
            Self::Normal => "NORMAL",
            Self::Fin => "FIN",
            Self::TresFin => "TRÈS FIN",
        }
    }

    pub(crate) fn step(self, rotation: bool) -> f64 {
        if rotation {
            match self {
                Self::Grossier => 15.0,
                Self::Normal => 5.0,
                Self::Fin => 1.0,
                Self::TresFin => 0.1,
            }
        } else {
            match self {
                Self::Grossier => 0.50,
                Self::Normal => 0.05,
                Self::Fin => 0.01,
                Self::TresFin => 0.001,
            }
        }
    }

    pub(crate) fn decimals(self, rotation: bool) -> usize {
        if rotation {
            match self {
                Self::TresFin => 1,
                _ => 0,
            }
        } else {
            match self {
                Self::TresFin => 3,
                _ => 2,
            }
        }
    }

    pub(crate) fn caption(self) -> String {
        format!("Pas : {:.2} m / {:.1}°", self.step(false), self.step(true))
    }
}

/// Actions emitted by the compact inspector buttons. They are deliberately separate from
/// `UiAction` so the advanced editor can evolve without touching the generic navigation handler.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum InspectorAction {
    Precision(TransformPrecision),
    Step {
        part: String,
        axis: usize,
        rotation: bool,
        direction: i8,
    },
    ZeroAxis {
        part: String,
        axis: usize,
        rotation: bool,
    },
    ZeroTranslations,
    ZeroRotations,
    RestoreInitial,
}

#[derive(Component, Clone, Debug)]
pub(crate) struct InspectorButton(pub(crate) InspectorAction);

/// A clickable numeric value in the selected-part inspector.
#[derive(Component, Clone, Debug)]
pub(crate) struct TransformField {
    pub(crate) part: String,
    pub(crate) axis: usize,
    pub(crate) rotation: bool,
}

/// The text child of a [`TransformField`]. Keeping it separate lets the editor update the typed
/// buffer in place without rebuilding the whole workshop board on every keystroke.
#[derive(Component, Clone, Debug)]
pub(crate) struct TransformFieldText {
    part: String,
    axis: usize,
    rotation: bool,
}

#[derive(Clone, Debug)]
struct FieldEdit {
    part: String,
    axis: usize,
    rotation: bool,
    buffer: String,
    /// A click selects the current value: the first typed character replaces it instead of
    /// appending to it.
    replace_on_type: bool,
}

#[derive(Resource, Default)]
pub(crate) struct InspectorState {
    pub(crate) precision: TransformPrecision,
    editing: Option<FieldEdit>,
}

impl InspectorState {
    /// Board rebuild key. The buffer is intentionally excluded so typing does not destroy the
    /// field under the cursor.
    pub(crate) fn signature(&self) -> String {
        format!("{:?}", self.precision)
    }

    pub(crate) fn is_editing(&self) -> bool {
        self.editing.is_some()
    }
}

/// Show a clean decimal instead of floating-point noise (`0.6000000000000001`) while keeping the
/// values that matter for the finest step and direct entries. The precision selector controls the
/// `-`/`+` steps and their rounding, not how many typed decimals are kept.
fn format_value(value: f64, rotation: bool, _precision: TransformPrecision) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let decimals = if rotation { 3 } else { 4 };
    let mut text = format!("{value:.decimals$}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

fn round_to_precision(value: f64, rotation: bool, precision: TransformPrecision) -> f64 {
    let factor = 10f64.powi(precision.decimals(rotation) as i32);
    (value * factor).round() / factor
}

/// Direct entries are rounded to the same number of decimals the field displays, so the stored
/// transform and the value shown to the player cannot disagree by a hidden fraction.
fn round_to_display(value: f64, rotation: bool) -> f64 {
    let factor = if rotation { 1_000.0 } else { 10_000.0 };
    (value * factor).round() / factor
}

fn parse_value(text: &str, rotation: bool) -> Result<f64, String> {
    let normalized = text.trim().replace(',', ".");
    if normalized.is_empty() {
        return Err("Entrez une valeur numérique.".to_string());
    }
    let value: f64 = normalized
        .parse()
        .map_err(|_| "Valeur numérique invalide.".to_string())?;
    missile::validate_transform_value(value, rotation)?;
    Ok(value)
}

fn current_values(session: &MissileSession, part: &str) -> Option<[f64; 6]> {
    session.transform_values(part)
}

fn finish_operation(session: &MissileSession, campus: &mut CampusState, result: Result<(), String>, message: String) {
    match result {
        Ok(()) => {
            campus.message = message;
            if let Err(error) = missile::persist_missile(session) {
                warn!("Could not save the vehicle transform: {error}");
            }
        }
        Err(error) => campus.message = error,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_actions(
    buttons: Query<(&UiInteraction, &InspectorButton), Changed<UiInteraction>>,
    mut inspector: ResMut<InspectorState>,
    mut session: ResMut<MissileSession>,
    mut campus: ResMut<CampusState>,
) {
    for (interaction, button) in &buttons {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        match button.0.clone() {
            InspectorAction::Precision(precision) => {
                inspector.precision = precision;
                inspector.editing = None;
                campus.message = format!(
                    "Précision {} : pas de {:.2} m / {:.1}°.",
                    precision.label(),
                    precision.step(false),
                    precision.step(true)
                );
            }
            InspectorAction::Step {
                part,
                axis,
                rotation,
                direction,
            } => {
                let Some(values) = current_values(&session, &part) else {
                    campus.message = "Cette pièce n'existe plus.".to_string();
                    continue;
                };
                let index = axis + if rotation { 3 } else { 0 };
                let next = round_to_precision(
                    values[index] + f64::from(direction) * inspector.precision.step(rotation),
                    rotation,
                    inspector.precision,
                );
                let result = session.set_transform_value(&part, axis, rotation, next);
                finish_operation(
                    &session,
                    &mut campus,
                    result,
                    format!(
                        "{} {} : {}",
                        if rotation { "Rotation" } else { "Position" },
                        ["X", "Y", "Z"][axis],
                        format_value(next, rotation, inspector.precision)
                    ),
                );
            }
            InspectorAction::ZeroAxis { part, axis, rotation } => {
                let result = session.set_transform_value(&part, axis, rotation, 0.0);
                finish_operation(
                    &session,
                    &mut campus,
                    result,
                    format!(
                        "{} {} remis à zéro.",
                        if rotation { "Rotation" } else { "Position" },
                        ["X", "Y", "Z"][axis]
                    ),
                );
            }
            InspectorAction::ZeroTranslations => {
                let Some(selected) = session.selected.clone() else {
                    continue;
                };
                let Some(mut values) = current_values(&session, &selected) else {
                    continue;
                };
                values[0] = 0.0;
                values[1] = 0.0;
                values[2] = 0.0;
                let result = session.set_transform_values(&selected, values);
                finish_operation(
                    &session,
                    &mut campus,
                    result,
                    "Translations remises à zéro.".to_string(),
                );
            }
            InspectorAction::ZeroRotations => {
                let Some(selected) = session.selected.clone() else {
                    continue;
                };
                let Some(mut values) = current_values(&session, &selected) else {
                    continue;
                };
                values[3] = 0.0;
                values[4] = 0.0;
                values[5] = 0.0;
                let result = session.set_transform_values(&selected, values);
                finish_operation(&session, &mut campus, result, "Rotations remises à zéro.".to_string());
            }
            InspectorAction::RestoreInitial => {
                let Some(selected) = session.selected.clone() else {
                    continue;
                };
                let result = session.set_transform_values(&selected, [0.0; 6]);
                finish_operation(
                    &session,
                    &mut campus,
                    result,
                    "Transformation initiale restaurée.".to_string(),
                );
            }
        }
    }
}

pub(crate) fn begin_edit(
    mut fields: Query<(&TransformField, &UiInteraction), Changed<UiInteraction>>,
    session: Res<MissileSession>,
    mut inspector: ResMut<InspectorState>,
) {
    if session.selected.is_none() {
        inspector.editing = None;
        return;
    }
    if inspector
        .editing
        .as_ref()
        .is_some_and(|edit| session.selected.as_deref() != Some(edit.part.as_str()))
    {
        // Selecting another part (or losing the edited one) ends the modal text field.
        inspector.editing = None;
    }
    for (field, interaction) in &mut fields {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        if session.selected.as_deref() != Some(field.part.as_str()) {
            continue;
        }
        let Some(values) = current_values(&session, &field.part) else {
            continue;
        };
        let index = field.axis + if field.rotation { 3 } else { 0 };
        inspector.editing = Some(FieldEdit {
            part: field.part.clone(),
            axis: field.axis,
            rotation: field.rotation,
            buffer: format_value(values[index], field.rotation, inspector.precision),
            replace_on_type: true,
        });
    }
}

pub(crate) fn handle_keyboard(
    frontend: Res<Frontend>,
    mut typed: MessageReader<KeyboardInput>,
    mut inspector: ResMut<InspectorState>,
    mut session: ResMut<MissileSession>,
    mut campus: ResMut<CampusState>,
) {
    if *frontend != Frontend::Game || campus.view != CampusView::Missile {
        inspector.editing = None;
        for _ in typed.read() {}
        return;
    }
    let Some(mut edit) = inspector.editing.clone() else {
        // Do not replay keystrokes typed before a field was selected when the next edit starts.
        for _ in typed.read() {}
        return;
    };
    let mut commit = false;
    let mut cancel = false;
    for event in typed.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        match event.key_code {
            KeyCode::Enter | KeyCode::NumpadEnter => {
                commit = true;
                break;
            }
            KeyCode::Escape => {
                cancel = true;
                break;
            }
            KeyCode::Backspace => {
                edit.replace_on_type = false;
                edit.buffer.pop();
            }
            _ => {
                if let Key::Character(characters) = &event.logical_key {
                    for character in characters.chars() {
                        if character.is_ascii_digit() || matches!(character, '.' | ',' | '-' | '+') {
                            if edit.replace_on_type {
                                edit.buffer.clear();
                                edit.replace_on_type = false;
                            }
                            edit.buffer.push(if character == ',' { '.' } else { character });
                        }
                    }
                }
            }
        }
    }
    if cancel {
        inspector.editing = None;
        campus.message = "Saisie annulée.".to_string();
        return;
    }
    if commit {
        let value = match parse_value(&edit.buffer, edit.rotation) {
            Ok(value) => value,
            Err(error) => {
                // Discard the invalid input; the last valid value is still in the vehicle.
                inspector.editing = None;
                campus.message = error;
                return;
            }
        };
        let value = round_to_display(value, edit.rotation);
        let result = session.set_transform_value(&edit.part, edit.axis, edit.rotation, value);
        inspector.editing = None;
        finish_operation(
            &session,
            &mut campus,
            result,
            format!(
                "{} {} : {}",
                if edit.rotation { "Rotation" } else { "Position" },
                ["X", "Y", "Z"][edit.axis],
                format_value(value, edit.rotation, inspector.precision)
            ),
        );
        return;
    }
    inspector.editing = Some(edit);
}

/// Keep every field's text in sync with the vehicle and with the current edit buffer. This is what
/// makes a gizmo drag update the numbers immediately, without rebuilding the inspector.
pub(crate) fn refresh_fields(
    session: Res<MissileSession>,
    inspector: Res<InspectorState>,
    mut fields: Query<(&TransformFieldText, &mut Text, &mut TextColor)>,
) {
    for (field, mut text, mut color) in &mut fields {
        let values = current_values(&session, &field.part);
        let index = field.axis + if field.rotation { 3 } else { 0 };
        let editing = inspector
            .editing
            .as_ref()
            .is_some_and(|edit| edit.part == field.part && edit.axis == field.axis && edit.rotation == field.rotation);
        let wanted = if editing {
            format!(
                "{}▌",
                inspector.editing.as_ref().map_or("", |edit| edit.buffer.as_str())
            )
        } else {
            values.map_or_else(
                || "—".to_string(),
                |values| format_value(values[index], field.rotation, inspector.precision),
            )
        };
        if text.0 != wanted {
            text.0 = wanted;
        }
        let wanted_color = if editing { ui_kit::ACCENT } else { ui_kit::TEXT };
        if color.0 != wanted_color {
            color.0 = wanted_color;
        }
    }
}

fn compact_button(
    parent: &mut ChildSpawnerCommands<'_>,
    label: &str,
    action: InspectorAction,
    tone: ButtonTone,
    width: Val,
) {
    parent
        .spawn((
            UiInteraction::default(),
            FocusPolicy::Block,
            Node {
                width,
                min_width: Val::Px(0.0),
                min_height: Val::Px(24.0),
                flex_shrink: 1.0,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(tone.background(UiInteraction::None)),
            tone,
            InspectorButton(action),
        ))
        .with_children(|button| ui_kit::label(button, label, 9.5, ui_kit::TEXT));
}

fn field_row(
    parent: &mut ChildSpawnerCommands<'_>,
    part: &PlacedPart,
    axis: usize,
    rotation: bool,
    value: f64,
    precision: TransformPrecision,
) {
    let part_id = part.id.clone();
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(25.0),
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            ui_kit::label(row, ["X", "Y", "Z"][axis], 10.0, ui_kit::TEXT_DIM);
            compact_button(
                row,
                "−",
                InspectorAction::Step {
                    part: part_id.clone(),
                    axis,
                    rotation,
                    direction: -1,
                },
                ButtonTone::Secondary,
                Val::Px(25.0),
            );
            row.spawn((
                TransformField {
                    part: part_id.clone(),
                    axis,
                    rotation,
                },
                UiInteraction::default(),
                FocusPolicy::Block,
                Node {
                    flex_grow: 1.0,
                    min_width: Val::Px(60.0),
                    min_height: Val::Px(24.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    overflow: Overflow::clip_x(),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(ui_kit::CARD),
            ))
            .with_children(|field| {
                field.spawn((
                    Text::new(format_value(value, rotation, precision)),
                    TextFont {
                        font: ui_kit::ui_font(),
                        font_size: FontSize::Px(11.0),
                        ..default()
                    },
                    TextColor(ui_kit::TEXT),
                    TransformFieldText {
                        part: part_id.clone(),
                        axis,
                        rotation,
                    },
                ));
            });
            compact_button(
                row,
                "+",
                InspectorAction::Step {
                    part: part_id.clone(),
                    axis,
                    rotation,
                    direction: 1,
                },
                ButtonTone::Secondary,
                Val::Px(25.0),
            );
            compact_button(
                row,
                "0",
                InspectorAction::ZeroAxis {
                    part: part_id,
                    axis,
                    rotation,
                },
                ButtonTone::Ghost,
                Val::Px(25.0),
            );
        });
}

/// Append the advanced editor below the selected-part summary. The caller keeps the root-part
/// message and the other part details; this function only adds controls for mounted parts.
pub(crate) fn spawn_editor(parent: &mut ChildSpawnerCommands<'_>, part: &PlacedPart, precision: TransformPrecision) {
    let Some(attach) = part.attach.as_ref() else {
        ui_kit::label(
            parent,
            "La pièce racine reste verrouillée : ses transformations ne sont pas éditables.",
            9.5,
            ui_kit::TEXT_DIM,
        );
        return;
    };
    let values = [
        attach.offset_x_m,
        attach.offset_y_m,
        attach.offset_z_m,
        attach.rotation_x_deg,
        attach.rotation_y_deg,
        attach.rotation_z_deg,
    ];
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            margin: UiRect::top(Val::Px(2.0)),
            ..default()
        })
        .with_children(|section| {
            ui_kit::label(section, "TRANSFORMATION", 10.5, ui_kit::ACCENT);
            ui_kit::label(
                section,
                "Cliquez une valeur, puis Entrée pour valider ou Échap pour annuler.",
                8.5,
                ui_kit::TEXT_DIM,
            );
            ui_kit::label(
                section,
                format!("PRÉCISION · {}", precision.caption()),
                9.0,
                ui_kit::TEXT_DIM,
            );
            section
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(3.0),
                    ..default()
                })
                .with_children(|row| {
                    for choice in TransformPrecision::ALL {
                        compact_button(
                            row,
                            choice.label(),
                            InspectorAction::Precision(choice),
                            if choice == precision {
                                ButtonTone::Primary
                            } else {
                                ButtonTone::Ghost
                            },
                            Val::Percent(25.0),
                        );
                    }
                });
            ui_kit::label(section, "POSITION LOCALE (m)", 9.0, ui_kit::TEXT_DIM);
            for (axis, value) in values[..3].iter().enumerate() {
                field_row(section, part, axis, false, *value, precision);
            }
            ui_kit::label(section, "ROTATION LOCALE (°)", 9.0, ui_kit::TEXT_DIM);
            for (axis, value) in values[3..].iter().enumerate() {
                field_row(section, part, axis, true, *value, precision);
            }
            section
                .spawn(Node {
                    width: Val::Percent(100.0),
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|row| {
                    compact_button(
                        row,
                        "TRANSLATIONS 0",
                        InspectorAction::ZeroTranslations,
                        ButtonTone::Secondary,
                        Val::Percent(50.0),
                    );
                    compact_button(
                        row,
                        "ROTATIONS 0",
                        InspectorAction::ZeroRotations,
                        ButtonTone::Secondary,
                        Val::Percent(50.0),
                    );
                });
            compact_button(
                section,
                "RESTAURER LA TRANSFORMATION INITIALE",
                InspectorAction::RestoreInitial,
                ButtonTone::Ghost,
                Val::Percent(100.0),
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precision_steps_match_the_documented_table() {
        assert_eq!(TransformPrecision::Grossier.step(false), 0.50);
        assert_eq!(TransformPrecision::Normal.step(false), 0.05);
        assert_eq!(TransformPrecision::Fin.step(false), 0.01);
        assert_eq!(TransformPrecision::TresFin.step(false), 0.001);
        assert_eq!(TransformPrecision::Grossier.step(true), 15.0);
        assert_eq!(TransformPrecision::Normal.step(true), 5.0);
        assert_eq!(TransformPrecision::Fin.step(true), 1.0);
        assert_eq!(TransformPrecision::TresFin.step(true), 0.1);
    }

    #[test]
    fn decimal_comma_and_negative_values_are_accepted() {
        assert_eq!(parse_value("-12,5", false).unwrap(), -12.5);
        assert_eq!(parse_value("+0.001", false).unwrap(), 0.001);
        assert_eq!(parse_value("-45", true).unwrap(), -45.0);
    }

    #[test]
    fn invalid_and_out_of_bounds_values_are_refused() {
        assert!(parse_value("NaN", false).is_err());
        assert!(parse_value("inf", false).is_err());
        assert!(parse_value("1001", false).is_err());
        assert!(parse_value("3601", true).is_err());
        assert!(parse_value("", false).is_err());
        assert!(parse_value("not a number", false).is_err());
    }

    #[test]
    fn direct_entries_match_the_displayed_decimals() {
        assert_eq!(round_to_display(1.23456, false), 1.2346);
        assert_eq!(round_to_display(-123.4567, true), -123.457);
        assert_eq!(
            format_value(round_to_display(-123.4567, true), true, TransformPrecision::TresFin),
            "-123.457"
        );
    }

    #[test]
    fn stepping_rounds_to_the_selected_precision() {
        assert!((round_to_precision(1.23456, false, TransformPrecision::Fin) - 1.23).abs() < 1e-9);
        assert!((round_to_precision(-1.23456, false, TransformPrecision::TresFin) - -1.235).abs() < 1e-9);
    }
}
