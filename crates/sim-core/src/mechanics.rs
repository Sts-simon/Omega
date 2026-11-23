//! Linear-elastic 2D pin-jointed truss solver for educational civil structures.
//!
//! The model is intentionally limited to straight axial bar elements. It does not
//! model bending, buckling, joints with eccentricity, plasticity, fatigue, dynamics,
//! shells, or 3D structures. Results are numerical study outputs, not certification.

use crate::SimulationMetadata;
use rayon::prelude::*;
use rayon::ThreadPool;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const MIN_NODES: usize = 2;
const MAX_NODES: usize = 2_000;
const MAX_ELEMENTS: usize = 10_000;
const MAX_COORDINATE_M: f64 = 1.0e6;
const MAX_FORCE_N: f64 = 1.0e12;
const MAX_AREA_M2: f64 = 1.0e4;
const MAX_YOUNGS_MODULUS_PA: f64 = 1.0e15;
const MIN_ELEMENT_LENGTH_M: f64 = 1.0e-9;
const MAX_CG_ITERATIONS: usize = 100_000;
const MAX_MATRIX_VECTOR_ENTRY_UPDATES: u128 = 1_000_000_000;

/// A node in a two-dimensional civil truss model. Coordinates are in metres.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TrussNode {
    pub id: u32,
    pub x_m: f64,
    pub y_m: f64,
}

/// A straight, pin-jointed axial member between two nodes.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TrussElement {
    pub id: u32,
    pub node_a: u32,
    pub node_b: u32,
    pub cross_section_area_m2: f64,
    pub youngs_modulus_pa: f64,
}

/// Fixed translational degrees of freedom at a node.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TrussSupport {
    pub node_id: u32,
    pub fixed_x: bool,
    pub fixed_y: bool,
}

/// Concentrated external nodal force. Positive X is right; positive Y is up.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TrussLoad {
    pub node_id: u32,
    pub force_x_n: f64,
    pub force_y_n: f64,
}

/// Input deck for a small-strain, linear-elastic 2D truss analysis.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TrussConfig {
    pub name: String,
    pub nodes: Vec<TrussNode>,
    pub elements: Vec<TrussElement>,
    pub supports: Vec<TrussSupport>,
    pub loads: Vec<TrussLoad>,
    /// Relative Euclidean residual target for preconditioned conjugate gradients.
    pub relative_tolerance: f64,
    pub max_iterations: usize,
}

impl Default for TrussConfig {
    /// Small symmetric triangular civil support frame with an analytical solution.
    fn default() -> Self {
        Self {
            name: "Symmetric triangular support-frame demonstrator".to_string(),
            nodes: vec![
                TrussNode { id: 1, x_m: 0.0, y_m: 0.0 },
                TrussNode { id: 2, x_m: 2.0, y_m: 0.0 },
                TrussNode { id: 3, x_m: 1.0, y_m: 1.0 },
            ],
            elements: vec![
                TrussElement {
                    id: 1,
                    node_a: 1,
                    node_b: 2,
                    cross_section_area_m2: 1.0e-3,
                    youngs_modulus_pa: 200.0e9,
                },
                TrussElement {
                    id: 2,
                    node_a: 1,
                    node_b: 3,
                    cross_section_area_m2: 1.0e-3,
                    youngs_modulus_pa: 200.0e9,
                },
                TrussElement {
                    id: 3,
                    node_a: 2,
                    node_b: 3,
                    cross_section_area_m2: 1.0e-3,
                    youngs_modulus_pa: 200.0e9,
                },
            ],
            supports: vec![
                TrussSupport { node_id: 1, fixed_x: true, fixed_y: true },
                TrussSupport { node_id: 2, fixed_x: false, fixed_y: true },
            ],
            loads: vec![TrussLoad { node_id: 3, force_x_n: 0.0, force_y_n: -1_000.0 }],
            relative_tolerance: 1.0e-10,
            max_iterations: 2_000,
        }
    }
}

impl TrussConfig {
    /// Validate IDs, geometry, section properties, loads, supports and solver controls.
    pub fn validate(&self) -> Result<(), TrussError> {
        if self.name.trim().is_empty() {
            return Err(TrussError::InvalidInput("Study name cannot be empty".to_string()));
        }
        if !(MIN_NODES..=MAX_NODES).contains(&self.nodes.len()) {
            return Err(TrussError::InvalidInput(format!(
                "Node count must be between {MIN_NODES} and {MAX_NODES}"
            )));
        }
        if self.elements.is_empty() || self.elements.len() > MAX_ELEMENTS {
            return Err(TrussError::InvalidInput(format!(
                "Element count must be between 1 and {MAX_ELEMENTS}"
            )));
        }
        if !self.relative_tolerance.is_finite()
            || !(1.0e-13..=1.0e-3).contains(&self.relative_tolerance)
        {
            return Err(TrussError::InvalidInput(
                "Relative tolerance must be between 1e-13 and 1e-3".to_string(),
            ));
        }
        if self.max_iterations == 0 || self.max_iterations > MAX_CG_ITERATIONS {
            return Err(TrussError::InvalidInput(format!(
                "Maximum iterations must be between 1 and {MAX_CG_ITERATIONS}"
            )));
        }

        let mut node_index = HashMap::with_capacity(self.nodes.len());
        for (index, node) in self.nodes.iter().enumerate() {
            if node_index.insert(node.id, index).is_some() {
                return Err(TrussError::InvalidInput(format!(
                    "Node identifier {} is duplicated", node.id
                )));
            }
            if !node.x_m.is_finite()
                || !node.y_m.is_finite()
                || node.x_m.abs() > MAX_COORDINATE_M
                || node.y_m.abs() > MAX_COORDINATE_M
            {
                return Err(TrussError::InvalidInput(format!(
                    "Node {} has non-finite or out-of-range coordinates", node.id
                )));
            }
        }

        let mut element_ids = HashSet::with_capacity(self.elements.len());
        for element in &self.elements {
            if !element_ids.insert(element.id) {
                return Err(TrussError::InvalidInput(format!(
                    "Element identifier {} is duplicated", element.id
                )));
            }
            if element.node_a == element.node_b {
                return Err(TrussError::InvalidInput(format!(
                    "Element {} connects a node to itself", element.id
                )));
            }
            let Some(&a) = node_index.get(&element.node_a) else {
                return Err(TrussError::InvalidInput(format!(
                    "Element {} references missing node {}", element.id, element.node_a
                )));
            };
            let Some(&b) = node_index.get(&element.node_b) else {
                return Err(TrussError::InvalidInput(format!(
                    "Element {} references missing node {}", element.id, element.node_b
                )));
            };
            let dx = self.nodes[b].x_m - self.nodes[a].x_m;
            let dy = self.nodes[b].y_m - self.nodes[a].y_m;
            let length = dx.hypot(dy);
            if !length.is_finite() || length < MIN_ELEMENT_LENGTH_M {
                return Err(TrussError::InvalidInput(format!(
                    "Element {} has zero or numerically negligible length", element.id
                )));
            }
            if !element.cross_section_area_m2.is_finite()
                || element.cross_section_area_m2 <= 0.0
                || element.cross_section_area_m2 > MAX_AREA_M2
            {
                return Err(TrussError::InvalidInput(format!(
                    "Element {} has an invalid cross-sectional area", element.id
                )));
            }
            if !element.youngs_modulus_pa.is_finite()
                || element.youngs_modulus_pa <= 0.0
                || element.youngs_modulus_pa > MAX_YOUNGS_MODULUS_PA
            {
                return Err(TrussError::InvalidInput(format!(
                    "Element {} has an invalid Young's modulus", element.id
                )));
            }
            let axial_rigidity = element.cross_section_area_m2 * element.youngs_modulus_pa;
            if !axial_rigidity.is_finite() {
                return Err(TrussError::InvalidInput(format!(
                    "Element {} axial rigidity is outside the supported range", element.id
                )));
            }
        }

        let mut support_nodes = HashSet::new();
        let mut has_fixed_x = false;
        let mut has_fixed_y = false;
        for support in &self.supports {
            if !node_index.contains_key(&support.node_id) {
                return Err(TrussError::InvalidInput(format!(
                    "Support references missing node {}", support.node_id
                )));
            }
            if !support.fixed_x && !support.fixed_y {
                return Err(TrussError::InvalidInput(format!(
                    "Support at node {} fixes no degrees of freedom", support.node_id
                )));
            }
            if !support_nodes.insert(support.node_id) {
                return Err(TrussError::InvalidInput(format!(
                    "Node {} has duplicate support definitions", support.node_id
                )));
            }
            has_fixed_x |= support.fixed_x;
            has_fixed_y |= support.fixed_y;
        }
        if self.supports.is_empty() || !has_fixed_x || !has_fixed_y {
            return Err(TrussError::InvalidInput(
                "At least one X restraint and one Y restraint are required".to_string(),
            ));
        }

        let mut has_nonzero_load = false;
        for load in &self.loads {
            if !node_index.contains_key(&load.node_id) {
                return Err(TrussError::InvalidInput(format!(
                    "Load references missing node {}", load.node_id
                )));
            }
            if !load.force_x_n.is_finite()
                || !load.force_y_n.is_finite()
                || load.force_x_n.abs() > MAX_FORCE_N
                || load.force_y_n.abs() > MAX_FORCE_N
            {
                return Err(TrussError::InvalidInput(format!(
                    "Load at node {} is non-finite or exceeds the prototype force limit", load.node_id
                )));
            }
            has_nonzero_load |= load.force_x_n != 0.0 || load.force_y_n != 0.0;
        }
        if !has_nonzero_load {
            return Err(TrussError::InvalidInput(
                "At least one non-zero external load is required".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TrussNodeResult {
    pub node_id: u32,
    pub displacement_x_m: f64,
    pub displacement_y_m: f64,
    /// Reactions are near zero at free degrees of freedom; restrained values are support reactions.
    pub reaction_x_n: f64,
    pub reaction_y_n: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TrussElementResult {
    pub element_id: u32,
    pub length_m: f64,
    /// Positive is tension; negative is compression.
    pub axial_force_n: f64,
    /// Positive is tension; negative is compression.
    pub axial_stress_pa: f64,
    pub axial_strain: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TrussDiagnostics {
    pub matrix_nonzero_count: usize,
    pub iterations: usize,
    pub residual_norm_n: f64,
    pub relative_residual: f64,
    pub strain_energy_j: f64,
    pub external_work_j: f64,
    pub energy_balance_error_j: f64,
    pub energy_balance_relative_error: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TrussResult {
    pub metadata: SimulationMetadata,
    pub config: TrussConfig,
    pub elapsed_ms: u128,
    pub nodes: Vec<TrussNodeResult>,
    pub elements: Vec<TrussElementResult>,
    pub max_displacement_m: f64,
    pub max_absolute_stress_pa: f64,
    pub max_absolute_axial_force_n: f64,
    pub diagnostics: TrussDiagnostics,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TrussError {
    InvalidInput(String),
    MechanismOrSingularSystem(String),
    NotConverged { iterations: usize, relative_residual: f64 },
    NonPhysicalResult(String),
    Cancelled,
}

impl std::fmt::Display for TrussError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "{message}"),
            Self::MechanismOrSingularSystem(message) => write!(formatter, "Unstable or singular truss: {message}"),
            Self::NotConverged { iterations, relative_residual } => write!(
                formatter,
                "Conjugate-gradient solver did not converge after {iterations} iterations (relative residual {relative_residual:.3e})"
            ),
            Self::NonPhysicalResult(message) => write!(formatter, "Structural solver stopped: {message}"),
            Self::Cancelled => write!(formatter, "Structural study cancelled by user"),
        }
    }
}

impl std::error::Error for TrussError {}

#[derive(Clone)]
struct SparseMatrix {
    rows: Vec<Vec<(usize, f64)>>,
    diagonal: Vec<f64>,
}

impl SparseMatrix {
    fn from_rows(rows: Vec<BTreeMap<usize, f64>>) -> Result<Self, TrussError> {
        let mut diagonal = vec![0.0; rows.len()];
        let rows: Vec<Vec<(usize, f64)>> = rows
            .into_iter()
            .enumerate()
            .map(|(row_index, entries)| {
                let compact: Vec<(usize, f64)> = entries.into_iter().collect();
                diagonal[row_index] = compact
                    .iter()
                    .find_map(|(column, value)| (*column == row_index).then_some(*value))
                    .unwrap_or(0.0);
                compact
            })
            .collect();
        if diagonal.iter().any(|value| !value.is_finite() || *value <= 0.0)
            || rows.iter().flatten().any(|(_, value)| !value.is_finite())
        {
            return Err(TrussError::MechanismOrSingularSystem(
                "The assembled stiffness has a non-finite entry or a free degree of freedom with no positive stiffness".to_string(),
            ));
        }
        Ok(Self { rows, diagonal })
    }

    fn multiply(&self, vector: &[f64], pool: &ThreadPool) -> Vec<f64> {
        pool.install(|| {
            self.rows
                .par_iter()
                .map(|row| {
                    row.iter()
                        .map(|(column, value)| *value * vector[*column])
                        .sum::<f64>()
                })
                .collect()
        })
    }
}

/// Solve the linear-elastic static equilibrium of a 2D civil truss.
///
/// The sparse global stiffness matrix is assembled once and solved with Jacobi-
/// preconditioned conjugate gradients. Rows are independent during matrix-vector
/// products; each row retains a stable column order to keep results reproducible.
pub fn solve_truss<F>(
    config: &TrussConfig,
    pool: &ThreadPool,
    progress: F,
) -> Result<TrussResult, TrussError>
where
    F: FnMut(f64),
{
    let cancel = AtomicBool::new(false);
    solve_truss_cancellable(config, pool, progress, &cancel)
}

/// Cancellable form of [`solve_truss`]. Cancellation is checked during assembly
/// and between conjugate-gradient iterations.
pub fn solve_truss_cancellable<F>(
    config: &TrussConfig,
    pool: &ThreadPool,
    mut progress: F,
    cancel: &AtomicBool,
) -> Result<TrussResult, TrussError>
where
    F: FnMut(f64),
{
    let started = Instant::now();
    if cancel.load(Ordering::Relaxed) {
        return Err(TrussError::Cancelled);
    }
    config.validate()?;
    progress(0.0);

    let node_index: HashMap<u32, usize> = config
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id, index))
        .collect();
    let degree_count = config.nodes.len() * 2;
    let mut fixed = vec![false; degree_count];
    for support in &config.supports {
        let index = node_index[&support.node_id];
        fixed[index * 2] = support.fixed_x;
        fixed[index * 2 + 1] = support.fixed_y;
    }

    let mut free_map = vec![None; degree_count];
    let mut free_count = 0;
    for dof in 0..degree_count {
        if !fixed[dof] {
            free_map[dof] = Some(free_count);
            free_count += 1;
        }
    }
    if free_count == 0 {
        return Err(TrussError::InvalidInput(
            "All degrees of freedom are restrained; no displacement can be solved".to_string(),
        ));
    }

    let mut matrix_rows = vec![BTreeMap::<usize, f64>::new(); free_count];
    for (element_index, element) in config.elements.iter().enumerate() {
        if element_index % 64 == 0 && cancel.load(Ordering::Relaxed) {
            return Err(TrussError::Cancelled);
        }
        let index_a = node_index[&element.node_a];
        let index_b = node_index[&element.node_b];
        let node_a = &config.nodes[index_a];
        let node_b = &config.nodes[index_b];
        let dx = node_b.x_m - node_a.x_m;
        let dy = node_b.y_m - node_a.y_m;
        let length = dx.hypot(dy);
        let direction = [dx / length, dy / length, -dx / length, -dy / length];
        let local_dofs = [index_a * 2, index_a * 2 + 1, index_b * 2, index_b * 2 + 1];
        let axial_rigidity_per_length =
            element.youngs_modulus_pa * element.cross_section_area_m2 / length;
        for local_row in 0..4 {
            let Some(global_row) = free_map[local_dofs[local_row]] else {
                continue;
            };
            for local_column in 0..4 {
                let Some(global_column) = free_map[local_dofs[local_column]] else {
                    continue;
                };
                let value = axial_rigidity_per_length
                    * direction[local_row]
                    * direction[local_column];
                *matrix_rows[global_row].entry(global_column).or_insert(0.0) += value;
            }
        }
    }
    let matrix = SparseMatrix::from_rows(matrix_rows)?;
    let matrix_nonzero_count: usize = matrix.rows.iter().map(Vec::len).sum();
    let estimated_matrix_vector_updates =
        (matrix_nonzero_count as u128) * (config.max_iterations as u128);
    if estimated_matrix_vector_updates > MAX_MATRIX_VECTOR_ENTRY_UPDATES {
        return Err(TrussError::InvalidInput(format!(
            "Solver budget exceeded: {matrix_nonzero_count} sparse entries x {} iterations; limit is {MAX_MATRIX_VECTOR_ENTRY_UPDATES} matrix-vector entry updates",
            config.max_iterations
        )));
    }

    let mut full_load = vec![0.0_f64; degree_count];
    for load in &config.loads {
        let index = node_index[&load.node_id];
        full_load[index * 2] += load.force_x_n;
        full_load[index * 2 + 1] += load.force_y_n;
    }
    let mut rhs = vec![0.0_f64; free_count];
    for (global_dof, mapped) in free_map.iter().enumerate() {
        if let Some(free_dof) = mapped {
            rhs[*free_dof] = full_load[global_dof];
        }
    }

    if cancel.load(Ordering::Relaxed) {
        return Err(TrussError::Cancelled);
    }
    let (free_displacements, iterations, residual_norm, relative_residual) =
        conjugate_gradient(&matrix, &rhs, config, pool, cancel)?;
    progress(0.85);

    let mut full_displacements = vec![0.0_f64; degree_count];
    for (global_dof, mapped) in free_map.iter().enumerate() {
        if let Some(free_dof) = mapped {
            full_displacements[global_dof] = free_displacements[*free_dof];
        }
    }

    let mut internal_forces = vec![0.0_f64; degree_count];
    let mut element_results = Vec::with_capacity(config.elements.len());
    let mut strain_energy = KahanSum::default();
    let mut max_absolute_stress_pa = 0.0_f64;
    let mut max_absolute_axial_force_n = 0.0_f64;

    for element in &config.elements {
        let index_a = node_index[&element.node_a];
        let index_b = node_index[&element.node_b];
        let node_a = &config.nodes[index_a];
        let node_b = &config.nodes[index_b];
        let dx = node_b.x_m - node_a.x_m;
        let dy = node_b.y_m - node_a.y_m;
        let length = dx.hypot(dy);
        let c = dx / length;
        let s = dy / length;
        let extension = c * (full_displacements[index_b * 2] - full_displacements[index_a * 2])
            + s * (full_displacements[index_b * 2 + 1] - full_displacements[index_a * 2 + 1]);
        let strain = extension / length;
        let stress = element.youngs_modulus_pa * strain;
        let axial_force = stress * element.cross_section_area_m2;
        let force_x = axial_force * c;
        let force_y = axial_force * s;

        // Internal nodal force vector for a two-node axial bar.
        internal_forces[index_a * 2] -= force_x;
        internal_forces[index_a * 2 + 1] -= force_y;
        internal_forces[index_b * 2] += force_x;
        internal_forces[index_b * 2 + 1] += force_y;
        strain_energy.add(0.5 * axial_force * extension);
        max_absolute_stress_pa = max_absolute_stress_pa.max(stress.abs());
        max_absolute_axial_force_n = max_absolute_axial_force_n.max(axial_force.abs());

        element_results.push(TrussElementResult {
            element_id: element.id,
            length_m: length,
            axial_force_n: axial_force,
            axial_stress_pa: stress,
            axial_strain: strain,
        });
    }

    let mut node_results = Vec::with_capacity(config.nodes.len());
    let mut max_displacement_m = 0.0_f64;
    let mut external_work = KahanSum::default();
    for (index, node) in config.nodes.iter().enumerate() {
        let ux = full_displacements[index * 2];
        let uy = full_displacements[index * 2 + 1];
        max_displacement_m = max_displacement_m.max(ux.hypot(uy));
        external_work.add(0.5 * (full_load[index * 2] * ux + full_load[index * 2 + 1] * uy));
        node_results.push(TrussNodeResult {
            node_id: node.id,
            displacement_x_m: ux,
            displacement_y_m: uy,
            reaction_x_n: internal_forces[index * 2] - full_load[index * 2],
            reaction_y_n: internal_forces[index * 2 + 1] - full_load[index * 2 + 1],
        });
    }

    let strain_energy_j = strain_energy.value();
    let external_work_j = external_work.value();
    let energy_balance_error_j = strain_energy_j - external_work_j;
    let energy_balance_relative_error = energy_balance_error_j.abs()
        / strain_energy_j.abs().max(external_work_j.abs()).max(1.0e-30);
    let non_finite_node_result = node_results.iter().any(|node| {
        !node.displacement_x_m.is_finite()
            || !node.displacement_y_m.is_finite()
            || !node.reaction_x_n.is_finite()
            || !node.reaction_y_n.is_finite()
    });
    let non_finite_element_result = element_results.iter().any(|element| {
        !element.length_m.is_finite()
            || !element.axial_force_n.is_finite()
            || !element.axial_stress_pa.is_finite()
            || !element.axial_strain.is_finite()
    });
    if !max_displacement_m.is_finite()
        || !max_absolute_stress_pa.is_finite()
        || !max_absolute_axial_force_n.is_finite()
        || !strain_energy_j.is_finite()
        || !external_work_j.is_finite()
        || !energy_balance_relative_error.is_finite()
        || non_finite_node_result
        || non_finite_element_result
    {
        return Err(TrussError::NonPhysicalResult(
            "A displacement, force, stress or energy diagnostic is non-finite".to_string(),
        ));
    }

    let metadata = SimulationMetadata::new(
        "civil_structures.linear_elastic_2d_truss",
        "1.0.0",
        "sparse_jacobi_preconditioned_conjugate_gradient",
        "1.0.0",
        "SI",
        config,
    )
    .map_err(TrussError::InvalidInput)?;

    progress(1.0);
    Ok(TrussResult {
        metadata,
        config: config.clone(),
        elapsed_ms: started.elapsed().as_millis(),
        nodes: node_results,
        elements: element_results,
        max_displacement_m,
        max_absolute_stress_pa,
        max_absolute_axial_force_n,
        diagnostics: TrussDiagnostics {
            matrix_nonzero_count,
            iterations,
            residual_norm_n: residual_norm,
            relative_residual,
            strain_energy_j,
            external_work_j,
            energy_balance_error_j,
            energy_balance_relative_error,
        },
    })
}

fn conjugate_gradient(
    matrix: &SparseMatrix,
    rhs: &[f64],
    config: &TrussConfig,
    pool: &ThreadPool,
    cancel: &AtomicBool,
) -> Result<(Vec<f64>, usize, f64, f64), TrussError> {
    let rhs_norm = dot(rhs, rhs).sqrt();
    if !rhs_norm.is_finite() || rhs_norm <= 1.0e-20 {
        return Err(TrussError::InvalidInput(
            "Reduced force vector is zero or numerically negligible".to_string(),
        ));
    }
    let target = config.relative_tolerance * rhs_norm;
    let mut x = vec![0.0_f64; rhs.len()];
    let mut residual = rhs.to_vec();
    let mut z: Vec<f64> = residual
        .iter()
        .zip(&matrix.diagonal)
        .map(|(value, diagonal)| value / diagonal)
        .collect();
    let mut direction = z.clone();
    let mut rz = dot(&residual, &z);
    let mut residual_norm = rhs_norm;

    for iteration in 1..=config.max_iterations {
        if cancel.load(Ordering::Relaxed) {
            return Err(TrussError::Cancelled);
        }
        let product = matrix.multiply(&direction, pool);
        let denominator = dot(&direction, &product);
        if !denominator.is_finite() || denominator <= 0.0 {
            return Err(TrussError::MechanismOrSingularSystem(
                "The stiffness matrix is not positive definite; check supports and member layout".to_string(),
            ));
        }
        let alpha = rz / denominator;
        if !alpha.is_finite() {
            return Err(TrussError::NonPhysicalResult(
                "Conjugate-gradient step length is non-finite".to_string(),
            ));
        }
        pool.install(|| {
            x.par_iter_mut()
                .zip(direction.par_iter())
                .for_each(|(value, step)| *value += alpha * step);
            residual
                .par_iter_mut()
                .zip(product.par_iter())
                .for_each(|(value, applied)| *value -= alpha * applied);
        });

        // Recompute the true residual periodically to limit recurrence drift.
        let recomputed_residual = iteration % 32 == 0;
        if recomputed_residual {
            let applied = matrix.multiply(&x, pool);
            pool.install(|| {
                residual
                    .par_iter_mut()
                    .zip(rhs.par_iter().zip(applied.par_iter()))
                    .for_each(|(value, (load, stiffness))| *value = load - stiffness);
            });
        }

        residual_norm = dot(&residual, &residual).sqrt();
        if !residual_norm.is_finite() {
            return Err(TrussError::NonPhysicalResult(
                "Conjugate-gradient residual became non-finite".to_string(),
            ));
        }
        if residual_norm <= target {
            return Ok((x, iteration, residual_norm, residual_norm / rhs_norm));
        }

        z = residual
            .iter()
            .zip(&matrix.diagonal)
            .map(|(value, diagonal)| value / diagonal)
            .collect();
        let next_rz = dot(&residual, &z);
        if !next_rz.is_finite() || next_rz < 0.0 || rz <= 0.0 {
            return Err(TrussError::MechanismOrSingularSystem(
                "Preconditioned residual lost positive definiteness".to_string(),
            ));
        }
        if recomputed_residual {
            // Residual replacement breaks the old conjugacy relation. Restart
            // from the freshly preconditioned residual rather than reusing p.
            direction.clone_from(&z);
        } else {
            let beta = next_rz / rz;
            pool.install(|| {
                direction
                    .par_iter_mut()
                    .zip(z.par_iter())
                    .for_each(|(value, preconditioned)| *value = preconditioned + beta * *value);
            });
        }
        rz = next_rz;
    }

    Err(TrussError::NotConverged {
        iterations: config.max_iterations,
        relative_residual: residual_norm / rhs_norm,
    })
}

fn dot(first: &[f64], second: &[f64]) -> f64 {
    let mut sum = 0.0_f64;
    let mut correction = 0.0_f64;
    for (left, right) in first.iter().zip(second) {
        let term = left * right;
        let adjusted = term - correction;
        let next = sum + adjusted;
        correction = (next - sum) - adjusted;
        sum = next;
    }
    sum
}

#[derive(Default)]
struct KahanSum {
    sum: f64,
    correction: f64,
}

impl KahanSum {
    fn add(&mut self, value: f64) {
        let adjusted = value - self.correction;
        let next = self.sum + adjusted;
        self.correction = (next - self.sum) - adjusted;
        self.sum = next;
    }

    fn value(&self) -> f64 { self.sum }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rayon::ThreadPoolBuilder;

    fn pool(threads: usize) -> ThreadPool {
        ThreadPoolBuilder::new().num_threads(threads).build().unwrap()
    }

    #[test]
    fn default_triangle_matches_analytical_displacements() {
        let config = TrussConfig::default();
        let result = solve_truss(&config, &pool(2), |_| {}).unwrap();
        let top = result.nodes.iter().find(|node| node.node_id == 3).unwrap();
        let rigidity = 200.0e9 * 1.0e-3;
        let expected_horizontal = 1_000.0 / (2.0 * rigidity);
        let expected_vertical = -1_000.0 * (6.0 + 5.0 * 2.0_f64.sqrt())
            / (2.0 * rigidity * (2.0_f64.sqrt() + 2.0));
        assert!((top.displacement_x_m - expected_horizontal).abs() < 1.0e-10);
        assert!((top.displacement_y_m - expected_vertical).abs() < 1.0e-10);
        assert!(result.diagnostics.relative_residual < 1.0e-9);
    }

    #[test]
    fn element_stress_sign_distinguishes_tension_and_compression() {
        let result = solve_truss(&TrussConfig::default(), &pool(1), |_| {}).unwrap();
        let base = result.elements.iter().find(|element| element.element_id == 1).unwrap();
        let diagonals: Vec<_> = result.elements.iter().filter(|element| element.element_id != 1).collect();
        assert!(base.axial_stress_pa > 0.0);
        assert!(diagonals.iter().all(|element| element.axial_stress_pa < 0.0));
    }

    #[test]
    fn support_reactions_balance_applied_vertical_load() {
        let result = solve_truss(&TrussConfig::default(), &pool(2), |_| {}).unwrap();
        let vertical_reaction: f64 = result.nodes.iter().map(|node| node.reaction_y_n).sum();
        let horizontal_reaction: f64 = result.nodes.iter().map(|node| node.reaction_x_n).sum();
        assert!((vertical_reaction - 1_000.0).abs() < 1.0e-6);
        assert!(horizontal_reaction.abs() < 1.0e-6);
    }

    #[test]
    fn strain_energy_matches_external_work() {
        let result = solve_truss(&TrussConfig::default(), &pool(2), |_| {}).unwrap();
        assert!(result.diagnostics.energy_balance_relative_error < 1.0e-8);
        assert!(result.diagnostics.strain_energy_j > 0.0);
    }

    #[test]
    fn one_and_four_threads_produce_identical_results() {
        let config = TrussConfig::default();
        let one = solve_truss(&config, &pool(1), |_| {}).unwrap();
        let four = solve_truss(&config, &pool(4), |_| {}).unwrap();
        assert_eq!(one.nodes, four.nodes);
        assert_eq!(one.elements, four.elements);
        assert_eq!(one.diagnostics, four.diagnostics);
    }

    #[test]
    fn pre_cancelled_study_returns_cancelled() {
        let cancel = AtomicBool::new(true);
        let result = solve_truss_cancellable(&TrussConfig::default(), &pool(1), |_| {}, &cancel);
        assert_eq!(result.unwrap_err(), TrussError::Cancelled);
    }

    #[test]
    fn cancellation_requested_from_progress_callback_stops_assembly() {
        let cancel = AtomicBool::new(false);
        let result = solve_truss_cancellable(
            &TrussConfig::default(),
            &pool(1),
            |fraction| {
                if fraction == 0.0 {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
            &cancel,
        );
        assert_eq!(result.unwrap_err(), TrussError::Cancelled);
    }

    #[test]
    fn unstable_unrestrained_layout_is_rejected() {
        let mut config = TrussConfig::default();
        config.supports.clear();
        assert!(matches!(config.validate(), Err(TrussError::InvalidInput(_))));
    }

    #[test]
    fn unsupported_mechanism_is_reported_by_the_solver() {
        let mut config = TrussConfig::default();
        config.elements.retain(|element| element.id != 3);
        assert!(matches!(
            solve_truss(&config, &pool(1), |_| {}),
            Err(TrussError::MechanismOrSingularSystem(_))
        ));
    }

    #[test]
    fn complete_result_round_trips_through_json() {
        let result = solve_truss(&TrussConfig::default(), &pool(1), |_| {}).unwrap();
        let json = serde_json::to_vec(&result).unwrap();
        let restored: TrussResult = serde_json::from_slice(&json).unwrap();
        assert_eq!(restored.config, result.config);
        assert_eq!(restored.nodes, result.nodes);
        assert_eq!(restored.elements, result.elements);
        assert_eq!(restored.diagnostics, result.diagnostics);
        assert_eq!(restored.metadata.input_fingerprint, result.metadata.input_fingerprint);
    }

    #[test]
    fn missing_node_reference_is_rejected() {
        let mut config = TrussConfig::default();
        config.elements[0].node_b = 99;
        assert!(matches!(config.validate(), Err(TrussError::InvalidInput(_))));
    }

    #[test]
    fn zero_length_member_is_rejected() {
        let mut config = TrussConfig::default();
        config.nodes[1].x_m = 0.0;
        config.nodes[1].y_m = 0.0;
        assert!(matches!(config.validate(), Err(TrussError::InvalidInput(_))));
    }

    #[test]
    fn json_deck_round_trips_exactly() {
        let config = TrussConfig::default();
        let json = serde_json::to_vec(&config).unwrap();
        let restored: TrussConfig = serde_json::from_slice(&json).unwrap();
        assert_eq!(restored, config);
    }

    #[test]
    fn bundled_civil_bridge_example_parses_and_solves() {
        let config: TrussConfig = serde_json::from_str(include_str!("../examples/civil-bridge-truss.json")).unwrap();
        let result = solve_truss(&config, &pool(2), |_| {}).unwrap();
        assert!(result.max_displacement_m > 0.0);
        assert!(result.max_absolute_stress_pa > 0.0);
    }
}
