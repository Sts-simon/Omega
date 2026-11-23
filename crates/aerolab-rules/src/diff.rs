use crate::metrics::{derive_metrics, PartGeometry};

/// Difference between two versions of a design, by part id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DesignDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    /// Part id and distance moved, in metres.
    pub moved: Vec<(String, f64)>,
    pub resized: Vec<String>,
    pub mass_delta_kg: f64,
    /// Change of `cg_offset_ratio` (fraction of length), when both designs have a defined value.
    pub cg_offset_ratio_delta: Option<f64>,
}

const EPS: f64 = 1e-6;

impl DesignDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.moved.is_empty()
            && self.resized.is_empty()
            && self.mass_delta_kg.abs() < EPS
    }

    /// One-line summary such as `1 added, 2 moved (max 0.20 m), mass +0.40 kg, CG offset +2.1 pts`.
    pub fn summary(&self) -> String {
        if self.is_empty() {
            return "no changes".to_string();
        }
        let mut parts = Vec::new();
        if !self.added.is_empty() {
            parts.push(format!("{} added", self.added.len()));
        }
        if !self.removed.is_empty() {
            parts.push(format!("{} removed", self.removed.len()));
        }
        if !self.moved.is_empty() {
            let max = self.moved.iter().map(|(_, d)| *d).fold(0.0, f64::max);
            parts.push(format!("{} moved (max {:.2} m)", self.moved.len(), max));
        }
        if !self.resized.is_empty() {
            parts.push(format!("{} resized", self.resized.len()));
        }
        if self.mass_delta_kg.abs() >= 0.005 {
            parts.push(format!("mass {:+.2} kg", self.mass_delta_kg));
        }
        if let Some(delta) = self.cg_offset_ratio_delta {
            if delta.abs() >= 0.0005 {
                parts.push(format!("CG offset {:+.1} pts", delta * 100.0));
            }
        }
        parts.join(", ")
    }
}

fn total_mass(parts: &[PartGeometry]) -> f64 {
    parts.iter().map(|p| p.mass_kg).sum()
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Compare `before` with `after`. Parts are matched by id; duplicate ids are not supported
/// (the first occurrence wins), which matches how assemblies validate their part ids.
pub fn diff_parts(before: &[PartGeometry], after: &[PartGeometry]) -> DesignDiff {
    let mut diff = DesignDiff {
        mass_delta_kg: total_mass(after) - total_mass(before),
        ..DesignDiff::default()
    };
    for part in after {
        match before.iter().find(|b| b.id == part.id) {
            None => diff.added.push(part.id.clone()),
            Some(old) => {
                let moved = distance(old.position_m, part.position_m);
                if moved > EPS {
                    diff.moved.push((part.id.clone(), moved));
                }
                if old.dimensions_m.iter().zip(&part.dimensions_m).any(|(a, b)| (a - b).abs() > EPS) {
                    diff.resized.push(part.id.clone());
                }
            }
        }
    }
    for old in before {
        if !after.iter().any(|a| a.id == old.id) {
            diff.removed.push(old.id.clone());
        }
    }
    if let (Ok(a), Ok(b)) = (derive_metrics(before), derive_metrics(after)) {
        if let (Some(x), Some(y)) = (a.get("cg_offset_ratio"), b.get("cg_offset_ratio")) {
            diff.cg_offset_ratio_delta = Some(y.value - x.value);
        }
    }
    diff
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(id: &str, mass: f64, x: f64, size: f64) -> PartGeometry {
        PartGeometry { id: id.into(), mass_kg: mass, position_m: [x, 0.0, 0.0], dimensions_m: [size, 0.1, 0.1] }
    }

    #[test]
    fn identical_designs_have_an_empty_diff() {
        let design = [part("a", 1.0, 0.0, 1.0), part("b", 2.0, 1.0, 0.5)];
        let diff = diff_parts(&design, &design);
        assert!(diff.is_empty());
        assert_eq!(diff.summary(), "no changes");
        assert_eq!(diff.cg_offset_ratio_delta, Some(0.0));
    }

    #[test]
    fn additions_removals_moves_and_resizes_are_detected() {
        let before = [part("a", 1.0, 0.0, 1.0), part("b", 2.0, 1.0, 0.5), part("gone", 1.0, -1.0, 0.2)];
        let after = [part("a", 1.0, 0.0, 1.5), part("b", 2.0, 1.2, 0.5), part("new", 0.5, 2.0, 0.2)];
        let diff = diff_parts(&before, &after);
        assert_eq!(diff.added, vec!["new".to_string()]);
        assert_eq!(diff.removed, vec!["gone".to_string()]);
        assert_eq!(diff.resized, vec!["a".to_string()]);
        assert_eq!(diff.moved.len(), 1);
        assert!((diff.moved[0].1 - 0.2).abs() < 1e-9);
        assert!((diff.mass_delta_kg - (-0.5)).abs() < 1e-9);
        let text = diff.summary();
        assert!(text.contains("1 added") && text.contains("1 removed") && text.contains("1 moved") && text.contains("1 resized"), "{text}");
        assert!(text.contains("mass -0.50 kg"), "{text}");
    }

    #[test]
    fn moving_the_heavy_part_reports_a_balance_change() {
        let before = [part("light", 1.0, 0.0, 4.0), part("heavy", 9.0, 0.0, 0.2)];
        let after = [part("light", 1.0, 0.0, 4.0), part("heavy", 9.0, 1.5, 0.2)];
        let diff = diff_parts(&before, &after);
        assert!(diff.cg_offset_ratio_delta.unwrap() > 0.2);
        assert!(diff.summary().contains("CG offset +"));
    }

    #[test]
    fn empty_designs_do_not_panic() {
        let diff = diff_parts(&[], &[part("a", 1.0, 0.0, 1.0)]);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.cg_offset_ratio_delta, None);
        assert!(diff_parts(&[], &[]).is_empty());
    }
}
