//! Design revisions: named snapshots of the assembly with a parent link, a comparison against
//! the current design, and live budget figures. Persisted in the save slot.

use super::{assembly_content_hash, assembly_metrics, Assembly, DesignSession, InspectorState, PolygonSketchSession};
use crate::campus::{CampusState, CampusView};
use crate::save::{self, SaveFile};
use crate::sequence::SequenceWorkspace;
use aerolab_rules::{diff_parts, DesignDiff, PartGeometry};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

const REVISION_SCHEMA: u32 = 1;
const MAX_REVISIONS: usize = 100;
const SHOWN_REVISIONS: usize = 4;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DesignRevision {
    pub id: u32,
    pub parent: Option<u32>,
    pub label: String,
    pub hash: String,
    pub assembly: Assembly,
}

#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct RevisionLog {
    schema_version: u32,
    revisions: Vec<DesignRevision>,
    next_id: u32,
    head: Option<u32>,
}

impl Default for RevisionLog {
    fn default() -> Self {
        Self { schema_version: REVISION_SCHEMA, revisions: Vec::new(), next_id: 1, head: None }
    }
}

fn geometry(assembly: &Assembly) -> Vec<PartGeometry> {
    assembly
        .parts
        .iter()
        .map(|part| PartGeometry {
            id: part.id.clone(),
            mass_kg: part.estimated_mass_kg(),
            position_m: part.position_m,
            dimensions_m: part.dimensions_m,
        })
        .collect()
}

impl RevisionLog {
    pub fn head(&self) -> Option<&DesignRevision> {
        self.head.and_then(|id| self.revisions.iter().find(|r| r.id == id))
    }

    pub fn len(&self) -> usize {
        self.revisions.len()
    }

    /// Record the assembly as a new revision whose parent is the current head.
    /// Refuses to record a design identical to the head.
    pub fn commit(&mut self, assembly: &Assembly) -> Result<u32, String> {
        assembly.validate().map_err(|issues| issues.join("; "))?;
        let hash = assembly_content_hash(assembly)?.as_str().to_string();
        if self.head().is_some_and(|head| head.hash == hash) {
            return Err("nothing changed since the last revision".to_string());
        }
        let id = self.next_id;
        self.next_id += 1;
        self.revisions.push(DesignRevision {
            id,
            parent: self.head,
            label: format!("Revision {id}"),
            hash,
            assembly: assembly.clone(),
        });
        self.head = Some(id);
        if self.revisions.len() > MAX_REVISIONS {
            let removed = self.revisions.remove(0);
            for revision in &mut self.revisions {
                if revision.parent == Some(removed.id) {
                    revision.parent = None;
                }
            }
        }
        Ok(id)
    }

    /// Changes between the head revision and `current`, or `None` when there is no revision yet.
    pub fn diff_from_head(&self, current: &Assembly) -> Option<DesignDiff> {
        let head = self.head()?;
        Some(diff_parts(&geometry(&head.assembly), &geometry(current)))
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(self).map_err(|error| error.to_string())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let log: Self = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        if log.schema_version != REVISION_SCHEMA {
            return Err(format!("unsupported revision log schema {}", log.schema_version));
        }
        if let Some(head) = log.head {
            if !log.revisions.iter().any(|r| r.id == head) {
                return Err("revision log head points to a missing revision".to_string());
            }
        }
        Ok(log)
    }

    fn metrics_text(assembly: &Assembly) -> String {
        match assembly_metrics(assembly) {
            Ok(m) => {
                let mass = m.get("mass_kg").map_or(0.0, |e| e.value);
                let cg = m.get("cg_offset_ratio").map_or(0.0, |e| e.value * 100.0);
                format!("{mass:.2} kg, CG {cg:.1}%")
            }
            Err(_) => "no valid geometry".to_string(),
        }
    }

    /// Text block for the workshop inspector: live budget, revision list and pending changes.
    pub fn hud_text(&self, current: &Assembly) -> String {
        let mut lines = vec!["DESIGN BUDGET".to_string()];
        match assembly_metrics(current) {
            Ok(m) => {
                let mass = m.get("mass_kg").unwrap_or(aerolab_rules::Estimate { value: 0.0, uncertainty: 0.0 });
                let length = m.get("length_m").map_or(0.0, |e| e.value);
                lines.push(format!("Mass {:.2} kg (+/-{:.2}) | length {:.2} m", mass.value, mass.uncertainty, length));
                if let Some(cg) = m.get("cg_offset_ratio") {
                    lines.push(format!("CG offset {:.1}% of length (+/-{:.1})", cg.value * 100.0, cg.uncertainty * 100.0));
                }
            }
            Err(error) => lines.push(format!("Budget unavailable: {error}")),
        }
        lines.push(String::new());
        lines.push("REVISIONS (Ctrl+B: record | Ctrl+Backspace: restore latest)".to_string());
        if self.revisions.is_empty() {
            lines.push("None yet. Ctrl+B records the current design.".to_string());
        } else {
            for revision in self.revisions.iter().rev().take(SHOWN_REVISIONS) {
                let marker = if Some(revision.id) == self.head { ">" } else { " " };
                let parent = revision.parent.map_or(String::new(), |p| format!(" (from {p})"));
                lines.push(format!("{marker} rev {}{} | {}", revision.id, parent, Self::metrics_text(&revision.assembly)));
            }
            if let (Some(head), Some(diff)) = (self.head(), self.diff_from_head(current)) {
                lines.push(format!("Since rev {}: {}", head.id, diff.summary()));
            }
        }
        lines.join("\n")
    }
}

pub(crate) fn initialize_revision_log(mut log: ResMut<RevisionLog>) {
    match save::read(SaveFile::Revisions) {
        Ok(Some(bytes)) => match RevisionLog::from_bytes(&bytes) {
            Ok(loaded) => {
                info!("Loaded {} design revision(s)", loaded.len());
                *log = loaded;
            }
            Err(error) => warn!("Could not read the revision log: {error}"),
        },
        Ok(None) => {}
        Err(error) => warn!("Could not read the revision log: {error}"),
    }
}

/// Record the current design as a revision and persist the log. Returns a status message.
pub(crate) fn record_revision(log: &mut RevisionLog, session: &DesignSession) -> String {
    match log.commit(&session.assembly) {
        Ok(id) => match log.to_bytes().and_then(|bytes| save::write(SaveFile::Revisions, &bytes)) {
            Ok(()) => {
                info!("Recorded design revision {id}");
                format!("Recorded revision {id}")
            }
            Err(error) => {
                error!("Revision {id} recorded but not saved: {error}");
                format!("Revision {id} recorded but could not be saved: {error}")
            }
        },
        Err(error) => {
            warn!("Revision not recorded: {error}");
            format!("Revision not recorded: {error}")
        }
    }
}

/// Replace the working design with the latest revision. Returns a status message.
pub(crate) fn restore_revision(log: &RevisionLog, session: &mut DesignSession) -> String {
    match log.head() {
        Some(head) => {
            session.replace_assembly(head.assembly.clone());
            info!("Restored design revision {}", head.id);
            format!("Restored revision {}", head.id)
        }
        None => "No revision to restore yet".to_string(),
    }
}

pub(crate) fn handle_revision_input(
    keys: Res<ButtonInput<KeyCode>>,
    campus: Res<CampusState>,
    sequence: Res<SequenceWorkspace>,
    sketch: Res<PolygonSketchSession>,
    inspector: Res<InspectorState>,
    mut session: ResMut<DesignSession>,
    mut log: ResMut<RevisionLog>,
) {
    if campus.view != CampusView::Construction || campus.profile_editor_open || sequence.graph_is_open() || sketch.active || inspector.is_editing() {
        return;
    }
    let control = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !control {
        return;
    }
    if keys.just_pressed(KeyCode::KeyB) {
        record_revision(&mut log, &session);
    }
    if keys.just_pressed(KeyCode::Backspace) {
        restore_revision(&log, &mut session);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_links_parents_and_refuses_unchanged_designs() {
        let mut log = RevisionLog::default();
        let mut assembly = Assembly::demo();
        let first = log.commit(&assembly).unwrap();
        assert!(log.commit(&assembly).is_err(), "identical design must not create a revision");
        assembly.parts[1].position_m[0] += 0.1;
        let second = log.commit(&assembly).unwrap();
        assert_eq!((first, second), (1, 2));
        assert_eq!(log.head().unwrap().parent, Some(1));
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn diff_from_head_reflects_pending_edits() {
        let mut log = RevisionLog::default();
        let mut assembly = Assembly::demo();
        assert!(log.diff_from_head(&assembly).is_none());
        log.commit(&assembly).unwrap();
        assert!(log.diff_from_head(&assembly).unwrap().is_empty());
        assembly.parts[1].position_m[0] -= 0.3;
        let diff = log.diff_from_head(&assembly).unwrap();
        assert_eq!(diff.moved.len(), 1);
        assert!(log.hud_text(&assembly).contains("1 moved"));
    }

    #[test]
    fn log_round_trips_and_rejects_a_corrupt_head() {
        let mut log = RevisionLog::default();
        log.commit(&Assembly::demo()).unwrap();
        let bytes = log.to_bytes().unwrap();
        assert_eq!(RevisionLog::from_bytes(&bytes).unwrap(), log);
        let mut broken = log.clone();
        broken.head = Some(99);
        assert!(RevisionLog::from_bytes(&broken.to_bytes().unwrap()).is_err());
        let mut future = log;
        future.schema_version = 99;
        assert!(RevisionLog::from_bytes(&future.to_bytes().unwrap()).is_err());
    }

    #[test]
    fn oldest_revision_is_dropped_past_the_cap_and_children_are_detached() {
        let mut log = RevisionLog::default();
        let mut assembly = Assembly::demo();
        for _ in 0..(MAX_REVISIONS + 2) {
            assembly.parts[0].position_m[0] += 0.01;
            log.commit(&assembly).unwrap();
        }
        assert_eq!(log.len(), MAX_REVISIONS);
        assert_eq!(log.revisions[0].parent, None);
        assert!(log.revisions.iter().skip(1).all(|r| r.parent.is_some()));
    }

    #[test]
    fn hud_text_shows_budget_and_empty_state() {
        let log = RevisionLog::default();
        let text = log.hud_text(&Assembly::demo());
        assert!(text.contains("DESIGN BUDGET") && text.contains("Mass 5.10 kg"), "{text}");
        assert!(text.contains("None yet"));
    }
}
