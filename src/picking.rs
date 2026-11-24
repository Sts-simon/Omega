//! Small, testable geometry used by the workshop pointer: a camera ray against upright cylinders
//! (one part is a vertical box of revolution) and the screen-space snap test for attachment nodes.
//!
//! Kept apart from Bevy so the picking rules can be tested without a window; `workshop.rs` builds
//! the ray and the node list, this module decides what is under the cursor.

use bevy::prelude::{Vec2, Vec3};

/// Nearest positive distance along `direction` where the ray enters the cylinder, if it does.
/// `direction` does not need to be normalised. The cylinder is upright: its axis is Y and
/// `center` is its middle.
pub fn ray_cylinder(origin: Vec3, direction: Vec3, center: Vec3, radius: f32, half_height: f32) -> Option<f32> {
    let o = origin - center;
    let a = direction.x * direction.x + direction.z * direction.z;
    let b = 2.0 * (o.x * direction.x + o.z * direction.z);
    let c = o.x * o.x + o.z * o.z - radius * radius;
    // A ray parallel to the axis still hits the flat ends, handled below.
    let mut best: Option<f32> = None;
    if a > 1e-9 {
        let discriminant = b * b - 4.0 * a * c;
        if discriminant >= 0.0 {
            let root = discriminant.sqrt();
            for t in [(-b - root) / (2.0 * a), (-b + root) / (2.0 * a)] {
                if t > 0.0 {
                    let y = o.y + direction.y * t;
                    if y.abs() <= half_height {
                        best = Some(best.map_or(t, |current: f32| current.min(t)));
                        break;
                    }
                }
            }
        }
    }
    // Cap hit: the ray crosses one of the two discs.
    if direction.y.abs() > 1e-6 {
        for cap in [-half_height, half_height] {
            let t = (cap - o.y) / direction.y;
            if t > 0.0 {
                let x = o.x + direction.x * t;
                let z = o.z + direction.z * t;
                if x * x + z * z <= radius * radius {
                    best = Some(best.map_or(t, |current: f32| current.min(t)));
                }
            }
        }
    }
    best
}

/// The part whose surface the ray reaches first, as an index into `parts` (origin, radius, height).
pub fn nearest_part(origin: Vec3, direction: Vec3, parts: &[(Vec3, f32, f32)]) -> Option<(usize, f32)> {
    parts
        .iter()
        .enumerate()
        .filter_map(|(index, (center, radius, height))| {
            ray_cylinder(origin, direction, *center, *radius, *height * 0.5).map(|t| (index, t))
        })
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
}

/// Snap radius in screen pixels. Documented in `docs/DECISIONS.md`.
pub const SNAP_PIXELS: f32 = 28.0;

/// Nearest node whose projected position is within the snap radius, if any.
pub fn nearest_snap(cursor: Vec2, nodes: &[Vec2], radius: f32) -> Option<usize> {
    nodes
        .iter()
        .enumerate()
        .filter_map(|(index, screen)| {
            let distance = screen.distance(cursor);
            (distance <= radius).then_some((index, distance))
        })
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn down() -> Vec3 {
        Vec3::new(0.0, -1.0, 0.0)
    }

    #[test]
    fn a_ray_down_the_axis_hits_the_top_cap() {
        let t = ray_cylinder(Vec3::new(0.0, 10.0, 0.0), down(), Vec3::ZERO, 0.5, 1.0).unwrap();
        assert!((t - 9.0).abs() < 1e-6, "{t}");
    }

    #[test]
    fn a_ray_beside_the_body_misses() {
        assert!(ray_cylinder(Vec3::new(2.0, 10.0, 0.0), down(), Vec3::ZERO, 0.5, 1.0).is_none());
    }

    #[test]
    fn the_height_limit_is_respected() {
        // Grazing the side from close range but above the cap: passes over the part.
        let origin = Vec3::new(-2.0, 5.0, 0.0);
        let direction = Vec3::new(1.0, 0.0, 0.0);
        assert!(ray_cylinder(origin, direction, Vec3::ZERO, 0.5, 1.0).is_none());
        let origin = Vec3::new(-2.0, 0.5, 0.0);
        assert!(ray_cylinder(origin, direction, Vec3::ZERO, 0.5, 1.0).is_some());
    }

    #[test]
    fn the_nearest_part_wins() {
        let parts = [
            (Vec3::new(0.0, 5.0, 0.0), 0.4, 1.0),
            (Vec3::new(0.0, 2.0, 0.0), 0.4, 1.0),
        ];
        let (index, _) = nearest_part(Vec3::new(0.0, 10.0, 0.0), down(), &parts).unwrap();
        assert_eq!(index, 0);
    }

    #[test]
    fn snapping_uses_the_nearest_node_inside_the_radius() {
        let nodes = [Vec2::new(10.0, 10.0), Vec2::new(30.0, 10.0), Vec2::new(200.0, 200.0)];
        assert_eq!(nearest_snap(Vec2::new(34.0, 12.0), &nodes, SNAP_PIXELS), Some(1));
        assert_eq!(nearest_snap(Vec2::new(60.0, 12.0), &nodes, SNAP_PIXELS), None);
        assert_eq!(nearest_snap(Vec2::new(100.0, 100.0), &[], SNAP_PIXELS), None);
    }
}
