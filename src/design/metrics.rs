//! Adapter between the editable [`Assembly`] and the Bevy-free rules crate.
//!
//! Contracts evaluate the *actual geometry* through these metrics instead of free-form numbers.

use super::model::Assembly;
use omega_data::{hash_serializable, ContentHash};
use omega_rules::{derive_metrics, DerivedMetrics, PartGeometry};

pub fn assembly_metrics(assembly: &Assembly) -> Result<DerivedMetrics, String> {
    let parts: Vec<PartGeometry> = assembly
        .parts
        .iter()
        .map(|part| PartGeometry {
            id: part.id.clone(),
            mass_kg: part.estimated_mass_kg(),
            position_m: part.position_m,
            dimensions_m: part.dimensions_m,
        })
        .collect();
    derive_metrics(&parts)
}

/// Fingerprint of the full serialized assembly (parts and connections).
pub fn assembly_content_hash(assembly: &Assembly) -> Result<ContentHash, String> {
    hash_serializable(assembly)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_assembly_yields_metrics_consistent_with_its_mass() {
        let assembly = Assembly::demo();
        let metrics = assembly_metrics(&assembly).unwrap();
        let mass = metrics.get("mass_kg").unwrap().value;
        assert!((mass - assembly.estimated_mass_kg()).abs() < 1e-9);
        assert_eq!(metrics.get("part_count").unwrap().value, assembly.parts.len() as f64);
    }

    #[test]
    fn editing_geometry_changes_metrics_and_hash() {
        let before = Assembly::demo();
        let mut after = before.clone();
        after.parts[1].position_m[0] -= 0.5;
        let m0 = assembly_metrics(&before).unwrap().get("cg_x_m").unwrap().value;
        let m1 = assembly_metrics(&after).unwrap().get("cg_x_m").unwrap().value;
        assert!(m1 < m0);
        assert_ne!(assembly_content_hash(&before).unwrap(), assembly_content_hash(&after).unwrap());
        assert_eq!(assembly_content_hash(&before).unwrap(), assembly_content_hash(&before.clone()).unwrap());
    }
}
