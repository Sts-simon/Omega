//! Adapter from workshop planar plate primitives to the Bevy-independent
//! triangular thermal mesh model. Only flat platforms and panels are meshed.

use super::{Assembly, MaterialKind, PartKind};
use omega_sim_core::{
    Material, MeshNode2D, MeshThermalConfig, MeshTriangle2D, PlanarPolygonRegion,
    PlanarRectangle, TriMesh2D, mesh_planar_polygon, mesh_planar_rectangles,
};
use std::collections::{HashMap, HashSet};

const MAX_SKETCH_VERTICES: usize = 8_192;

#[derive(Clone, Debug)]
pub struct AssemblyMeshBuild {
    pub config: MeshThermalConfig,
    pub meshed_part_ids: Vec<String>,
    pub skipped_part_ids: Vec<String>,
    pub workshop_plane_y_m: f64,
}

/// Builds a 2D thermal mesh from coplanar horizontal platform/panel components.
///
/// The current solver is a planar slab model: it deliberately rejects vertical
/// rotations and stacked/overlapping solids instead of silently projecting a
/// general 3D assembly into an inaccurate 2D shape.
pub fn build_assembly_mesh_thermal_config(
    assembly: &Assembly,
    max_edge_length_m: f64,
) -> Result<AssemblyMeshBuild, String> {
    assembly.validate().map_err(|issues| issues.join("; "))?;
    if assembly.parts.is_empty() {
        return Err("Assembly has no components".into());
    }

    let materials = vec![
        Material {
            name: "Aluminium (illustrative library value)".into(),
            conductivity_w_m_k: 205.0,
            density_kg_m3: 2_700.0,
            specific_heat_j_kg_k: 900.0,
        },
        Material {
            name: "Steel (illustrative library value)".into(),
            conductivity_w_m_k: 45.0,
            density_kg_m3: 7_850.0,
            specific_heat_j_kg_k: 470.0,
        },
        Material {
            name: "Polymer (illustrative library value)".into(),
            conductivity_w_m_k: 0.22,
            density_kg_m3: 1_200.0,
            specific_heat_j_kg_k: 1_400.0,
        },
        Material {
            name: "Composite (placeholder library value)".into(),
            conductivity_w_m_k: 5.0,
            density_kg_m3: 1_600.0,
            specific_heat_j_kg_k: 900.0,
        },
    ];

    let mut rectangles = Vec::new();
    let mut meshed_part_ids = Vec::new();
    let mut skipped_part_ids = Vec::new();
    let mut common_plane_y: Option<f64> = None;

    for part in &assembly.parts {
        if !matches!(&part.kind, PartKind::Platform | PartKind::Panel) {
            skipped_part_ids.push(part.id.clone());
            continue;
        }
        if part.rotation_rad[0].abs() > 1.0e-7 || part.rotation_rad[2].abs() > 1.0e-7 {
            return Err(format!(
                "Part '{}' is tilted out of the workshop X/Z plane; the current 2D plate mesher only accepts rotation around the vertical axis",
                part.id
            ));
        }
        match common_plane_y {
            None => common_plane_y = Some(part.position_m[1]),
            Some(y) if (part.position_m[1] - y).abs() <= 1.0e-7 => {}
            Some(_) => return Err(format!(
                "Part '{}' is on a different vertical plane; stacked 3D solids are not supported by this 2D mesher",
                part.id
            )),
        }
        let material_id = match &part.material {
            MaterialKind::Aluminum => 0,
            MaterialKind::Steel => 1,
            MaterialKind::Polymer => 2,
            MaterialKind::Composite => 3,
        };
        rectangles.push(PlanarRectangle {
            source_part_id: part.id.clone(),
            center_m: [part.position_m[0], part.position_m[2]],
            size_m: [part.dimensions_m[0], part.dimensions_m[2]],
            rotation_rad: part.rotation_rad[1],
            material_id,
            thickness_m: part.dimensions_m[1],
        });
        meshed_part_ids.push(part.id.clone());
    }

    if rectangles.is_empty() {
        return Err("No supported planar components found. Add a horizontal Platform or Panel in the Workshop.".into());
    }
    let mesh: TriMesh2D = mesh_planar_rectangles(&rectangles, max_edge_length_m)
        .map_err(|error| error.to_string())?;

    let config = MeshThermalConfig {
        name: format!("{} | workshop geometry thermal study", assembly.name),
        mesh,
        materials,
        initial_temperature_k: 293.15,
        fixed_boundary_temperature_k: Some(293.15),
        time_step_s: 0.05,
        steps: 600,
        volumetric_heat_source_w_m3: 50_000.0,
    };
    Ok(AssemblyMeshBuild {
        config,
        meshed_part_ids,
        skipped_part_ids,
        workshop_plane_y_m: common_plane_y.unwrap_or(0.0),
    })
}


/// Converts several independently drawn polygon regions into one conforming
/// thermal mesh. Shared complete edges are welded by coordinate; overlapping
/// domains, point contacts and partial-edge contacts are rejected.
pub fn build_polygon_sketch_thermal_config(
    regions: &[PlanarPolygonRegion],
) -> Result<MeshThermalConfig, String> {
    if regions.is_empty() {
        return Err("No polygon regions have been drawn in the Workshop. Press V in the Workshop to start.".into());
    }
    if regions.len() > 64 {
        return Err("A sketch may contain at most 64 material regions".into());
    }

    let total_input_vertices = regions.iter().fold(0_usize, |total, region| {
        let hole_vertices = region.holes_m.iter().fold(0_usize, |count, hole| count.saturating_add(hole.len()));
        total
            .saturating_add(region.outer_boundary_m.len())
            .saturating_add(hole_vertices)
    });
    if total_input_vertices > MAX_SKETCH_VERTICES {
        return Err(format!("Combined polygon sketch exceeds the {MAX_SKETCH_VERTICES}-vertex input budget"));
    }

    let mut region_meshes = Vec::with_capacity(regions.len());
    let mut source_ids = HashSet::new();
    let mut aggregate_nodes = 0_usize;
    let mut aggregate_triangles = 0_usize;
    for region in regions {
        if !source_ids.insert(region.source_part_id.as_str()) {
            return Err(format!("Duplicate polygon region ID '{}'", region.source_part_id));
        }
        if region.material_id >= 4 {
            return Err(format!("Region '{}' refers to a material outside the four-entry workshop library", region.source_part_id));
        }
        let mesh = mesh_planar_polygon(region).map_err(|error| error.to_string())?;
        aggregate_nodes = aggregate_nodes.saturating_add(mesh.nodes.len());
        aggregate_triangles = aggregate_triangles.saturating_add(mesh.triangles.len());
        if aggregate_nodes > 150_000 || aggregate_triangles > 250_000 {
            return Err("Combined polygon sketch exceeds the 150,000-node or 250,000-triangle mesh budget".into());
        }
        region_meshes.push(mesh);
    }
    for first in 0..regions.len() {
        for second in (first + 1)..regions.len() {
            validate_region_pair(&regions[first], &region_meshes[first], &regions[second], &region_meshes[second])?;
        }
    }

    const MERGE_TOLERANCE_M: f64 = 1.0e-8;
    let mut nodes = Vec::<MeshNode2D>::new();
    let mut triangles = Vec::<MeshTriangle2D>::new();
    let mut node_lookup = HashMap::<(i64, i64), usize>::new();

    for mesh in region_meshes {
        let mut remap = Vec::with_capacity(mesh.nodes.len());
        for node in &mesh.nodes {
            let key = (
                (node.position_m[0] / MERGE_TOLERANCE_M).round() as i64,
                (node.position_m[1] / MERGE_TOLERANCE_M).round() as i64,
            );
            let node_id = if let Some(existing) = node_lookup.get(&key) {
                let position = nodes[*existing].position_m;
                if (position[0] - node.position_m[0]).hypot(position[1] - node.position_m[1]) > MERGE_TOLERANCE_M * 1.5 {
                    return Err("Two sketch nodes fall inside the coordinate-welding tolerance but are not coincident".into());
                }
                *existing
            } else {
                let id = nodes.len();
                nodes.push(node.clone());
                node_lookup.insert(key, id);
                id
            };
            remap.push(node_id);
        }
        for triangle in mesh.triangles {
            triangles.push(MeshTriangle2D {
                node_ids: triangle.node_ids.map(|node_id| remap[node_id]),
                material_id: triangle.material_id,
                thickness_m: triangle.thickness_m,
                source_part_id: triangle.source_part_id,
            });
        }
    }

    let mesh = TriMesh2D { nodes, triangles };
    mesh.validate(4).map_err(|error| error.to_string())?;
    Ok(MeshThermalConfig {
        name: format!("Workshop polygon sketch | {} regions", regions.len()),
        mesh,
        materials: sketch_material_library(),
        initial_temperature_k: 293.15,
        fixed_boundary_temperature_k: Some(293.15),
        time_step_s: 0.0001,
        steps: 1_200,
        volumetric_heat_source_w_m3: 35_000.0,
    })
}

fn sketch_material_library() -> Vec<Material> {
    vec![
        Material { name: "Aluminium (illustrative library value)".into(), conductivity_w_m_k: 205.0, density_kg_m3: 2_700.0, specific_heat_j_kg_k: 900.0 },
        Material { name: "Steel (illustrative library value)".into(), conductivity_w_m_k: 45.0, density_kg_m3: 7_850.0, specific_heat_j_kg_k: 470.0 },
        Material { name: "Polymer (illustrative library value)".into(), conductivity_w_m_k: 0.22, density_kg_m3: 1_200.0, specific_heat_j_kg_k: 1_400.0 },
        Material { name: "Composite (placeholder library value)".into(), conductivity_w_m_k: 5.0, density_kg_m3: 1_600.0, specific_heat_j_kg_k: 900.0 },
    ]
}

fn validate_region_pair(
    first: &PlanarPolygonRegion,
    first_mesh: &TriMesh2D,
    second: &PlanarPolygonRegion,
    second_mesh: &TriMesh2D,
) -> Result<(), String> {
    let mut shared_edges = Vec::<([f64; 2], [f64; 2])>::new();
    let mut point_touches = Vec::<[f64; 2]>::new();
    for ring_a in all_rings(first) {
        for edge_a in ring_edges(ring_a) {
            for ring_b in all_rings(second) {
                for edge_b in ring_edges(ring_b) {
                    match classify_segment_contact(edge_a.0, edge_a.1, edge_b.0, edge_b.1) {
                        SegmentContact::None => {}
                        SegmentContact::CompleteSharedEdge => shared_edges.push(edge_a),
                        SegmentContact::PointTouch => {
                            if let Some(point) = point_touch_location(edge_a, edge_b) {
                                point_touches.push(point);
                            }
                        }
                        SegmentContact::ProperCrossing => return Err(format!("Sketch regions '{}' and '{}' cross or overlap", first.source_part_id, second.source_part_id)),
                        SegmentContact::PartialOverlap => return Err(format!("Sketch regions '{}' and '{}' share only a partial edge", first.source_part_id, second.source_part_id)),
                    }
                }
            }
        }
    }
    // A valid shared edge naturally creates point touches at its endpoints
    // when adjacent ring edges are examined. An additional isolated touch or
    // a T-junction in the middle of a shared edge is still non-conforming.
    let unsupported_point_touch = point_touches.iter().any(|point| {
        !point_touch_is_shared_edge_endpoint(*point, &shared_edges)
    });
    if !point_touches.is_empty() && (shared_edges.is_empty() || unsupported_point_touch) {
        return Err(format!("Sketch regions '{}' and '{}' contain an isolated point contact or a T-junction", first.source_part_id, second.source_part_id));
    }

    if first_mesh.triangles.iter().any(|triangle| {
        let a = first_mesh.nodes[triangle.node_ids[0]].position_m;
        let b = first_mesh.nodes[triangle.node_ids[1]].position_m;
        let c = first_mesh.nodes[triangle.node_ids[2]].position_m;
        point_in_region([(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0], second)
    }) || second_mesh.triangles.iter().any(|triangle| {
        let a = second_mesh.nodes[triangle.node_ids[0]].position_m;
        let b = second_mesh.nodes[triangle.node_ids[1]].position_m;
        let c = second_mesh.nodes[triangle.node_ids[2]].position_m;
        point_in_region([(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0], first)
    }) {
        return Err(format!("Sketch regions '{}' and '{}' overlap in area", first.source_part_id, second.source_part_id));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SegmentContact { None, CompleteSharedEdge, PartialOverlap, PointTouch, ProperCrossing }

fn classify_segment_contact(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> SegmentContact {
    const EPS: f64 = 1.0e-8;
    let cross = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    let on_segment = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        cross(p, q, r).abs() <= EPS
            && r[0] >= p[0].min(q[0]) - EPS && r[0] <= p[0].max(q[0]) + EPS
            && r[1] >= p[1].min(q[1]) - EPS && r[1] <= p[1].max(q[1]) + EPS
    };
    let o1 = cross(a, b, c);
    let o2 = cross(a, b, d);
    let o3 = cross(c, d, a);
    let o4 = cross(c, d, b);
    if ((o1 > EPS && o2 < -EPS) || (o1 < -EPS && o2 > EPS))
        && ((o3 > EPS && o4 < -EPS) || (o3 < -EPS && o4 > EPS)) {
        return SegmentContact::ProperCrossing;
    }
    if o1.abs() <= EPS && o2.abs() <= EPS && o3.abs() <= EPS && o4.abs() <= EPS {
        let axis = if (b[0] - a[0]).abs() >= (b[1] - a[1]).abs() { 0 } else { 1 };
        let low = a[axis].min(b[axis]).max(c[axis].min(d[axis]));
        let high = a[axis].max(b[axis]).min(c[axis].max(d[axis]));
        if high - low > EPS {
            let same = (distance(a, c) <= EPS && distance(b, d) <= EPS) || (distance(a, d) <= EPS && distance(b, c) <= EPS);
            return if same { SegmentContact::CompleteSharedEdge } else { SegmentContact::PartialOverlap };
        }
    }
    if on_segment(a, b, c) || on_segment(a, b, d) || on_segment(c, d, a) || on_segment(c, d, b) {
        SegmentContact::PointTouch
    } else { SegmentContact::None }
}

fn point_touch_is_shared_edge_endpoint(
    point: [f64; 2],
    shared_edges: &[([f64; 2], [f64; 2])],
) -> bool {
    const EPS: f64 = 1.0e-8;
    shared_edges.iter().any(|(a, b)| distance(point, *a) <= EPS || distance(point, *b) <= EPS)
}

fn point_touch_location(
    first: ([f64; 2], [f64; 2]),
    second: ([f64; 2], [f64; 2]),
) -> Option<[f64; 2]> {
    const EPS: f64 = 1.0e-8;
    let on_segment = |point: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        let cross = (b[0] - a[0]) * (point[1] - a[1]) - (b[1] - a[1]) * (point[0] - a[0]);
        cross.abs() <= EPS
            && point[0] >= a[0].min(b[0]) - EPS
            && point[0] <= a[0].max(b[0]) + EPS
            && point[1] >= a[1].min(b[1]) - EPS
            && point[1] <= a[1].max(b[1]) + EPS
    };
    [first.0, first.1, second.0, second.1]
        .into_iter()
        .find(|point| on_segment(*point, first.0, first.1) && on_segment(*point, second.0, second.1))
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 { (a[0] - b[0]).hypot(a[1] - b[1]) }

fn all_rings(region: &PlanarPolygonRegion) -> impl Iterator<Item = &[[f64; 2]]> {
    std::iter::once(region.outer_boundary_m.as_slice()).chain(region.holes_m.iter().map(Vec::as_slice))
}

fn ring_edges(ring: &[[f64; 2]]) -> impl Iterator<Item = ([f64; 2], [f64; 2])> + '_ {
    (0..ring.len()).map(|index| (ring[index], ring[(index + 1) % ring.len()]))
}

fn point_in_region(point: [f64; 2], region: &PlanarPolygonRegion) -> bool {
    point_in_ring(point, &region.outer_boundary_m)
        && !region.holes_m.iter().any(|hole| point_in_ring(point, hole))
}

fn point_in_ring(point: [f64; 2], ring: &[[f64; 2]]) -> bool {
    let mut inside = false;
    if ring.len() < 3 { return false; }
    let mut previous = ring.len() - 1;
    for current in 0..ring.len() {
        let a = ring[current];
        let b = ring[previous];
        if (a[1] > point[1]) != (b[1] > point[1]) {
            let crossing_x = (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]) + a[0];
            if point[0] < crossing_x { inside = !inside; }
        }
        previous = current;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::{Assembly, Part, PartKind};

    #[test]
    fn workshop_demo_generates_mesh_from_its_platform() {
        let assembly = Assembly::demo();
        let built = build_assembly_mesh_thermal_config(&assembly, 0.05).unwrap();
        assert_eq!(built.meshed_part_ids, vec!["base-plate"]);
        assert_eq!(built.skipped_part_ids.len(), 2);
        assert!(!built.config.mesh.triangles.is_empty());
        assert!(built.config.mesh.validate(built.config.materials.len()).is_ok());
    }

    #[test]
    fn stacked_platforms_are_rejected_instead_of_projected() {
        let mut assembly = Assembly::demo();
        let mut second = assembly.parts[0].clone();
        second.id = "upper-platform".into();
        second.position_m[1] += 0.1;
        assembly.parts.push(second);
        assert!(build_assembly_mesh_thermal_config(&assembly, 0.05).unwrap_err().contains("different vertical plane"));
    }

    #[test]
    fn overlapping_platforms_are_rejected() {
        let mut assembly = Assembly::demo();
        let mut second = Part {
            id: "overlap".into(), label: "Overlapping platform".into(),
            kind: PartKind::Platform, dimensions_m: [0.5, 0.01, 0.3],
            position_m: [0.0, 0.0, 0.0], rotation_rad: [0.0; 3],
            material: MaterialKind::Steel,
        };
        second.position_m[1] = assembly.parts[0].position_m[1];
        assembly.parts.push(second);
        assert!(build_assembly_mesh_thermal_config(&assembly, 0.05).unwrap_err().contains("overlap"));
    }

    fn square_region(id: &str, x0: f64, x1: f64, material_id: u16) -> PlanarPolygonRegion {
        PlanarPolygonRegion {
            source_part_id: id.into(),
            outer_boundary_m: vec![[x0, 0.0], [x1, 0.0], [x1, 1.0], [x0, 1.0]],
            holes_m: Vec::new(),
            material_id,
            thickness_m: 0.01,
        }
    }

    #[test]
    fn multiple_material_regions_share_complete_edge_and_weld_nodes() {
        let regions = vec![square_region("left", 0.0, 1.0, 0), square_region("right", 1.0, 2.0, 1)];
        let config = build_polygon_sketch_thermal_config(&regions).unwrap();
        assert!((config.mesh.validate(config.materials.len()).unwrap().total_area_m2 - 2.0).abs() < 1.0e-10);
        assert!(config.mesh.triangles.iter().any(|triangle| triangle.material_id == 0));
        assert!(config.mesh.triangles.iter().any(|triangle| triangle.material_id == 1));
        let shared_nodes = config.mesh.nodes.iter().filter(|node| (node.position_m[0] - 1.0).abs() < 1.0e-10).count();
        assert!(shared_nodes >= 2);
    }

    #[test]
    fn multiple_material_regions_reject_area_overlap() {
        let regions = vec![square_region("first", 0.0, 1.0, 0), square_region("second", 0.5, 1.5, 1)];
        assert!(build_polygon_sketch_thermal_config(&regions).is_err());
    }

    #[test]
    fn multiple_regions_reject_point_only_contact() {
        let first = square_region("first", 0.0, 1.0, 0);
        let second = PlanarPolygonRegion {
            source_part_id: "second".into(),
            outer_boundary_m: vec![[1.0, 1.0], [2.0, 1.0], [2.0, 2.0], [1.0, 2.0]],
            holes_m: Vec::new(), material_id: 1, thickness_m: 0.01,
        };
        assert!(build_polygon_sketch_thermal_config(&[first, second]).unwrap_err().contains("isolated point contact"));
    }

    #[test]
    fn isolated_touch_inside_an_otherwise_shared_edge_is_not_an_endpoint_contact() {
        let shared_edges = vec![([1.0, 0.0], [1.0, 1.0])];
        assert!(point_touch_is_shared_edge_endpoint([1.0, 1.0], &shared_edges));
        assert!(!point_touch_is_shared_edge_endpoint([1.0, 0.5], &shared_edges));
    }

    #[test]
    fn bundled_workshop_sketch_builds_a_two_material_mesh_with_a_hole() {
        let value: serde_json::Value = serde_json::from_str(include_str!("../../examples/workshop-polygon-sketch.json")).unwrap();
        let regions: Vec<PlanarPolygonRegion> = serde_json::from_value(value["regions"].clone()).unwrap();
        let config = build_polygon_sketch_thermal_config(&regions).unwrap();
        let stats = config.mesh.validate(config.materials.len()).unwrap();
        assert!((stats.total_area_m2 - 5.8).abs() < 1.0e-8);
        assert!(config.mesh.triangles.iter().any(|triangle| triangle.material_id == 0));
        assert!(config.mesh.triangles.iter().any(|triangle| triangle.material_id == 1));
        assert!(config.mesh.triangles.iter().any(|triangle| triangle.source_part_id == "sketch-region-001"));
        assert!(config.mesh.triangles.iter().any(|triangle| triangle.source_part_id == "sketch-region-002"));
    }

    #[test]
    fn combined_polygon_input_budget_is_checked_before_triangulation() {
        let mut oversized = square_region("oversized", 0.0, 1.0, 0);
        oversized.outer_boundary_m = vec![[0.0, 0.0]; MAX_SKETCH_VERTICES + 1];
        let error = build_polygon_sketch_thermal_config(&[oversized]).unwrap_err();
        assert!(error.contains("input budget"));
    }

    #[test]
    fn polygon_sketch_requires_at_least_one_region() {
        assert!(build_polygon_sketch_thermal_config(&[]).unwrap_err().contains("No polygon regions"));
    }

}
