//! Small, testable geometry used by the workshop pointer: a camera ray against upright cylinders
//! (one part is a vertical box of revolution) and the screen-space snap test for attachment nodes.
//!
//! Kept apart from Bevy so the picking rules can be tested without a window; `workshop.rs` builds
//! the ray and the node list, this module decides what is under the cursor.

use bevy::prelude::{Quat, Vec2, Vec3};

/// Nearest positive distance along `direction` where the ray enters the cylinder, if it does.
/// `direction` does not need to be normalised. The cylinder is upright: its axis is Y and
/// `center` is its middle.
pub fn ray_cylinder(origin: Vec3, direction: Vec3, center: Vec3, radius: f32, half_height: f32, rotation: Quat) -> Option<f32> {
    let inverse = rotation.inverse();
    let origin = inverse * (origin - center);
    let direction = inverse * direction;
    let o = origin;
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

/// What the pointer can hit. Bodies are cylinders; wings, fins and control surfaces are thin
/// oriented plates with their span on the local X axis and a yaw about Y.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PickShape {
    Cylinder {
        center: Vec3,
        radius: f32,
        half_height: f32,
        rotation: Quat,
    },
    Box {
        center: Vec3,
        half: Vec3,
        /// Full local-to-world orientation, matching the 3D mesh.
        rotation: Quat,
    },
}

fn ray_box(origin: Vec3, direction: Vec3, center: Vec3, half: Vec3, rotation: Quat) -> Option<f32> {
    let inverse = rotation.inverse();
    let local_origin = inverse * (origin - center);
    let local_direction = inverse * direction;
    let mut near = f32::NEG_INFINITY;
    let mut far = f32::INFINITY;
    for axis in 0..3 {
        let d = local_direction[axis];
        let o = local_origin[axis];
        let h = half[axis];
        if d.abs() < 1e-9 {
            if o.abs() > h {
                return None;
            }
            continue;
        }
        let t1 = (-h - o) / d;
        let t2 = (h - o) / d;
        near = near.max(t1.min(t2));
        far = far.min(t1.max(t2));
        if near > far {
            return None;
        }
    }
    if far > 0.0 && near > 0.0 {
        Some(near)
    } else if far > 0.0 {
        Some(far)
    } else {
        None
    }
}

/// Nearest hit among mixed shapes, as an index into `shapes`.
pub fn nearest_hit(origin: Vec3, direction: Vec3, shapes: &[PickShape]) -> Option<(usize, f32)> {
    shapes
        .iter()
        .enumerate()
        .filter_map(|(index, shape)| {
            let t = match *shape {
                PickShape::Cylinder {
                    center,
                    radius,
                    half_height,
                    rotation,
                } => ray_cylinder(origin, direction, center, radius, half_height, rotation),
                PickShape::Box { center, half, rotation } => ray_box(origin, direction, center, half, rotation),
            };
            t.map(|t| (index, t))
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
        let t = ray_cylinder(Vec3::new(0.0, 10.0, 0.0), down(), Vec3::ZERO, 0.5, 1.0, Quat::IDENTITY).unwrap();
        assert!((t - 9.0).abs() < 1e-6, "{t}");
    }

    #[test]
    fn a_ray_beside_the_body_misses() {
        assert!(ray_cylinder(Vec3::new(2.0, 10.0, 0.0), down(), Vec3::ZERO, 0.5, 1.0, Quat::IDENTITY).is_none());
    }

    #[test]
    fn the_height_limit_is_respected() {
        // Grazing the side from close range but above the cap: passes over the part.
        let origin = Vec3::new(-2.0, 5.0, 0.0);
        let direction = Vec3::new(1.0, 0.0, 0.0);
        assert!(ray_cylinder(origin, direction, Vec3::ZERO, 0.5, 1.0, Quat::IDENTITY).is_none());
        let origin = Vec3::new(-2.0, 0.5, 0.0);
        assert!(ray_cylinder(origin, direction, Vec3::ZERO, 0.5, 1.0).is_some());
    }

    #[test]
    fn the_nearest_part_wins() {
        let shapes = [
            PickShape::Cylinder {
                center: Vec3::new(0.0, 5.0, 0.0),
                radius: 0.4,
                half_height: 0.5,
                rotation: Quat::IDENTITY,
            },
            PickShape::Cylinder {
                center: Vec3::new(0.0, 2.0, 0.0),
                radius: 0.4,
                half_height: 0.5,
                rotation: Quat::IDENTITY,
            },
        ];
        let (index, _) = nearest_hit(Vec3::new(0.0, 10.0, 0.0), down(), &shapes).unwrap();
        assert_eq!(index, 0);
    }

    #[test]
    fn a_box_is_hit_on_its_plate_and_rotates_with_its_mount() {
        // A wing plate 2 m wide (local X), 0.4 m chord, 0.06 m thick, at the origin, yawed 90°.
        let half = Vec3::new(1.0, 0.2, 0.03);
        let from_above = nearest_hit(
            Vec3::new(0.0, 5.0, 0.0),
            down(),
            &[PickShape::Box {
                center: Vec3::ZERO,
                half,
                rotation: Quat::IDENTITY,
            }],
        );
        assert!(from_above.is_some(), "the plate is hit from above");
        // Yawed by 90°, the wide axis now runs along Z: a ray along X passes through it.
        let rotated = PickShape::Box {
            center: Vec3::ZERO,
            half,
            rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        };
        assert!(nearest_hit(Vec3::new(5.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0), &[rotated]).is_some());
        let edge = nearest_hit(
            Vec3::new(0.0, 5.0, 3.0),
            down(),
            &[PickShape::Box {
                center: Vec3::ZERO,
                half,
                rotation: Quat::IDENTITY,
            }],
        );
        assert!(edge.is_none(), "outside the plate's span");
    }

    #[test]
    fn mixed_shapes_report_the_nearest_hit() {
        let shapes = [
            PickShape::Cylinder {
                center: Vec3::new(0.0, 0.0, 0.0),
                radius: 0.4,
                half_height: 1.0,
                rotation: Quat::IDENTITY,
            },
            PickShape::Box {
                center: Vec3::new(0.0, 0.0, 2.0),
                half: Vec3::new(1.0, 0.2, 0.03),
                rotation: Quat::IDENTITY,
            },
        ];
        let (index, _) = nearest_hit(Vec3::new(0.0, 5.0, 2.0), down(), &shapes).unwrap();
        assert_eq!(index, 1);
        let (index, _) = nearest_hit(Vec3::new(0.0, 5.0, 0.0), down(), &shapes).unwrap();
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
