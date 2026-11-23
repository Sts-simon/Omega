use crate::SimulationMetadata;
use rayon::prelude::*;
use rayon::ThreadPool;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const MIN_GRID_SIDE: usize = 8;
const MAX_GRID_SIDE: usize = 512;
const MAX_GRID_CELLS: usize = 1_000_000;
const MAX_STEPS: usize = 200_000;
const MAX_CELL_UPDATES: u64 = 5_000_000_000;
const MAX_MATERIALS: usize = 64;
const STABILITY_SAFETY_FACTOR: f64 = 0.90;

/// Isotropic solid material properties in SI units.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Material {
    pub name: String,
    pub conductivity_w_m_k: f64,
    pub density_kg_m3: f64,
    pub specific_heat_j_kg_k: f64,
}

impl Material {
    pub fn aluminium() -> Self {
        Self {
            name: "Aluminium (reference sample)".to_string(),
            conductivity_w_m_k: 205.0,
            density_kg_m3: 2_700.0,
            specific_heat_j_kg_k: 900.0,
        }
    }

    fn validate(&self) -> Result<(), ThermalError> {
        if self.name.trim().is_empty() {
            return Err(ThermalError::InvalidInput(
                "Material names cannot be empty".to_string(),
            ));
        }
        for (name, value) in [
            ("thermal conductivity", self.conductivity_w_m_k),
            ("density", self.density_kg_m3),
            ("specific heat capacity", self.specific_heat_j_kg_k),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(ThermalError::InvalidInput(format!(
                    "Material {name} must be finite and greater than zero"
                )));
            }
        }
        if !self.volumetric_heat_capacity().is_finite() {
            return Err(ThermalError::InvalidInput(
                "Material volumetric heat capacity is outside the supported numeric range".to_string(),
            ));
        }
        Ok(())
    }

    fn volumetric_heat_capacity(&self) -> f64 {
        self.density_kg_m3 * self.specific_heat_j_kg_k
    }
}

/// Thermal boundary on one edge of the rectangular sample.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BoundaryCondition {
    /// Zero heat flux through this edge.
    Insulated,
    /// Edge temperature is fixed. The boundary is located half a cell outside
    /// the outermost cell centre.
    FixedTemperature { temperature_k: f64 },
}

impl BoundaryCondition {
    fn validate(self) -> Result<(), ThermalError> {
        if let Self::FixedTemperature { temperature_k } = self {
            validate_absolute_temperature(temperature_k, "Boundary temperature")?;
        }
        Ok(())
    }

    fn fixed_temperature(self) -> Option<f64> {
        match self {
            Self::Insulated => None,
            Self::FixedTemperature { temperature_k } => Some(temperature_k),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct BoundaryConditions {
    pub left: BoundaryCondition,
    pub right: BoundaryCondition,
    pub top: BoundaryCondition,
    pub bottom: BoundaryCondition,
}

impl BoundaryConditions {
    pub fn fixed_temperature(temperature_k: f64) -> Self {
        let edge = BoundaryCondition::FixedTemperature { temperature_k };
        Self {
            left: edge,
            right: edge,
            top: edge,
            bottom: edge,
        }
    }

    pub fn insulated() -> Self {
        Self {
            left: BoundaryCondition::Insulated,
            right: BoundaryCondition::Insulated,
            top: BoundaryCondition::Insulated,
            bottom: BoundaryCondition::Insulated,
        }
    }

    fn validate(self) -> Result<(), ThermalError> {
        self.left.validate()?;
        self.right.validate()?;
        self.top.validate()?;
        self.bottom.validate()?;
        Ok(())
    }
}

/// Rectangular hot region centred in the sample at initial time.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct HeatPatch {
    pub half_width_cells: usize,
    pub half_height_cells: usize,
    pub temperature_k: f64,
}

/// Full input deck for the transient, two-dimensional finite-volume model.
///
/// `thickness_m` turns the 2D cross-section into a slab with a defined volume,
/// allowing the solver to report energy in joules. An empty `cell_material_ids`
/// vector means every cell uses `materials[0]`; otherwise it must contain one
/// material index per cell in row-major order.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ThermalConfig {
    pub width: usize,
    pub height: usize,
    pub cell_size_x_m: f64,
    pub cell_size_y_m: f64,
    pub thickness_m: f64,
    pub time_step_s: f64,
    pub steps: usize,
    pub initial_temperature_k: f64,
    pub heat_patch: Option<HeatPatch>,
    pub boundary_conditions: BoundaryConditions,
    pub materials: Vec<Material>,
    #[serde(default)]
    pub cell_material_ids: Vec<u16>,
    /// Uniform volumetric source term. Positive values add heat; negative values
    /// remove heat. Units: W/m³.
    pub volumetric_heat_source_w_m3: f64,
}

impl Default for ThermalConfig {
    fn default() -> Self {
        let ambient = 293.15;
        Self {
            width: 256,
            height: 192,
            cell_size_x_m: 0.01,
            cell_size_y_m: 0.01,
            thickness_m: 0.01,
            time_step_s: 0.025,
            steps: 800,
            initial_temperature_k: ambient,
            heat_patch: Some(HeatPatch {
                half_width_cells: 10,
                half_height_cells: 8,
                temperature_k: 380.0,
            }),
            boundary_conditions: BoundaryConditions::fixed_temperature(ambient),
            materials: vec![Material::aluminium()],
            cell_material_ids: Vec::new(),
            volumetric_heat_source_w_m3: 0.0,
        }
    }
}

impl ThermalConfig {
    /// Validates the input and returns the maximum allowed explicit time step.
    /// The returned limit already includes a 10% stability safety margin.
    pub fn validate(&self) -> Result<f64, ThermalError> {
        if !(MIN_GRID_SIDE..=MAX_GRID_SIDE).contains(&self.width)
            || !(MIN_GRID_SIDE..=MAX_GRID_SIDE).contains(&self.height)
        {
            return Err(ThermalError::InvalidInput(format!(
                "Grid dimensions must be between {MIN_GRID_SIDE} and {MAX_GRID_SIDE} cells per side"
            )));
        }
        let cell_count = self
            .width
            .checked_mul(self.height)
            .ok_or_else(|| ThermalError::InvalidInput("Grid size overflow".to_string()))?;
        if cell_count > MAX_GRID_CELLS {
            return Err(ThermalError::InvalidInput(format!(
                "Grid exceeds the {MAX_GRID_CELLS}-cell prototype limit"
            )));
        }
        for (name, value) in [
            ("cell_size_x_m", self.cell_size_x_m),
            ("cell_size_y_m", self.cell_size_y_m),
            ("thickness_m", self.thickness_m),
            ("time_step_s", self.time_step_s),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(ThermalError::InvalidInput(format!(
                    "{name} must be finite and greater than zero"
                )));
            }
        }
        if self.steps == 0 || self.steps > MAX_STEPS {
            return Err(ThermalError::InvalidInput(format!(
                "Step count must be between 1 and {MAX_STEPS}"
            )));
        }
        // The prior grid and step caps keep this product far below u64::MAX,
        // including on 32-bit hosts where usize itself may be narrower.
        let cell_updates = (cell_count as u64) * (self.steps as u64);
        if cell_updates > MAX_CELL_UPDATES {
            return Err(ThermalError::InvalidInput(format!(
                "Study requests {cell_updates} cell updates; prototype limit is {MAX_CELL_UPDATES}"
            )));
        }
        let simulated_time_s = self.time_step_s * self.steps as f64;
        if !simulated_time_s.is_finite() || simulated_time_s <= 0.0 {
            return Err(ThermalError::InvalidInput(
                "Total simulated duration is outside the supported numeric range".to_string(),
            ));
        }
        validate_absolute_temperature(self.initial_temperature_k, "Initial temperature")?;
        if !self.volumetric_heat_source_w_m3.is_finite() {
            return Err(ThermalError::InvalidInput(
                "Volumetric heat source must be finite".to_string(),
            ));
        }
        if self.materials.is_empty() || self.materials.len() > MAX_MATERIALS {
            return Err(ThermalError::InvalidInput(format!(
                "Material catalogue must contain between 1 and {MAX_MATERIALS} entries"
            )));
        }
        for material in &self.materials {
            material.validate()?;
        }
        if !self.cell_material_ids.is_empty() {
            if self.cell_material_ids.len() != cell_count {
                return Err(ThermalError::InvalidInput(format!(
                    "Material map contains {} entries, expected {cell_count}",
                    self.cell_material_ids.len()
                )));
            }
            if self
                .cell_material_ids
                .iter()
                .any(|id| usize::from(*id) >= self.materials.len())
            {
                return Err(ThermalError::InvalidInput(
                    "Material map references an unknown material".to_string(),
                ));
            }
        }
        self.boundary_conditions.validate()?;
        if let Some(patch) = self.heat_patch {
            validate_absolute_temperature(patch.temperature_k, "Heat-patch temperature")?;
            if patch.half_width_cells >= self.width / 2
                || patch.half_height_cells >= self.height / 2
            {
                return Err(ThermalError::InvalidInput(
                    "Heat patch must fit inside the grid".to_string(),
                ));
            }
        }

        let cell_volume_m3 = self.cell_size_x_m * self.cell_size_y_m * self.thickness_m;
        if !cell_volume_m3.is_finite() || cell_volume_m3 <= 0.0 {
            return Err(ThermalError::InvalidInput(
                "Cell volume is outside the supported numeric range".to_string(),
            ));
        }
        let total_source_power_w = self.volumetric_heat_source_w_m3
            * cell_volume_m3
            * cell_count as f64;
        if !total_source_power_w.is_finite() {
            return Err(ThermalError::InvalidInput(
                "Total heat-source power is outside the supported numeric range".to_string(),
            ));
        }

        let mut maximum_rate_per_second = 0.0_f64;
        for y in 0..self.height {
            for x in 0..self.width {
                let index = y * self.width + x;
                let material = self.material_at(index);
                let mut conductance_sum_w_per_k = 0.0;

                if x > 0 {
                    conductance_sum_w_per_k += self.horizontal_conductance(index, index - 1);
                } else if self.boundary_conditions.left.fixed_temperature().is_some() {
                    conductance_sum_w_per_k +=
                        2.0 * material.conductivity_w_m_k * self.thickness_m
                            * self.cell_size_y_m / self.cell_size_x_m;
                }
                if x + 1 < self.width {
                    conductance_sum_w_per_k += self.horizontal_conductance(index, index + 1);
                } else if self.boundary_conditions.right.fixed_temperature().is_some() {
                    conductance_sum_w_per_k +=
                        2.0 * material.conductivity_w_m_k * self.thickness_m
                            * self.cell_size_y_m / self.cell_size_x_m;
                }
                if y > 0 {
                    conductance_sum_w_per_k += self.vertical_conductance(index, index - self.width);
                } else if self.boundary_conditions.top.fixed_temperature().is_some() {
                    conductance_sum_w_per_k +=
                        2.0 * material.conductivity_w_m_k * self.thickness_m
                            * self.cell_size_x_m / self.cell_size_y_m;
                }
                if y + 1 < self.height {
                    conductance_sum_w_per_k +=
                        self.vertical_conductance(index, index + self.width);
                } else if self.boundary_conditions.bottom.fixed_temperature().is_some() {
                    conductance_sum_w_per_k +=
                        2.0 * material.conductivity_w_m_k * self.thickness_m
                            * self.cell_size_x_m / self.cell_size_y_m;
                }

                let capacity_j_per_k = material.volumetric_heat_capacity() * cell_volume_m3;
                if !capacity_j_per_k.is_finite() || capacity_j_per_k <= 0.0 {
                    return Err(ThermalError::InvalidInput(
                        "Cell heat capacity is outside the supported numeric range".to_string(),
                    ));
                }
                maximum_rate_per_second = maximum_rate_per_second
                    .max(conductance_sum_w_per_k / capacity_j_per_k);
            }
        }
        if !maximum_rate_per_second.is_finite() || maximum_rate_per_second <= 0.0 {
            return Err(ThermalError::InvalidInput(
                "Could not derive a finite stability limit from the material and grid".to_string(),
            ));
        }
        let safe_limit_s = STABILITY_SAFETY_FACTOR / maximum_rate_per_second;
        if !safe_limit_s.is_finite() || safe_limit_s <= 0.0 {
            return Err(ThermalError::InvalidInput(
                "Computed stable time-step limit is outside the supported numeric range".to_string(),
            ));
        }
        if self.time_step_s > safe_limit_s {
            return Err(ThermalError::UnstableTimeStep {
                requested_s: self.time_step_s,
                maximum_s: safe_limit_s,
            });
        }
        Ok(safe_limit_s)
    }

    fn material_at(&self, index: usize) -> &Material {
        if self.cell_material_ids.is_empty() {
            &self.materials[0]
        } else {
            &self.materials[usize::from(self.cell_material_ids[index])]
        }
    }

    fn horizontal_conductance(&self, first: usize, second: usize) -> f64 {
        let first_k = self.material_at(first).conductivity_w_m_k;
        let second_k = self.material_at(second).conductivity_w_m_k;
        let face_k = harmonic_mean(first_k, second_k);
        face_k * self.thickness_m * self.cell_size_y_m / self.cell_size_x_m
    }

    fn vertical_conductance(&self, first: usize, second: usize) -> f64 {
        let first_k = self.material_at(first).conductivity_w_m_k;
        let second_k = self.material_at(second).conductivity_w_m_k;
        let face_k = harmonic_mean(first_k, second_k);
        face_k * self.thickness_m * self.cell_size_x_m / self.cell_size_y_m
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThermalDiagnostics {
    pub stable_time_step_limit_s: f64,
    pub initial_energy_j: f64,
    pub final_energy_j: f64,
    /// Signed heat transferred through fixed-temperature boundaries. Positive
    /// means energy entered the sample; negative means the sample lost energy.
    pub boundary_energy_j: f64,
    pub source_energy_j: f64,
    pub energy_balance_error_j: f64,
    pub energy_balance_relative_error: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThermalResult {
    pub metadata: SimulationMetadata,
    pub config: ThermalConfig,
    pub width: usize,
    pub height: usize,
    pub steps_completed: usize,
    pub simulated_time_s: f64,
    pub elapsed_ms: u128,
    pub min_temperature_k: f64,
    pub max_temperature_k: f64,
    pub mean_temperature_k: f64,
    pub diagnostics: ThermalDiagnostics,
    /// Final temperature field, row-major, in kelvin. Retained for plots and
    /// later comparisons; the renderer should downsample before displaying it.
    pub final_temperature_k: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ThermalError {
    InvalidInput(String),
    UnstableTimeStep { requested_s: f64, maximum_s: f64 },
    NonPhysicalState(String),
    Cancelled,
}

impl std::fmt::Display for ThermalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(formatter, "{message}"),
            Self::UnstableTimeStep { requested_s, maximum_s } => write!(
                formatter,
                "Time step {requested_s:.6e} s exceeds the safe explicit limit {maximum_s:.6e} s"
            ),
            Self::NonPhysicalState(message) => write!(formatter, "Numerical solver stopped: {message}"),
            Self::Cancelled => write!(formatter, "Cancelled by user"),
        }
    }
}

impl std::error::Error for ThermalError {}

/// Precomputed coefficients used by every time step. Building this once avoids
/// repeated material lookups and harmonic means in the hot numerical loop.
struct ThermalCoefficients {
    capacity_j_per_k: Vec<f64>,
    horizontal_faces_w_per_k: Vec<f64>,
    vertical_faces_w_per_k: Vec<f64>,
    left_boundary_w_per_k: Vec<f64>,
    right_boundary_w_per_k: Vec<f64>,
    top_boundary_w_per_k: Vec<f64>,
    bottom_boundary_w_per_k: Vec<f64>,
    source_power_per_cell_w: f64,
}

impl ThermalCoefficients {
    fn build(config: &ThermalConfig, cell_volume_m3: f64) -> Result<Self, ThermalError> {
        let width = config.width;
        let height = config.height;
        let cell_count = width * height;

        let mut capacity_j_per_k = Vec::with_capacity(cell_count);
        for index in 0..cell_count {
            let capacity = config.material_at(index).volumetric_heat_capacity() * cell_volume_m3;
            if !capacity.is_finite() || capacity <= 0.0 {
                return Err(ThermalError::InvalidInput(
                    "Cell heat capacity is outside the supported numeric range".to_string(),
                ));
            }
            capacity_j_per_k.push(capacity);
        }

        let mut horizontal_faces_w_per_k = Vec::with_capacity(height * (width - 1));
        for y in 0..height {
            for x in 0..(width - 1) {
                let first = y * width + x;
                let conductance = config.horizontal_conductance(first, first + 1);
                if !conductance.is_finite() || conductance <= 0.0 {
                    return Err(ThermalError::InvalidInput(
                        "Horizontal face conductance is outside the supported numeric range".to_string(),
                    ));
                }
                horizontal_faces_w_per_k.push(conductance);
            }
        }

        let mut vertical_faces_w_per_k = Vec::with_capacity((height - 1) * width);
        for y in 0..(height - 1) {
            for x in 0..width {
                let first = y * width + x;
                let conductance = config.vertical_conductance(first, first + width);
                if !conductance.is_finite() || conductance <= 0.0 {
                    return Err(ThermalError::InvalidInput(
                        "Vertical face conductance is outside the supported numeric range".to_string(),
                    ));
                }
                vertical_faces_w_per_k.push(conductance);
            }
        }

        let mut left_boundary_w_per_k = vec![0.0; height];
        let mut right_boundary_w_per_k = vec![0.0; height];
        let mut top_boundary_w_per_k = vec![0.0; width];
        let mut bottom_boundary_w_per_k = vec![0.0; width];

        if matches!(config.boundary_conditions.left, BoundaryCondition::FixedTemperature { .. }) {
            for y in 0..height {
                let conductivity = config.material_at(y * width).conductivity_w_m_k;
                left_boundary_w_per_k[y] =
                    2.0 * conductivity * config.thickness_m * config.cell_size_y_m
                        / config.cell_size_x_m;
            }
        }
        if matches!(config.boundary_conditions.right, BoundaryCondition::FixedTemperature { .. }) {
            for y in 0..height {
                let conductivity = config.material_at(y * width + width - 1).conductivity_w_m_k;
                right_boundary_w_per_k[y] =
                    2.0 * conductivity * config.thickness_m * config.cell_size_y_m
                        / config.cell_size_x_m;
            }
        }
        if matches!(config.boundary_conditions.top, BoundaryCondition::FixedTemperature { .. }) {
            for x in 0..width {
                let conductivity = config.material_at(x).conductivity_w_m_k;
                top_boundary_w_per_k[x] =
                    2.0 * conductivity * config.thickness_m * config.cell_size_x_m
                        / config.cell_size_y_m;
            }
        }
        if matches!(config.boundary_conditions.bottom, BoundaryCondition::FixedTemperature { .. }) {
            let row_start = (height - 1) * width;
            for x in 0..width {
                let conductivity = config.material_at(row_start + x).conductivity_w_m_k;
                bottom_boundary_w_per_k[x] =
                    2.0 * conductivity * config.thickness_m * config.cell_size_x_m
                        / config.cell_size_y_m;
            }
        }

        for conductance in left_boundary_w_per_k
            .iter()
            .chain(&right_boundary_w_per_k)
            .chain(&top_boundary_w_per_k)
            .chain(&bottom_boundary_w_per_k)
        {
            if !conductance.is_finite() || *conductance < 0.0 {
                return Err(ThermalError::InvalidInput(
                    "Boundary conductance is outside the supported numeric range".to_string(),
                ));
            }
        }

        let source_power_per_cell_w = config.volumetric_heat_source_w_m3 * cell_volume_m3;
        if !source_power_per_cell_w.is_finite() {
            return Err(ThermalError::InvalidInput(
                "Per-cell heat-source power is outside the supported numeric range".to_string(),
            ));
        }

        Ok(Self {
            capacity_j_per_k,
            horizontal_faces_w_per_k,
            vertical_faces_w_per_k,
            left_boundary_w_per_k,
            right_boundary_w_per_k,
            top_boundary_w_per_k,
            bottom_boundary_w_per_k,
            source_power_per_cell_w,
        })
    }
}

/// Runs the solver without cancellation. Progress is reported from 0 to 1.
pub fn simulate<F>(
    config: &ThermalConfig,
    pool: &ThreadPool,
    report_progress: F,
) -> Result<ThermalResult, ThermalError>
where
    F: FnMut(f32) + Send,
{
    let never_cancel = AtomicBool::new(false);
    simulate_cancellable(config, pool, report_progress, &never_cancel)
}

/// Solves transient heat conduction in a 2D solid slab with explicit finite
/// volumes, harmonic face conductivity, per-cell material properties, source
/// terms and insulated or fixed-temperature boundaries.
///
/// The update is conservative: internal face fluxes appear with equal and
/// opposite signs in adjacent cells. The requested time step is rejected when
/// it exceeds a per-cell monotonicity bound computed from the actual material map.
/// Material capacities and all face conductances are precomputed once before
/// the time loop, keeping the parallel step kernel free from material lookups.
pub fn simulate_cancellable<F>(
    config: &ThermalConfig,
    pool: &ThreadPool,
    mut report_progress: F,
    cancel: &AtomicBool,
) -> Result<ThermalResult, ThermalError>
where
    F: FnMut(f32) + Send,
{
    let started = Instant::now();
    if cancel.load(Ordering::Relaxed) {
        return Err(ThermalError::Cancelled);
    }
    let stable_time_step_limit_s = config.validate()?;
    let metadata = SimulationMetadata::new(
        "solid.heat_conduction.2d",
        "2.1.0",
        "explicit_finite_volume_forward_euler_precomputed",
        "1.1.0",
        "SI (m, kg, s, K, W, J); 2D slab with explicit thickness",
        config,
    )
    .map_err(ThermalError::InvalidInput)?;

    let width = config.width;
    let height = config.height;
    let cell_count = width * height;
    let cell_volume_m3 = config.cell_size_x_m * config.cell_size_y_m * config.thickness_m;
    let coefficients = ThermalCoefficients::build(config, cell_volume_m3)?;
    if cancel.load(Ordering::Relaxed) {
        return Err(ThermalError::Cancelled);
    }
    let mut current = vec![config.initial_temperature_k; cell_count];
    let mut next = vec![config.initial_temperature_k; cell_count];

    if let Some(patch) = config.heat_patch {
        let centre_x = width / 2;
        let centre_y = height / 2;
        let x_start = centre_x.saturating_sub(patch.half_width_cells);
        let x_end = (centre_x + patch.half_width_cells + 1).min(width);
        let y_start = centre_y.saturating_sub(patch.half_height_cells);
        let y_end = (centre_y + patch.half_height_cells + 1).min(height);
        for y in y_start..y_end {
            for x in x_start..x_end {
                current[y * width + x] = patch.temperature_k;
            }
        }
    }

    let initial_energy_j = total_energy(&coefficients.capacity_j_per_k, &current);
    if !initial_energy_j.is_finite() {
        return Err(ThermalError::NonPhysicalState(
            "initial energy is outside the supported numeric range".to_string(),
        ));
    }
    let source_power_w = coefficients.source_power_per_cell_w * cell_count as f64;
    let source_energy_j = source_power_w * config.time_step_s * config.steps as f64;
    if !source_energy_j.is_finite() {
        return Err(ThermalError::NonPhysicalState(
            "integrated heat-source energy is outside the supported numeric range".to_string(),
        ));
    }
    let progress_interval = (config.steps / 100).max(1);
    let mut boundary_energy = KahanSum::default();
    let dt = config.time_step_s;
    let boundaries = config.boundary_conditions;

    for step in 0..config.steps {
        if cancel.load(Ordering::Relaxed) {
            return Err(ThermalError::Cancelled);
        }

        boundary_energy.add(boundary_power_w(config, &coefficients, &current) * dt);

        pool.install(|| {
            next.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
                for (x, value) in row.iter_mut().enumerate() {
                    let index = y * width + x;
                    let temperature = current[index];
                    let mut net_power_w = coefficients.source_power_per_cell_w;

                    if x > 0 {
                        let face = y * (width - 1) + x - 1;
                        let conductance = coefficients.horizontal_faces_w_per_k[face];
                        net_power_w += conductance * (current[index - 1] - temperature);
                    } else if let Some(boundary_k) = boundaries.left.fixed_temperature() {
                        net_power_w += coefficients.left_boundary_w_per_k[y]
                            * (boundary_k - temperature);
                    }

                    if x + 1 < width {
                        let face = y * (width - 1) + x;
                        let conductance = coefficients.horizontal_faces_w_per_k[face];
                        net_power_w += conductance * (current[index + 1] - temperature);
                    } else if let Some(boundary_k) = boundaries.right.fixed_temperature() {
                        net_power_w += coefficients.right_boundary_w_per_k[y]
                            * (boundary_k - temperature);
                    }

                    if y > 0 {
                        let face = (y - 1) * width + x;
                        let conductance = coefficients.vertical_faces_w_per_k[face];
                        net_power_w += conductance * (current[index - width] - temperature);
                    } else if let Some(boundary_k) = boundaries.top.fixed_temperature() {
                        net_power_w += coefficients.top_boundary_w_per_k[x]
                            * (boundary_k - temperature);
                    }

                    if y + 1 < height {
                        let face = y * width + x;
                        let conductance = coefficients.vertical_faces_w_per_k[face];
                        net_power_w += conductance * (current[index + width] - temperature);
                    } else if let Some(boundary_k) = boundaries.bottom.fixed_temperature() {
                        net_power_w += coefficients.bottom_boundary_w_per_k[x]
                            * (boundary_k - temperature);
                    }

                    *value = temperature
                        + dt * net_power_w / coefficients.capacity_j_per_k[index];
                }
            });
        });

        if next.iter().any(|temperature| !temperature.is_finite() || *temperature <= 0.0) {
            return Err(ThermalError::NonPhysicalState(
                "temperature became non-finite or reached absolute zero".to_string(),
            ));
        }
        std::mem::swap(&mut current, &mut next);

        if (step + 1) % progress_interval == 0 || step + 1 == config.steps {
            report_progress((step + 1) as f32 / config.steps as f32);
        }
    }

    let min_temperature_k = current.iter().copied().fold(f64::INFINITY, f64::min);
    let max_temperature_k = current.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mean_temperature_k = stable_mean(&current);
    let final_energy_j = total_energy(&coefficients.capacity_j_per_k, &current);
    let boundary_energy_j = boundary_energy.value();
    let energy_balance_error_j =
        final_energy_j - initial_energy_j - source_energy_j - boundary_energy_j;
    let energy_scale_j = initial_energy_j
        .abs()
        .max(source_energy_j.abs())
        .max(boundary_energy_j.abs())
        .max(1.0);
    if !final_energy_j.is_finite()
        || !boundary_energy_j.is_finite()
        || !energy_balance_error_j.is_finite()
    {
        return Err(ThermalError::NonPhysicalState(
            "energy diagnostics are outside the supported numeric range".to_string(),
        ));
    }
    let relative_energy_error = energy_balance_error_j.abs() / energy_scale_j;
    if !mean_temperature_k.is_finite() || !relative_energy_error.is_finite() {
        return Err(ThermalError::NonPhysicalState(
            "final summary statistics are outside the supported numeric range".to_string(),
        ));
    }
    let diagnostics = ThermalDiagnostics {
        stable_time_step_limit_s,
        initial_energy_j,
        final_energy_j,
        boundary_energy_j,
        source_energy_j,
        energy_balance_error_j,
        energy_balance_relative_error: relative_energy_error,
    };

    Ok(ThermalResult {
        metadata,
        config: config.clone(),
        width,
        height,
        steps_completed: config.steps,
        simulated_time_s: config.time_step_s * config.steps as f64,
        elapsed_ms: started.elapsed().as_millis(),
        min_temperature_k,
        max_temperature_k,
        mean_temperature_k,
        diagnostics,
        final_temperature_k: current,
    })
}

fn validate_absolute_temperature(value: f64, label: &str) -> Result<(), ThermalError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(ThermalError::InvalidInput(format!(
            "{label} must be finite and greater than zero kelvin"
        )));
    }
    Ok(())
}

fn harmonic_mean(first: f64, second: f64) -> f64 {
    // Ratio form avoids both the product and 2*low overflow cases.
    let low = first.min(second);
    let high = first.max(second);
    low / (0.5 + 0.5 * low / high)
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

    fn value(&self) -> f64 {
        self.sum
    }
}

fn stable_mean(values: &[f64]) -> f64 {
    let mut mean = 0.0;
    for (index, value) in values.iter().copied().enumerate() {
        mean += (value - mean) / (index + 1) as f64;
    }
    mean
}

fn total_energy(capacity_j_per_k: &[f64], field: &[f64]) -> f64 {
    let mut energy = KahanSum::default();
    for (capacity, temperature) in capacity_j_per_k.iter().zip(field) {
        energy.add(capacity * temperature);
    }
    energy.value()
}

fn boundary_power_w(
    config: &ThermalConfig,
    coefficients: &ThermalCoefficients,
    field: &[f64],
) -> f64 {
    let mut power = KahanSum::default();
    let width = config.width;
    let height = config.height;

    if let Some(boundary_k) = config.boundary_conditions.left.fixed_temperature() {
        for y in 0..height {
            power.add(coefficients.left_boundary_w_per_k[y] * (boundary_k - field[y * width]));
        }
    }
    if let Some(boundary_k) = config.boundary_conditions.right.fixed_temperature() {
        for y in 0..height {
            let index = y * width + width - 1;
            power.add(coefficients.right_boundary_w_per_k[y] * (boundary_k - field[index]));
        }
    }
    if let Some(boundary_k) = config.boundary_conditions.top.fixed_temperature() {
        for x in 0..width {
            power.add(coefficients.top_boundary_w_per_k[x] * (boundary_k - field[x]));
        }
    }
    if let Some(boundary_k) = config.boundary_conditions.bottom.fixed_temperature() {
        let row_start = (height - 1) * width;
        for x in 0..width {
            let index = row_start + x;
            power.add(
                coefficients.bottom_boundary_w_per_k[x] * (boundary_k - field[index]),
            );
        }
    }
    power.value()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rayon::ThreadPoolBuilder;
    fn pool(threads: usize) -> ThreadPool {
        ThreadPoolBuilder::new().num_threads(threads).build().unwrap()
    }

    fn small_config() -> ThermalConfig {
        ThermalConfig {
            width: 32,
            height: 28,
            steps: 120,
            time_step_s: 0.025,
            ..ThermalConfig::default()
        }
    }

    #[test]
    fn fixed_temperature_edges_cool_the_hot_patch() {
        let config = small_config();
        let result = simulate(&config, &pool(2), |_| {}).unwrap();
        assert!(result.max_temperature_k < 380.0);
        assert!(result.max_temperature_k > result.min_temperature_k);
        assert!(result.diagnostics.boundary_energy_j < 0.0);
        assert!(result.diagnostics.energy_balance_relative_error < 1.0e-9);
        assert_eq!(result.final_temperature_k.len(), config.width * config.height);
    }

    #[test]
    fn insulated_sample_conserves_energy_without_sources() {
        let config = ThermalConfig {
            boundary_conditions: BoundaryConditions::insulated(),
            ..small_config()
        };
        let result = simulate(&config, &pool(2), |_| {}).unwrap();
        assert!(result.diagnostics.boundary_energy_j.abs() < 1.0e-12);
        assert!(result.diagnostics.energy_balance_relative_error < 1.0e-10);
        assert!(
            (result.diagnostics.final_energy_j - result.diagnostics.initial_energy_j).abs()
                < 1.0e-7
        );
    }

    #[test]
    fn uniform_field_remains_uniform_when_edges_match() {
        let config = ThermalConfig {
            initial_temperature_k: 300.0,
            heat_patch: None,
            boundary_conditions: BoundaryConditions::fixed_temperature(300.0),
            volumetric_heat_source_w_m3: 0.0,
            ..small_config()
        };
        let result = simulate(&config, &pool(2), |_| {}).unwrap();
        assert!(result.final_temperature_k.iter().all(|value| (*value - 300.0).abs() < 1.0e-10));
    }

    #[test]
    fn unstable_time_step_is_rejected_before_calculation() {
        let config = ThermalConfig {
            time_step_s: 10_000.0,
            ..small_config()
        };
        assert!(matches!(
            simulate(&config, &pool(1), |_| {}),
            Err(ThermalError::UnstableTimeStep { .. })
        ));
    }

    #[test]
    fn heterogeneous_material_map_is_supported() {
        let width = 32;
        let height = 28;
        let mut ids = vec![0_u16; width * height];
        for y in 0..height {
            for x in width / 2..width {
                ids[y * width + x] = 1;
            }
        }
        let config = ThermalConfig {
            cell_material_ids: ids,
            materials: vec![
                Material::aluminium(),
                Material {
                    name: "Reference polymer".to_string(),
                    conductivity_w_m_k: 0.2,
                    density_kg_m3: 1_200.0,
                    specific_heat_j_kg_k: 1_500.0,
                },
            ],
            ..small_config()
        };
        let result = simulate(&config, &pool(2), |_| {}).unwrap();
        assert!(result.max_temperature_k.is_finite());
        assert!(result.diagnostics.energy_balance_relative_error < 1.0e-9);
    }

    #[test]
    fn one_and_four_threads_produce_the_same_field() {
        let config = small_config();
        let one = simulate(&config, &pool(1), |_| {}).unwrap();
        let four = simulate(&config, &pool(4), |_| {}).unwrap();
        assert_eq!(one.final_temperature_k, four.final_temperature_k);
        assert_eq!(one.diagnostics.energy_balance_error_j, four.diagnostics.energy_balance_error_j);
    }

    #[test]
    fn pre_cancelled_job_returns_cancelled() {
        let cancel = AtomicBool::new(true);
        let result = simulate_cancellable(&small_config(), &pool(1), |_| {}, &cancel);
        assert_eq!(result.unwrap_err(), ThermalError::Cancelled);
    }

    #[test]
    fn invalid_material_map_is_rejected() {
        let config = ThermalConfig {
            cell_material_ids: vec![7; 32 * 28],
            ..small_config()
        };
        assert!(matches!(config.validate(), Err(ThermalError::InvalidInput(_))));
    }

    #[test]
    fn mixed_edge_temperatures_transfer_energy_consistently() {
        let config = ThermalConfig {
            initial_temperature_k: 300.0,
            heat_patch: None,
            boundary_conditions: BoundaryConditions {
                left: BoundaryCondition::FixedTemperature { temperature_k: 350.0 },
                right: BoundaryCondition::FixedTemperature { temperature_k: 290.0 },
                top: BoundaryCondition::Insulated,
                bottom: BoundaryCondition::Insulated,
            },
            steps: 150,
            time_step_s: 0.01,
            ..small_config()
        };
        let result = simulate(&config, &pool(2), |_| {}).unwrap();
        let left_mean = (0..config.height)
            .map(|y| result.final_temperature_k[y * config.width])
            .sum::<f64>() / config.height as f64;
        let right_mean = (0..config.height)
            .map(|y| result.final_temperature_k[y * config.width + config.width - 1])
            .sum::<f64>() / config.height as f64;
        assert!(left_mean > right_mean);
        assert!(result.diagnostics.energy_balance_relative_error < 1.0e-9);
    }

    #[test]
    fn excessive_cell_update_budget_is_rejected_before_allocation() {
        let config = ThermalConfig {
            width: 512,
            height: 512,
            steps: 20_000,
            ..ThermalConfig::default()
        };
        assert!(matches!(config.validate(), Err(ThermalError::InvalidInput(_))));
    }

    #[test]
    fn bundled_material_interface_input_deck_parses_and_validates() {
        let json = include_str!("../examples/material-interface-study.json");
        let config: ThermalConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.cell_material_ids.len(), config.width * config.height);
        assert_eq!(config.materials.len(), 2);
        assert!(config.validate().is_ok());
    }


    #[test]
    fn uniform_volumetric_source_matches_the_analytical_temperature_rise() {
        let source = 100_000.0;
        let config = ThermalConfig {
            initial_temperature_k: 300.0,
            heat_patch: None,
            boundary_conditions: BoundaryConditions::insulated(),
            volumetric_heat_source_w_m3: source,
            steps: 120,
            time_step_s: 0.025,
            ..small_config()
        };
        let result = simulate(&config, &pool(2), |_| {}).unwrap();
        let material = &config.materials[0];
        let expected = config.initial_temperature_k
            + source * result.simulated_time_s
                / (material.density_kg_m3 * material.specific_heat_j_kg_k);
        assert!(result.final_temperature_k.iter().all(|value| (*value - expected).abs() < 1.0e-9));
        assert!(result.diagnostics.energy_balance_relative_error < 1.0e-10);
    }

    #[test]
    fn result_and_configuration_round_trip_through_json() {
        let result = simulate(&small_config(), &pool(1), |_| {}).unwrap();
        let json = serde_json::to_vec(&result).unwrap();
        let restored: ThermalResult = serde_json::from_slice(&json).unwrap();
        assert_eq!(restored.config, result.config);
        assert_eq!(restored.final_temperature_k, result.final_temperature_k);
        assert_eq!(restored.metadata.input_fingerprint, result.metadata.input_fingerprint);
    }

    #[test]
    fn json_round_trip_preserves_f64_values_exactly() {
        // The default serde_json float parser uses a faster, best-effort path.
        // Scientific result fields need exact f64 round-tripping so persisted
        // data can be reloaded without silently changing the recorded values.
        let values = [
            293.6244654164955_f64,
            379.7855194070244_f64,
            368.26628276499116_f64,
            1.0 / 7.0,
            f64::MIN_POSITIVE,
        ];
        let json = serde_json::to_vec(&values).unwrap();
        let restored: Vec<f64> = serde_json::from_slice(&json).unwrap();
        assert_eq!(restored.as_slice(), values.as_slice());
    }

    #[test]
    fn stable_step_limit_is_reported_and_respected() {
        let config = small_config();
        let limit = config.validate().unwrap();
        assert!(config.time_step_s <= limit);
        let result = simulate(&config, &pool(1), |_| {}).unwrap();
        assert_eq!(result.diagnostics.stable_time_step_limit_s, limit);
    }
}
