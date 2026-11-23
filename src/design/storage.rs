use super::{Assembly, Connection, ConnectionKind};
use bevy::prelude::{EulerRot, Quat, Resource, Vec3};
use std::collections::VecDeque;
use std::fs;
use std::path::Path;

const HISTORY_LIMIT: usize = 64;

#[derive(Resource)]
pub struct DesignSession {
    pub assembly: Assembly,
    pub selected_part_id: Option<String>,
    pub revision: u64,
    pub snap_grid: bool,
    history: VecDeque<Assembly>,
    redo: Vec<Assembly>,
    next_part_number: u64,
    gizmo_edit_active: bool,
}

impl Default for DesignSession {
    fn default() -> Self {
        let assembly = Assembly::demo();
        let selected_part_id = assembly.parts.first().map(|part| part.id.clone());
        Self {
            assembly,
            selected_part_id,
            revision: 1,
            snap_grid: false,
            history: VecDeque::new(),
            redo: Vec::new(),
            next_part_number: 1,
            gizmo_edit_active: false,
        }
    }
}

impl DesignSession {
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_part_id.as_ref().and_then(|id| {
            self.assembly.parts.iter().position(|part| &part.id == id)
        })
    }

    pub fn select_next(&mut self, reverse: bool) {
        if self.assembly.parts.is_empty() {
            self.selected_part_id = None;
            return;
        }

        let current = self.selected_index().unwrap_or(0);
        let next = if reverse {
            if current == 0 { self.assembly.parts.len() - 1 } else { current - 1 }
        } else {
            (current + 1) % self.assembly.parts.len()
        };
        self.selected_part_id = Some(self.assembly.parts[next].id.clone());
        self.touch();
    }

    pub fn add_part(&mut self, kind: super::PartKind) {
        use super::{MaterialKind, Part};

        let number = loop {
            let candidate = self.next_part_number;
            self.next_part_number += 1;
            let candidate_id = format!("part-{candidate:04}");
            if !self.assembly.parts.iter().any(|part| part.id == candidate_id) {
                break candidate;
            }
        };
        let (label, dimensions) = match &kind {
            super::PartKind::Panel => ("Panel", [0.8, 0.005, 0.5]),
            super::PartKind::Beam => ("Beam", [0.015, 0.12, 0.015]),
            super::PartKind::Enclosure => ("Enclosure", [0.18, 0.1, 0.08]),
            super::PartKind::Platform => ("Platform", [0.7, 0.006, 0.5]),
            super::PartKind::Cylinder => ("Cylinder", [0.1, 0.15, 0.1]),
        };

        self.checkpoint();
        let slot = self.assembly.parts.len().saturating_sub(3);
        let column = (slot % 3) as f64;
        let row = (slot / 3) as f64;
        let part = Part {
            id: format!("part-{number:04}"),
            label: format!("{label} {number}"),
            kind,
            dimensions_m: dimensions,
            position_m: [1.5 + column * 1.1, 0.8, -1.0 + row * 1.1],
            rotation_rad: [0.0, 0.0, 0.0],
            material: MaterialKind::Aluminum,
        };
        self.selected_part_id = Some(part.id.clone());
        self.assembly.parts.push(part);
        self.touch();
    }

    pub fn remove_selected(&mut self) {
        let Some(index) = self.selected_index() else { return };
        self.checkpoint();
        let removed_id = self.assembly.parts[index].id.clone();
        self.assembly.parts.remove(index);
        self.assembly.connections.retain(|connection| {
            connection.parent_part_id != removed_id && connection.child_part_id != removed_id
        });
        self.selected_part_id = self
            .assembly
            .parts
            .get(index.min(self.assembly.parts.len().saturating_sub(1)))
            .map(|part| part.id.clone());
        self.touch();
    }

    pub fn move_selected(&mut self, delta: [f64; 3]) {
        let Some(index) = self.selected_index() else { return };
        if delta.iter().all(|value| *value == 0.0) { return; }
        self.checkpoint();
        let snap_grid = self.snap_grid;
        let part = &mut self.assembly.parts[index];
        for (position, amount) in part.position_m.iter_mut().zip(delta) {
            let moved = (*position + amount).clamp(-100.0, 100.0);
            *position = if snap_grid { (moved / 0.25).round() * 0.25 } else { moved };
        }
        self.touch();
    }

    pub fn resize_selected(&mut self, factor: f64) {
        let Some(index) = self.selected_index() else { return };
        if !factor.is_finite() || factor <= 0.0 || (factor - 1.0).abs() < f64::EPSILON { return; }
        self.checkpoint();
        let part = &mut self.assembly.parts[index];
        for dimension in &mut part.dimensions_m {
            *dimension = (*dimension * factor).clamp(0.002, 100.0);
        }
        self.touch();
    }

    /// Adjusts one dimension by an absolute SI increment, useful for fine numeric edits.
    pub fn adjust_selected_dimension(&mut self, axis: usize, delta_m: f64) {
        if axis >= 3 || !delta_m.is_finite() || delta_m == 0.0 { return; }
        let Some(index) = self.selected_index() else { return };
        self.checkpoint();
        let dimension = &mut self.assembly.parts[index].dimensions_m[axis];
        *dimension = (*dimension + delta_m).clamp(0.002, 100.0);
        self.touch();
    }

    /// Sets an absolute assembly-local position from the numeric inspector.
    /// Explicit numeric edits bypass grid snapping so typed values remain exact.
    pub fn set_selected_position_axis(&mut self, axis: usize, value_m: f64) -> bool {
        if axis >= 3 || !value_m.is_finite() { return false; }
        let Some(index) = self.selected_index() else { return false };
        let value_m = value_m.clamp(-100.0, 100.0);
        if (self.assembly.parts[index].position_m[axis] - value_m).abs() < 1.0e-12 {
            return false;
        }
        self.checkpoint();
        self.assembly.parts[index].position_m[axis] = value_m;
        self.touch();
        true
    }

    /// Sets an absolute Euler rotation in degrees, normalizing it to [-180, 180).
    pub fn set_selected_rotation_axis_degrees(&mut self, axis: usize, value_degrees: f64) -> bool {
        if axis >= 3 || !value_degrees.is_finite() { return false; }
        let Some(index) = self.selected_index() else { return false };
        let bounded_degrees = value_degrees.clamp(-3600.0, 3600.0);
        let radians = (bounded_degrees.to_radians() + std::f64::consts::PI)
            .rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
        if (self.assembly.parts[index].rotation_rad[axis] - radians).abs() < 1.0e-12 {
            return false;
        }
        self.checkpoint();
        self.assembly.parts[index].rotation_rad[axis] = radians;
        self.touch();
        true
    }

    /// Sets an absolute component dimension in metres.
    pub fn set_selected_dimension_axis(&mut self, axis: usize, value_m: f64) -> bool {
        if axis >= 3 || !value_m.is_finite() { return false; }
        let Some(index) = self.selected_index() else { return false };
        let value_m = value_m.clamp(0.002, 100.0);
        if (self.assembly.parts[index].dimensions_m[axis] - value_m).abs() < 1.0e-12 {
            return false;
        }
        self.checkpoint();
        self.assembly.parts[index].dimensions_m[axis] = value_m;
        self.touch();
        true
    }

    pub fn rotate_selected(&mut self, delta_rad: [f64; 3]) {
        let Some(index) = self.selected_index() else { return };
        if delta_rad.iter().all(|value| *value == 0.0) { return; }
        self.checkpoint();
        let part = &mut self.assembly.parts[index];
        for (rotation, amount) in part.rotation_rad.iter_mut().zip(delta_rad) {
            let mut updated = *rotation + amount;
            while updated > std::f64::consts::PI { updated -= std::f64::consts::TAU; }
            while updated < -std::f64::consts::PI { updated += std::f64::consts::TAU; }
            *rotation = updated;
        }
        self.touch();
    }

    /// Ends the current mouse-driven transform transaction. Its first actual change
    /// creates exactly one undo snapshot, regardless of how many drag frames follow.
    pub fn finish_gizmo_edit(&mut self) {
        self.gizmo_edit_active = false;
    }

    /// Sets an absolute assembly-local position during a mouse-driven gizmo drag.
    /// Absolute updates keep grid snapping stable instead of accumulating rounded deltas.
    pub fn set_selected_gizmo_position(&mut self, mut target_m: [f64; 3]) -> bool {
        if target_m.iter().any(|value| !value.is_finite()) { return false; }
        let Some(index) = self.selected_index() else { return false };
        for value in &mut target_m {
            *value = value.clamp(-100.0, 100.0);
            if self.snap_grid { *value = (*value / 0.25).round() * 0.25; }
        }
        if self.assembly.parts[index].position_m.iter().zip(target_m).all(|(a, b)| (a - b).abs() < 1.0e-9) {
            return false;
        }
        if !self.gizmo_edit_active {
            self.checkpoint();
            self.gizmo_edit_active = true;
        }
        self.assembly.parts[index].position_m = target_m;
        self.touch();
        true
    }

    /// Sets a local-axis rotation from the transform at mouse-down and the total drag angle.
    /// This avoids accumulating floating-point drift over many pointer events.
    pub fn set_selected_gizmo_rotation(&mut self, axis: usize, base_rotation_rad: [f64; 3], delta_rad: f64) -> bool {
        if axis >= 3 || !delta_rad.is_finite() || base_rotation_rad.iter().any(|value| !value.is_finite()) {
            return false;
        }
        let Some(index) = self.selected_index() else { return false };
        let base = Quat::from_euler(
            EulerRot::XYZ,
            base_rotation_rad[0] as f32,
            base_rotation_rad[1] as f32,
            base_rotation_rad[2] as f32,
        );
        let local_axis = match axis { 0 => Vec3::X, 1 => Vec3::Y, _ => Vec3::Z };
        let updated = base * Quat::from_axis_angle(local_axis, delta_rad as f32);
        let (x, y, z) = updated.to_euler(EulerRot::XYZ);
        let next = [x as f64, y as f64, z as f64];
        if self.assembly.parts[index].rotation_rad.iter().zip(next).all(|(a, b)| (a - b).abs() < 1.0e-8) {
            return false;
        }
        if !self.gizmo_edit_active {
            self.checkpoint();
            self.gizmo_edit_active = true;
        }
        self.assembly.parts[index].rotation_rad = next;
        self.touch();
        true
    }

    pub fn resize_selected_axis(&mut self, axis: usize, factor: f64) {
        if axis >= 3 || !factor.is_finite() || factor <= 0.0 || (factor - 1.0).abs() < f64::EPSILON {
            return;
        }
        let Some(index) = self.selected_index() else { return };
        self.checkpoint();
        let part = &mut self.assembly.parts[index];
        part.dimensions_m[axis] = (part.dimensions_m[axis] * factor).clamp(0.002, 100.0);
        self.touch();
    }

    /// Adds a generic parent-child relationship to the nearest eligible part.
    /// This is assembly topology only; geometry snapping and physical joints are future work.
    pub fn connect_selected_to_nearest(&mut self) -> Result<String, String> {
        let Some(parent_index) = self.selected_index() else {
            return Err("Select a parent component first".to_string());
        };
        let parent = &self.assembly.parts[parent_index];
        let parent_id = parent.id.clone();
        let parent_position = parent.position_m;

        let nearest = self.assembly.parts.iter()
            .filter(|part| part.id != parent_id)
            .filter(|part| self.assembly.can_add_parent_child(&parent_id, &part.id))
            .min_by(|a, b| {
                distance_squared(parent_position, a.position_m)
                    .total_cmp(&distance_squared(parent_position, b.position_m))
            })
            .map(|part| part.id.clone())
            .ok_or_else(|| "No eligible unconnected component was found".to_string())?;

        let mut next_number = 1_u64;
        let connection_id = loop {
            let candidate = format!("link-{next_number:04}");
            if !self.assembly.connections.iter().any(|connection| connection.id == candidate) {
                break candidate;
            }
            next_number += 1;
        };

        self.checkpoint();
        self.assembly.connections.push(Connection {
            id: connection_id.clone(),
            parent_part_id: parent_id,
            child_part_id: nearest.clone(),
            kind: ConnectionKind::Rigid,
        });
        self.selected_part_id = Some(nearest);
        self.touch();
        Ok(connection_id)
    }

    /// Removes every graph edge touching the selected component.
    pub fn disconnect_selected(&mut self) -> usize {
        let Some(selected_id) = self.selected_part_id.clone() else { return 0 };
        let before = self.assembly.connections.len();
        let remaining: Vec<_> = self.assembly.connections.iter().filter(|connection| {
            connection.parent_part_id != selected_id && connection.child_part_id != selected_id
        }).cloned().collect();
        let removed = before - remaining.len();
        if removed == 0 { return 0; }
        self.checkpoint();
        self.assembly.connections = remaining;
        self.touch();
        removed
    }

    pub fn toggle_snap_grid(&mut self) {
        self.snap_grid = !self.snap_grid;
    }

    pub fn cycle_selected_material(&mut self) {
        let Some(index) = self.selected_index() else { return };
        self.checkpoint();
        let part = &mut self.assembly.parts[index];
        part.material = match &part.material {
            super::MaterialKind::Aluminum => super::MaterialKind::Steel,
            super::MaterialKind::Steel => super::MaterialKind::Polymer,
            super::MaterialKind::Polymer => super::MaterialKind::Composite,
            super::MaterialKind::Composite => super::MaterialKind::Aluminum,
        };
        self.touch();
    }

    pub fn can_undo(&self) -> bool { !self.history.is_empty() }
    pub fn can_redo(&self) -> bool { !self.redo.is_empty() }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.history.pop_back() else { return false };
        self.redo.push(self.assembly.clone());
        self.assembly = previous;
        self.ensure_selection();
        self.touch();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else { return false };
        self.push_history_snapshot();
        self.assembly = next;
        self.ensure_selection();
        self.touch();
        true
    }

    pub fn history_depth(&self) -> usize { self.history.len() }

    pub fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn replace_assembly(&mut self, assembly: Assembly) {
        self.selected_part_id = assembly.parts.first().map(|part| part.id.clone());
        self.assembly = assembly;
        self.history.clear();
        self.redo.clear();
        self.next_part_number = self.assembly.parts.len() as u64 + 1;
        self.gizmo_edit_active = false;
        self.ensure_unique_next_part_number();
        self.touch();
    }

    fn checkpoint(&mut self) {
        if self.history.back().is_none_or(|snapshot| snapshot != &self.assembly) {
            self.push_history_snapshot();
        }
        self.redo.clear();
    }

    fn push_history_snapshot(&mut self) {
        self.history.push_back(self.assembly.clone());
        while self.history.len() > HISTORY_LIMIT {
            self.history.pop_front();
        }
    }

    fn ensure_selection(&mut self) {
        if self.selected_part_id.as_ref().is_none_or(|id| {
            !self.assembly.parts.iter().any(|part| &part.id == id)
        }) {
            self.selected_part_id = self.assembly.parts.first().map(|part| part.id.clone());
        }
    }

    fn ensure_unique_next_part_number(&mut self) {
        while self.assembly.parts.iter().any(|part| part.id == format!("part-{:04}", self.next_part_number)) {
            self.next_part_number += 1;
        }
    }
}

fn distance_squared(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

impl Assembly {
    /// Validated JSON form used by every save path.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate().map_err(|issues| issues.join("; "))?;
        serde_json::to_vec_pretty(self).map_err(|error| error.to_string())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let assembly: Assembly = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        let assembly = assembly.migrate()?;
        assembly.validate().map_err(|issues| issues.join("; "))?;
        Ok(assembly)
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let json = self.to_bytes()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let temporary_path = path.with_extension("json.tmp");
        fs::write(&temporary_path, json).map_err(|error| error.to_string())?;
        fs::rename(&temporary_path, path).map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn load_from(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|error| error.to_string())?;
        Self::from_bytes(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::PartKind;

    #[test]
    fn selecting_and_adding_parts_updates_revision() {
        let mut session = DesignSession::default();
        let initial_revision = session.revision;
        session.add_part(PartKind::Panel);
        assert!(session.revision > initial_revision);
        assert_eq!(session.assembly.parts.len(), 4);
        assert_eq!(session.selected_index(), Some(3));
    }

    #[test]
    fn gizmo_drag_is_one_undoable_transaction() {
        let mut session = DesignSession::default();
        let index = session.selected_index().unwrap();
        let initial = session.assembly.parts[index].position_m;

        assert!(session.set_selected_gizmo_position([initial[0] + 0.1, initial[1], initial[2]]));
        assert!(session.set_selected_gizmo_position([initial[0] + 0.3, initial[1], initial[2]]));
        session.finish_gizmo_edit();

        let moved = session.selected_index().unwrap();
        assert!((session.assembly.parts[moved].position_m[0] - initial[0] - 0.3).abs() < 1.0e-9);
        assert_eq!(session.history_depth(), 1);
        assert!(session.undo());
        let restored = session.selected_index().unwrap();
        assert_eq!(session.assembly.parts[restored].position_m, initial);
    }

    #[test]
    fn gizmo_grid_snap_uses_absolute_position_targets() {
        let mut session = DesignSession::default();
        session.snap_grid = true;
        let initial = session.assembly.parts[0].position_m;
        assert!(!session.set_selected_gizmo_position([0.12, initial[1], initial[2]]));
        assert!(session.set_selected_gizmo_position([0.14, initial[1], initial[2]]));
        assert!((session.assembly.parts[0].position_m[0] - 0.25).abs() < 1.0e-9);
        assert!(!session.set_selected_gizmo_position([0.18, initial[1], initial[2]]));
        assert!((session.assembly.parts[0].position_m[0] - 0.25).abs() < 1.0e-9);
        session.finish_gizmo_edit();
    }

    #[test]
    fn gizmo_rotation_updates_local_orientation() {
        let mut session = DesignSession::default();
        let index_before = session.selected_index().unwrap();
        let base = session.assembly.parts[index_before].rotation_rad;
        assert!(session.set_selected_gizmo_rotation(1, base, 0.25));
        session.finish_gizmo_edit();
        let index = session.selected_index().unwrap();
        assert!((session.assembly.parts[index].rotation_rad[1] - 0.25).abs() < 1.0e-5);
    }

    #[test]
    fn rotation_updates_selected_part() {
        let mut session = DesignSession::default();
        session.rotate_selected([0.0, 0.25, 0.0]);
        let index = session.selected_index().unwrap();
        assert!((session.assembly.parts[index].rotation_rad[1] - 0.25).abs() < 1e-9);
    }

    #[test]
    fn snap_grid_rounds_moves_to_quarter_metres() {
        let mut session = DesignSession::default();
        session.snap_grid = true;
        session.move_selected([0.13, 0.0, 0.0]);
        let index = session.selected_index().unwrap();
        assert!((session.assembly.parts[index].position_m[0] - 0.25).abs() < 1e-9);
    }

    #[test]
    fn removing_last_part_leaves_a_valid_selection() {
        let mut session = DesignSession::default();
        session.remove_selected();
        assert_eq!(session.assembly.parts.len(), 2);
        assert!(session.selected_index().is_some());
    }

    #[test]
    fn undo_and_redo_restore_assembly_snapshots() {
        let mut session = DesignSession::default();
        let initial_mass = session.assembly.estimated_mass_kg();
        session.add_part(PartKind::Panel);
        let changed_mass = session.assembly.estimated_mass_kg();
        assert!(changed_mass > initial_mass);
        assert!(session.undo());
        assert_eq!(session.assembly.parts.len(), 3);
        assert!((session.assembly.estimated_mass_kg() - initial_mass).abs() < 1e-9);
        assert!(session.redo());
        assert_eq!(session.assembly.parts.len(), 4);
        assert!((session.assembly.estimated_mass_kg() - changed_mass).abs() < 1e-9);
    }

    #[test]
    fn connecting_selected_part_creates_valid_parent_child_graph() {
        let mut session = DesignSession::default();
        let id = session.connect_selected_to_nearest().unwrap();
        assert_eq!(id, "link-0001");
        assert_eq!(session.assembly.connections.len(), 1);
        assert!(session.assembly.validate().is_ok());
        // base-plate is selected by default; sensor-case (0.17 m away) is nearer than support-left (~0.23 m).
        assert_eq!(session.selected_part_id.as_deref(), Some("sensor-case"));
    }

    #[test]
    fn removing_a_part_also_removes_its_connections_and_is_undoable() {
        let mut session = DesignSession::default();
        session.connect_selected_to_nearest().unwrap();
        session.remove_selected();
        assert!(session.assembly.connections.is_empty());
        assert_eq!(session.assembly.parts.len(), 2);
        assert!(session.undo());
        assert_eq!(session.assembly.connections.len(), 1);
        assert_eq!(session.assembly.parts.len(), 3);
    }

    #[test]
    fn numeric_inspector_edits_are_single_undoable_snapshots() {
        let mut session = DesignSession::default();
        let original = session.assembly.parts[0].position_m[0];
        assert!(session.set_selected_position_axis(0, 1.234));
        assert_eq!(session.history_depth(), 1);
        assert!(session.undo());
        assert!((session.assembly.parts[0].position_m[0] - original).abs() < 1.0e-12);
    }

    #[test]
    fn numeric_rotation_is_entered_in_degrees_and_normalized() {
        let mut session = DesignSession::default();
        assert!(session.set_selected_rotation_axis_degrees(2, 270.0));
        let degrees = session.assembly.parts[0].rotation_rad[2].to_degrees();
        assert!((degrees + 90.0).abs() < 1.0e-10);
    }

    #[test]
    fn numeric_dimensions_respect_minimum_and_maximum() {
        let mut session = DesignSession::default();
        assert!(session.set_selected_dimension_axis(0, 0.0001));
        assert!((session.assembly.parts[0].dimensions_m[0] - 0.002).abs() < 1.0e-12);
        assert!(session.set_selected_dimension_axis(0, 500.0));
        assert!((session.assembly.parts[0].dimensions_m[0] - 100.0).abs() < 1.0e-12);
    }
}
