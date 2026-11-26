//! Reusable subassemblies: a group of parts captured as a small local tree, saved beside the
//! vehicle and exported as JSON so it can be pasted into another design or another campaign.
//!
//! A subassembly stores relative attachments only: the root of each captured tree is mounted when
//! the set is inserted, and every inner edge is replayed exactly. That is what keeps the structure
//! intact through save, reload and insertion.

use omega_rules::PartSpec;
use omega_rules::assembly::{NodeId, PlacedPart, Vehicle};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Version of one exported subassembly. Independent from the vehicle schema so a library can
/// evolve without touching `designs/vehicle.json`.
pub const SUBASSEMBLY_SCHEMA: u32 = 1;
/// Exported sets live outside the save slot so they can be imported into any campaign.
pub const EXPORT_DIR: &str = "exports/subassemblies";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubassemblyKind {
    Surfaces,
    Electronics,
    Structure,
    Stage,
    Configuration,
}

impl SubassemblyKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Surfaces => "Surfaces",
            Self::Electronics => "Électronique",
            Self::Structure => "Structure",
            Self::Stage => "Étage",
            Self::Configuration => "Configuration",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::Surfaces => "SURFACES",
            Self::Electronics => "ÉLECTRONIQUE",
            Self::Structure => "STRUCTURE",
            Self::Stage => "ÉTAGE",
            Self::Configuration => "CONFIG",
        }
    }
}

fn one() -> f64 {
    1.0
}

fn schema() -> u32 {
    SUBASSEMBLY_SCHEMA
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubassemblyPart {
    pub spec: PartSpec,
    /// Index of the parent inside `Subassembly::parts`; `None` for a captured root.
    #[serde(default)]
    pub parent: Option<usize>,
    #[serde(default)]
    pub node: Option<NodeId>,
    #[serde(default)]
    pub angle_deg: f64,
    #[serde(default)]
    pub offset_x_m: f64,
    #[serde(default)]
    pub offset_y_m: f64,
    #[serde(default)]
    pub offset_z_m: f64,
    #[serde(default)]
    pub rotation_x_deg: f64,
    #[serde(default)]
    pub rotation_y_deg: f64,
    #[serde(default)]
    pub rotation_z_deg: f64,
    /// Explicit stage boundary carried by a migrated design or a decoupler-less cut.
    #[serde(default)]
    pub separation: bool,
    #[serde(default = "one")]
    pub size_scale: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Subassembly {
    #[serde(default = "schema")]
    pub schema_version: u32,
    pub name: String,
    pub kind: SubassemblyKind,
    pub parts: Vec<SubassemblyPart>,
}

impl Subassembly {
    #[cfg(test)]
    pub fn root_count(&self) -> usize {
        self.parts.iter().filter(|part| part.parent.is_none()).count()
    }

    /// Capture the selected parts and everything below them. The roots keep their relative order
    /// from the vehicle so inserting the set reproduces the same silhouette.
    pub fn capture(
        vehicle: &Vehicle,
        ids: &[String],
        name: impl Into<String>,
        kind: SubassemblyKind,
    ) -> Result<Self, String> {
        let selected: BTreeSet<&str> = ids
            .iter()
            .map(String::as_str)
            .filter(|id| vehicle.part(id).is_some())
            .collect();
        if selected.is_empty() {
            return Err("Sélectionnez au moins une pièce à enregistrer.".to_string());
        }
        let roots: Vec<&PlacedPart> = vehicle
            .parts
            .iter()
            .filter(|part| selected.contains(part.id.as_str()))
            .filter(|part| {
                part.attach
                    .as_ref()
                    .is_none_or(|attach| !selected.contains(attach.parent.as_str()))
            })
            .collect();
        let mut parts = Vec::new();
        for root in roots {
            let mut stack = vec![(root.id.clone(), None::<usize>)];
            while let Some((id, parent_index)) = stack.pop() {
                let Some(part) = vehicle.part(&id) else {
                    continue;
                };
                let index = parts.len();
                let attach = part.attach.as_ref();
                parts.push(SubassemblyPart {
                    spec: part.spec,
                    parent: parent_index,
                    node: attach.map(|attach| attach.node),
                    angle_deg: attach.map_or(0.0, |attach| attach.angle_deg),
                    offset_x_m: attach.map_or(0.0, |attach| attach.offset_x_m),
                    offset_y_m: attach.map_or(0.0, |attach| attach.offset_y_m),
                    offset_z_m: attach.map_or(0.0, |attach| attach.offset_z_m),
                    rotation_x_deg: attach.map_or(0.0, |attach| attach.rotation_x_deg),
                    rotation_y_deg: attach.map_or(0.0, |attach| attach.rotation_y_deg),
                    rotation_z_deg: attach.map_or(0.0, |attach| attach.rotation_z_deg),
                    separation: attach.is_some_and(|attach| attach.separation),
                    size_scale: part.size_scale,
                });
                let mut children: Vec<&PlacedPart> = vehicle.children(&id);
                children.sort_by(|a, b| {
                    let node = |part: &PlacedPart| part.attach.as_ref().map(|attach| attach.node);
                    node(a).cmp(&node(b)).then_with(|| a.id.cmp(&b.id))
                });
                for child in children.into_iter().rev() {
                    stack.push((child.id.clone(), Some(index)));
                }
            }
        }
        if parts.is_empty() {
            return Err("La sélection ne contient aucune pièce.".to_string());
        }
        Ok(Self {
            schema_version: SUBASSEMBLY_SCHEMA,
            name: name.into(),
            kind,
            parts,
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version > SUBASSEMBLY_SCHEMA {
            return Err(format!(
                "sous-ensemble version {} plus récent que le jeu (version {SUBASSEMBLY_SCHEMA})",
                self.schema_version
            ));
        }
        if self.parts.is_empty() {
            return Err("sous-ensemble vide".to_string());
        }
        for (index, part) in self.parts.iter().enumerate() {
            part.spec
                .validate()
                .map_err(|error| format!("pièce {} : {error}", index + 1))?;
            if part.parent.is_some_and(|parent| parent >= index) {
                return Err(format!("pièce {} : parent invalide", index + 1));
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct LibraryFile {
    #[serde(default = "schema")]
    pub schema_version: u32,
    #[serde(default)]
    pub entries: Vec<Subassembly>,
}

impl Default for LibraryFile {
    fn default() -> Self {
        Self {
            schema_version: SUBASSEMBLY_SCHEMA,
            entries: Vec::new(),
        }
    }
}

pub fn library_bytes(entries: &[Subassembly]) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(&LibraryFile {
        schema_version: SUBASSEMBLY_SCHEMA,
        entries: entries.to_vec(),
    })
    .map_err(|error| error.to_string())
}

pub fn parse_library(bytes: &[u8]) -> Result<Vec<Subassembly>, String> {
    let file: LibraryFile = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if file.schema_version > SUBASSEMBLY_SCHEMA {
        return Err(format!(
            "bibliothèque version {} plus récente que le jeu (version {SUBASSEMBLY_SCHEMA})",
            file.schema_version
        ));
    }
    for entry in &file.entries {
        entry.validate()?;
    }
    Ok(file.entries)
}

/// Filesystem-safe file name for an exported set.
pub fn slug(name: &str) -> String {
    let mut slug: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "sous-ensemble".to_string()
    } else {
        slug
    }
}

/// Write one set to `exports/subassemblies/<slug>.json`, overwriting an older export of the same
/// name. Returns the path written.
pub fn export(entry: &Subassembly) -> Result<PathBuf, String> {
    entry.validate()?;
    let directory = Path::new(EXPORT_DIR);
    std::fs::create_dir_all(directory).map_err(|error| format!("Dossier d'export : {error}"))?;
    let path = directory.join(format!("{}.json", slug(&entry.name)));
    let bytes = serde_json::to_vec_pretty(entry).map_err(|error| error.to_string())?;
    std::fs::write(&path, bytes).map_err(|error| format!("Export : {error}"))?;
    Ok(path)
}

/// Read every exported set in `exports/subassemblies`. Unreadable files are reported but do not
/// hide the valid ones.
pub fn import_all() -> Result<(Vec<Subassembly>, Vec<String>), String> {
    let directory = Path::new(EXPORT_DIR);
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!(
                "Aucun dossier d'export ({EXPORT_DIR}). Exportez d'abord un ensemble."
            ));
        }
        Err(error) => return Err(format!("Lecture des exports : {error}")),
    };
    let mut imported = Vec::new();
    let mut problems = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            problems.push(format!("{} illisible", path.display()));
            continue;
        };
        match serde_json::from_slice::<Subassembly>(&bytes) {
            Ok(set) => match set.validate() {
                Ok(()) => imported.push(set),
                Err(error) => problems.push(format!("{} : {error}", path.display())),
            },
            Err(error) => problems.push(format!("{} : {error}", path.display())),
        }
    }
    imported.sort_by(|a, b| a.name.cmp(&b.name));
    Ok((imported, problems))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_rules::Stage;

    fn rocket() -> Vehicle {
        Vehicle::from_stages(
            "Test",
            vec![
                Stage {
                    parts: vec![omega_rules::Part {
                        id: "motor".to_string(),
                        spec: PartSpec::SolidMotor {
                            length_m: 2.0,
                            diameter_m: 0.4,
                            burn_time_s: 20.0,
                        },
                    }],
                },
                Stage {
                    parts: vec![
                        omega_rules::Part {
                            id: "tank".to_string(),
                            spec: PartSpec::FuelTank {
                                length_m: 1.0,
                                diameter_m: 0.4,
                            },
                        },
                        omega_rules::Part {
                            id: "computer".to_string(),
                            spec: PartSpec::Computer {
                                class: omega_rules::ComputerClass::Basic,
                            },
                        },
                    ],
                },
            ],
        )
    }

    #[test]
    fn capture_keeps_inner_edges_and_roots() {
        let vehicle = rocket();
        let ids: Vec<String> = vehicle.parts.iter().map(|part| part.id.clone()).collect();
        let set = Subassembly::capture(&vehicle, &ids, "Pile", SubassemblyKind::Structure).unwrap();
        assert_eq!(set.parts.len(), vehicle.parts.len());
        assert_eq!(set.root_count(), 1);
        assert!(set.parts[0].parent.is_none());
        for (index, part) in set.parts.iter().enumerate().skip(1) {
            assert!(part.parent.unwrap() < index);
        }
        set.validate().unwrap();
    }

    #[test]
    fn a_captured_subtree_excludes_its_parent() {
        let vehicle = rocket();
        let set = Subassembly::capture(
            &vehicle,
            &["tank".to_string(), "computer".to_string()],
            "Étage haut",
            SubassemblyKind::Stage,
        )
        .unwrap();
        assert_eq!(set.parts.len(), 2);
        assert_eq!(set.root_count(), 1);
        assert!(set.parts.iter().all(|part| part.spec != PartSpec::Decoupler));
    }

    #[test]
    fn library_and_export_round_trip() {
        let vehicle = rocket();
        let set = Subassembly::capture(
            &vehicle,
            &["tank".to_string()],
            "Réservoir seul",
            SubassemblyKind::Configuration,
        )
        .unwrap();
        let bytes = library_bytes(std::slice::from_ref(&set)).unwrap();
        assert_eq!(parse_library(&bytes).unwrap(), vec![set]);
        assert_eq!(slug("Étage haut / test"), "tage-haut-test");
    }
}
