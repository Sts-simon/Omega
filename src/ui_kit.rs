//! Shared look and small widgets for the Omega interface: palette, cards, status chips and
//! buttons whose colour depends on a tone and on hover/press state.

/// Bevy 0.20.1 deprecates `Interaction` in favour of `picking::hover::Hovered` and `ui::Pressed`,
/// but the `bevy_ui` focus system in this release still only drives `Interaction`: nothing in the
/// engine maintains the replacement components for UI nodes yet. Re-exporting the deprecated type
/// here keeps the migration to a single place, and `expect` fails the build once a Bevy upgrade
/// makes the alias unnecessary, so the cleanup cannot be forgotten.
#[expect(
    deprecated,
    reason = "bevy_ui 0.20.1 focus only maintains Interaction; Hovered/Pressed are not wired for UI nodes yet"
)]
pub(crate) type UiInteraction = bevy::ui::Interaction;

use crate::interface::{UiAction, UiActionButton};
use bevy::prelude::*;

pub const PANEL: Color = Color::srgba(0.016, 0.030, 0.044, 0.93);
pub const CARD: Color = Color::srgba(0.050, 0.078, 0.104, 0.97);
pub const CARD_SELECTED: Color = Color::srgba(0.060, 0.120, 0.150, 0.98);
pub const TEXT: Color = Color::srgb(0.92, 0.95, 0.97);
pub const TEXT_DIM: Color = Color::srgb(0.58, 0.67, 0.73);
pub const ACCENT: Color = Color::srgb(0.36, 0.84, 0.94);
pub const GOOD: Color = Color::srgb(0.22, 0.66, 0.42);
pub const WARN: Color = Color::srgb(0.88, 0.64, 0.18);
pub const BAD: Color = Color::srgb(0.82, 0.30, 0.30);
pub const NEUTRAL: Color = Color::srgb(0.30, 0.40, 0.52);
pub const PURPLE: Color = Color::srgb(0.58, 0.46, 0.86);

#[derive(Component)]
pub struct NavStatus;

/// Visual weight of a button.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonTone {
    Secondary,
    Primary,
    Danger,
    Ghost,
    Disabled,
}

impl ButtonTone {
    pub fn background(self, interaction: UiInteraction) -> Color {
        let (idle, hover, pressed) = match self {
            Self::Secondary => ((0.110, 0.170, 0.220), (0.170, 0.270, 0.330), (0.120, 0.400, 0.350)),
            Self::Primary => ((0.100, 0.440, 0.520), (0.130, 0.560, 0.640), (0.170, 0.680, 0.560)),
            Self::Danger => ((0.380, 0.130, 0.130), (0.500, 0.170, 0.170), (0.620, 0.220, 0.220)),
            Self::Ghost => ((0.040, 0.065, 0.088), (0.100, 0.165, 0.210), (0.120, 0.300, 0.330)),
            Self::Disabled => ((0.060, 0.080, 0.095), (0.060, 0.080, 0.095), (0.060, 0.080, 0.095)),
        };
        let (r, g, b) = match interaction {
            UiInteraction::None => idle,
            UiInteraction::Hovered => hover,
            UiInteraction::Pressed => pressed,
        };
        Color::srgb(r, g, b)
    }
}

pub fn label(parent: &mut ChildSpawnerCommands<'_>, text: impl Into<String>, size: f32, color: Color) {
    parent.spawn((
        Text::new(text.into()),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    ));
}

/// Small rounded pill with a coloured background.
pub fn chip(parent: &mut ChildSpawnerCommands<'_>, text: impl Into<String>, color: Color) {
    parent
        .spawn((
            Node {
                padding: UiRect {
                    left: Val::Px(8.0),
                    right: Val::Px(8.0),
                    top: Val::Px(3.0),
                    bottom: Val::Px(3.0),
                },
                border_radius: BorderRadius::all(Val::Px(9.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(color),
        ))
        .with_children(|c| label(c, text, 10.0, Color::srgb(0.04, 0.06, 0.08)));
}

/// Card container: rounded, with a coloured top edge.
pub fn card_node(width: Val, accent: Color) -> (Node, BackgroundColor, BorderColor) {
    (
        Node {
            width,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(7.0),
            padding: UiRect::all(Val::Px(12.0)),
            border: UiRect {
                top: Val::Px(3.0),
                ..default()
            },
            border_radius: BorderRadius::all(Val::Px(7.0)),
            ..default()
        },
        BackgroundColor(CARD),
        BorderColor::all(accent),
    )
}

pub fn button(
    parent: &mut ChildSpawnerCommands<'_>,
    text: impl Into<String>,
    action: UiAction,
    tone: ButtonTone,
    width: Val,
) {
    parent
        .spawn((
            UiInteraction::default(),
            bevy::ui::FocusPolicy::Block,
            Node {
                width,
                min_height: Val::Px(32.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect {
                    left: Val::Px(10.0),
                    right: Val::Px(10.0),
                    top: Val::Px(6.0),
                    bottom: Val::Px(6.0),
                },
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(tone.background(UiInteraction::None)),
            tone,
            UiActionButton(action),
        ))
        .with_children(|b| {
            let color = if tone == ButtonTone::Disabled { TEXT_DIM } else { TEXT };
            label(b, text, 11.0, color);
        });
}

/// Button that shares the width of its row equally with its siblings.
pub fn button_row_item(
    parent: &mut ChildSpawnerCommands<'_>,
    text: impl Into<String>,
    action: UiAction,
    tone: ButtonTone,
) {
    parent
        .spawn((
            UiInteraction::default(),
            bevy::ui::FocusPolicy::Block,
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                min_height: Val::Px(28.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect {
                    left: Val::Px(3.0),
                    right: Val::Px(3.0),
                    top: Val::Px(5.0),
                    bottom: Val::Px(5.0),
                },
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(tone.background(UiInteraction::None)),
            tone,
            UiActionButton(action),
        ))
        .with_children(|b| label(b, text, 10.0, TEXT));
}

/// `48085` -> `48 085`.
pub fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_groups_digits() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1 000");
        assert_eq!(thousands(48_085), "48 085");
        assert_eq!(thousands(1_234_567), "1 234 567");
    }

    #[test]
    fn hovering_and_pressing_change_the_colour() {
        for tone in [
            ButtonTone::Secondary,
            ButtonTone::Primary,
            ButtonTone::Danger,
            ButtonTone::Ghost,
        ] {
            assert_ne!(
                tone.background(UiInteraction::None),
                tone.background(UiInteraction::Hovered)
            );
            assert_ne!(
                tone.background(UiInteraction::Hovered),
                tone.background(UiInteraction::Pressed)
            );
        }
        assert_eq!(
            ButtonTone::Disabled.background(UiInteraction::None),
            ButtonTone::Disabled.background(UiInteraction::Pressed)
        );
    }
}
