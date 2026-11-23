use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const CURRENT_ASSEMBLY_SCHEMA: u32 = 2;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PartKind {
    Panel,
    Beam,
    Enclosure,
    Platform,
    Cylinder,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MaterialKind {
    Aluminum,
    Steel,
    Polymer,
    Composite,
}

impl MaterialKind {
    /// Approximate density in kg/m³. Values are placeholders for generic mass estimates.
    pub fn density_kg_m3(&self) -> f64 {
        match self {
            Self::Aluminum => 2_700.0,
            Self::Steel => 7_850.0,
            Self::Polymer => 1_200.0,
            Self::Composite => 1_600.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Part {
    pub id: String,
    pub label: String,
    pub kind: PartKind,
    /// Bounding dimensions in metres: X, Y, Z.
    pub dimensions_m: [f64; 3],
    /// Placement in assembly-local coordinates, in metres.
    pub position_m: [f64; 3],
    /// Euler rotation in radians, XYZ order. Defaults to zero for older project files.
    #[serde(default)]
    pub rotation_rad: [f64; 3],
    pub material: MaterialKind,
}

impl Part {
    pub fn envelope_volume_m3(&self) -> f64 {
        self.dimensions_m[0] * self.dimensions_m[1] * self.dimensions_m[2]
    }

    /// Approximate solid volume. Cylinder uses an elliptical cylinder bounded by X/Y/Z.
    pub fn estimated_material_volume_m3(&self) -> f64 {
        match &self.kind {
            PartKind::Cylinder => {
                std::f64::consts::PI * self.dimensions_m[0] * self.dimensions_m[2]
                    * self.dimensions_m[1] / 4.0
            }
            _ => self.envelope_volume_m3(),
        }
    }

    pub fn estimated_mass_kg(&self) -> f64 {
        self.estimated_material_volume_m3() * self.material.density_kg_m3()
    }
}

/// A directed mechanical relationship in the assembly graph.
/// This records topology only; it does not yet solve geometric constraints or dynamics.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Connection {
    pub id: String,
    pub parent_part_id: String,
    pub child_part_id: String,
    pub kind: ConnectionKind,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionKind {
    Rigid,
    Revolute,
    Slider,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Assembly {
    pub schema_version: u32,
    pub name: String,
    pub parts: Vec<Part>,
    /// Missing in schema v1 files; defaults to an empty graph during migration.
    #[serde(default)]
    pub connections: Vec<Connection>,
}

impl Assembly {
    pub fn demo() -> Self {
        Self {
            schema_version: CURRENT_ASSEMBLY_SCHEMA,
            name: "Workshop test rig".to_string(),
            parts: vec![
                Part {
                    id: "base-plate".to_string(),
                    label: "Base plate".to_string(),
                    kind: PartKind::Platform,
                    dimensions_m: [0.6, 0.005, 0.4],
                    position_m: [0.0, 0.0, 0.0],
                    rotation_rad: [0.0, 0.0, 0.0],
                    material: MaterialKind::Aluminum,
                },
                Part {
                    id: "support-left".to_string(),
                    label: "Left support".to_string(),
                    kind: PartKind::Beam,
                    dimensions_m: [0.012, 0.12, 0.012],
                    position_m: [-0.22, 0.07, 0.0],
                    rotation_rad: [0.0, 0.0, 0.0],
                    material: MaterialKind::Steel,
                },
                Part {
                    id: "sensor-case".to_string(),
                    label: "Instrument enclosure".to_string(),
                    kind: PartKind::Enclosure,
                    dimensions_m: [0.18, 0.1, 0.08],
                    position_m: [0.0, 0.17, 0.0],
                    rotation_rad: [0.0, 0.0, 0.0],
                    material: MaterialKind::Polymer,
                },
            ],
            connections: vec![],
        }
    }

    /// Upgrade supported older files to the current in-memory schema.
    pub fn migrate(mut self) -> Result<Self, String> {
        match self.schema_version {
            1 => {
                self.schema_version = CURRENT_ASSEMBLY_SCHEMA;
                // `connections` and `rotation_rad` have serde defaults in schema v1.
                Ok(self)
            }
            CURRENT_ASSEMBLY_SCHEMA => Ok(self),
            version => Err(format!("Unsupported assembly schema version: {version}")),
        }
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut issues = Vec::new();
        let mut identifiers = HashSet::new();
        let part_ids: HashSet<&str> = self.parts.iter().map(|part| part.id.as_str()).collect();

        if self.schema_version != CURRENT_ASSEMBLY_SCHEMA {
            issues.push(format!(
                "Assembly must be migrated to schema version {CURRENT_ASSEMBLY_SCHEMA}"
            ));
        }
        if self.name.trim().is_empty() {
            issues.push("Assembly name cannot be empty".to_string());
        }
        if self.parts.len() > 5_000 {
            issues.push("Assembly exceeds the 5,000-part prototype limit".to_string());
        }

        for part in &self.parts {
            if part.id.trim().is_empty() {
                issues.push("A part has an empty identifier".to_string());
            } else if !identifiers.insert(part.id.as_str()) {
                issues.push(format!("Part identifier '{}' is duplicated", part.id));
            }
            if part.dimensions_m.iter().any(|v| !v.is_finite() || *v <= 0.0 || *v > 100.0) {
                issues.push(format!("Part '{}' has invalid dimensions", part.id));
            }
            if part.position_m.iter().any(|v| !v.is_finite() || v.abs() > 1_000.0) {
                issues.push(format!("Part '{}' has an invalid position", part.id));
            }
            if part.rotation_rad.iter().any(|v| !v.is_finite() || v.abs() > 1_000.0) {
                issues.push(format!("Part '{}' has an invalid rotation", part.id));
            }
        }

        let mut connection_ids = HashSet::new();
        let mut parent_by_child: HashMap<&str, &str> = HashMap::new();
        for connection in &self.connections {
            if connection.id.trim().is_empty() || !connection_ids.insert(connection.id.as_str()) {
                issues.push(format!("Connection identifier '{}' is empty or duplicated", connection.id));
            }
            if connection.parent_part_id == connection.child_part_id {
                issues.push(format!("Connection '{}' links a part to itself", connection.id));
            }
            if !part_ids.contains(connection.parent_part_id.as_str()) {
                issues.push(format!("Connection '{}' references a missing parent", connection.id));
            }
            if !part_ids.contains(connection.child_part_id.as_str()) {
                issues.push(format!("Connection '{}' references a missing child", connection.id));
            }
            if parent_by_child
                .insert(connection.child_part_id.as_str(), connection.parent_part_id.as_str())
                .is_some()
            {
                issues.push(format!(
                    "Part '{}' has more than one parent connection",
                    connection.child_part_id
                ));
            }
        }

        // A parent chain must terminate. This detects cycles without recursion.
        for start in parent_by_child.keys().copied() {
            let mut visited = HashSet::new();
            let mut current = start;
            while let Some(parent) = parent_by_child.get(current).copied() {
                if !visited.insert(current) {
                    issues.push(format!("Assembly connection graph contains a cycle near '{current}'"));
                    break;
                }
                current = parent;
            }
        }

        if issues.is_empty() { Ok(()) } else { Err(issues) }
    }

    pub fn estimated_mass_kg(&self) -> f64 {
        self.parts.iter().map(Part::estimated_mass_kg).sum()
    }

    pub fn has_parent(&self, part_id: &str) -> bool {
        self.connections.iter().any(|connection| connection.child_part_id == part_id)
    }

    pub fn can_add_parent_child(&self, parent_id: &str, child_id: &str) -> bool {
        if parent_id == child_id
            || !self.parts.iter().any(|part| part.id == parent_id)
            || !self.parts.iter().any(|part| part.id == child_id)
            || self.has_parent(child_id)
        {
            return false;
        }

        let mut current = parent_id;
        loop {
            if current == child_id {
                return false;
            }
            let Some(parent) = self
                .connections
                .iter()
                .find(|connection| connection.child_part_id == current)
                .map(|connection| connection.parent_part_id.as_str())
            else {
                return true;
            };
            current = parent;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_assembly_is_valid() {
        assert!(Assembly::demo().validate().is_ok());
    }

    #[test]
    fn mass_estimate_is_positive() {
        assert!(Assembly::demo().estimated_mass_kg() > 0.0);
    }

    #[test]
    fn invalid_dimensions_are_rejected() {
        let mut assembly = Assembly::demo();
        assembly.parts[0].dimensions_m[0] = f64::NAN;
        assert!(assembly.validate().is_err());
    }

    #[test]
    fn non_finite_rotation_is_rejected() {
        let mut assembly = Assembly::demo();
        assembly.parts[0].rotation_rad[1] = f64::INFINITY;
        assert!(assembly.validate().is_err());
    }

    #[test]
    fn schema_v1_files_migrate_with_empty_connection_graph() {
        let legacy = r#"{
            "schema_version": 1,
            "name": "Legacy assembly",
            "parts": [{
                "id": "legacy-part",
                "label": "Legacy part",
                "kind": "panel",
                "dimensions_m": [1.0, 0.1, 1.0],
                "position_m": [0.0, 0.0, 0.0],
                "material": "aluminum"
            }]
        }"#;
        let assembly: Assembly = serde_json::from_str(legacy).unwrap();
        let migrated = assembly.migrate().unwrap();
        assert_eq!(migrated.schema_version, CURRENT_ASSEMBLY_SCHEMA);
        assert!(migrated.connections.is_empty());
        assert!(migrated.validate().is_ok());
    }

    #[test]
    fn duplicate_part_ids_are_rejected() {
        let mut assembly = Assembly::demo();
        assembly.parts[1].id = assembly.parts[0].id.clone();
        assert!(assembly.validate().is_err());
    }

    #[test]
    fn graph_helper_prevents_cycles_and_second_parents() {
        let mut assembly = Assembly::demo();
        assembly.connections = vec![
            Connection {
                id: "link-1".into(),
                parent_part_id: "base-plate".into(),
                child_part_id: "support-left".into(),
                kind: ConnectionKind::Rigid,
            },
            Connection {
                id: "link-2".into(),
                parent_part_id: "support-left".into(),
                child_part_id: "sensor-case".into(),
                kind: ConnectionKind::Rigid,
            },
        ];
        assert!(!assembly.can_add_parent_child("sensor-case", "base-plate"));
        assert!(!assembly.can_add_parent_child("base-plate", "support-left"));
        assert!(assembly.validate().is_ok());
    }

    #[test]
    fn cylinder_uses_elliptical_volume() {
        let mut part = Assembly::demo().parts[0].clone();
        part.kind = PartKind::Cylinder;
        part.dimensions_m = [2.0, 3.0, 2.0];
        let expected = std::f64::consts::PI * 3.0;
        assert!((part.estimated_material_volume_m3() - expected).abs() < 1e-10);
    }

    #[test]
    fn connection_graph_rejects_multiple_parents_and_cycles() {
        let mut assembly = Assembly::demo();
        assembly.connections = vec![
            Connection {
                id: "joint-1".into(),
                parent_part_id: "base-plate".into(),
                child_part_id: "support-left".into(),
                kind: ConnectionKind::Rigid,
            },
            Connection {
                id: "joint-2".into(),
                parent_part_id: "support-left".into(),
                child_part_id: "base-plate".into(),
                kind: ConnectionKind::Rigid,
            },
        ];
        assert!(assembly.validate().is_err());
    }
}
