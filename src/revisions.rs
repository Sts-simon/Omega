//! Design revisions: named snapshots of the missile design with a parent link, a change summary
//! against the head and a persisted log. Only missile designs are revisions now that the civil
//! assembly is gone; older logs that held assembly snapshots are migrated by dropping them.

use crate::save::{self, SaveFile};
use bevy::prelude::*;
use omega_data::hash_serializable;
use omega_rules::Vehicle;
use serde::{Deserialize, Serialize};

const REVISION_SCHEMA: u32 = 3;
const MAX_REVISIONS: usize = 100;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DesignRevision {
    pub id: u32,
    pub parent: Option<u32>,
    pub label: String,
    pub hash: String,
    pub vehicle: Vehicle,
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
        match save::read(SaveFile::Revisions) {
            Ok(Some(bytes)) => match Self::from_bytes(&bytes) {
                Ok(loaded) => {
                    info!("Loaded {} design revision(s)", loaded.len());
                    return loaded;
                }
                Err(error) => warn!("Could not read the revision log: {error}"),
            },
            Ok(None) => {}
            Err(error) => warn!("Could not read the revision log: {error}"),
        }
        Self {
            schema_version: REVISION_SCHEMA,
            revisions: Vec::new(),
            next_id: 1,
            head: None,
        }
    }
}

impl RevisionLog {
    pub fn head(&self) -> Option<&DesignRevision> {
        self.head.and_then(|id| self.revisions.iter().find(|r| r.id == id))
    }

    pub fn len(&self) -> usize {
        self.revisions.len()
    }

    /// Record the current missile design. Refuses a design identical to the head.
    pub fn commit_vehicle(&mut self, vehicle: &Vehicle) -> Result<u32, String> {
        vehicle.validate().map_err(|issues| issues.join("; "))?;
        let hash = hash_serializable(vehicle)?.as_str().to_string();
        if self.head().is_some_and(|head| head.hash == hash) {
            return Err("nothing changed since the last revision".to_string());
        }
        let id = self.next_id;
        self.next_id += 1;
        self.revisions.push(DesignRevision {
            id,
            parent: self.head,
            label: format!("Missile revision {id}"),
            hash,
            vehicle: vehicle.clone(),
        });
        self.head = Some(id);
        self.trim();
        Ok(id)
    }

    fn trim(&mut self) {
        if self.revisions.len() > MAX_REVISIONS {
            let removed = self.revisions.remove(0);
            for revision in &mut self.revisions {
                if revision.parent == Some(removed.id) {
                    revision.parent = None;
                }
            }
        }
    }

    pub fn head_vehicle(&self) -> Option<&DesignRevision> {
        self.head()
    }

    /// Missile revisions in reverse order, with their hashes.
    pub fn vehicle_revisions(&self) -> impl Iterator<Item = &DesignRevision> {
        self.revisions.iter().rev()
    }

    /// Workshop panel text: the revisions that hold a missile design.
    pub fn missile_hud_text(&self, vehicle: &Vehicle) -> String {
        let mut lines = vec!["DESIGN REVISIONS".to_string()];
        match self.head_vehicle() {
            Some(head) => {
                lines.push(format!("Head {} | {}", head.hash, head.label));
                lines.push(format!(
                    "Since head: {}",
                    crate::missile::vehicle_diff(&head.vehicle, vehicle)
                ));
            }
            None => lines.push("No missile revision recorded yet.".to_string()),
        }
        for revision in self.vehicle_revisions().take(3) {
            lines.push(format!("rev {} | {}", revision.id, revision.hash));
        }
        lines.join("\n")
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(self).map_err(|error| error.to_string())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        let schema = value
            .get("schema_version")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        if schema > u64::from(REVISION_SCHEMA) {
            return Err(format!("unsupported revision log schema {schema}"));
        }
        if schema < u64::from(REVISION_SCHEMA) {
            // Schema 1 and 2 could hold legacy assembly snapshots; those entries can no longer be
            // restored, so the log starts empty rather than failing to load.
            return Ok(Self {
                schema_version: REVISION_SCHEMA,
                revisions: Vec::new(),
                next_id: 1,
                head: None,
            });
        }
        let log: Self = serde_json::from_value(value).map_err(|error| error.to_string())?;
        if let Some(head) = log.head
            && !log.revisions.iter().any(|r| r.id == head)
        {
            return Err("revision log head points to a missing revision".to_string());
        }
        Ok(log)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_rules::{ComputerClass, Part, PartSpec, Stage};

    fn vehicle(length: f64) -> Vehicle {
        Vehicle::from_stages(
            "Revision rocket",
            vec![Stage {
                parts: vec![
                    Part {
                        id: "motor".into(),
                        spec: PartSpec::SolidMotor {
                            length_m: length,
                            diameter_m: 0.4,
                            burn_time_s: 20.0,
                        },
                    },
                    Part {
                        id: "computer".into(),
                        spec: PartSpec::Computer {
                            class: ComputerClass::Basic,
                        },
                    },
                ],
            }],
        )
    }

    #[test]
    fn commit_links_parents_and_refuses_unchanged_designs() {
        let mut log = RevisionLog {
            schema_version: REVISION_SCHEMA,
            revisions: Vec::new(),
            next_id: 1,
            head: None,
        };
        let first = log.commit_vehicle(&vehicle(2.0)).unwrap();
        assert!(log.commit_vehicle(&vehicle(2.0)).is_err());
        let second = log.commit_vehicle(&vehicle(2.2)).unwrap();
        assert_eq!((first, second), (1, 2));
        assert_eq!(log.head().unwrap().parent, Some(1));
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn log_round_trips_and_rejects_a_corrupt_head() {
        let mut log = RevisionLog {
            schema_version: REVISION_SCHEMA,
            revisions: Vec::new(),
            next_id: 1,
            head: None,
        };
        log.commit_vehicle(&vehicle(2.0)).unwrap();
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
    fn assembly_era_logs_migrate_to_an_empty_missile_log() {
        let legacy = r#"{"schema_version":2,"next_id":3,"head":2,
            "revisions":[
                {"id":1,"parent":null,"label":"Revision 1","hash":"aa","assembly":{"parts":[]}},
                {"id":2,"parent":1,"label":"Revision 2","hash":"bb","assembly":{"parts":[]}}
            ]}"#;
        let log = RevisionLog::from_bytes(legacy.as_bytes()).unwrap();
        assert_eq!(log.len(), 0);
        assert!(log.head().is_none());
        assert_eq!(log.schema_version, REVISION_SCHEMA);
    }

    #[test]
    fn oldest_revision_is_dropped_past_the_cap_and_children_are_detached() {
        let mut log = RevisionLog {
            schema_version: REVISION_SCHEMA,
            revisions: Vec::new(),
            next_id: 1,
            head: None,
        };
        for index in 0..(MAX_REVISIONS + 2) {
            log.commit_vehicle(&vehicle(2.0 + index as f64 * 0.01)).unwrap();
        }
        assert_eq!(log.len(), MAX_REVISIONS);
        assert_eq!(log.revisions[0].parent, None);
        assert!(log.revisions.iter().skip(1).all(|r| r.parent.is_some()));
    }
}
