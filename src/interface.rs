//! First-pass desktop UX: persistent navigation, context tool shelves and
//! mouse-driven camera controls are implemented separately from the solvers.
use crate::campus::{CampusState, CampusView};
use crate::design::{
    Assembly, DesignSession, GizmoMode, GizmoState, PartKind, PolygonSketchSession, build_assembly_mesh_thermal_config,
    build_polygon_sketch_thermal_config,
};
use crate::jobs::JobManager;
use crate::sequence::SequenceWorkspace;
use crate::ui_kit::UiInteraction;
use crate::ui_kit::{self, ButtonTone, NavStatus};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolGroup {
    Workshop,
    Programming,
    Computation,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum UiAction {
    Navigate(CampusView),
    RefreshContracts,
    AcceptOffer(usize),
    DeliverContract,
    AddPanel,
    AddBeam,
    AddEnclosure,
    AddPlatform,
    AddCylinder,
    SelectPrevious,
    SelectNext,
    MoveSelected([f64; 3]),
    SetGizmoMode(GizmoMode),
    DeleteSelected,
    ConnectNearest,
    DisconnectSelected,
    CycleMaterial,
    ToggleSnap,
    Undo,
    Redo,
    SaveAssembly,
    LoadAssembly,
    ToggleSketch,
    ToggleProfile,
    SubmitDesign,
    OpenSequencer,
    QueueThermal,
    QueueTruss,
    MeshAssembly,
    MeshSketch,
    QueueContractAnalysis,
    RunFieldTest,
    OpenLatestReport,
    CancelLatestJob,
    SelectContract(u32),
    RecordRevision,
    RestoreRevision,
    ToggleHelp,
    Missile(crate::missile::MissileAction),
    Program(crate::program_editor::ProgramAction),
    StartResearch(usize),
    NextDay,
}

#[derive(Component)]
pub(crate) struct UiActionButton(pub(crate) UiAction);

#[derive(Component)]
pub(crate) struct NavigationButton(CampusView);

#[derive(Component)]
pub(crate) struct ToolPanel(ToolGroup);

#[derive(Component)]
pub(crate) struct NavigationBar;

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
            NavigationBar,
        ))
        .id();

    commands.entity(nav).with_children(|parent| {
        parent.spawn((
            Text::new("OMEGA"),
            TextFont {
                font_size: FontSize::Px(19.0),
                ..default()
            },
            TextColor(Color::srgb(0.36, 0.84, 0.94)),
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
        add_nav_button(parent, "SEQUENCER", CampusView::Programming);
        add_nav_button(parent, "COMPUTE", CampusView::Computation);
        add_nav_button(parent, "LEGACY BENCH", CampusView::Construction);
        // Status messages ("Opened the contract office", "Dossier rejected: ...") live in the bar.
        parent
            .spawn(Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                max_height: Val::Px(46.0),
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
                font_size: FontSize::Px(13.0),
                ..default()
            },
            TextColor(ui_kit::ACCENT),
            NavStatus,
        ));
    });

    // The Contract Office and the Research Centre are card boards built in `campus::board`.
    spawn_tool_panel(
        &mut commands,
        ToolGroup::Workshop,
        "WORKSHOP",
        "Pick a part in the 3D view. Rotation and size are in the exact inspector on the right.",
        |parent| {
            add_section_heading(parent, "ADD COMPONENT");
            add_row(
                parent,
                &[
                    ("+ PANEL", UiAction::AddPanel, ButtonTone::Secondary),
                    ("+ BEAM", UiAction::AddBeam, ButtonTone::Secondary),
                    ("+ CYL", UiAction::AddCylinder, ButtonTone::Secondary),
                ],
            );
            add_row(
                parent,
                &[
                    ("+ ENCLOSURE", UiAction::AddEnclosure, ButtonTone::Secondary),
                    ("+ PLATFORM", UiAction::AddPlatform, ButtonTone::Secondary),
                ],
            );
            add_section_heading(parent, "GIZMO");
            add_row(
                parent,
                &[
                    (
                        "TRANSLATE (T)",
                        UiAction::SetGizmoMode(GizmoMode::Translate),
                        ButtonTone::Secondary,
                    ),
                    (
                        "ROTATE (R)",
                        UiAction::SetGizmoMode(GizmoMode::Rotate),
                        ButtonTone::Secondary,
                    ),
                ],
            );
            add_section_heading(parent, "SELECTION & LINKS");
            add_row(
                parent,
                &[
                    ("PREV", UiAction::SelectPrevious, ButtonTone::Secondary),
                    ("NEXT", UiAction::SelectNext, ButtonTone::Secondary),
                ],
            );
            add_row(
                parent,
                &[
                    ("CONNECT", UiAction::ConnectNearest, ButtonTone::Secondary),
                    ("UNLINK", UiAction::DisconnectSelected, ButtonTone::Secondary),
                ],
            );
            add_row(
                parent,
                &[("DELETE SELECTED", UiAction::DeleteSelected, ButtonTone::Danger)],
            );
            add_section_heading(parent, "MOVE  0.10 M");
            add_row(
                parent,
                &[
                    ("X-", UiAction::MoveSelected([-0.1, 0.0, 0.0]), ButtonTone::Ghost),
                    ("X+", UiAction::MoveSelected([0.1, 0.0, 0.0]), ButtonTone::Ghost),
                    ("Y-", UiAction::MoveSelected([0.0, -0.1, 0.0]), ButtonTone::Ghost),
                    ("Y+", UiAction::MoveSelected([0.0, 0.1, 0.0]), ButtonTone::Ghost),
                    ("Z-", UiAction::MoveSelected([0.0, 0.0, -0.1]), ButtonTone::Ghost),
                    ("Z+", UiAction::MoveSelected([0.0, 0.0, 0.1]), ButtonTone::Ghost),
                ],
            );
            add_section_heading(parent, "HISTORY & FILES");
            add_row(
                parent,
                &[
                    ("UNDO", UiAction::Undo, ButtonTone::Secondary),
                    ("REDO", UiAction::Redo, ButtonTone::Secondary),
                ],
            );
            add_row(
                parent,
                &[
                    ("SAVE", UiAction::SaveAssembly, ButtonTone::Secondary),
                    ("LOAD", UiAction::LoadAssembly, ButtonTone::Secondary),
                ],
            );
            add_row(
                parent,
                &[
                    ("MATERIAL", UiAction::CycleMaterial, ButtonTone::Secondary),
                    ("SNAP GRID", UiAction::ToggleSnap, ButtonTone::Secondary),
                ],
            );
            add_section_heading(parent, "REVISIONS");
            add_row(
                parent,
                &[
                    ("RECORD (CTRL+B)", UiAction::RecordRevision, ButtonTone::Secondary),
                    ("RESTORE", UiAction::RestoreRevision, ButtonTone::Secondary),
                ],
            );
            add_section_heading(parent, "WORKFLOW");
            add_row(
                parent,
                &[
                    ("DRAW SURFACE", UiAction::ToggleSketch, ButtonTone::Secondary),
                    ("PROFILE", UiAction::ToggleProfile, ButtonTone::Secondary),
                ],
            );
            add_row(parent, &[("SHORTCUTS (H)", UiAction::ToggleHelp, ButtonTone::Ghost)]);
            add_row(
                parent,
                &[("SUBMIT DESIGN", UiAction::SubmitDesign, ButtonTone::Primary)],
            );
        },
    );

    spawn_tool_panel(
        &mut commands,
        ToolGroup::Programming,
        "PROGRAMMING STUDIO",
        "Create and test abstract event-driven measurement and data-processing sequences.",
        |parent| {
            add_section_heading(parent, "PROGRAM FLOW");
            add_button(parent, "OPEN VISUAL PROGRAM EDITOR", UiAction::OpenSequencer);
        },
    );

    spawn_tool_panel(
        &mut commands,
        ToolGroup::Computation,
        "COMPUTATION CENTRE",
        "Queue scientific studies. Jobs run on worker threads while the interface remains available.",
        |parent| {
            add_button(parent, "RUN THERMAL STUDY", UiAction::QueueThermal);
            add_button(parent, "RUN CIVIL TRUSS STUDY", UiAction::QueueTruss);
            add_button(parent, "MESH WORKSHOP ASSEMBLY", UiAction::MeshAssembly);
            add_button(parent, "MESH POLYGON SKETCH", UiAction::MeshSketch);
            add_button(parent, "ANALYSE ACTIVE PROJECT", UiAction::QueueContractAnalysis);
            add_button(parent, "RUN FIELD TEST", UiAction::RunFieldTest);
            add_button(parent, "OPEN LATEST REPORT", UiAction::OpenLatestReport);
            add_button(parent, "CANCEL LATEST JOB", UiAction::CancelLatestJob);
        },
    );
}

fn spawn_tool_panel(
    commands: &mut Commands,
    group: ToolGroup,
    title: &'static str,
    description: &'static str,
    build: impl FnOnce(&mut ChildSpawnerCommands<'_>),
) {
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(14.0),
                top: Val::Px(78.0),
                width: Val::Px(226.0),
                max_height: Val::Percent(87.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(7.0),
                padding: UiRect::all(Val::Px(12.0)),
                overflow: Overflow::scroll_y(),
                display: Display::None,
                ..default()
            },
            UiInteraction::None,
            ScrollPosition::default(),
            BackgroundColor(Color::srgba(0.018, 0.032, 0.046, 0.94)),
            ToolPanel(group),
        ))
        .id();

    commands.entity(panel).with_children(|parent| {
        parent.spawn((
            Text::new(title),
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(Color::srgb(0.37, 0.84, 0.94)),
            Node {
                margin: UiRect {
                    bottom: Val::Px(2.0),
                    ..default()
                },
                ..default()
            },
        ));
        parent.spawn((
            Text::new(description),
            TextFont {
                font_size: FontSize::Px(10.5),
                ..default()
            },
            TextColor(Color::srgb(0.63, 0.70, 0.75)),
            Node {
                margin: UiRect {
                    bottom: Val::Px(6.0),
                    ..default()
                },
                ..default()
            },
        ));
        build(parent);
    });
}

fn add_section_heading(parent: &mut ChildSpawnerCommands<'_>, label: &'static str) {
    parent.spawn((
        Text::new(label),
        TextFont {
            font_size: FontSize::Px(9.5),
            ..default()
        },
        TextColor(Color::srgb(0.35, 0.72, 0.80)),
        Node {
            width: Val::Percent(100.0),
            margin: UiRect {
                top: Val::Px(8.0),
                bottom: Val::Px(2.0),
                ..default()
            },
            ..default()
        },
    ));
}

fn add_nav_button(parent: &mut ChildSpawnerCommands<'_>, label: &'static str, target: CampusView) {
    parent
        .spawn((
            // Bevy 0.20 deprecated the `Button` marker; recreate the components it required.
            UiInteraction::default(),
            bevy::ui::FocusPolicy::Block,
            Node {
                width: Val::Px(112.0),
                height: Val::Px(36.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect {
                    left: Val::Px(10.0),
                    right: Val::Px(10.0),
                    ..default()
                },
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.12, 0.16)),
            NavigationButton(target),
            UiActionButton(UiAction::Navigate(target)),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.90, 0.94, 0.97)),
            ));
        });
}

fn add_row(parent: &mut ChildSpawnerCommands<'_>, items: &[(&str, UiAction, ButtonTone)]) {
    parent
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|row| {
            for (label, action, tone) in items {
                ui_kit::button_row_item(row, *label, *action, *tone);
            }
        });
}

#[allow(dead_code)]
fn add_button(parent: &mut ChildSpawnerCommands<'_>, label: &'static str, action: UiAction) {
    parent
        .spawn((
            // Bevy 0.20 deprecated the `Button` marker; recreate the components it required.
            UiInteraction::default(),
            bevy::ui::FocusPolicy::Block,
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(32.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect {
                    left: Val::Px(8.0),
                    right: Val::Px(8.0),
                    top: Val::Px(6.0),
                    bottom: Val::Px(6.0),
                },
                ..default()
            },
            BackgroundColor(Color::srgb(0.075, 0.12, 0.16)),
            UiActionButton(action),
        ))
        .with_children(|button| {
            button.spawn((
                Text::new(label),
                TextFont {
                    font_size: FontSize::Px(10.5),
                    ..default()
                },
                TextColor(Color::srgb(0.88, 0.92, 0.95)),
            ));
        });
}

pub fn handle_actions(
    mut buttons: Query<(&UiInteraction, &UiActionButton), Changed<UiInteraction>>,
    mut campus: ResMut<CampusState>,
    mut design: ResMut<DesignSession>,
    mut sketch: ResMut<PolygonSketchSession>,
    mut sequence: ResMut<SequenceWorkspace>,
    mut jobs: ResMut<JobManager>,
    mut gizmo: ResMut<GizmoState>,
) {
    for (interaction, button) in &mut buttons {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        match button.0 {
            UiAction::Navigate(view) => {
                campus.ui_navigate(view);
                sketch.cancel_vertex_drag();
                sketch.active = false;
                sketch.edit_vertices = false;
            }
            UiAction::RefreshContracts => campus.ui_refresh_contracts(),
            UiAction::AcceptOffer(index) => campus.ui_accept_offer(index),
            UiAction::DeliverContract => campus.ui_deliver_selected(&design),
            // Missile workshop and program actions are owned by `campus::handle_extra_actions`,
            // which also has the research capabilities and the vehicle.
            UiAction::Program(_) => {}
            UiAction::AddPanel => design.add_part(PartKind::Panel),
            UiAction::AddBeam => design.add_part(PartKind::Beam),
            UiAction::AddEnclosure => design.add_part(PartKind::Enclosure),
            UiAction::AddPlatform => design.add_part(PartKind::Platform),
            UiAction::AddCylinder => design.add_part(PartKind::Cylinder),
            UiAction::SelectPrevious => design.select_next(true),
            UiAction::SelectNext => design.select_next(false),
            UiAction::MoveSelected(delta) => design.move_selected(delta),
            UiAction::SetGizmoMode(mode) => {
                design.finish_gizmo_edit();
                gizmo.set_mode(mode);
            }
            UiAction::DeleteSelected => design.remove_selected(),
            UiAction::ConnectNearest => match design.connect_selected_to_nearest() {
                Ok(id) => info!("Created assembly link {id}"),
                Err(message) => info!("Connection not created: {message}"),
            },
            UiAction::DisconnectSelected => {
                let removed = design.disconnect_selected();
                info!("Removed {removed} connection(s)");
            }
            UiAction::CycleMaterial => design.cycle_selected_material(),
            UiAction::ToggleSnap => design.toggle_snap_grid(),
            UiAction::Undo => {
                design.undo();
            }
            UiAction::Redo => {
                design.redo();
            }
            UiAction::SaveAssembly => {
                let result = design
                    .assembly
                    .to_bytes()
                    .and_then(|bytes| crate::save::write(crate::save::SaveFile::Assembly, &bytes));
                match result {
                    Ok(()) => info!("Assembly saved to the save slot"),
                    Err(error) => warn!("Assembly save failed: {error}"),
                }
            }
            UiAction::LoadAssembly => match crate::save::read(crate::save::SaveFile::Assembly)
                .and_then(|bytes| bytes.ok_or_else(|| "no saved assembly found".to_string()))
                .and_then(|bytes| Assembly::from_bytes(&bytes))
            {
                Ok(assembly) => design.replace_assembly(assembly),
                Err(error) => warn!("Assembly load failed: {error}"),
            },
            UiAction::ToggleSketch => {
                sketch.cancel_vertex_drag();
                sketch.active = !sketch.active;
                if !sketch.active {
                    sketch.edit_vertices = false;
                }
                sketch.revision = sketch.revision.wrapping_add(1);
            }
            UiAction::ToggleProfile => campus.profile_editor_open = !campus.profile_editor_open,
            UiAction::SubmitDesign => campus.ui_submit_design(&design),
            UiAction::OpenSequencer => sequence.toggle_graph_from_ui(),
            UiAction::QueueThermal => {
                jobs.enqueue_thermal_demo();
            }
            UiAction::QueueTruss => {
                jobs.enqueue_truss_demo();
            }
            UiAction::MeshAssembly => match build_assembly_mesh_thermal_config(&design.assembly, 0.1) {
                Ok(built) => {
                    jobs.enqueue_mesh_thermal(built.config);
                }
                Err(error) => warn!("Assembly meshing failed: {error}"),
            },
            UiAction::MeshSketch => match build_polygon_sketch_thermal_config(&sketch.document.regions) {
                Ok(config) => {
                    jobs.enqueue_mesh_thermal(config);
                }
                Err(error) => warn!("Polygon sketch meshing failed: {error}"),
            },
            UiAction::QueueContractAnalysis => campus.queue_selected_analysis(&mut jobs, &design),
            UiAction::RunFieldTest => campus.run_selected_field_test(&design),
            UiAction::OpenLatestReport => match jobs.open_latest_report() {
                Ok(()) => info!("Opened latest scientific report"),
                Err(error) => warn!("Could not open report: {error}"),
            },
            UiAction::CancelLatestJob => {
                jobs.cancel_latest();
            }
            UiAction::SelectContract(id) => campus.ui_select_contract(id),
            // Handled by `campus::board::handle_extra_actions`.
            UiAction::StartResearch(index) => campus.ui_start_research(index),
            UiAction::NextDay => campus.ui_next_day(),
            UiAction::RecordRevision | UiAction::RestoreRevision | UiAction::ToggleHelp | UiAction::Missile(_) => {}
        }
    }
}

pub fn sync_layout(
    campus: Res<CampusState>,
    sequence: Res<SequenceWorkspace>,
    mut panels: Query<(&ToolPanel, &mut Node), Without<NavigationBar>>,
    mut nav: Query<&mut Node, With<NavigationBar>>,
    gizmo: Res<GizmoState>,
    mut nav_buttons: Query<(&UiInteraction, &NavigationButton, &mut BackgroundColor)>,
    mut action_buttons: Query<
        (
            &UiInteraction,
            &UiActionButton,
            &mut BackgroundColor,
            Option<&ButtonTone>,
        ),
        Without<NavigationButton>,
    >,
) {
    let overlays_hidden = sequence.graph_is_open();
    if let Ok(mut node) = nav.single_mut() {
        node.display = if overlays_hidden { Display::None } else { Display::Flex };
    }
    for (panel, mut node) in &mut panels {
        let visible_group = matches!(
            (panel.0, campus.view),
            (ToolGroup::Workshop, CampusView::Construction)
                | (ToolGroup::Programming, CampusView::Programming)
                | (ToolGroup::Computation, CampusView::Computation)
        );
        node.display = if visible_group && !overlays_hidden {
            Display::Flex
        } else {
            Display::None
        };
    }
    for (interaction, target, mut background) in &mut nav_buttons {
        let active = target.0 == campus.view;
        background.0 = if active {
            Color::srgb(0.10, 0.31, 0.36)
        } else if *interaction == UiInteraction::Hovered {
            Color::srgb(0.13, 0.20, 0.25)
        } else if *interaction == UiInteraction::Pressed {
            Color::srgb(0.16, 0.38, 0.43)
        } else {
            Color::srgb(0.08, 0.12, 0.16)
        };
    }
    for (interaction, action, mut background, tone) in &mut action_buttons {
        let is_active_gizmo_mode = match action.0 {
            UiAction::SetGizmoMode(mode) => mode == gizmo.mode,
            _ => false,
        };
        background.0 = if is_active_gizmo_mode {
            Color::srgb(0.10, 0.34, 0.38)
        } else {
            tone.copied().unwrap_or(ButtonTone::Secondary).background(*interaction)
        };
    }
}
