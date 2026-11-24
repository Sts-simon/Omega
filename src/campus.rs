//! Shared campus state: the day clock, credits, research and view routing. The contract office,
//! workshop, program editor and trial range own their own content; this module holds what they
//! share, the island picking and the save round trip.

use crate::ui_kit::UiInteraction;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use omega_rules::ResearchState;
use serde::{Deserialize, Serialize};

mod board;

pub(crate) use board::{
    ToastText, handle_extra_actions, poll_trial_batch, scroll_program_list, setup_boards, sync_boards,
    update_batch_progress, update_nav_status, update_toast,
};

const CAMPAIGN_SCHEMA: u32 = 2;
/// Contracts a fresh office can hold at once; `parallel_projects` raises it.
pub(crate) const BASE_ACTIVE_PROJECTS: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FacilityKind {
    Contracts,
    Construction,
    Research,
    Programming,
}

#[derive(Component)]
pub struct BuildingVisual {
    pub facility: FacilityKind,
    pub half_extents: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampusView {
    Island,
    Contracts,
    /// Missile workshop (staged, resizable parts).
    Missile,
    /// Onboard missile program editor.
    Program,
    /// Deterministic flight trials of the current vehicle and program.
    Trials,
    Research,
}

#[derive(Serialize, Deserialize)]
struct CampaignSave {
    schema_version: u32,
    credits: u64,
    #[serde(default)]
    research: ResearchState,
    #[serde(default)]
    day: u32,
}

#[derive(Resource)]
pub struct CampusState {
    pub view: CampusView,
    pub credits: u64,
    pub message: String,
    pub(crate) research: ResearchState,
    pub(crate) day: u32,
    dirty: bool,
}

impl Default for CampusState {
    fn default() -> Self {
        match load_campaign() {
            Some(save) => Self {
                view: CampusView::Island,
                credits: save.credits,
                message: "Campaign loaded".to_string(),
                research: save.research,
                day: save.day.max(1),
                dirty: false,
            },
            None => Self {
                view: CampusView::Island,
                credits: 25_000,
                message: "Select a facility on the island".to_string(),
                research: ResearchState::default(),
                day: 1,
                dirty: true,
            },
        }
    }
}

impl CampusState {
    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub(crate) fn ui_start_research(&mut self, index: usize) {
        let tree = omega_rules::default_tree();
        let Some(tech) = tree.get(index) else { return };
        self.message = match self.research.start(&tree, &tech.id, self.credits) {
            Ok(cost) => {
                self.credits -= cost;
                self.mark_dirty();
                format!(
                    "Research started: {} ({} days, {} cr)",
                    tech.label, tech.duration_days, cost
                )
            }
            Err(error) => format!("Cannot research {}: {error}", tech.label),
        };
    }

    pub(crate) fn ui_next_day(&mut self) {
        self.day += 1;
        let finished = self.research.advance_days(1);
        let tree = omega_rules::default_tree();
        self.message = match finished.first().and_then(|id| tree.iter().find(|t| &t.id == id)) {
            Some(tech) => format!("Day {}: research completed - {}", self.day, tech.label),
            None => format!("Day {}", self.day),
        };
        self.mark_dirty();
    }

    pub(crate) fn owned_capabilities(&self) -> std::collections::BTreeSet<String> {
        self.research.owned_capabilities(&omega_rules::default_tree())
    }

    /// Project slots the research grants currently allow.
    pub(crate) fn max_active_projects(&self) -> usize {
        self.research
            .max_active_projects(&omega_rules::default_tree(), BASE_ACTIVE_PROJECTS)
    }

    pub(crate) fn ui_navigate(&mut self, view: CampusView) {
        self.view = view;
        self.message = match view {
            CampusView::Island => "Returned to the research island".to_string(),
            CampusView::Contracts => "Opened the contract office".to_string(),
            CampusView::Missile => "Opened the missile workshop".to_string(),
            CampusView::Program => "Opened the missile program editor".to_string(),
            CampusView::Trials => "Opened the trial range".to_string(),
            CampusView::Research => "Opened the research centre".to_string(),
        };
    }
}

fn load_campaign() -> Option<CampaignSave> {
    let bytes = match crate::save::read(crate::save::SaveFile::Campaign) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return None,
        Err(error) => {
            warn!("Could not read the campaign save: {error}");
            return None;
        }
    };
    let save: CampaignSave = serde_json::from_slice(&bytes).ok()?;
    // Schema 1 (0.26) only carries credits; the research and day fields default sensibly.
    (save.schema_version >= 1 && save.schema_version <= CAMPAIGN_SCHEMA).then_some(save)
}

pub fn persist_campaign(mut campus: ResMut<CampusState>) {
    if !campus.dirty {
        return;
    }
    let save = CampaignSave {
        schema_version: CAMPAIGN_SCHEMA,
        credits: campus.credits,
        research: campus.research.clone(),
        day: campus.day,
    };
    let result = serde_json::to_vec_pretty(&save)
        .map_err(|error| error.to_string())
        .and_then(|bytes| crate::save::write(crate::save::SaveFile::Campaign, &bytes));
    match result {
        Ok(()) => campus.dirty = false,
        Err(error) => warn!("Could not save campaign: {error}"),
    }
}

fn view_for_shortcut(key: KeyCode) -> Option<CampusView> {
    match key {
        KeyCode::F1 => Some(CampusView::Contracts),
        KeyCode::F2 => Some(CampusView::Missile),
        KeyCode::F3 => Some(CampusView::Research),
        KeyCode::F4 => Some(CampusView::Program),
        KeyCode::F5 => Some(CampusView::Trials),
        _ => None,
    }
}

fn enter_view(campus: &mut CampusState, view: CampusView) {
    campus.view = view;
    campus.message = match view {
        CampusView::Island => "Returned to the island",
        CampusView::Contracts => "Opened the contract office",
        CampusView::Missile => "Opened the missile workshop",
        CampusView::Program => "Opened the missile program editor",
        CampusView::Trials => "Opened the trial range",
        CampusView::Research => "Opened the research centre",
    }
    .to_string();
}

fn enter_facility(campus: &mut CampusState, facility: FacilityKind) {
    let view = match facility {
        FacilityKind::Contracts => CampusView::Contracts,
        FacilityKind::Construction => CampusView::Missile,
        FacilityKind::Research => CampusView::Research,
        FacilityKind::Programming => CampusView::Program,
    };
    enter_view(campus, view);
}

/// Accept an offer from the missile contract office. Shared by the board and the number keys.
pub(crate) fn accept_missile_offer(
    campus: &mut CampusState,
    procurement: &mut crate::procurement::Procurement,
    index: usize,
) {
    let max_active = campus.max_active_projects();
    campus.message = procurement.accept(index, campus.day, max_active, &campus.owned_capabilities());
}

/// Deliver one active missile contract and credit the reward. Refused dossiers keep the contract.
pub(crate) fn deliver_missile_contract(
    campus: &mut CampusState,
    procurement: &mut crate::procurement::Procurement,
    contract_id: Option<u32>,
    vehicle: &omega_rules::Vehicle,
    program: &omega_rules::Program,
) {
    match procurement.deliver(campus.day, contract_id, vehicle, program) {
        Ok(delivery) => {
            campus.credits = campus.credits.saturating_add(delivery.reward);
            campus.message = format!(
                "Contract #{} delivered: +{} cr ({}% dossier)",
                delivery.id, delivery.reward, delivery.score_percent
            );
            campus.mark_dirty();
        }
        Err(message) => campus.message = message,
    }
}

pub fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    buildings: Query<(&GlobalTransform, &BuildingVisual)>,
    mut campus: ResMut<CampusState>,
    missile: Res<crate::missile::MissileSession>,
    program: Res<crate::program_editor::MissileProgramSession>,
    mut procurement: ResMut<crate::procurement::Procurement>,
    workshop: Res<crate::workshop::WorkshopInput>,
    ui_buttons: Query<&UiInteraction>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        if workshop.drag_active {
            // The workshop cancels its drag this frame and keeps the board open.
            return;
        }
        if campus.view != CampusView::Island {
            campus.view = CampusView::Island;
            campus.message = "Returned to the island".to_string();
        }
        return;
    }

    match campus.view {
        CampusView::Island => {
            for key in [KeyCode::F1, KeyCode::F2, KeyCode::F3, KeyCode::F4, KeyCode::F5] {
                if keys.just_pressed(key)
                    && let Some(view) = view_for_shortcut(key)
                {
                    enter_view(&mut campus, view);
                    return;
                }
            }

            if !mouse.just_pressed(MouseButton::Left)
                || ui_buttons.iter().any(|interaction| *interaction != UiInteraction::None)
            {
                return;
            }
            let Some(cursor) = window.cursor_position() else {
                return;
            };
            let (camera, camera_transform) = *camera;
            let Ok(ray) = camera.viewport_to_world(camera_transform, cursor) else {
                return;
            };
            let origin = ray.origin;
            let direction = ray.direction.as_vec3();
            let mut closest: Option<(f32, FacilityKind)> = None;
            for (transform, building) in &buildings {
                let inverse = transform.to_matrix().inverse();
                let local_origin = inverse.transform_point3(origin);
                let local_direction = inverse.transform_vector3(direction);
                if let Some(distance) = ray_aabb(
                    local_origin,
                    local_direction,
                    -building.half_extents,
                    building.half_extents,
                ) && closest.as_ref().is_none_or(|(best, _)| distance < *best)
                {
                    closest = Some((distance, building.facility));
                }
            }
            if let Some((_, facility)) = closest {
                enter_facility(&mut campus, facility);
            }
        }
        CampusView::Contracts => {
            if keys.just_pressed(KeyCode::KeyV) {
                let selected = procurement.selected_id();
                deliver_missile_contract(
                    &mut campus,
                    &mut procurement,
                    selected,
                    &missile.vehicle,
                    &program.program,
                );
                return;
            }
            for (key, index) in [
                (KeyCode::Digit1, 0),
                (KeyCode::Digit2, 1),
                (KeyCode::Digit3, 2),
                (KeyCode::Digit4, 3),
            ] {
                if keys.just_pressed(key) {
                    accept_missile_offer(&mut campus, &mut procurement, index);
                    return;
                }
            }
        }
        CampusView::Research | CampusView::Missile | CampusView::Program | CampusView::Trials => {}
    }
}

fn ray_aabb(origin: Vec3, direction: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let mut near = f32::NEG_INFINITY;
    let mut far = f32::INFINITY;
    for axis in 0..3 {
        let o = origin[axis];
        let d = direction[axis];
        if d.abs() < 1e-6 {
            if o < min[axis] || o > max[axis] {
                return None;
            }
            continue;
        }
        let mut t1 = (min[axis] - o) / d;
        let mut t2 = (max[axis] - o) / d;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        near = near.max(t1);
        far = far.min(t2);
        if near > far {
            return None;
        }
    }
    if far < 0.0 { None } else { Some(near.max(0.0)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn campus() -> CampusState {
        CampusState {
            view: CampusView::Island,
            credits: 25_000,
            message: String::new(),
            research: ResearchState::default(),
            day: 1,
            dirty: false,
        }
    }

    #[test]
    fn every_main_view_has_a_matching_function_key() {
        assert_eq!(view_for_shortcut(KeyCode::F1), Some(CampusView::Contracts));
        assert_eq!(view_for_shortcut(KeyCode::F2), Some(CampusView::Missile));
        assert_eq!(view_for_shortcut(KeyCode::F3), Some(CampusView::Research));
        assert_eq!(view_for_shortcut(KeyCode::F4), Some(CampusView::Program));
        assert_eq!(view_for_shortcut(KeyCode::F5), Some(CampusView::Trials));
        assert_eq!(view_for_shortcut(KeyCode::F6), None);
    }

    #[test]
    fn research_costs_credits_and_advances_the_clock() {
        let mut state = campus();
        let tree = omega_rules::default_tree();
        let index = tree.iter().position(|tech| tech.id == "barometer").unwrap();
        state.ui_start_research(index);
        assert_eq!(state.credits, 25_000 - tree[index].cost_cr);
        assert!(state.research.active.is_some());
        for _ in 0..tree[index].duration_days {
            state.ui_next_day();
        }
        assert!(state.research.is_completed("barometer"));
        assert!(state.owned_capabilities().contains("sensor:barometer"));
        assert_eq!(state.day, 1 + u32::from(tree[index].duration_days));
    }

    #[test]
    fn the_project_limit_grows_with_the_parallel_projects_grant() {
        let mut state = campus();
        assert_eq!(state.max_active_projects(), 1);
        let tree = omega_rules::default_tree();
        let index = tree.iter().position(|tech| tech.id == "parallel_projects").unwrap();
        state.ui_start_research(index);
        for _ in 0..tree[index].duration_days {
            state.ui_next_day();
        }
        assert_eq!(state.max_active_projects(), 2);
    }

    #[test]
    fn ray_aabb_detects_and_rejects_hits() {
        assert_eq!(
            ray_aabb(
                Vec3::new(0.0, 0.0, 5.0),
                Vec3::NEG_Z,
                Vec3::splat(-1.0),
                Vec3::splat(1.0)
            ),
            Some(4.0)
        );
        assert_eq!(
            ray_aabb(
                Vec3::new(3.0, 0.0, 5.0),
                Vec3::NEG_Z,
                Vec3::splat(-1.0),
                Vec3::splat(1.0)
            ),
            None
        );
    }
}
