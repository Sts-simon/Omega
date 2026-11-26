//! Navigation bar and the shared UI action dispatch. Boards live in `campus::board`; this module
//! only builds the top bar and routes the actions that need the campaign or the office.

use crate::campus::{CampusState, CampusView};
use crate::main_menu::MenuAction;
use crate::ui_kit::{self, NavStatus, UiInteraction};
use bevy::prelude::*;

#[derive(Clone, Debug)]
pub(crate) enum UiAction {
    Navigate(CampusView),
    StartResearch(usize),
    NextDay,
    AcceptMissileOffer(usize),
    DeliverMissile(u32),
    SelectMissileContract(u32),
    /// Fly the current design and program once, so a stale contract gets fresh evidence.
    ReflyMissileContract,
    RecordMissileRevision,
    BeginRevisionName(u32),
    CommitRevisionName,
    CancelRevisionName,
    CompareRevision(u32),
    ClearRevisionComparison,
    RestoreRevision(u32),
    /// Main menu and save-slot actions, handled by `main_menu`.
    Menu(MenuAction),
    Missile(crate::missile::MissileAction),
    Camera(crate::scene::CameraAction),
    Program(crate::program_editor::ProgramAction),
    Trial(crate::trials::TrialAction),
}

#[derive(Component)]
pub(crate) struct UiActionButton(pub(crate) UiAction);

#[derive(Component)]
pub(crate) struct NavRoot;

pub fn setup(mut commands: Commands) {
    let nav = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(62.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect {
                    left: Val::Px(18.0),
                    right: Val::Px(18.0),
                    top: Val::Px(8.0),
                    bottom: Val::Px(8.0),
                },
                ..default()
            },
            UiInteraction::None,
            BackgroundColor(Color::srgba(0.012, 0.024, 0.036, 0.96)),
            NavRoot,
        ))
        .id();

    commands.entity(nav).with_children(|parent| {
        parent.spawn((
            Text::new("OMEGA"),
            TextFont {
                font: ui_kit::ui_font(),
                font_size: FontSize::Px(19.0),
                ..default()
            },
            TextColor(ui_kit::ACCENT),
            Node {
                margin: UiRect {
                    right: Val::Px(16.0),
                    ..default()
                },
                ..default()
            },
        ));
        add_nav_button(parent, "ISLAND", CampusView::Island);
        add_nav_button(parent, "CONTRACTS", CampusView::Contracts);
        add_nav_button(parent, "WORKSHOP", CampusView::Missile);
        add_nav_button(parent, "RESEARCH", CampusView::Research);
        add_nav_button(parent, "PROGRAM", CampusView::Program);
        add_nav_button(parent, "TRIALS", CampusView::Trials);
        add_menu_button(parent);
        parent
            .spawn(Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                height: Val::Px(18.0),
                align_items: AlignItems::Center,
                margin: UiRect {
                    left: Val::Px(14.0),
                    right: Val::Px(14.0),
                    ..default()
                },
                overflow: Overflow::clip(),
                ..default()
            })
            .with_children(|lane| {
                lane.spawn((
                    Text::new(""),
                    TextFont {
                        font: ui_kit::ui_font(),
                        font_size: FontSize::Px(11.5),
                        ..default()
                    },
                    TextColor(ui_kit::TEXT),
                    crate::campus::ToastText,
                ));
            });
        parent.spawn((
            Text::new(""),
            TextFont {
                font: ui_kit::ui_font(),
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(ui_kit::ACCENT),
            NavStatus,
        ));
    });
}

fn add_menu_button(parent: &mut ChildSpawnerCommands<'_>) {
    ui_kit::button(
        parent,
        "MENU",
        UiAction::Menu(MenuAction::Open),
        ui_kit::ButtonTone::Ghost,
        Val::Px(64.0),
    );
}

fn add_nav_button(parent: &mut ChildSpawnerCommands<'_>, label: &'static str, target: CampusView) {
    let entity = parent
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(ui_kit::CARD),
            UiInteraction::None,
            UiActionButton(UiAction::Navigate(target)),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(ui_kit::fr(label)),
                TextFont {
                    font: ui_kit::ui_font(),
                    font_size: FontSize::Px(12.5),
                    ..default()
                },
                TextColor(ui_kit::TEXT),
            ));
        })
        .id();
    let _ = entity;
}

pub fn handle_actions(
    buttons: Query<(&UiInteraction, &UiActionButton), Changed<UiInteraction>>,
    mut campus: ResMut<CampusState>,
    missile: Res<crate::missile::MissileSession>,
    program: Res<crate::program_editor::MissileProgramSession>,
    mut log: ResMut<crate::revisions::RevisionLog>,
    trials: Res<crate::trials::TrialSession>,
    mut procurement: ResMut<crate::procurement::Procurement>,
    mut camera: ResMut<crate::scene::CameraCommand>,
) {
    for (interaction, button) in &buttons {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        match button.0.clone() {
            UiAction::Navigate(view) => campus.ui_navigate(view),
            UiAction::StartResearch(index) => campus.ui_start_research(index),
            UiAction::NextDay => campus.ui_next_day(),
            UiAction::AcceptMissileOffer(index) => {
                crate::campus::accept_missile_offer(&mut campus, &mut procurement, index)
            }
            UiAction::DeliverMissile(id) => crate::campus::deliver_missile_contract(
                &mut campus,
                &mut procurement,
                Some(id),
                &missile.vehicle,
                &program.program,
                log.head().map(|revision| revision.id),
                &trials.history,
                &mut log,
            ),
            UiAction::SelectMissileContract(id) => campus.message = procurement.select(id),
            UiAction::Camera(action) => camera.0 = Some(action),
            // Program, trial, workshop and revision actions need more resources than the campaign;
            // `campus::board::handle_extra_actions` runs after this system and dispatches them.
            UiAction::RecordMissileRevision
            | UiAction::BeginRevisionName(_)
            | UiAction::CommitRevisionName
            | UiAction::CancelRevisionName
            | UiAction::CompareRevision(_)
            | UiAction::ClearRevisionComparison
            | UiAction::RestoreRevision(_)
            | UiAction::ReflyMissileContract
            | UiAction::Menu(_)
            | UiAction::Missile(_)
            | UiAction::Program(_)
            | UiAction::Trial(_) => {}
        }
    }
}
