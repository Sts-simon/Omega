//! Title screen and campaign slot manager. The game starts behind this screen with no active
//! slot; choosing a campaign selects a `saves/slot_N` directory and loads its resources.

use crate::campus::CampusState;
use crate::interface::{UiAction, UiActionButton};
use crate::missile::MissileSession;
use crate::procurement::Procurement;
use crate::program_editor::MissileProgramSession;
use crate::revisions::RevisionLog;
use crate::save::{self, SaveSlotSummary};
use crate::trials::TrialSession;
use crate::ui_kit::{self, ButtonTone, UiInteraction};
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum Frontend {
    #[default]
    Menu,
    Game,
}

#[derive(Clone, Debug)]
pub(crate) enum MenuAction {
    Open,
    Resume,
    New,
    Load(u32),
    Create(u32),
    Delete(u32),
    ConfirmDelete(u32),
    CancelDelete,
    Refresh,
}

#[derive(Component)]
pub(crate) struct MenuRoot;

#[derive(Component)]
pub(crate) struct MenuBody;

#[derive(Resource, Default)]
pub(crate) struct MainMenuState {
    slots: Vec<SaveSlotSummary>,
    message: String,
    confirm_delete: Option<u32>,
}

impl MainMenuState {
    fn refresh(&mut self) {
        self.slots = save::list_slots();
        self.confirm_delete = None;
    }
}

pub(crate) fn setup(mut commands: Commands, mut menu: ResMut<MainMenuState>) {
    save::ensure_legacy_import();
    menu.refresh();
    menu.message = "Choisissez une campagne pour continuer, ou créez-en une.".to_string();
    commands
        .spawn((
            MenuRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(Val::Px(24.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.008, 0.015, 0.022, 1.0)),
            GlobalZIndex(1000),
            FocusPolicy::Block,
            UiInteraction::None,
        ))
        .with_children(|root| {
            root.spawn((
                MenuBody,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
            ))
            .with_children(|body| build_menu(body, &menu, save::active_slot()));
        });
}

fn build_menu(body: &mut ChildSpawnerCommands<'_>, menu: &MainMenuState, active: Option<u32>) {
    body.spawn((
        Node {
            width: Val::Percent(88.0),
            max_width: Val::Px(1180.0),
            max_height: Val::Percent(92.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(12.0),
            padding: UiRect::all(Val::Px(24.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(9.0)),
            ..default()
        },
        BackgroundColor(ui_kit::PANEL),
        BorderColor::all(ui_kit::ACCENT),
    ))
    .with_children(|panel| {
        panel
            .spawn(Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(16.0),
                ..default()
            })
            .with_children(|header| {
                header
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        flex_grow: 1.0,
                        row_gap: Val::Px(3.0),
                        ..default()
                    })
                    .with_children(|title| {
                        ui_kit::label(title, "OMEGA", 34.0, ui_kit::ACCENT);
                        ui_kit::label(
                            title,
                            "CONCEPTION DE VÉHICULES · GESTION DES CAMPAGNES",
                            10.5,
                            ui_kit::TEXT_DIM,
                        );
                    });
                ui_kit::chip(header, "ÉCRAN PRINCIPAL", ui_kit::ACCENT);
            });

        panel
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                min_height: Val::Px(0.0),
                row_gap: Val::Px(6.0),
                overflow: Overflow::scroll_y(),
                ..default()
            })
            .with_children(|list| {
                for summary in &menu.slots {
                    slot_row(list, summary, active, menu.confirm_delete);
                }
            });

        panel
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            })
            .with_children(|footer| {
                ui_kit::label(footer, menu.message.clone(), 10.5, ui_kit::TEXT_DIM);
                footer
                    .spawn(Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|buttons| {
                        if menu.slots.iter().any(|slot| !slot.exists) {
                            ui_kit::button(
                                buttons,
                                "NOUVELLE CAMPAGNE",
                                UiAction::Menu(MenuAction::New),
                                ButtonTone::Primary,
                                Val::Px(185.0),
                            );
                        } else {
                            ui_kit::button(
                                buttons,
                                "NOUVELLE CAMPAGNE",
                                UiAction::Menu(MenuAction::New),
                                ButtonTone::Disabled,
                                Val::Px(185.0),
                            );
                        }
                        if active.is_some() {
                            ui_kit::button(
                                buttons,
                                "REPRENDRE",
                                UiAction::Menu(MenuAction::Resume),
                                ButtonTone::Secondary,
                                Val::Px(110.0),
                            );
                        }
                        ui_kit::button(
                            buttons,
                            "RAFRAÎCHIR",
                            UiAction::Menu(MenuAction::Refresh),
                            ButtonTone::Ghost,
                            Val::Px(110.0),
                        );
                    });
            });
    });
}

fn slot_row(
    parent: &mut ChildSpawnerCommands<'_>,
    summary: &SaveSlotSummary,
    active: Option<u32>,
    confirm_delete: Option<u32>,
) {
    let is_active = active == Some(summary.id);
    let (background, border) = if is_active {
        (ui_kit::CARD_SELECTED, ui_kit::ACCENT)
    } else {
        (ui_kit::CARD, ui_kit::NEUTRAL)
    };
    parent
        .spawn((
            Node {
                min_height: Val::Px(64.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(background),
            BorderColor::all(border),
        ))
        .with_children(|row| {
            row.spawn(Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                row_gap: Val::Px(3.0),
                ..default()
            })
            .with_children(|info| {
                info.spawn(Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    ..default()
                })
                .with_children(|head| {
                    ui_kit::label(head, format!("CAMPAGNE {}", summary.id), 14.0, ui_kit::TEXT);
                    if is_active {
                        ui_kit::chip(head, "ACTIVE", ui_kit::ACCENT);
                    } else if !summary.exists {
                        ui_kit::chip(head, "VIDE", ui_kit::NEUTRAL);
                    } else if summary.error.is_some() {
                        ui_kit::chip(head, "ILLISIBLE", ui_kit::BAD);
                    }
                });
                if !summary.exists {
                    ui_kit::label(
                        info,
                        "Emplacement vide · créez une nouvelle campagne.",
                        10.0,
                        ui_kit::TEXT_DIM,
                    );
                } else if let Some(error) = &summary.error {
                    ui_kit::label(info, format!("Sauvegarde illisible : {error}"), 10.0, ui_kit::BAD);
                } else {
                    ui_kit::label(
                        info,
                        format!(
                            "Jour {} · {} crédits · {} technologies",
                            summary.day.max(1),
                            ui_kit::thousands(summary.credits),
                            summary.technologies
                        ),
                        10.5,
                        ui_kit::TEXT_DIM,
                    );
                    ui_kit::label(
                        info,
                        format!(
                            "{} contrats actifs · {} livrés",
                            summary.active_contracts, summary.completed_contracts
                        ),
                        9.5,
                        ui_kit::TEXT_DIM,
                    );
                }
            });

            row.spawn(Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                ..default()
            })
            .with_children(|actions| {
                if !summary.exists {
                    ui_kit::button(
                        actions,
                        "CRÉER",
                        UiAction::Menu(MenuAction::Create(summary.id)),
                        ButtonTone::Primary,
                        Val::Px(92.0),
                    );
                    return;
                }
                if is_active {
                    ui_kit::button(
                        actions,
                        "REPRENDRE",
                        UiAction::Menu(MenuAction::Resume),
                        ButtonTone::Primary,
                        Val::Px(92.0),
                    );
                } else {
                    let tone = if summary.error.is_some() {
                        ButtonTone::Disabled
                    } else {
                        ButtonTone::Secondary
                    };
                    ui_kit::button(
                        actions,
                        "CHARGER",
                        UiAction::Menu(MenuAction::Load(summary.id)),
                        tone,
                        Val::Px(92.0),
                    );
                }
                if confirm_delete == Some(summary.id) {
                    ui_kit::button(
                        actions,
                        "CONFIRMER",
                        UiAction::Menu(MenuAction::ConfirmDelete(summary.id)),
                        ButtonTone::Danger,
                        Val::Px(92.0),
                    );
                    ui_kit::button(
                        actions,
                        "ANNULER",
                        UiAction::Menu(MenuAction::CancelDelete),
                        ButtonTone::Ghost,
                        Val::Px(80.0),
                    );
                } else if !is_active {
                    ui_kit::button(
                        actions,
                        "SUPPRIMER",
                        UiAction::Menu(MenuAction::Delete(summary.id)),
                        ButtonTone::Danger,
                        Val::Px(92.0),
                    );
                }
            });
        });
}

pub(crate) fn sync_frontend(frontend: Res<Frontend>, mut nav: Query<&mut Node, With<crate::interface::NavRoot>>) {
    if let Ok(mut node) = nav.single_mut() {
        node.display = if *frontend == Frontend::Game {
            Display::Flex
        } else {
            Display::None
        };
    }
}

fn set_menu_visible(roots: &mut Query<&mut Node, With<MenuRoot>>, visible: bool) {
    if let Ok(mut root) = roots.single_mut() {
        root.display = if visible { Display::Flex } else { Display::None };
    }
}

fn start_new(
    id: u32,
    campus: &mut CampusState,
    log: &mut RevisionLog,
    missile: &mut MissileSession,
    program: &mut MissileProgramSession,
    trials: &mut TrialSession,
    procurement: &mut Procurement,
) -> Result<(), String> {
    save::select_slot(id)?;
    campus.reset_new();
    log.reset();
    *missile = MissileSession::new_default();
    *program = MissileProgramSession::default();
    trials.reset_new();
    procurement.new_campaign();
    Ok(())
}

fn load_slot(
    id: u32,
    campus: &mut CampusState,
    log: &mut RevisionLog,
    missile: &mut MissileSession,
    program: &mut MissileProgramSession,
    trials: &mut TrialSession,
    procurement: &mut Procurement,
) -> Result<(), String> {
    save::select_slot(id)?;
    campus.load_active();
    log.load_active();
    missile.load_active();
    program.load_active();
    procurement.load_active();
    trials.load_active();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_actions(
    mut commands: Commands,
    buttons: Query<(&UiInteraction, &UiActionButton), Changed<UiInteraction>>,
    mut frontend: ResMut<Frontend>,
    mut menu: ResMut<MainMenuState>,
    mut campus: ResMut<CampusState>,
    mut log: ResMut<RevisionLog>,
    mut missile: ResMut<MissileSession>,
    mut program: ResMut<MissileProgramSession>,
    mut trials: ResMut<TrialSession>,
    mut procurement: ResMut<Procurement>,
    mut roots: Query<&mut Node, With<MenuRoot>>,
    body: Query<Entity, With<MenuBody>>,
) {
    let Ok(body) = body.single() else {
        return;
    };
    for (interaction, button) in &buttons {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        let UiAction::Menu(action) = button.0.clone() else {
            continue;
        };
        match action {
            MenuAction::Open => {
                *frontend = Frontend::Menu;
                menu.refresh();
                menu.message = "Choisissez une campagne pour continuer, ou créez-en une.".to_string();
                set_menu_visible(&mut roots, true);
                rebuild_menu(&mut commands, &menu, body);
            }
            MenuAction::Resume => {
                if save::active_slot().is_some() {
                    *frontend = Frontend::Game;
                    set_menu_visible(&mut roots, false);
                } else {
                    menu.message = "Aucune campagne active : choisissez une sauvegarde.".to_string();
                    rebuild_menu(&mut commands, &menu, body);
                }
            }
            MenuAction::Refresh => {
                menu.refresh();
                menu.message = "Liste des campagnes actualisée.".to_string();
                rebuild_menu(&mut commands, &menu, body);
            }
            MenuAction::Load(id) => {
                if menu
                    .slots
                    .iter()
                    .any(|slot| slot.id == id && slot.exists && slot.error.is_none())
                {
                    match load_slot(
                        id,
                        &mut campus,
                        &mut log,
                        &mut missile,
                        &mut program,
                        &mut trials,
                        &mut procurement,
                    ) {
                        Ok(()) => {
                            menu.message = format!("Campagne {id} chargée.");
                            *frontend = Frontend::Game;
                            set_menu_visible(&mut roots, false);
                        }
                        Err(error) => {
                            menu.message = format!("Impossible de charger la campagne {id} : {error}");
                            rebuild_menu(&mut commands, &menu, body);
                        }
                    }
                } else {
                    menu.message = format!("La sauvegarde {id} est illisible.");
                    rebuild_menu(&mut commands, &menu, body);
                }
            }
            MenuAction::Create(id) => {
                if menu.slots.iter().any(|slot| slot.id == id && !slot.exists) {
                    match start_new(
                        id,
                        &mut campus,
                        &mut log,
                        &mut missile,
                        &mut program,
                        &mut trials,
                        &mut procurement,
                    ) {
                        Ok(()) => {
                            menu.message = format!("Nouvelle campagne {id}.");
                            *frontend = Frontend::Game;
                            set_menu_visible(&mut roots, false);
                        }
                        Err(error) => {
                            menu.message = format!("Impossible de créer la campagne {id} : {error}");
                            rebuild_menu(&mut commands, &menu, body);
                        }
                    }
                } else {
                    menu.message = format!("L'emplacement {id} est déjà occupé.");
                    rebuild_menu(&mut commands, &menu, body);
                }
            }
            MenuAction::New => {
                if let Some(id) = menu.slots.iter().find(|slot| !slot.exists).map(|slot| slot.id) {
                    match start_new(
                        id,
                        &mut campus,
                        &mut log,
                        &mut missile,
                        &mut program,
                        &mut trials,
                        &mut procurement,
                    ) {
                        Ok(()) => {
                            menu.message = format!("Nouvelle campagne {id}.");
                            *frontend = Frontend::Game;
                            set_menu_visible(&mut roots, false);
                        }
                        Err(error) => {
                            menu.message = format!("Impossible de créer la campagne {id} : {error}");
                            rebuild_menu(&mut commands, &menu, body);
                        }
                    }
                } else {
                    menu.message = "Les six emplacements sont occupés : supprimez une campagne.".to_string();
                    rebuild_menu(&mut commands, &menu, body);
                }
            }
            MenuAction::Delete(id) => {
                if save::active_slot() == Some(id) {
                    menu.message = "La campagne active ne peut pas être supprimée.".to_string();
                } else {
                    menu.confirm_delete = Some(id);
                    menu.message = format!("Confirmer la suppression de la campagne {id} ?");
                }
                rebuild_menu(&mut commands, &menu, body);
            }
            MenuAction::ConfirmDelete(id) => {
                if save::active_slot() == Some(id) {
                    menu.message = "La campagne active ne peut pas être supprimée.".to_string();
                } else {
                    match save::delete_slot(id) {
                        Ok(()) => {
                            menu.refresh();
                            menu.message = format!("Campagne {id} supprimée.");
                        }
                        Err(error) => menu.message = format!("Suppression impossible : {error}"),
                    }
                }
                rebuild_menu(&mut commands, &menu, body);
            }
            MenuAction::CancelDelete => {
                menu.confirm_delete = None;
                menu.message = "Suppression annulée.".to_string();
                rebuild_menu(&mut commands, &menu, body);
            }
        }
    }
}

fn rebuild_menu(commands: &mut Commands, menu: &MainMenuState, body: Entity) {
    let active = save::active_slot();
    commands.entity(body).despawn_related::<Children>();
    commands
        .entity(body)
        .with_children(|body| build_menu(body, menu, active));
}
