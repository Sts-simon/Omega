use crate::requirement::Estimate;
use std::collections::BTreeMap;

/// Minimal geometric/mass description of one part. The Bevy crate adapts its own `Part`
/// type into this, which keeps the rules crate independent of the design editor.
#[derive(Clone, Debug, PartialEq)]
pub struct PartGeometry {
    pub id: String,
    pub mass_kg: f64,
    pub position_m: [f64; 3],
    pub dimensions_m: [f64; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct DerivedMetrics {
    values: BTreeMap<String, Estimate>,
}

impl DerivedMetrics {
    pub fn get(&self, metric: &str) -> Option<Estimate> {
        self.values.get(metric).copied()
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }
}

/// Relative uncertainty of design-level (analysis) mass estimates.
const MASS_RELATIVE_UNCERTAINTY: f64 = 0.03;

/// Derive gameplay metrics from the actual geometry.
///
/// Limitations (deliberate, documented): each part is treated as an axis-aligned box centred on
/// its position with its mass at the centre; part rotations are ignored. Values are baseline
/// *analysis-level* estimates; use `Estimate::rescaled_for` for other verification methods.
///
/// Metrics: `mass_kg`, `part_count`, `length_m` (longest envelope axis), `cg_x_m`, `cg_y_m`,
/// `cg_z_m`, `cg_offset_m` (distance between centre of gravity and envelope centre) and
/// `cg_offset_ratio` (that distance divided by `length_m`, absent for a zero-length envelope).
pub fn derive_metrics(parts: &[PartGeometry]) -> Result<DerivedMetrics, String> {
    if parts.is_empty() {
        return Err("design has no parts".to_string());
    }
    let mut total_mass = 0.0;
    let mut moment = [0.0_f64; 3];
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for part in parts {
        let finite = part.mass_kg.is_finite()
            && part
                .position_m
                .iter()
                .chain(part.dimensions_m.iter())
                .all(|v| v.is_finite());
        if !finite || part.mass_kg < 0.0 || part.dimensions_m.iter().any(|d| *d < 0.0) {
            return Err(format!("part '{}' has invalid mass or geometry", part.id));
        }
        total_mass += part.mass_kg;
        for axis in 0..3 {
            moment[axis] += part.mass_kg * part.position_m[axis];
            min[axis] = min[axis].min(part.position_m[axis] - part.dimensions_m[axis] / 2.0);
            max[axis] = max[axis].max(part.position_m[axis] + part.dimensions_m[axis] / 2.0);
        }
    }
    if total_mass <= 0.0 {
        return Err("total mass is zero: centre of gravity is undefined".to_string());
    }
    let cg = [moment[0] / total_mass, moment[1] / total_mass, moment[2] / total_mass];
    let extent = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
    let length = extent.iter().copied().fold(0.0, f64::max);
    let centre = [
        (min[0] + max[0]) / 2.0,
        (min[1] + max[1]) / 2.0,
        (min[2] + max[2]) / 2.0,
    ];
    let offset = ((cg[0] - centre[0]).powi(2) + (cg[1] - centre[1]).powi(2) + (cg[2] - centre[2]).powi(2)).sqrt();

    let position_uncertainty = 0.005 * length + 0.002;
    let mut values = BTreeMap::new();
    let mut put = |name: &str, value: f64, unc: f64| {
        values.insert(
            name.to_string(),
            Estimate {
                value,
                uncertainty: unc,
            },
        );
    };
    put("mass_kg", total_mass, total_mass * MASS_RELATIVE_UNCERTAINTY);
    put("part_count", parts.len() as f64, 0.0);
    put("length_m", length, 0.005 * length);
    put("cg_x_m", cg[0], position_uncertainty);
    put("cg_y_m", cg[1], position_uncertainty);
    put("cg_z_m", cg[2], position_uncertainty);
    put("cg_offset_m", offset, 2.0 * position_uncertainty);
    if length > 0.0 {
        put("cg_offset_ratio", offset / length, 2.0 * position_uncertainty / length);
    }
    Ok(DerivedMetrics { values })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(id: &str, mass: f64, pos: [f64; 3], dims: [f64; 3]) -> PartGeometry {
        PartGeometry {
            id: id.into(),
            mass_kg: mass,
            position_m: pos,
            dimensions_m: dims,
        }
    }

    #[test]
    fn symmetric_design_has_centred_gravity() {
        let m = derive_metrics(&[
            part("a", 10.0, [-1.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            part("b", 10.0, [1.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
        ])
        .unwrap();
        assert_eq!(m.get("mass_kg").unwrap().value, 20.0);
        assert!((m.get("length_m").unwrap().value - 3.0).abs() < 1e-12);
        assert!(m.get("cg_x_m").unwrap().value.abs() < 1e-12);
        assert!(m.get("cg_offset_m").unwrap().value < 1e-12);
        assert_eq!(m.get("part_count").unwrap().value, 2.0);
    }

    #[test]
    fn moving_a_heavy_part_moves_the_centre_of_gravity() {
        let base = |x| {
            derive_metrics(&[
                part("light", 1.0, [0.0, 0.0, 0.0], [4.0, 0.2, 0.2]),
                part("heavy", 9.0, [x, 0.0, 0.0], [0.2, 0.2, 0.2]),
            ])
            .unwrap()
        };
        let near = base(0.0);
        let far = base(1.5);
        assert!(far.get("cg_offset_m").unwrap().value > near.get("cg_offset_m").unwrap().value + 1.0);
        // Geometry changes the evaluated metric: this is the causal link the contracts need.
        assert!(far.get("cg_x_m").unwrap().value > 1.0);
        assert!(far.get("cg_offset_ratio").unwrap().value > near.get("cg_offset_ratio").unwrap().value);
    }

    #[test]
    fn invalid_inputs_are_rejected() {
        assert!(derive_metrics(&[]).is_err());
        assert!(derive_metrics(&[part("z", 0.0, [0.0; 3], [1.0; 3])]).is_err());
        assert!(derive_metrics(&[part("n", f64::NAN, [0.0; 3], [1.0; 3])]).is_err());
        assert!(derive_metrics(&[part("neg", 1.0, [0.0; 3], [-1.0, 1.0, 1.0])]).is_err());
    }

    #[test]
    fn metrics_are_deterministic() {
        let parts = [
            part("a", 3.0, [0.1, 0.2, 0.3], [1.0, 2.0, 0.5]),
            part("b", 2.0, [1.0, 0.0, -1.0], [0.5; 3]),
        ];
        assert_eq!(derive_metrics(&parts).unwrap(), derive_metrics(&parts).unwrap());
    }
}
