//! Explicit transient heat conduction on an unstructured 2D triangular mesh.
//!
//! Uses linear triangular finite elements with a lumped heat-capacity matrix.
//! The model is intended for generic planar plates and industrial solids. It is
//! not an automatic high-order or 3D continuum solver.

use crate::mesh::{MeshError, TriMesh2D};
use crate::{Material, SimulationMetadata};
use rayon::ThreadPool;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const MAX_ELEMENTS: usize = 250_000;
const MAX_STEPS: usize = 200_000;
const MAX_NODE_UPDATES: u64 = 2_000_000_000;
const STABILITY_SAFETY_FACTOR: f64 = 0.85;
const MIN_TEMPERATURE_K: f64 = 0.0;
const MAX_TEMPERATURE_K: f64 = 10_000.0;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MeshThermalConfig {
    pub name: String,
    pub mesh: TriMesh2D,
    pub materials: Vec<Material>,
    pub initial_temperature_k: f64,
    pub fixed_boundary_temperature_k: Option<f64>,
    pub time_step_s: f64,
    pub steps: usize,
    /// Uniform volumetric heat source applied to every element, in W/m³.
    pub volumetric_heat_source_w_m3: f64,
}

impl Default for MeshThermalConfig {
    fn default() -> Self {
        let mesh = crate::mesh::mesh_planar_rectangles(
            &[crate::mesh::PlanarRectangle {
                source_part_id: "reference-plate".into(),
                center_m: [0.0, 0.0],
                size_m: [0.6, 0.4],
                rotation_rad: 0.0,
                material_id: 0,
                thickness_m: 0.01,
            }],
            0.05,
        )
        .expect("bundled reference plate geometry must mesh");
        Self {
            name: "Reference planar plate mesh study".into(),
            mesh,
            materials: vec![Material::aluminium()],
            initial_temperature_k: 293.15,
            fixed_boundary_temperature_k: Some(293.15),
            time_step_s: 0.1,
            steps: 500,
            volumetric_heat_source_w_m3: 25_000.0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MeshThermalDiagnostics {
    pub total_area_m2: f64,
    pub total_volume_m3: f64,
    pub total_heat_capacity_j_k: f64,
    pub initial_energy_j: f64,
    pub final_energy_j: f64,
    /// Net energy entering through fixed-temperature boundary nodes.
    pub boundary_energy_j: f64,
    pub source_energy_j: f64,
    pub energy_balance_error_j: f64,
    pub energy_balance_relative_error: f64,
    pub stable_time_step_limit_s: f64,
    pub maximum_row_rate_per_s: f64,
    pub boundary_node_count: usize,
    pub iterations_completed: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MeshThermalResult {
    pub config: MeshThermalConfig,
    pub final_temperature_k: Vec<f64>,
    pub min_temperature_k: f64,
    pub max_temperature_k: f64,
    pub mean_temperature_k: f64,
    pub simulated_time_s: f64,
    pub elapsed_ms: u128,
    pub diagnostics: MeshThermalDiagnostics,
    pub metadata: SimulationMetadata,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MeshThermalError {
    InvalidInput(String),
    Mesh(String),
    Cancelled,
    Numerical(String),
}

impl fmt::Display for MeshThermalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(f, "Invalid mesh thermal input: {message}"),
            Self::Mesh(message) => write!(f, "Invalid mesh: {message}"),
            Self::Cancelled => write!(f, "Mesh thermal simulation cancelled"),
            Self::Numerical(message) => write!(f, "Mesh thermal numerical failure: {message}"),
        }
    }
}

impl std::error::Error for MeshThermalError {}

impl From<MeshError> for MeshThermalError {
    fn from(value: MeshError) -> Self {
        Self::Mesh(value.to_string())
    }
}

#[derive(Debug)]
struct CsrMatrix {
    row_offsets: Vec<usize>,
    columns: Vec<usize>,
    values: Vec<f64>,
}

impl CsrMatrix {
    fn multiply_row(&self, row: usize, vector: &[f64]) -> f64 {
        let start = self.row_offsets[row];
        let end = self.row_offsets[row + 1];
        let mut sum = 0.0;
        for index in start..end {
            sum += self.values[index] * vector[self.columns[index]];
        }
        sum
    }
}

struct AssembledSystem {
    stiffness: CsrMatrix,
    capacities_j_k: Vec<f64>,
    source_power_w: Vec<f64>,
    fixed_nodes: Vec<bool>,
    stats: crate::mesh::MeshStats,
    maximum_row_rate_per_s: f64,
    stable_time_step_limit_s: f64,
}

pub fn simulate_mesh_thermal(
    config: &MeshThermalConfig,
    pool: &ThreadPool,
) -> Result<MeshThermalResult, MeshThermalError> {
    let cancel = AtomicBool::new(false);
    simulate_mesh_thermal_cancellable(config, pool, |_| {}, &cancel)
}

pub fn simulate_mesh_thermal_cancellable<F>(
    config: &MeshThermalConfig,
    pool: &ThreadPool,
    mut progress: F,
    cancel: &AtomicBool,
) -> Result<MeshThermalResult, MeshThermalError>
where
    F: FnMut(f64),
{
    validate_config(config)?;
    if cancel.load(Ordering::Relaxed) {
        return Err(MeshThermalError::Cancelled);
    }
    let start = Instant::now();
    let system = assemble_system(config)?;
    if config.time_step_s > system.stable_time_step_limit_s {
        return Err(MeshThermalError::InvalidInput(format!(
            "time step {:.6e} s exceeds the explicit stability limit {:.6e} s",
            config.time_step_s, system.stable_time_step_limit_s
        )));
    }

    let node_count = config.mesh.nodes.len();
    let mut current = vec![config.initial_temperature_k; node_count];
    if let Some(boundary_temperature) = config.fixed_boundary_temperature_k {
        for (index, fixed) in system.fixed_nodes.iter().enumerate() {
            if *fixed {
                current[index] = boundary_temperature;
            }
        }
    }
    let mut next = vec![config.initial_temperature_k; node_count];
    let initial_energy = compensated_weighted_sum(&system.capacities_j_k, &current);
    let total_source_power = compensated_sum(&system.source_power_w);
    let expected_source_energy_per_step = total_source_power * config.time_step_s;
    let mut source_energy = 0.0;
    let mut boundary_energy = 0.0;
    let progress_stride = (config.steps / 100).max(1);

    for step in 0..config.steps {
        if cancel.load(Ordering::Relaxed) {
            return Err(MeshThermalError::Cancelled);
        }
        // For a fixed-temperature node, K*T - Q is the heat supplied by the
        // prescribed boundary. Summing these reactions gives an independent
        // boundary-energy term for the discrete global balance.
        let mut boundary_power = 0.0;
        for index in 0..node_count {
            if system.fixed_nodes[index] {
                boundary_power += system.stiffness.multiply_row(index, &current) - system.source_power_w[index];
            }
        }
        boundary_energy += boundary_power * config.time_step_s;
        source_energy += expected_source_energy_per_step;

        pool.install(|| {
            next.par_iter_mut().enumerate().for_each(|(node, output)| {
                if system.fixed_nodes[node] {
                    *output = config.fixed_boundary_temperature_k.unwrap_or(current[node]);
                } else {
                    let conductive_power = system.stiffness.multiply_row(node, &current);
                    let net_power = system.source_power_w[node] - conductive_power;
                    *output = current[node] + config.time_step_s * net_power / system.capacities_j_k[node];
                }
            });
        });
        if next
            .iter()
            .any(|value| !value.is_finite() || *value < MIN_TEMPERATURE_K || *value > MAX_TEMPERATURE_K)
        {
            return Err(MeshThermalError::Numerical(format!(
                "temperature left the supported range at step {}",
                step + 1
            )));
        }
        std::mem::swap(&mut current, &mut next);
        if (step + 1) % progress_stride == 0 || step + 1 == config.steps {
            progress((step + 1) as f64 / config.steps as f64);
        }
    }

    let final_energy = compensated_weighted_sum(&system.capacities_j_k, &current);
    let energy_change = final_energy - initial_energy;
    let balance_error = energy_change - source_energy - boundary_energy;
    let energy_scale = initial_energy
        .abs()
        .max(final_energy.abs())
        .max(source_energy.abs())
        .max(boundary_energy.abs())
        .max(1.0);
    let relative_error = balance_error.abs() / energy_scale;
    let total_capacity = compensated_sum(&system.capacities_j_k);
    let min_temperature = current.iter().copied().fold(f64::INFINITY, f64::min);
    let max_temperature = current.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mean_temperature = final_energy / total_capacity;
    let simulated_time = config.time_step_s * config.steps as f64;
    let metadata = SimulationMetadata::new(
        "materials.heat_conduction.unstructured_2d",
        "1.0.0",
        "lumped_p1_fem_explicit_euler_csr",
        "1.0.0",
        "SI",
        config,
    )
    .map_err(MeshThermalError::InvalidInput)?;

    Ok(MeshThermalResult {
        config: config.clone(),
        final_temperature_k: current,
        min_temperature_k: min_temperature,
        max_temperature_k: max_temperature,
        mean_temperature_k: mean_temperature,
        simulated_time_s: simulated_time,
        elapsed_ms: start.elapsed().as_millis(),
        diagnostics: MeshThermalDiagnostics {
            total_area_m2: system.stats.total_area_m2,
            total_volume_m3: system.stats.total_volume_m3,
            total_heat_capacity_j_k: total_capacity,
            initial_energy_j: initial_energy,
            final_energy_j: final_energy,
            boundary_energy_j: boundary_energy,
            source_energy_j: source_energy,
            energy_balance_error_j: balance_error,
            energy_balance_relative_error: relative_error,
            stable_time_step_limit_s: system.stable_time_step_limit_s,
            maximum_row_rate_per_s: system.maximum_row_rate_per_s,
            boundary_node_count: system.stats.boundary_node_count,
            iterations_completed: config.steps,
        },
        metadata,
    })
}

fn validate_config(config: &MeshThermalConfig) -> Result<(), MeshThermalError> {
    if config.name.trim().is_empty() {
        return Err(MeshThermalError::InvalidInput("study name cannot be empty".into()));
    }
    if config.materials.is_empty() || config.materials.len() > 64 {
        return Err(MeshThermalError::InvalidInput(
            "material count must be between 1 and 64".into(),
        ));
    }
    for material in &config.materials {
        if material.name.trim().is_empty() {
            return Err(MeshThermalError::InvalidInput("material name cannot be empty".into()));
        }
        for (name, value) in [
            ("conductivity", material.conductivity_w_m_k),
            ("density", material.density_kg_m3),
            ("specific heat", material.specific_heat_j_kg_k),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(MeshThermalError::InvalidInput(format!(
                    "material {} has invalid {name}",
                    material.name
                )));
            }
        }
    }
    if !config.initial_temperature_k.is_finite()
        || !(MIN_TEMPERATURE_K..=MAX_TEMPERATURE_K).contains(&config.initial_temperature_k)
    {
        return Err(MeshThermalError::InvalidInput(
            "initial temperature is outside the supported range".into(),
        ));
    }
    if let Some(temperature) = config.fixed_boundary_temperature_k
        && (!temperature.is_finite() || !(MIN_TEMPERATURE_K..=MAX_TEMPERATURE_K).contains(&temperature))
    {
        return Err(MeshThermalError::InvalidInput(
            "fixed boundary temperature is outside the supported range".into(),
        ));
    }
    if !config.time_step_s.is_finite() || config.time_step_s <= 0.0 {
        return Err(MeshThermalError::InvalidInput(
            "time step must be finite and positive".into(),
        ));
    }
    if config.steps == 0 || config.steps > MAX_STEPS {
        return Err(MeshThermalError::InvalidInput(format!(
            "step count must be between 1 and {MAX_STEPS}"
        )));
    }
    if !config.volumetric_heat_source_w_m3.is_finite() {
        return Err(MeshThermalError::InvalidInput(
            "volumetric heat source must be finite".into(),
        ));
    }
    if config.mesh.triangles.len() > MAX_ELEMENTS {
        return Err(MeshThermalError::InvalidInput(format!(
            "mesh exceeds the {MAX_ELEMENTS}-triangle solve limit"
        )));
    }
    let node_updates = (config.mesh.nodes.len() as u64).saturating_mul(config.steps as u64);
    if node_updates > MAX_NODE_UPDATES {
        return Err(MeshThermalError::InvalidInput(format!(
            "study requests {node_updates} node updates; limit is {MAX_NODE_UPDATES}"
        )));
    }
    config.mesh.validate(config.materials.len())?;
    Ok(())
}

fn assemble_system(config: &MeshThermalConfig) -> Result<AssembledSystem, MeshThermalError> {
    let mesh = &config.mesh;
    let stats = mesh.validate(config.materials.len())?;
    let node_count = mesh.nodes.len();
    let mut capacities = vec![0.0; node_count];
    let mut source_power = vec![0.0; node_count];
    let mut rows: Vec<Vec<(usize, f64)>> = vec![Vec::with_capacity(8); node_count];

    for (element_index, triangle) in mesh.triangles.iter().enumerate() {
        let [i0, i1, i2] = triangle.node_ids;
        let p0 = mesh.nodes[i0].position_m;
        let p1 = mesh.nodes[i1].position_m;
        let p2 = mesh.nodes[i2].position_m;
        let twice_area = (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p2[0] - p0[0]) * (p1[1] - p0[1]);
        if !twice_area.is_finite() || twice_area <= 1.0e-14 {
            return Err(MeshThermalError::Mesh(format!(
                "triangle {element_index} has invalid signed area"
            )));
        }
        let area = 0.5 * twice_area;
        let material = &config.materials[usize::from(triangle.material_id)];
        let thickness = triangle.thickness_m;
        let conductivity = material.conductivity_w_m_k;
        let capacity_per_area = material.density_kg_m3 * material.specific_heat_j_kg_k * thickness * area / 3.0;
        if !capacity_per_area.is_finite() || capacity_per_area <= 0.0 {
            return Err(MeshThermalError::Numerical(format!(
                "triangle {element_index} has invalid heat capacity"
            )));
        }
        for node in [i0, i1, i2] {
            capacities[node] += capacity_per_area;
            source_power[node] += config.volumetric_heat_source_w_m3 * thickness * area / 3.0;
        }

        let b = [p1[1] - p2[1], p2[1] - p0[1], p0[1] - p1[1]];
        let c = [p2[0] - p1[0], p0[0] - p2[0], p1[0] - p0[0]];
        let ids = [i0, i1, i2];
        let scale = conductivity * thickness / (4.0 * area);
        for local_i in 0..3 {
            for local_j in 0..3 {
                let value = scale * (b[local_i] * b[local_j] + c[local_i] * c[local_j]);
                rows[ids[local_i]].push((ids[local_j], value));
            }
        }
    }
    if capacities.iter().any(|value| !value.is_finite() || *value <= 0.0) {
        return Err(MeshThermalError::Mesh(
            "mesh contains a node with no positive thermal capacity".into(),
        ));
    }

    let mut row_offsets = Vec::with_capacity(node_count + 1);
    let mut columns = Vec::new();
    let mut values = Vec::new();
    row_offsets.push(0);
    for row in &mut rows {
        row.sort_by_key(|entry| entry.0);
        let mut cursor = 0;
        while cursor < row.len() {
            let column = row[cursor].0;
            let mut sum = 0.0;
            while cursor < row.len() && row[cursor].0 == column {
                sum += row[cursor].1;
                cursor += 1;
            }
            if sum.abs() > 1.0e-24 {
                columns.push(column);
                values.push(sum);
            }
        }
        row_offsets.push(columns.len());
    }
    let stiffness = CsrMatrix {
        row_offsets,
        columns,
        values,
    };

    let boundary_ids = mesh.boundary_node_ids()?;
    let mut fixed_nodes = vec![false; node_count];
    if config.fixed_boundary_temperature_k.is_some() {
        for index in boundary_ids {
            fixed_nodes[index] = true;
        }
    }
    let mut maximum_row_rate: f64 = 0.0;
    for node in 0..node_count {
        if fixed_nodes[node] {
            continue;
        }
        let row_sum_abs: f64 = (stiffness.row_offsets[node]..stiffness.row_offsets[node + 1])
            .map(|index| stiffness.values[index].abs())
            .sum();
        maximum_row_rate = maximum_row_rate.max(row_sum_abs / capacities[node]);
    }
    let stable_limit = if maximum_row_rate <= f64::MIN_POSITIVE {
        f64::MAX
    } else {
        STABILITY_SAFETY_FACTOR * 2.0 / maximum_row_rate
    };
    if stable_limit.is_nan() || stable_limit <= 0.0 {
        return Err(MeshThermalError::Numerical(
            "could not calculate a positive stability limit".into(),
        ));
    }
    Ok(AssembledSystem {
        stiffness,
        capacities_j_k: capacities,
        source_power_w: source_power,
        fixed_nodes,
        stats,
        maximum_row_rate_per_s: maximum_row_rate,
        stable_time_step_limit_s: stable_limit,
    })
}

fn compensated_sum(values: &[f64]) -> f64 {
    let mut sum = 0.0;
    let mut compensation = 0.0;
    for value in values {
        let adjusted = *value - compensation;
        let next = sum + adjusted;
        compensation = (next - sum) - adjusted;
        sum = next;
    }
    sum
}

fn compensated_weighted_sum(weights: &[f64], values: &[f64]) -> f64 {
    let mut sum = 0.0;
    let mut compensation = 0.0;
    for (weight, value) in weights.iter().zip(values) {
        let adjusted = weight * value - compensation;
        let next = sum + adjusted;
        compensation = (next - sum) - adjusted;
        sum = next;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{PlanarRectangle, mesh_planar_rectangles};
    use rayon::ThreadPoolBuilder;

    fn pool(threads: usize) -> ThreadPool {
        ThreadPoolBuilder::new().num_threads(threads).build().unwrap()
    }

    fn small_config() -> MeshThermalConfig {
        let mesh = mesh_planar_rectangles(
            &[PlanarRectangle {
                source_part_id: "test-plate".into(),
                center_m: [0.0, 0.0],
                size_m: [0.4, 0.3],
                rotation_rad: 0.0,
                material_id: 0,
                thickness_m: 0.01,
            }],
            0.05,
        )
        .unwrap();
        MeshThermalConfig {
            name: "small test plate".into(),
            mesh,
            materials: vec![Material::aluminium()],
            initial_temperature_k: 293.15,
            fixed_boundary_temperature_k: Some(293.15),
            time_step_s: 0.05,
            steps: 100,
            volumetric_heat_source_w_m3: 5_000.0,
        }
    }

    #[test]
    fn uniform_field_remains_uniform_without_source() {
        let mut config = small_config();
        config.fixed_boundary_temperature_k = None;
        config.volumetric_heat_source_w_m3 = 0.0;
        let result = simulate_mesh_thermal(&config, &pool(2)).unwrap();
        assert!(
            result
                .final_temperature_k
                .iter()
                .all(|value| (*value - 293.15).abs() < 1.0e-12)
        );
        assert!(result.diagnostics.energy_balance_relative_error < 1.0e-12);
    }

    #[test]
    fn insulated_mesh_matches_uniform_source_energy_rise() {
        let mut config = small_config();
        config.fixed_boundary_temperature_k = None;
        let result = simulate_mesh_thermal(&config, &pool(2)).unwrap();
        let expected_energy = config.volumetric_heat_source_w_m3
            * result.diagnostics.total_volume_m3
            * config.time_step_s
            * config.steps as f64;
        assert!(
            (result.diagnostics.final_energy_j - result.diagnostics.initial_energy_j - expected_energy).abs() < 1.0e-8
        );
        assert!(result.diagnostics.energy_balance_relative_error < 1.0e-10);
        assert!(result.max_temperature_k > config.initial_temperature_k);
    }

    #[test]
    fn fixed_boundary_keeps_edge_nodes_at_reference_temperature() {
        let config = small_config();
        let boundary = config.mesh.boundary_node_ids().unwrap();
        let result = simulate_mesh_thermal(&config, &pool(2)).unwrap();
        for node in boundary {
            assert_eq!(
                result.final_temperature_k[node],
                config.fixed_boundary_temperature_k.unwrap()
            );
        }
        assert!(result.max_temperature_k > result.min_temperature_k);
    }

    #[test]
    fn one_and_four_threads_produce_identical_fields() {
        let config = small_config();
        let one = simulate_mesh_thermal(&config, &pool(1)).unwrap();
        let four = simulate_mesh_thermal(&config, &pool(4)).unwrap();
        assert_eq!(one.final_temperature_k, four.final_temperature_k);
    }

    #[test]
    fn polygon_with_holes_can_be_solved_by_the_triangular_thermal_solver() {
        let region = crate::PlanarPolygonRegion {
            source_part_id: "perforated-test-panel".into(),
            outer_boundary_m: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 0.8], [1.2, 0.8], [1.2, 1.8], [0.0, 1.8]],
            holes_m: vec![
                vec![[0.2, 0.2], [0.6, 0.2], [0.6, 0.6], [0.2, 0.6]],
                vec![[0.7, 1.1], [1.0, 1.1], [1.0, 1.5], [0.7, 1.5]],
            ],
            material_id: 0,
            thickness_m: 0.012,
        };
        let mesh = crate::mesh_planar_polygon(&region).unwrap();
        let mut config = small_config();
        config.mesh = mesh;
        config.time_step_s = 0.01;
        config.steps = 10;
        config.fixed_boundary_temperature_k = None;
        let result = simulate_mesh_thermal(&config, &pool(2)).unwrap();
        assert!((result.diagnostics.total_area_m2 - 2.52).abs() < 1.0e-10);
        assert!(result.max_temperature_k > config.initial_temperature_k);
        assert!(result.diagnostics.energy_balance_relative_error < 1.0e-10);
    }

    #[test]
    fn unstable_time_step_is_rejected_before_time_loop() {
        let mut config = small_config();
        config.time_step_s = 1.0e9;
        assert!(matches!(
            simulate_mesh_thermal(&config, &pool(1)),
            Err(MeshThermalError::InvalidInput(_))
        ));
    }

    #[test]
    fn cancellation_is_cooperative() {
        let config = small_config();
        let cancelled = AtomicBool::new(true);
        assert_eq!(
            simulate_mesh_thermal_cancellable(&config, &pool(1), |_| {}, &cancelled).unwrap_err(),
            MeshThermalError::Cancelled
        );
    }

    #[test]
    fn bundled_planar_mesh_study_parses_and_validates() {
        let config: MeshThermalConfig =
            serde_json::from_str(include_str!("../examples/planar-mesh-thermal-study.json")).unwrap();
        validate_config(&config).unwrap();
        assert_eq!(config.mesh.triangles.len(), 32);
        assert_eq!(config.materials.len(), 2);
    }

    #[test]
    fn result_round_trips_through_json() {
        let result = simulate_mesh_thermal(&small_config(), &pool(1)).unwrap();
        let json = serde_json::to_vec(&result).unwrap();
        let restored: MeshThermalResult = serde_json::from_slice(&json).unwrap();
        assert_eq!(restored, result);
    }
}
