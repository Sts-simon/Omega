//! Deterministic meshing primitives for generic planar civil/industrial regions.
//!
//! The module supports regular meshing of coplanar rectangles and constrained
//! triangulation of one simple concave polygonal region with interior holes.
//! These are 2D slab domains, not a general 3D CAD tessellator.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

const MAX_SUBDIVISIONS_PER_AXIS: usize = 256;
const MAX_MESH_NODES: usize = 150_000;
const MAX_MESH_TRIANGLES: usize = 250_000;
const MAX_POLYGON_VERTICES: usize = 2_048;
const MAX_POLYGON_COORDINATE_ABS_M: f64 = 10_000.0;
const COORDINATE_MERGE_TOLERANCE_M: f64 = 1.0e-8;
const OVERLAP_TOLERANCE_M: f64 = 1.0e-8;
const ANGLE_TOLERANCE: f64 = 1.0e-10;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MeshNode2D {
    /// Planar coordinates in metres. The application maps X/Z workshop
    /// coordinates to this X/Y plane.
    pub position_m: [f64; 2],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MeshTriangle2D {
    pub node_ids: [usize; 3],
    pub material_id: u16,
    pub thickness_m: f64,
    pub source_part_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TriMesh2D {
    pub nodes: Vec<MeshNode2D>,
    pub triangles: Vec<MeshTriangle2D>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlanarRectangle {
    pub source_part_id: String,
    pub center_m: [f64; 2],
    pub size_m: [f64; 2],
    pub rotation_rad: f64,
    pub material_id: u16,
    pub thickness_m: f64,
}

/// One planar polygonal region for meshing. The outer contour may be concave;
/// `holes_m` contains zero or more interior void contours. Rings are open
/// vertex lists (the first point must not be repeated at the end). The mesher
/// normalizes winding and removes harmless collinear intermediate vertices.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlanarPolygonRegion {
    pub source_part_id: String,
    pub outer_boundary_m: Vec<[f64; 2]>,
    #[serde(default)]
    pub holes_m: Vec<Vec<[f64; 2]>>,
    pub material_id: u16,
    pub thickness_m: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeshStats {
    pub node_count: usize,
    pub triangle_count: usize,
    pub boundary_edge_count: usize,
    pub boundary_node_count: usize,
    pub total_area_m2: f64,
    pub total_volume_m3: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeshQualityStats {
    pub minimum_angle_deg: f64,
    /// Circumradius / (2 * inradius); an equilateral triangle has value 1.
    pub maximum_radius_ratio: f64,
    pub maximum_edge_length_m: f64,
    pub minimum_triangle_area_m2: f64,
}

#[derive(Clone, Debug)]
pub struct MeshRefinementResult {
    pub mesh: TriMesh2D,
    pub rounds: usize,
    /// Number of old elements replaced by smaller elements, accumulated over rounds.
    pub elements_split: usize,
    /// Number of unique edges bisected, accumulated over rounds.
    pub edges_split: usize,
    pub stats: MeshStats,
    pub quality: MeshQualityStats,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MeshError {
    InvalidInput(String),
    OverlappingRegions { first: String, second: String },
    NonConformingContact { first: String, second: String },
    LimitExceeded(String),
}

impl fmt::Display for MeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(f, "Invalid mesh input: {message}"),
            Self::OverlappingRegions { first, second } => write!(
                f,
                "Planar regions '{first}' and '{second}' overlap in area; overlapping solids are not supported by this 2D mesher"
            ),
            Self::NonConformingContact { first, second } => write!(
                f,
                "Planar regions '{first}' and '{second}' touch without sharing one complete matching edge; point contacts and partial edge contacts are unsupported"
            ),
            Self::LimitExceeded(message) => write!(f, "Mesh limit exceeded: {message}"),
        }
    }
}

impl std::error::Error for MeshError {}

impl TriMesh2D {
    pub fn validate(&self, material_count: usize) -> Result<MeshStats, MeshError> {
        if self.nodes.len() < 3 {
            return Err(MeshError::InvalidInput("a triangular mesh needs at least three nodes".into()));
        }
        if self.triangles.is_empty() {
            return Err(MeshError::InvalidInput("mesh contains no triangles".into()));
        }
        if self.nodes.len() > MAX_MESH_NODES || self.triangles.len() > MAX_MESH_TRIANGLES {
            return Err(MeshError::LimitExceeded("mesh exceeds the configured node or triangle budget".into()));
        }
        for (index, node) in self.nodes.iter().enumerate() {
            if node.position_m.iter().any(|value| !value.is_finite() || value.abs() > 1.0e4) {
                return Err(MeshError::InvalidInput(format!("node {index} has invalid coordinates")));
            }
        }

        let mut area_sum = 0.0;
        let mut volume_sum = 0.0;
        let mut edge_counts: HashMap<(usize, usize), u8> = HashMap::new();
        let mut used = vec![false; self.nodes.len()];
        for (index, triangle) in self.triangles.iter().enumerate() {
            if triangle.node_ids.iter().any(|id| *id >= self.nodes.len()) {
                return Err(MeshError::InvalidInput(format!("triangle {index} references a missing node")));
            }
            let [a, b, c] = triangle.node_ids;
            if a == b || b == c || c == a {
                return Err(MeshError::InvalidInput(format!("triangle {index} repeats a node")));
            }
            if usize::from(triangle.material_id) >= material_count {
                return Err(MeshError::InvalidInput(format!("triangle {index} references missing material {}", triangle.material_id)));
            }
            if triangle.source_part_id.trim().is_empty() {
                return Err(MeshError::InvalidInput(format!("triangle {index} has an empty source part ID")));
            }
            if !triangle.thickness_m.is_finite() || triangle.thickness_m <= 0.0 {
                return Err(MeshError::InvalidInput(format!("triangle {index} has invalid thickness")));
            }
            let p0 = self.nodes[a].position_m;
            let p1 = self.nodes[b].position_m;
            let p2 = self.nodes[c].position_m;
            let twice_area = cross(sub(p1, p0), sub(p2, p0));
            if !twice_area.is_finite() || twice_area <= 1.0e-14 {
                return Err(MeshError::InvalidInput(format!("triangle {index} is degenerate or not counter-clockwise")));
            }
            let area = 0.5 * twice_area;
            area_sum += area;
            volume_sum += area * triangle.thickness_m;
            for node in [a, b, c] { used[node] = true; }
            for (left, right) in [(a, b), (b, c), (c, a)] {
                let key = if left < right { (left, right) } else { (right, left) };
                let count = edge_counts.entry(key).or_default();
                *count = count.saturating_add(1);
                if *count > 2 {
                    return Err(MeshError::InvalidInput(format!("non-manifold edge ({}, {}) belongs to more than two triangles", key.0, key.1)));
                }
            }
        }
        if used.iter().any(|value| !value) {
            return Err(MeshError::InvalidInput("mesh contains orphan nodes".into()));
        }
        if !area_sum.is_finite() || area_sum <= 0.0 || !volume_sum.is_finite() || volume_sum <= 0.0 {
            return Err(MeshError::InvalidInput("mesh has invalid total area or volume".into()));
        }
        let boundary_edges: Vec<_> = edge_counts.iter().filter_map(|(edge, count)| (*count == 1).then_some(*edge)).collect();
        let mut boundary_nodes = vec![false; self.nodes.len()];
        for (a, b) in &boundary_edges { boundary_nodes[*a] = true; boundary_nodes[*b] = true; }
        Ok(MeshStats {
            node_count: self.nodes.len(),
            triangle_count: self.triangles.len(),
            boundary_edge_count: boundary_edges.len(),
            boundary_node_count: boundary_nodes.iter().filter(|value| **value).count(),
            total_area_m2: area_sum,
            total_volume_m3: volume_sum,
        })
    }

    /// Computes useful geometric quality indicators for the current triangles.
    pub fn quality_stats(&self) -> Result<MeshQualityStats, MeshError> {
        let material_count = self.triangles.iter()
            .map(|triangle| usize::from(triangle.material_id) + 1)
            .max().unwrap_or(0);
        self.validate(material_count)?;

        let mut minimum_angle = f64::INFINITY;
        let mut maximum_radius_ratio: f64 = 0.0;
        let mut maximum_edge: f64 = 0.0;
        let mut minimum_area = f64::INFINITY;
        for triangle in &self.triangles {
            let [a, b, c] = triangle.node_ids;
            let pa = self.nodes[a].position_m;
            let pb = self.nodes[b].position_m;
            let pc = self.nodes[c].position_m;
            let side_ab = distance(pa, pb);
            let side_bc = distance(pb, pc);
            let side_ca = distance(pc, pa);
            let twice_area = cross(sub(pb, pa), sub(pc, pa));
            let area = 0.5 * twice_area;
            let angle_a = angle_from_sides(side_ab, side_ca, side_bc);
            let angle_b = angle_from_sides(side_ab, side_bc, side_ca);
            let angle_c = angle_from_sides(side_ca, side_bc, side_ab);
            minimum_angle = minimum_angle.min(angle_a).min(angle_b).min(angle_c);
            let perimeter = side_ab + side_bc + side_ca;
            let radius_ratio = side_ab * side_bc * side_ca * perimeter / (16.0 * area * area);
            maximum_radius_ratio = maximum_radius_ratio.max(radius_ratio);
            maximum_edge = maximum_edge.max(side_ab).max(side_bc).max(side_ca);
            minimum_area = minimum_area.min(area);
        }
        Ok(MeshQualityStats {
            minimum_angle_deg: minimum_angle.to_degrees(),
            maximum_radius_ratio,
            maximum_edge_length_m: maximum_edge,
            minimum_triangle_area_m2: minimum_area,
        })
    }

    /// Returns mesh boundary nodes, based on edges referenced by one triangle.
    pub fn boundary_node_ids(&self) -> Result<Vec<usize>, MeshError> {
        self.validate_material_independent()?;
        let mut counts: HashMap<(usize, usize), u8> = HashMap::new();
        for triangle in &self.triangles {
            let [a, b, c] = triangle.node_ids;
            for (left, right) in [(a, b), (b, c), (c, a)] {
                let edge = if left < right { (left, right) } else { (right, left) };
                *counts.entry(edge).or_default() += 1;
            }
        }
        let mut nodes = vec![false; self.nodes.len()];
        for ((a, b), count) in counts {
            if count == 1 { nodes[a] = true; nodes[b] = true; }
        }
        Ok(nodes.iter().enumerate().filter_map(|(index, value)| (*value).then_some(index)).collect())
    }

    fn validate_material_independent(&self) -> Result<(), MeshError> {
        if self.nodes.len() < 3 || self.triangles.is_empty() {
            return Err(MeshError::InvalidInput("mesh needs nodes and triangles".into()));
        }
        for (index, node) in self.nodes.iter().enumerate() {
            if node.position_m.iter().any(|value| !value.is_finite()) {
                return Err(MeshError::InvalidInput(format!("node {index} has non-finite coordinates")));
            }
        }
        for (index, tri) in self.triangles.iter().enumerate() {
            if tri.node_ids.iter().any(|id| *id >= self.nodes.len()) {
                return Err(MeshError::InvalidInput(format!("triangle {index} references a missing node")));
            }
        }
        Ok(())
    }
}

/// Creates a conforming triangular mesh from coplanar, non-overlapping rectangles.
/// Adjacent rectangles share nodes and interior edges. Each rectangle may use a
/// different material and thickness.
pub fn mesh_planar_rectangles(
    rectangles: &[PlanarRectangle],
    max_edge_length_m: f64,
) -> Result<TriMesh2D, MeshError> {
    if rectangles.is_empty() {
        return Err(MeshError::InvalidInput("no planar regions were supplied".into()));
    }
    if !max_edge_length_m.is_finite() || max_edge_length_m <= 0.0 {
        return Err(MeshError::InvalidInput("maximum edge length must be finite and positive".into()));
    }
    if rectangles.len() > 5_000 {
        return Err(MeshError::LimitExceeded("at most 5,000 rectangular regions are supported".into()));
    }
    for (index, rect) in rectangles.iter().enumerate() {
        if rect.source_part_id.trim().is_empty() {
            return Err(MeshError::InvalidInput(format!("rectangle {index} has an empty source ID")));
        }
        if rect.center_m.iter().any(|value| !value.is_finite() || value.abs() > 1.0e4)
            || rect.size_m.iter().any(|value| !value.is_finite() || *value <= 0.0 || *value > 1_000.0)
            || !rect.rotation_rad.is_finite()
            || !rect.thickness_m.is_finite() || rect.thickness_m <= 0.0
            || rect.material_id >= 64
        {
            return Err(MeshError::InvalidInput(format!("rectangle '{}' has invalid geometry or thickness", rect.source_part_id)));
        }
    }
    for first in 0..rectangles.len() {
        for second in (first + 1)..rectangles.len() {
            let a = &rectangles[first];
            let b = &rectangles[second];
            if rectangles_overlap(a, b) {
                return Err(MeshError::OverlappingRegions {
                    first: a.source_part_id.clone(),
                    second: b.source_part_id.clone(),
                });
            }
            if rectangles_have_nonconforming_contact(a, b) {
                return Err(MeshError::NonConformingContact {
                    first: a.source_part_id.clone(),
                    second: b.source_part_id.clone(),
                });
            }
        }
    }

    let mut mesh = TriMesh2D { nodes: Vec::new(), triangles: Vec::new() };
    let mut node_lookup: HashMap<(i64, i64), usize> = HashMap::new();
    for rectangle in rectangles {
        let nx = (rectangle.size_m[0] / max_edge_length_m).ceil().max(1.0) as usize;
        let ny = (rectangle.size_m[1] / max_edge_length_m).ceil().max(1.0) as usize;
        if nx > MAX_SUBDIVISIONS_PER_AXIS || ny > MAX_SUBDIVISIONS_PER_AXIS {
            return Err(MeshError::LimitExceeded(format!(
                "region '{}' requires {nx} x {ny} subdivisions; maximum per axis is {MAX_SUBDIVISIONS_PER_AXIS}",
                rectangle.source_part_id
            )));
        }
        let estimated_new_triangles = 2usize.saturating_mul(nx).saturating_mul(ny);
        if mesh.triangles.len().saturating_add(estimated_new_triangles) > MAX_MESH_TRIANGLES {
            return Err(MeshError::LimitExceeded("generated mesh would exceed 250,000 triangles".into()));
        }
        let mut grid = vec![0usize; (nx + 1) * (ny + 1)];
        let (sin, cos) = rectangle.rotation_rad.sin_cos();
        for iy in 0..=ny {
            for ix in 0..=nx {
                let local_x = (ix as f64 / nx as f64 - 0.5) * rectangle.size_m[0];
                let local_y = (iy as f64 / ny as f64 - 0.5) * rectangle.size_m[1];
                let point = [
                    rectangle.center_m[0] + cos * local_x - sin * local_y,
                    rectangle.center_m[1] + sin * local_x + cos * local_y,
                ];
                let key = coordinate_key(point)?;
                let node_id = if let Some(id) = node_lookup.get(&key).copied() {
                    id
                } else {
                    if mesh.nodes.len() >= MAX_MESH_NODES {
                        return Err(MeshError::LimitExceeded("generated mesh would exceed 150,000 nodes".into()));
                    }
                    let id = mesh.nodes.len();
                    mesh.nodes.push(MeshNode2D { position_m: point });
                    node_lookup.insert(key, id);
                    id
                };
                grid[iy * (nx + 1) + ix] = node_id;
            }
        }
        for iy in 0..ny {
            for ix in 0..nx {
                let a = grid[iy * (nx + 1) + ix];
                let b = grid[iy * (nx + 1) + ix + 1];
                let c = grid[(iy + 1) * (nx + 1) + ix];
                let d = grid[(iy + 1) * (nx + 1) + ix + 1];
                // Alternate the diagonal to reduce directional bias on a regular grid.
                let pair = if (ix + iy) % 2 == 0 { [[a, b, d], [a, d, c]] } else { [[a, b, c], [b, d, c]] };
                for node_ids in pair {
                    mesh.triangles.push(MeshTriangle2D {
                        node_ids,
                        material_id: rectangle.material_id,
                        thickness_m: rectangle.thickness_m,
                        source_part_id: rectangle.source_part_id.clone(),
                    });
                }
            }
        }
    }
    mesh.validate(rectangles.iter().map(|rect| usize::from(rect.material_id) + 1).max().unwrap_or(0))?;
    Ok(mesh)
}

/// Triangulates one simple polygonal planar region, including concave outer
/// boundaries and interior holes. The resulting mesh is conforming and retains
/// material, thickness and source-part metadata on every triangle.
///
/// Self-intersecting contours, touching holes, nested holes and holes outside
/// the outer contour are rejected before triangulation. The Earcut dependency
/// performs the constrained polygon triangulation after these validations.
pub fn mesh_planar_polygon(region: &PlanarPolygonRegion) -> Result<TriMesh2D, MeshError> {
    if region.source_part_id.trim().is_empty() {
        return Err(MeshError::InvalidInput("polygon region has an empty source part ID".into()));
    }
    if region.material_id >= 64 {
        return Err(MeshError::InvalidInput("polygon material ID must be below 64".into()));
    }
    if !region.thickness_m.is_finite() || region.thickness_m <= 0.0 {
        return Err(MeshError::InvalidInput("polygon thickness must be finite and positive".into()));
    }
    if region.holes_m.len() > 64 {
        return Err(MeshError::LimitExceeded("a polygon may contain at most 64 holes".into()));
    }
    let input_vertex_count = region.outer_boundary_m.len()
        .saturating_add(region.holes_m.iter().map(Vec::len).sum::<usize>());
    if input_vertex_count > MAX_POLYGON_VERTICES {
        return Err(MeshError::LimitExceeded(format!(
            "polygon has {input_vertex_count} input vertices; maximum is {MAX_POLYGON_VERTICES}"
        )));
    }

    let mut outer = clean_polygon_ring(&region.outer_boundary_m, "outer boundary")?;
    if signed_ring_area(&outer) < 0.0 {
        outer.reverse();
    }
    validate_simple_ring(&outer, "outer boundary")?;

    let mut holes = Vec::with_capacity(region.holes_m.len());
    for (index, source_ring) in region.holes_m.iter().enumerate() {
        let label = format!("hole {index}");
        let mut hole = clean_polygon_ring(source_ring, &label)?;
        if signed_ring_area(&hole) > 0.0 {
            hole.reverse();
        }
        validate_simple_ring(&hole, &label)?;
        if rings_intersect_or_touch(&outer, &hole) {
            return Err(MeshError::InvalidInput(format!("{label} touches or crosses the outer boundary")));
        }
        if !point_in_ring(hole[0], &outer) {
            return Err(MeshError::InvalidInput(format!("{label} is not strictly inside the outer boundary")));
        }
        holes.push(hole);
    }

    for first in 0..holes.len() {
        for second in (first + 1)..holes.len() {
            if rings_intersect_or_touch(&holes[first], &holes[second])
                || point_in_ring(holes[first][0], &holes[second])
                || point_in_ring(holes[second][0], &holes[first])
            {
                return Err(MeshError::InvalidInput(format!(
                    "holes {first} and {second} overlap, touch or contain one another"
                )));
            }
        }
    }

    let total_vertices = outer.len() + holes.iter().map(Vec::len).sum::<usize>();
    if total_vertices > MAX_POLYGON_VERTICES {
        return Err(MeshError::LimitExceeded(format!(
            "polygon has {total_vertices} vertices; maximum is {MAX_POLYGON_VERTICES}"
        )));
    }

    let expected_area = signed_ring_area(&outer).abs()
        - holes.iter().map(|ring| signed_ring_area(ring).abs()).sum::<f64>();
    if !expected_area.is_finite() || expected_area <= 1.0e-14 {
        return Err(MeshError::InvalidInput("polygon has no positive area after subtracting holes".into()));
    }

    let mut coordinates = Vec::with_capacity(total_vertices * 2);
    let mut hole_indices = Vec::with_capacity(holes.len());
    let mut points = Vec::with_capacity(total_vertices);
    for point in &outer {
        coordinates.extend_from_slice(point);
        points.push(*point);
    }
    let mut next_index = outer.len();
    for hole in &holes {
        hole_indices.push(next_index);
        for point in hole {
            coordinates.extend_from_slice(point);
            points.push(*point);
        }
        next_index += hole.len();
    }

    let indices = earcutr::earcut(&coordinates, &hole_indices, 2)
        .map_err(|error| MeshError::InvalidInput(format!("polygon triangulation failed: {error:?}")))?;
    if indices.is_empty() || indices.len() % 3 != 0 {
        return Err(MeshError::InvalidInput("polygon triangulator returned an incomplete triangle index list".into()));
    }
    if indices.len() / 3 > MAX_MESH_TRIANGLES {
        return Err(MeshError::LimitExceeded("polygon triangulation exceeds 250,000 triangles".into()));
    }

    let mut used = vec![false; points.len()];
    let mut mesh = TriMesh2D {
        nodes: points.iter().map(|position_m| MeshNode2D { position_m: *position_m }).collect(),
        triangles: Vec::with_capacity(indices.len() / 3),
    };
    let mut triangulated_area = 0.0;
    for triangle_indices in indices.chunks_exact(3) {
        let mut node_ids = [triangle_indices[0], triangle_indices[1], triangle_indices[2]];
        if node_ids.iter().any(|index| *index >= points.len()) {
            return Err(MeshError::InvalidInput("polygon triangulator returned an out-of-range vertex index".into()));
        }
        let signed_twice_area = cross(
            sub(points[node_ids[1]], points[node_ids[0]]),
            sub(points[node_ids[2]], points[node_ids[0]]),
        );
        if !signed_twice_area.is_finite() || signed_twice_area.abs() <= 1.0e-14 {
            return Err(MeshError::InvalidInput("polygon triangulator returned a degenerate triangle".into()));
        }
        if signed_twice_area < 0.0 {
            node_ids.swap(1, 2);
        }
        triangulated_area += 0.5 * signed_twice_area.abs();
        for id in node_ids { used[id] = true; }
        mesh.triangles.push(MeshTriangle2D {
            node_ids,
            material_id: region.material_id,
            thickness_m: region.thickness_m,
            source_part_id: region.source_part_id.clone(),
        });
    }

    if used.iter().any(|is_used| !is_used) {
        return Err(MeshError::InvalidInput(
            "polygon triangulator omitted one or more contour vertices; simplify collinear boundary points".into(),
        ));
    }
    let area_error = (triangulated_area - expected_area).abs();
    if area_error > 1.0e-8 * expected_area.max(1.0) {
        return Err(MeshError::InvalidInput(format!(
            "triangulated area differs from polygon area by {:.6e} m²",
            area_error
        )));
    }
    mesh.validate(usize::from(region.material_id) + 1)?;
    Ok(mesh)
}

/// Bisects the longest edge of each marked triangle. Every triangle sharing a
/// marked edge is split too, so the returned mesh has no hanging midpoint nodes.
pub fn refine_tri_mesh_longest_edges(
    mesh: &TriMesh2D,
    marked_triangles: &[usize],
) -> Result<MeshRefinementResult, MeshError> {
    let material_count = inferred_material_count(mesh);
    let original_stats = mesh.validate(material_count)?;
    if marked_triangles.is_empty() {
        return Err(MeshError::InvalidInput("at least one triangle must be marked for refinement".into()));
    }

    let mut marked_edges = BTreeSet::new();
    for &triangle_id in marked_triangles {
        let triangle = mesh.triangles.get(triangle_id).ok_or_else(|| {
            MeshError::InvalidInput(format!("marked triangle index {triangle_id} is out of range"))
        })?;
        let [a, b, c] = triangle.node_ids;
        let candidates = [
            (edge_key(a, b), distance_squared(mesh.nodes[a].position_m, mesh.nodes[b].position_m)),
            (edge_key(b, c), distance_squared(mesh.nodes[b].position_m, mesh.nodes[c].position_m)),
            (edge_key(c, a), distance_squared(mesh.nodes[c].position_m, mesh.nodes[a].position_m)),
        ];
        // Strict comparison preserves a deterministic AB -> BC -> CA tie break.
        let mut longest = candidates[0];
        for candidate in candidates.into_iter().skip(1) {
            if candidate.1 > longest.1 { longest = candidate; }
        }
        marked_edges.insert(longest.0);
    }

    if marked_edges.is_empty() {
        return Err(MeshError::InvalidInput("refinement selected no edges".into()));
    }
    let mut new_nodes = mesh.nodes.clone();
    let mut midpoint_by_edge = BTreeMap::new();
    for edge in &marked_edges {
        if new_nodes.len() >= MAX_MESH_NODES {
            return Err(MeshError::LimitExceeded("refinement would exceed 150,000 nodes".into()));
        }
        let [a, b] = [edge.0, edge.1];
        let pa = mesh.nodes[a].position_m;
        let pb = mesh.nodes[b].position_m;
        let midpoint = [0.5 * (pa[0] + pb[0]), 0.5 * (pa[1] + pb[1])];
        let id = new_nodes.len();
        new_nodes.push(MeshNode2D { position_m: midpoint });
        midpoint_by_edge.insert(*edge, id);
    }

    let mut projected_triangle_count = 0usize;
    let mut elements_split = 0usize;
    for triangle in &mesh.triangles {
        let [a, b, c] = triangle.node_ids;
        let split_count = (marked_edges.contains(&edge_key(a, b)) as usize)
            + (marked_edges.contains(&edge_key(b, c)) as usize)
            + (marked_edges.contains(&edge_key(c, a)) as usize);
        if split_count > 0 { elements_split += 1; }
        projected_triangle_count = projected_triangle_count.saturating_add(match split_count {
            0 => 1,
            1 => 2,
            2 => 3,
            _ => 4,
        });
    }
    if projected_triangle_count > MAX_MESH_TRIANGLES {
        return Err(MeshError::LimitExceeded("refinement would exceed 250,000 triangles".into()));
    }

    let midpoint = |a: usize, b: usize| midpoint_by_edge.get(&edge_key(a, b)).copied();
    let mut refined = TriMesh2D {
        nodes: new_nodes,
        triangles: Vec::with_capacity(projected_triangle_count),
    };
    for triangle in &mesh.triangles {
        let [a, b, c] = triangle.node_ids;
        let m_ab = midpoint(a, b);
        let m_bc = midpoint(b, c);
        let m_ca = midpoint(c, a);
        let count = (m_ab.is_some() as usize) + (m_bc.is_some() as usize) + (m_ca.is_some() as usize);
        match count {
            0 => push_refined_triangle(&mut refined, triangle, [a, b, c]),
            1 if m_ab.is_some() => {
                let m = m_ab.unwrap();
                push_refined_triangle(&mut refined, triangle, [a, m, c]);
                push_refined_triangle(&mut refined, triangle, [m, b, c]);
            }
            1 if m_bc.is_some() => {
                let m = m_bc.unwrap();
                push_refined_triangle(&mut refined, triangle, [b, m, a]);
                push_refined_triangle(&mut refined, triangle, [m, c, a]);
            }
            1 => {
                let m = m_ca.unwrap();
                push_refined_triangle(&mut refined, triangle, [c, m, b]);
                push_refined_triangle(&mut refined, triangle, [m, a, b]);
            }
            2 if m_ab.is_some() && m_bc.is_some() => {
                let (m0, m1) = (m_ab.unwrap(), m_bc.unwrap());
                push_refined_triangle(&mut refined, triangle, [a, m0, c]);
                push_refined_triangle(&mut refined, triangle, [m0, m1, c]);
                push_refined_triangle(&mut refined, triangle, [m0, b, m1]);
            }
            2 if m_bc.is_some() && m_ca.is_some() => {
                let (m1, m2) = (m_bc.unwrap(), m_ca.unwrap());
                push_refined_triangle(&mut refined, triangle, [b, m1, a]);
                push_refined_triangle(&mut refined, triangle, [m1, m2, a]);
                push_refined_triangle(&mut refined, triangle, [m1, c, m2]);
            }
            2 => {
                let (m2, m0) = (m_ca.unwrap(), m_ab.unwrap());
                push_refined_triangle(&mut refined, triangle, [c, m2, b]);
                push_refined_triangle(&mut refined, triangle, [m2, m0, b]);
                push_refined_triangle(&mut refined, triangle, [m2, a, m0]);
            }
            _ => {
                let (m0, m1, m2) = (m_ab.unwrap(), m_bc.unwrap(), m_ca.unwrap());
                push_refined_triangle(&mut refined, triangle, [a, m0, m2]);
                push_refined_triangle(&mut refined, triangle, [m0, b, m1]);
                push_refined_triangle(&mut refined, triangle, [m2, m1, c]);
                push_refined_triangle(&mut refined, triangle, [m0, m1, m2]);
            }
        }
    }

    let stats = refined.validate(material_count)?;
    let quality = refined.quality_stats()?;
    let original_area_error = (stats.total_area_m2 - original_stats.total_area_m2).abs();
    let area_tolerance = 1.0e-10 * original_stats.total_area_m2.max(1.0e-12);
    let volume_tolerance = 1.0e-10 * original_stats.total_volume_m3.max(1.0e-15);
    if original_area_error > area_tolerance
        || (stats.total_volume_m3 - original_stats.total_volume_m3).abs() > volume_tolerance
    {
        return Err(MeshError::InvalidInput("refinement failed to preserve mesh area or volume".into()));
    }

    Ok(MeshRefinementResult {
        mesh: refined,
        rounds: 1,
        elements_split,
        edges_split: marked_edges.len(),
        stats,
        quality,
    })
}

/// Refines triangles whose centroids lie inside an axis-aligned region until
/// their longest edges meet the requested length. Shared edges are always
/// bisected on both sides to preserve conformity with neighbouring elements.
pub fn refine_tri_mesh_local(
    mesh: &TriMesh2D,
    region_min_m: [f64; 2],
    region_max_m: [f64; 2],
    target_edge_length_m: f64,
    max_rounds: usize,
) -> Result<MeshRefinementResult, MeshError> {
    if region_min_m.iter().chain(region_max_m.iter()).any(|value| !value.is_finite())
        || (0..2).any(|axis| region_min_m[axis] >= region_max_m[axis])
    {
        return Err(MeshError::InvalidInput("local refinement region must have finite, increasing bounds".into()));
    }
    if !target_edge_length_m.is_finite() || target_edge_length_m <= 0.0 {
        return Err(MeshError::InvalidInput("target edge length must be finite and positive".into()));
    }
    if max_rounds == 0 || max_rounds > 16 {
        return Err(MeshError::InvalidInput("maximum refinement rounds must be between 1 and 16".into()));
    }
    let mut current = mesh.clone();
    current.validate(inferred_material_count(&current))?;
    let mut total_split_elements = 0usize;
    let mut total_split_edges = 0usize;
    let mut rounds = 0usize;

    loop {
        let mut selected = Vec::new();
        for (index, triangle) in current.triangles.iter().enumerate() {
            let [a, b, c] = triangle.node_ids;
            let pa = current.nodes[a].position_m;
            let pb = current.nodes[b].position_m;
            let pc = current.nodes[c].position_m;
            let centroid = [
                (pa[0] + pb[0] + pc[0]) / 3.0,
                (pa[1] + pb[1] + pc[1]) / 3.0,
            ];
            let inside = (0..2).all(|axis| centroid[axis] >= region_min_m[axis] && centroid[axis] <= region_max_m[axis]);
            let longest = distance(pa, pb).max(distance(pb, pc)).max(distance(pc, pa));
            if inside && longest > target_edge_length_m * (1.0 + 1.0e-12) {
                selected.push(index);
            }
        }
        if selected.is_empty() { break; }
        if rounds >= max_rounds {
            return Err(MeshError::LimitExceeded(format!(
                "local refinement did not reach {:.6e} m within {max_rounds} rounds",
                target_edge_length_m
            )));
        }
        let refined = refine_tri_mesh_longest_edges(&current, &selected)?;
        total_split_elements = total_split_elements.saturating_add(refined.elements_split);
        total_split_edges = total_split_edges.saturating_add(refined.edges_split);
        current = refined.mesh;
        rounds += 1;
    }

    let material_count = inferred_material_count(&current);
    let stats = current.validate(material_count)?;
    let quality = current.quality_stats()?;
    Ok(MeshRefinementResult {
        mesh: current,
        rounds,
        elements_split: total_split_elements,
        edges_split: total_split_edges,
        stats,
        quality,
    })
}

fn inferred_material_count(mesh: &TriMesh2D) -> usize {
    mesh.triangles.iter().map(|triangle| usize::from(triangle.material_id) + 1).max().unwrap_or(0)
}

fn clean_polygon_ring(points: &[[f64; 2]], label: &str) -> Result<Vec<[f64; 2]>, MeshError> {
    if points.len() < 3 || points.len() > MAX_POLYGON_VERTICES {
        return Err(MeshError::InvalidInput(format!(
            "{label} must contain between 3 and {MAX_POLYGON_VERTICES} vertices"
        )));
    }
    let mut ring = points.to_vec();
    for (index, point) in ring.iter().enumerate() {
        if point.iter().any(|value| !value.is_finite() || value.abs() > MAX_POLYGON_COORDINATE_ABS_M) {
            return Err(MeshError::InvalidInput(format!("{label} vertex {index} has invalid coordinates")));
        }
    }
    if ring.len() >= 2 && distance(ring[0], *ring.last().unwrap()) <= COORDINATE_MERGE_TOLERANCE_M {
        ring.pop();
    }
    if ring.len() < 3 {
        return Err(MeshError::InvalidInput(format!("{label} has fewer than three distinct vertices")));
    }
    for index in 0..ring.len() {
        let next = (index + 1) % ring.len();
        if distance(ring[index], ring[next]) <= COORDINATE_MERGE_TOLERANCE_M {
            return Err(MeshError::InvalidInput(format!("{label} contains consecutive duplicate vertices")));
        }
    }

    // Remove only vertices that lie between two neighbours on the same line.
    // This avoids zero-area ears while preserving genuine corners.
    let mut changed = true;
    while changed && ring.len() > 3 {
        changed = false;
        for index in 0..ring.len() {
            let previous = ring[(index + ring.len() - 1) % ring.len()];
            let current = ring[index];
            let next = ring[(index + 1) % ring.len()];
            let edge_scale = distance(previous, current).max(distance(current, next)).max(1.0);
            let collinear = cross(sub(current, previous), sub(next, current)).abs()
                <= 1.0e-12 * edge_scale * edge_scale;
            let between = dot(sub(current, previous), sub(current, next)) <= OVERLAP_TOLERANCE_M.powi(2);
            if collinear && between {
                ring.remove(index);
                changed = true;
                break;
            }
        }
    }
    if ring.len() < 3 || signed_ring_area(&ring).abs() <= 1.0e-14 {
        return Err(MeshError::InvalidInput(format!("{label} is degenerate or has zero area")));
    }
    Ok(ring)
}

fn signed_ring_area(ring: &[[f64; 2]]) -> f64 {
    let mut twice_area = 0.0;
    let mut compensation = 0.0;
    for index in 0..ring.len() {
        let next = (index + 1) % ring.len();
        let value = cross(ring[index], ring[next]) - compensation;
        let sum = twice_area + value;
        compensation = (sum - twice_area) - value;
        twice_area = sum;
    }
    0.5 * twice_area
}

fn validate_simple_ring(ring: &[[f64; 2]], label: &str) -> Result<(), MeshError> {
    let count = ring.len();
    for first in 0..count {
        let first_next = (first + 1) % count;
        for second in (first + 1)..count {
            let second_next = (second + 1) % count;
            if first == second || first_next == second || second_next == first {
                continue;
            }
            if segments_intersect_or_touch(
                ring[first], ring[first_next], ring[second], ring[second_next],
            ) {
                return Err(MeshError::InvalidInput(format!("{label} self-intersects or touches itself")));
            }
        }
    }
    Ok(())
}

fn rings_intersect_or_touch(first: &[[f64; 2]], second: &[[f64; 2]]) -> bool {
    for first_index in 0..first.len() {
        let first_next = (first_index + 1) % first.len();
        for second_index in 0..second.len() {
            let second_next = (second_index + 1) % second.len();
            if segments_intersect_or_touch(
                first[first_index], first[first_next], second[second_index], second[second_next],
            ) {
                return true;
            }
        }
    }
    false
}

fn segments_intersect_or_touch(
    a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2],
) -> bool {
    let ab_scale = distance(a, b).max(1.0);
    let cd_scale = distance(c, d).max(1.0);
    let eps_ab = OVERLAP_TOLERANCE_M * ab_scale;
    let eps_cd = OVERLAP_TOLERANCE_M * cd_scale;
    let o1 = cross(sub(b, a), sub(c, a));
    let o2 = cross(sub(b, a), sub(d, a));
    let o3 = cross(sub(d, c), sub(a, c));
    let o4 = cross(sub(d, c), sub(b, c));

    if o1.abs() <= eps_ab && point_on_segment(c, a, b) { return true; }
    if o2.abs() <= eps_ab && point_on_segment(d, a, b) { return true; }
    if o3.abs() <= eps_cd && point_on_segment(a, c, d) { return true; }
    if o4.abs() <= eps_cd && point_on_segment(b, c, d) { return true; }
    ((o1 > eps_ab && o2 < -eps_ab) || (o1 < -eps_ab && o2 > eps_ab))
        && ((o3 > eps_cd && o4 < -eps_cd) || (o3 < -eps_cd && o4 > eps_cd))
}

fn point_in_ring(point: [f64; 2], ring: &[[f64; 2]]) -> bool {
    let mut inside = false;
    for index in 0..ring.len() {
        let a = ring[index];
        let b = ring[(index + 1) % ring.len()];
        if point_on_segment(point, a, b) { return false; }
        let crosses_y = (a[1] > point[1]) != (b[1] > point[1]);
        if crosses_y {
            let intersection_x = a[0] + (point[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
            if point[0] < intersection_x { inside = !inside; }
        }
    }
    inside
}

fn edge_key(a: usize, b: usize) -> (usize, usize) {
    if a < b { (a, b) } else { (b, a) }
}

fn distance_squared(a: [f64; 2], b: [f64; 2]) -> f64 {
    let delta = sub(a, b);
    dot(delta, delta)
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    distance_squared(a, b).sqrt()
}

fn angle_from_sides(adjacent_a: f64, adjacent_b: f64, opposite: f64) -> f64 {
    let cosine = ((adjacent_a * adjacent_a + adjacent_b * adjacent_b - opposite * opposite)
        / (2.0 * adjacent_a * adjacent_b)).clamp(-1.0, 1.0);
    cosine.acos()
}

fn push_refined_triangle(mesh: &mut TriMesh2D, source: &MeshTriangle2D, node_ids: [usize; 3]) {
    mesh.triangles.push(MeshTriangle2D {
        node_ids,
        material_id: source.material_id,
        thickness_m: source.thickness_m,
        source_part_id: source.source_part_id.clone(),
    });
}

fn coordinate_key(point: [f64; 2]) -> Result<(i64, i64), MeshError> {
    let scaled_x = (point[0] / COORDINATE_MERGE_TOLERANCE_M).round();
    let scaled_y = (point[1] / COORDINATE_MERGE_TOLERANCE_M).round();
    if !scaled_x.is_finite() || !scaled_y.is_finite()
        || scaled_x.abs() > i64::MAX as f64 || scaled_y.abs() > i64::MAX as f64
    {
        return Err(MeshError::InvalidInput("mesh coordinate is outside the merge-key range".into()));
    }
    Ok((scaled_x as i64, scaled_y as i64))
}

fn rectangle_corners(rect: &PlanarRectangle) -> [[f64; 2]; 4] {
    let (sin, cos) = rect.rotation_rad.sin_cos();
    let local = [
        [-0.5 * rect.size_m[0], -0.5 * rect.size_m[1]],
        [0.5 * rect.size_m[0], -0.5 * rect.size_m[1]],
        [0.5 * rect.size_m[0], 0.5 * rect.size_m[1]],
        [-0.5 * rect.size_m[0], 0.5 * rect.size_m[1]],
    ];
    local.map(|point| [
        rect.center_m[0] + cos * point[0] - sin * point[1],
        rect.center_m[1] + sin * point[0] + cos * point[1],
    ])
}

fn rectangles_overlap(a: &PlanarRectangle, b: &PlanarRectangle) -> bool {
    let corners_a = rectangle_corners(a);
    let corners_b = rectangle_corners(b);
    for corners in [&corners_a, &corners_b] {
        for edge_index in 0..4 {
            let start = corners[edge_index];
            let end = corners[(edge_index + 1) % 4];
            let edge = sub(end, start);
            let axis = [-edge[1], edge[0]];
            let length = dot(axis, axis).sqrt();
            if length <= 0.0 { continue; }
            let axis = [axis[0] / length, axis[1] / length];
            let (min_a, max_a) = project(&corners_a, axis);
            let (min_b, max_b) = project(&corners_b, axis);
            if max_a.min(max_b) - min_a.max(min_b) <= OVERLAP_TOLERANCE_M {
                return false;
            }
        }
    }
    true
}

fn rectangles_have_nonconforming_contact(a: &PlanarRectangle, b: &PlanarRectangle) -> bool {
    let corners_a = rectangle_corners(a);
    let corners_b = rectangle_corners(b);
    let mut has_complete_shared_edge = false;

    for edge_a in 0..4 {
        let a0 = corners_a[edge_a];
        let a1 = corners_a[(edge_a + 1) % 4];
        let direction_a = sub(a1, a0);
        let length_a = dot(direction_a, direction_a).sqrt();
        if length_a <= OVERLAP_TOLERANCE_M { continue; }
        let unit_a = [direction_a[0] / length_a, direction_a[1] / length_a];

        for edge_b in 0..4 {
            let b0 = corners_b[edge_b];
            let b1 = corners_b[(edge_b + 1) % 4];
            let direction_b = sub(b1, b0);
            let length_b = dot(direction_b, direction_b).sqrt();
            if length_b <= OVERLAP_TOLERANCE_M { continue; }

            let parallel = cross(direction_a, direction_b).abs()
                <= ANGLE_TOLERANCE * length_a * length_b;
            let collinear = cross(sub(b0, a0), unit_a).abs() <= OVERLAP_TOLERANCE_M;
            if !parallel || !collinear { continue; }

            let projection_0 = dot(sub(b0, a0), unit_a);
            let projection_1 = dot(sub(b1, a0), unit_a);
            let overlap = length_a.min(projection_0.max(projection_1))
                - 0.0_f64.max(projection_0.min(projection_1));
            if overlap > OVERLAP_TOLERANCE_M {
                let full_a = (overlap - length_a).abs() <= OVERLAP_TOLERANCE_M;
                let full_b = (overlap - length_b).abs() <= OVERLAP_TOLERANCE_M;
                if full_a && full_b {
                    has_complete_shared_edge = true;
                } else {
                    return true;
                }
            }
        }
    }

    let point_contact = corners_a.iter().any(|point| {
        (0..4).any(|edge| point_on_segment(*point, corners_b[edge], corners_b[(edge + 1) % 4]))
    }) || corners_b.iter().any(|point| {
        (0..4).any(|edge| point_on_segment(*point, corners_a[edge], corners_a[(edge + 1) % 4]))
    });

    point_contact && !has_complete_shared_edge
}

fn point_on_segment(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> bool {
    let direction = sub(end, start);
    let length_squared = dot(direction, direction);
    if length_squared <= OVERLAP_TOLERANCE_M * OVERLAP_TOLERANCE_M {
        return dot(sub(point, start), sub(point, start)).sqrt() <= OVERLAP_TOLERANCE_M;
    }
    let length = length_squared.sqrt();
    if cross(sub(point, start), direction).abs() / length > OVERLAP_TOLERANCE_M {
        return false;
    }
    let projection = dot(sub(point, start), direction);
    projection >= -OVERLAP_TOLERANCE_M * length
        && projection <= length_squared + OVERLAP_TOLERANCE_M * length
}

fn project(points: &[[f64; 2]; 4], axis: [f64; 2]) -> (f64, f64) {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for point in points {
        let value = dot(*point, axis);
        min = min.min(value);
        max = max.max(value);
    }
    (min, max)
}

fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] { [a[0] - b[0], a[1] - b[1]] }
fn dot(a: [f64; 2], b: [f64; 2]) -> f64 { a[0] * b[0] + a[1] * b[1] }
fn cross(a: [f64; 2], b: [f64; 2]) -> f64 { a[0] * b[1] - a[1] * b[0] }

#[cfg(test)]
mod tests {
    use super::*;

    fn rectangle(id: &str, center: [f64; 2], size: [f64; 2]) -> PlanarRectangle {
        PlanarRectangle {
            source_part_id: id.into(), center_m: center, size_m: size,
            rotation_rad: 0.0, material_id: 0, thickness_m: 0.01,
        }
    }

    #[test]
    fn single_rectangle_generates_positive_triangles_and_expected_area() {
        let mesh = mesh_planar_rectangles(&[rectangle("plate", [0.0, 0.0], [0.6, 0.4])], 0.1).unwrap();
        let stats = mesh.validate(1).unwrap();
        assert_eq!(stats.triangle_count, 2 * 6 * 4);
        assert!((stats.total_area_m2 - 0.24).abs() < 1.0e-12);
        assert!((stats.total_volume_m3 - 0.0024).abs() < 1.0e-12);
        assert_eq!(stats.boundary_node_count, 2 * (6 + 4));
    }

    #[test]
    fn adjacent_rectangles_share_interface_nodes() {
        let mesh = mesh_planar_rectangles(&[
            rectangle("left", [-0.5, 0.0], [1.0, 1.0]),
            rectangle("right", [0.5, 0.0], [1.0, 1.0]),
        ], 0.25).unwrap();
        let stats = mesh.validate(1).unwrap();
        assert!((stats.total_area_m2 - 2.0).abs() < 1.0e-12);
        assert_eq!(stats.boundary_edge_count, 24);
        assert_eq!(stats.node_count, 45);
    }

    #[test]
    fn overlapping_rectangles_are_rejected() {
        let error = mesh_planar_rectangles(&[
            rectangle("first", [0.0, 0.0], [1.0, 1.0]),
            rectangle("second", [0.4, 0.0], [1.0, 1.0]),
        ], 0.25).unwrap_err();
        assert!(matches!(error, MeshError::OverlappingRegions { .. }));
    }

    #[test]
    fn rotated_rectangles_can_share_a_boundary() {
        let mut first = rectangle("first", [0.0, 0.0], [1.0, 0.5]);
        first.rotation_rad = 0.3;
        let mut second = rectangle("second", [-0.75 * 0.3_f64.sin(), 0.75 * 0.3_f64.cos()], [1.0, 1.0]);
        second.rotation_rad = 0.3;
        // Same orientation and a shared edge, expressed in world coordinates.
        let mesh = mesh_planar_rectangles(&[first, second], 0.2).unwrap();
        assert!(mesh.validate(1).is_ok());
    }

    #[test]
    fn partial_edge_contact_is_rejected_to_avoid_t_junctions() {
        let a = rectangle("full-edge", [0.0, 0.0], [1.0, 1.0]);
        let b = rectangle("partial-edge", [1.0, 0.25], [1.0, 0.5]);
        assert!(matches!(
            mesh_planar_rectangles(&[a, b], 0.2),
            Err(MeshError::NonConformingContact { .. })
        ));
    }

    #[test]
    fn point_only_contact_is_rejected_to_avoid_artificial_thermal_links() {
        let a = rectangle("lower-left", [0.0, 0.0], [1.0, 1.0]);
        let b = rectangle("upper-right", [1.0, 1.0], [1.0, 1.0]);
        assert!(matches!(
            mesh_planar_rectangles(&[a, b], 0.2),
            Err(MeshError::NonConformingContact { .. })
        ));
    }

    fn l_shaped_polygon() -> PlanarPolygonRegion {
        PlanarPolygonRegion {
            source_part_id: "concave-panel".into(),
            outer_boundary_m: vec![
                [0.0, 0.0], [2.0, 0.0], [2.0, 0.8],
                [1.2, 0.8], [1.2, 1.8], [0.0, 1.8],
            ],
            holes_m: Vec::new(),
            material_id: 0,
            thickness_m: 0.012,
        }
    }

    #[test]
    fn concave_polygon_is_triangulated_with_expected_area() {
        let mesh = mesh_planar_polygon(&l_shaped_polygon()).unwrap();
        let stats = mesh.validate(1).unwrap();
        assert!((stats.total_area_m2 - 2.8).abs() < 1.0e-10);
        assert!((stats.total_volume_m3 - 2.8 * 0.012).abs() < 1.0e-10);
        assert!(mesh.triangles.iter().all(|triangle| triangle.source_part_id == "concave-panel"));
    }

    #[test]
    fn polygon_with_one_hole_subtracts_hole_area_and_keeps_inner_boundary() {
        let region = PlanarPolygonRegion {
            source_part_id: "perforated-plate".into(),
            outer_boundary_m: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
            holes_m: vec![vec![[0.2, 0.2], [0.2, 0.5], [0.5, 0.5], [0.5, 0.2]]],
            material_id: 0,
            thickness_m: 0.01,
        };
        let mesh = mesh_planar_polygon(&region).unwrap();
        let stats = mesh.validate(1).unwrap();
        assert!((stats.total_area_m2 - 3.91).abs() < 1.0e-10);
        assert_eq!(stats.boundary_edge_count, 8);
        assert_eq!(stats.boundary_node_count, 8);
    }

    #[test]
    fn polygon_with_multiple_holes_preserves_material_and_volume() {
        let mut region = l_shaped_polygon();
        region.holes_m = vec![
            vec![[0.2, 0.2], [0.6, 0.2], [0.6, 0.6], [0.2, 0.6]],
            vec![[0.7, 1.1], [1.0, 1.1], [1.0, 1.5], [0.7, 1.5]],
        ];
        region.material_id = 1;
        let mesh = mesh_planar_polygon(&region).unwrap();
        let stats = mesh.validate(2).unwrap();
        let expected_area = 2.8 - 0.16 - 0.12;
        assert!((stats.total_area_m2 - expected_area).abs() < 1.0e-10);
        assert!((stats.total_volume_m3 - expected_area * 0.012).abs() < 1.0e-10);
        assert!(mesh.triangles.iter().all(|triangle| triangle.material_id == 1));
    }

    #[test]
    fn polygon_ring_winding_and_repeated_closing_point_are_normalized() {
        let mut region = l_shaped_polygon();
        region.outer_boundary_m.reverse();
        region.outer_boundary_m.push(region.outer_boundary_m[0]);
        let mesh = mesh_planar_polygon(&region).unwrap();
        assert!((mesh.validate(1).unwrap().total_area_m2 - 2.8).abs() < 1.0e-10);
    }

    #[test]
    fn polygon_rejects_hole_outside_outer_boundary() {
        let mut region = l_shaped_polygon();
        region.holes_m = vec![vec![[3.0, 3.0], [3.0, 3.2], [3.2, 3.2], [3.2, 3.0]]];
        assert!(mesh_planar_polygon(&region).unwrap_err().to_string().contains("not strictly inside"));
    }

    #[test]
    fn polygon_rejects_self_intersecting_outer_boundary() {
        let mut region = l_shaped_polygon();
        region.outer_boundary_m = vec![[0.0, 0.0], [3.0, 0.0], [0.0, 2.0], [2.0, 2.0]];
        assert!(mesh_planar_polygon(&region).unwrap_err().to_string().contains("self-intersects"));
    }

    #[test]
    fn polygon_rejects_touching_or_overlapping_holes() {
        let mut region = l_shaped_polygon();
        region.holes_m = vec![
            vec![[0.1, 0.1], [0.5, 0.1], [0.5, 0.5], [0.1, 0.5]],
            vec![[0.5, 0.2], [0.8, 0.2], [0.8, 0.5], [0.5, 0.5]],
        ];
        assert!(mesh_planar_polygon(&region).unwrap_err().to_string().contains("overlap, touch or contain"));
    }

    #[test]
    fn polygon_rejects_hole_crossing_outer_boundary() {
        let mut region = l_shaped_polygon();
        region.holes_m = vec![vec![[0.8, 1.2], [1.4, 1.2], [1.4, 1.5], [0.8, 1.5]]];
        assert!(mesh_planar_polygon(&region).is_err());
    }

    #[test]
    fn bundled_polygon_input_deck_parses_and_meshes() {
        let region: PlanarPolygonRegion = serde_json::from_str(include_str!("../examples/perforated-panel-polygon.json")).unwrap();
        let mesh = mesh_planar_polygon(&region).unwrap();
        let stats = mesh.validate(1).unwrap();
        assert!((stats.total_area_m2 - 2.52).abs() < 1.0e-10);
    }

    #[test]
    fn polygon_region_round_trips_through_json() {
        let region = l_shaped_polygon();
        let json = serde_json::to_vec(&region).unwrap();
        let restored: PlanarPolygonRegion = serde_json::from_slice(&json).unwrap();
        assert_eq!(restored, region);
    }

    #[test]
    fn polygon_removes_collinear_intermediate_vertices() {
        let mut region = l_shaped_polygon();
        region.outer_boundary_m.insert(1, [1.0, 0.0]);
        let mesh = mesh_planar_polygon(&region).unwrap();
        assert!((mesh.validate(1).unwrap().total_area_m2 - 2.8).abs() < 1.0e-10);
    }

    #[test]
    fn invalid_edge_length_is_rejected() {
        assert!(mesh_planar_rectangles(&[rectangle("plate", [0.0, 0.0], [1.0, 1.0])], 0.0).is_err());
    }

    #[test]
    fn refining_one_triangle_splits_a_shared_edge_on_both_sides() {
        let mesh = mesh_planar_rectangles(&[rectangle("plate", [0.0, 0.0], [1.0, 1.0])], 1.0).unwrap();
        let before = mesh.validate(1).unwrap();
        let refined = refine_tri_mesh_longest_edges(&mesh, &[0]).unwrap();
        assert_eq!(refined.edges_split, 1);
        assert_eq!(refined.elements_split, 2);
        assert_eq!(refined.mesh.triangles.len(), 4);
        assert!(refined.mesh.triangles.iter().all(|triangle| {
            triangle.source_part_id == "plate"
                && triangle.material_id == 0
                && (triangle.thickness_m - 0.01).abs() < 1.0e-14
        }));
        assert_eq!(refined.stats.boundary_edge_count, before.boundary_edge_count);
        assert!((refined.stats.total_area_m2 - before.total_area_m2).abs() < 1.0e-12);
        assert!(refined.mesh.validate(1).is_ok());
    }

    #[test]
    fn local_refinement_preserves_area_volume_and_only_refines_the_centre() {
        let mesh = mesh_planar_rectangles(&[rectangle("plate", [0.0, 0.0], [1.0, 1.0])], 0.25).unwrap();
        let before = mesh.validate(1).unwrap();
        let refined = refine_tri_mesh_local(&mesh, [-0.22, -0.22], [0.22, 0.22], 0.12, 8).unwrap();
        assert!(refined.rounds >= 1);
        assert!(refined.stats.triangle_count > before.triangle_count);
        assert!((refined.stats.total_area_m2 - before.total_area_m2).abs() < 1.0e-10);
        assert!((refined.stats.total_volume_m3 - before.total_volume_m3).abs() < 1.0e-10);
        assert_eq!(refined.stats.boundary_edge_count, before.boundary_edge_count);
        let outside_has_coarse_element = refined.mesh.triangles.iter().any(|triangle| {
            let [a, b, c] = triangle.node_ids;
            let pa = refined.mesh.nodes[a].position_m;
            let pb = refined.mesh.nodes[b].position_m;
            let pc = refined.mesh.nodes[c].position_m;
            let centre = [(pa[0] + pb[0] + pc[0]) / 3.0, (pa[1] + pb[1] + pc[1]) / 3.0];
            let outside = centre[0].abs() > 0.22 || centre[1].abs() > 0.22;
            let longest = distance(pa, pb).max(distance(pb, pc)).max(distance(pc, pa));
            outside && longest > 0.12
        });
        assert!(outside_has_coarse_element, "elements outside the local region should retain coarse edges");
    }

    #[test]
    fn triangle_quality_reports_equilateral_reference() {
        let mesh = TriMesh2D {
            nodes: vec![
                MeshNode2D { position_m: [0.0, 0.0] },
                MeshNode2D { position_m: [1.0, 0.0] },
                MeshNode2D { position_m: [0.5, 3.0_f64.sqrt() / 2.0] },
            ],
            triangles: vec![MeshTriangle2D {
                node_ids: [0, 1, 2], material_id: 0, thickness_m: 0.01, source_part_id: "equilateral".into(),
            }],
        };
        let quality = mesh.quality_stats().unwrap();
        assert!((quality.minimum_angle_deg - 60.0).abs() < 1.0e-10);
        assert!((quality.maximum_radius_ratio - 1.0).abs() < 1.0e-10);
        assert!((quality.maximum_edge_length_m - 1.0).abs() < 1.0e-10);
    }

    #[test]
    fn invalid_local_refinement_region_is_rejected() {
        let mesh = mesh_planar_rectangles(&[rectangle("plate", [0.0, 0.0], [1.0, 1.0])], 0.25).unwrap();
        assert!(refine_tri_mesh_local(&mesh, [1.0, 0.0], [0.0, 1.0], 0.1, 4).is_err());
        assert!(refine_tri_mesh_local(&mesh, [-0.2, -0.2], [0.2, 0.2], 0.0, 4).is_err());
        assert!(refine_tri_mesh_local(&mesh, [-0.2, -0.2], [0.2, 0.2], 0.1, 0).is_err());
    }

}
