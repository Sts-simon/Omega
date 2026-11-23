//! Self-contained scientific result visualizations (HTML + SVG + CSV).
//!
//! The exporter deliberately has no graphics-engine dependency. Reports can be
//! opened in any modern browser and retain the numerical metadata needed to
//! interpret the plotted fields.

use crate::{MeshThermalResult, ThermalResult, TrussResult};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const THERMAL_MAX_BINS: usize = 144;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisualizationFiles {
    pub html_path: PathBuf,
    pub data_path: PathBuf,
}

#[derive(Debug)]
pub enum VisualizationError {
    Io(std::io::Error),
    InvalidResult(String),
}

impl fmt::Display for VisualizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "Could not write visualization: {error}"),
            Self::InvalidResult(message) => write!(formatter, "Cannot visualize result: {message}"),
        }
    }
}

impl std::error::Error for VisualizationError {}

impl From<std::io::Error> for VisualizationError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Writes a responsive HTML heatmap and a full-resolution CSV temperature field.
/// The SVG is downsampled by block averaging for readability and file size; the
/// accompanying CSV retains every cell from the solver result.
pub fn export_thermal_visualization(
    result: &ThermalResult,
    output_directory: impl AsRef<Path>,
) -> Result<VisualizationFiles, VisualizationError> {
    let expected = result
        .width
        .checked_mul(result.height)
        .ok_or_else(|| VisualizationError::InvalidResult("grid dimensions overflow".into()))?;
    if result.width == 0
        || result.height == 0
        || result.final_temperature_k.len() != expected
        || result.config.width != result.width
        || result.config.height != result.height
    {
        return Err(VisualizationError::InvalidResult(format!(
            "expected {} temperatures for a matching {}x{} grid, found {}",
            expected,
            result.width,
            result.height,
            result.final_temperature_k.len()
        )));
    }
    if result.final_temperature_k.iter().any(|value| !value.is_finite())
        || !result.config.cell_size_x_m.is_finite()
        || !result.config.cell_size_y_m.is_finite()
        || result.config.cell_size_x_m <= 0.0
        || result.config.cell_size_y_m <= 0.0
        || !result.min_temperature_k.is_finite()
        || !result.max_temperature_k.is_finite()
        || !result.mean_temperature_k.is_finite()
        || result.min_temperature_k > result.max_temperature_k
    {
        return Err(VisualizationError::InvalidResult(
            "temperature field or grid metadata contains invalid values".into(),
        ));
    }

    let directory = output_directory.as_ref();
    fs::create_dir_all(directory)?;
    let stem = format!("thermal_{}", result.metadata.created_unix_ms);
    let csv_path = write_unique(
        directory,
        &format!("{stem}_field"),
        "csv",
        thermal_csv(result).as_bytes(),
    )?;
    let svg = thermal_svg(result);
    let html = report_html(
        "OMEGA | Thermal field",
        "Transient 2D finite-volume conduction",
        &format!(
            "{} · {} × {} cells · {:.3} simulated seconds",
            result.metadata.model_id, result.width, result.height, result.simulated_time_s
        ),
        &svg,
        &format!(
            "<div class=\"metrics\">
                <article><span>Minimum</span><strong>{:.3} K</strong></article>
                <article><span>Mean</span><strong>{:.3} K</strong></article>
                <article><span>Maximum</span><strong>{:.3} K</strong></article>
                <article><span>Wall time</span><strong>{:.2} ms</strong></article>
            </div>
            <section class=\"panel\"><h2>Numerical diagnostics</h2><dl>
                <dt>Steps completed</dt><dd>{}</dd>
                <dt>Requested time step</dt><dd>{:.6e} s</dd>
                <dt>Stable time-step limit</dt><dd>{:.6e} s</dd>
                <dt>Initial energy</dt><dd>{:.6e} J</dd>
                <dt>Final energy</dt><dd>{:.6e} J</dd>
                <dt>Boundary energy</dt><dd>{:.6e} J</dd>
                <dt>Source energy</dt><dd>{:.6e} J</dd>
                <dt>Energy balance residual</dt><dd>{:.6e} J</dd>
                <dt>Relative energy error</dt><dd>{:.6e}</dd>
            </dl></section>",
            result.min_temperature_k,
            result.mean_temperature_k,
            result.max_temperature_k,
            result.elapsed_ms as f64,
            result.steps_completed,
            result.config.time_step_s,
            result.diagnostics.stable_time_step_limit_s,
            result.diagnostics.initial_energy_j,
            result.diagnostics.final_energy_j,
            result.diagnostics.boundary_energy_j,
            result.diagnostics.source_energy_j,
            result.diagnostics.energy_balance_error_j,
            result.diagnostics.energy_balance_relative_error,
        ),
        csv_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("field.csv"),
        &result.metadata.input_fingerprint,
        &result.metadata.solver_id,
        &result.metadata.solver_version,
    );
    let html_path = write_unique(directory, &format!("{stem}_visualization"), "html", html.as_bytes())?;
    Ok(VisualizationFiles {
        html_path,
        data_path: csv_path,
    })
}

/// Writes an unstructured triangular-mesh thermal report. Triangle polygons are
/// coloured from their mean nodal temperature; the CSV retains full nodal and
/// element data, including source-part provenance.
pub fn export_mesh_thermal_visualization(
    result: &MeshThermalResult,
    output_directory: impl AsRef<Path>,
) -> Result<VisualizationFiles, VisualizationError> {
    let mesh = &result.config.mesh;
    if result.final_temperature_k.len() != mesh.nodes.len()
        || result.final_temperature_k.iter().any(|value| !value.is_finite())
        || result.config.materials.is_empty()
        || result.min_temperature_k > result.max_temperature_k
    {
        return Err(VisualizationError::InvalidResult(
            "mesh thermal result has inconsistent nodes, temperatures or material data".into(),
        ));
    }
    mesh.validate(result.config.materials.len())
        .map_err(|error| VisualizationError::InvalidResult(error.to_string()))?;
    let mesh_quality = mesh
        .quality_stats()
        .map_err(|error| VisualizationError::InvalidResult(error.to_string()))?;

    let directory = output_directory.as_ref();
    fs::create_dir_all(directory)?;
    let stem = format!("mesh_thermal_{}", result.metadata.created_unix_ms);
    let csv_path = write_unique(
        directory,
        &format!("{stem}_field"),
        "csv",
        mesh_thermal_csv(result).as_bytes(),
    )?;
    let svg = mesh_thermal_svg(result);
    let html = report_html(
        "OMEGA | Meshed thermal field",
        "Transient 2D finite-element heat conduction on a triangular mesh",
        &format!(
            "{} · {} nodes · {} triangles · {:.3} simulated seconds",
            result.config.name,
            mesh.nodes.len(),
            mesh.triangles.len(),
            result.simulated_time_s,
        ),
        &svg,
        &format!(
            "<div class=\"metrics\">
                <article><span>Minimum</span><strong>{:.3} K</strong></article>
                <article><span>Mean</span><strong>{:.3} K</strong></article>
                <article><span>Maximum</span><strong>{:.3} K</strong></article>
                <article><span>Mesh volume</span><strong>{:.4e} m³</strong></article>
            </div>
            <section class=\"panel\"><h2>Mesh quality</h2><dl>
                <dt>Minimum interior angle</dt><dd>{:.3}°</dd>
                <dt>Maximum radius ratio</dt><dd>{:.4}</dd>
                <dt>Maximum edge length</dt><dd>{:.6e} m</dd>
                <dt>Minimum triangle area</dt><dd>{:.6e} m²</dd>
            </dl></section>
            <section class=\"panel\"><h2>Numerical diagnostics</h2><dl>
                <dt>Time steps</dt><dd>{}</dd>
                <dt>Requested time step</dt><dd>{:.6e} s</dd>
                <dt>Stable time-step limit</dt><dd>{:.6e} s</dd>
                <dt>Boundary nodes</dt><dd>{}</dd>
                <dt>Total heat capacity</dt><dd>{:.6e} J/K</dd>
                <dt>Initial energy</dt><dd>{:.6e} J</dd>
                <dt>Final energy</dt><dd>{:.6e} J</dd>
                <dt>Boundary energy</dt><dd>{:.6e} J</dd>
                <dt>Source energy</dt><dd>{:.6e} J</dd>
                <dt>Energy balance residual</dt><dd>{:.6e} J</dd>
                <dt>Relative energy error</dt><dd>{:.6e}</dd>
            </dl></section>",
            result.min_temperature_k,
            result.mean_temperature_k,
            result.max_temperature_k,
            result.diagnostics.total_volume_m3,
            mesh_quality.minimum_angle_deg,
            mesh_quality.maximum_radius_ratio,
            mesh_quality.maximum_edge_length_m,
            mesh_quality.minimum_triangle_area_m2,
            result.diagnostics.iterations_completed,
            result.config.time_step_s,
            result.diagnostics.stable_time_step_limit_s,
            result.diagnostics.boundary_node_count,
            result.diagnostics.total_heat_capacity_j_k,
            result.diagnostics.initial_energy_j,
            result.diagnostics.final_energy_j,
            result.diagnostics.boundary_energy_j,
            result.diagnostics.source_energy_j,
            result.diagnostics.energy_balance_error_j,
            result.diagnostics.energy_balance_relative_error,
        ),
        csv_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("mesh-field.csv"),
        &result.metadata.input_fingerprint,
        &result.metadata.solver_id,
        &result.metadata.solver_version,
    );
    let html_path = write_unique(directory, &format!("{stem}_visualization"), "html", html.as_bytes())?;
    Ok(VisualizationFiles {
        html_path,
        data_path: csv_path,
    })
}

fn mesh_thermal_svg(result: &MeshThermalResult) -> String {
    let mesh = &result.config.mesh;
    let min_x = mesh
        .nodes
        .iter()
        .map(|node| node.position_m[0])
        .fold(f64::INFINITY, f64::min);
    let max_x = mesh
        .nodes
        .iter()
        .map(|node| node.position_m[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = mesh
        .nodes
        .iter()
        .map(|node| node.position_m[1])
        .fold(f64::INFINITY, f64::min);
    let max_y = mesh
        .nodes
        .iter()
        .map(|node| node.position_m[1])
        .fold(f64::NEG_INFINITY, f64::max);
    let span_x = (max_x - min_x).max(1.0e-9);
    let span_y = (max_y - min_y).max(1.0e-9);
    let plot_x = 70.0;
    let plot_y = 35.0;
    let plot_w = 840.0;
    let plot_h = 500.0;
    let scale = (plot_w / span_x).min(plot_h / span_y);
    let draw_w = span_x * scale;
    let draw_h = span_y * scale;
    let offset_x = plot_x + (plot_w - draw_w) * 0.5;
    let offset_y = plot_y + (plot_h - draw_h) * 0.5;
    let range = (result.max_temperature_k - result.min_temperature_k).max(1.0e-12);
    let mut triangles = String::new();
    for element in &mesh.triangles {
        let points = element.node_ids.map(|id| {
            let point = mesh.nodes[id].position_m;
            let x = offset_x + (point[0] - min_x) * scale;
            let y = offset_y + (max_y - point[1]) * scale;
            (x, y)
        });
        let temperature = element
            .node_ids
            .iter()
            .map(|id| result.final_temperature_k[*id])
            .sum::<f64>()
            / 3.0;
        let color = thermal_mesh_color((temperature - result.min_temperature_k) / range);
        triangles.push_str(&format!(
            "<polygon points=\"{:.3},{:.3} {:.3},{:.3} {:.3},{:.3}\" fill=\"{}\" stroke=\"#09111b\" stroke-width=\"0.55\"><title>{}: {:.4} K</title></polygon>",
            points[0].0, points[0].1, points[1].0, points[1].1, points[2].0, points[2].1,
            color, html_escape(&element.source_part_id), temperature,
        ));
    }
    format!(
        "<svg class=\"plot\" viewBox=\"0 0 980 650\" role=\"img\" aria-label=\"Triangular finite element mesh coloured by temperature\">
            <rect x=\"70\" y=\"35\" width=\"840\" height=\"500\" fill=\"#101a27\" stroke=\"#65778c\"/>
            {triangles}
            <defs><linearGradient id=\"thermal-legend\"><stop offset=\"0%\" stop-color=\"#315bce\"/><stop offset=\"35%\" stop-color=\"#29b7c5\"/><stop offset=\"65%\" stop-color=\"#e8cf55\"/><stop offset=\"100%\" stop-color=\"#d9473f\"/></linearGradient></defs>
            <rect x=\"245\" y=\"570\" width=\"490\" height=\"16\" fill=\"url(#thermal-legend)\"/>
            <text x=\"245\" y=\"610\" class=\"legend-label\">{:.2} K</text>
            <text x=\"735\" y=\"610\" text-anchor=\"end\" class=\"legend-label\">{:.2} K</text>
            <text x=\"490\" y=\"635\" text-anchor=\"middle\" class=\"small-note\">Triangle colours show the mean of the three nodal temperatures; mesh geometry is not smoothed.</text>
        </svg>",
        result.min_temperature_k,
        result.max_temperature_k,
        triangles = triangles,
    )
}

fn thermal_mesh_color(fraction: f64) -> String {
    let fraction = fraction.clamp(0.0, 1.0);
    let stops = [
        (0.0, [49.0, 91.0, 206.0]),
        (0.35, [41.0, 183.0, 197.0]),
        (0.65, [232.0, 207.0, 85.0]),
        (1.0, [217.0, 71.0, 63.0]),
    ];
    let pair = stops
        .windows(2)
        .find(|pair| fraction <= pair[1].0)
        .unwrap_or(&stops[2..4]);
    let amount = ((fraction - pair[0].0) / (pair[1].0 - pair[0].0)).clamp(0.0, 1.0);
    let channel = |index: usize| (pair[0].1[index] + (pair[1].1[index] - pair[0].1[index]) * amount).round() as u8;
    format!("#{:02x}{:02x}{:02x}", channel(0), channel(1), channel(2))
}

fn mesh_thermal_csv(result: &MeshThermalResult) -> String {
    let mut csv = String::from(
        "record_type,id,x_m,y_m,temperature_k,node_a,node_b,node_c,material_id,thickness_m,source_part_id,mean_temperature_k\n",
    );
    for (index, node) in result.config.mesh.nodes.iter().enumerate() {
        csv.push_str(&format!(
            "node,{},{:.12},{:.12},{:.12},,,,,,,\n",
            index, node.position_m[0], node.position_m[1], result.final_temperature_k[index],
        ));
    }
    for (index, triangle) in result.config.mesh.triangles.iter().enumerate() {
        let mean = triangle
            .node_ids
            .iter()
            .map(|id| result.final_temperature_k[*id])
            .sum::<f64>()
            / 3.0;
        csv.push_str(&format!(
            "triangle,{index},,,,{},{},{},{},{},\"{}\",{:.12}\n",
            triangle.node_ids[0],
            triangle.node_ids[1],
            triangle.node_ids[2],
            triangle.material_id,
            triangle.thickness_m,
            csv_escape(&triangle.source_part_id),
            mean,
        ));
    }
    csv
}

fn csv_escape(value: &str) -> String {
    value.replace('\"', "\"\"")
}

/// Writes a deformed-shape SVG report and full nodal/member CSV tables for a 2D
/// pin-jointed civil truss result. Deformation is visually amplified and the
/// scale factor is printed in the report; all CSV values remain physical SI values.
pub fn export_truss_visualization(
    result: &TrussResult,
    output_directory: impl AsRef<Path>,
) -> Result<VisualizationFiles, VisualizationError> {
    validate_truss_result(result)?;
    let directory = output_directory.as_ref();
    fs::create_dir_all(directory)?;
    let stem = format!("truss_{}", result.metadata.created_unix_ms);
    let csv_path = write_unique(
        directory,
        &format!("{stem}_results"),
        "csv",
        truss_csv(result).as_bytes(),
    )?;
    let svg = truss_svg(result);
    let html = report_html(
        "OMEGA | Structural analysis",
        "2D linear-elastic pin-jointed truss",
        &format!(
            "{} · {} nodes · {} members",
            result.config.name,
            result.config.nodes.len(),
            result.config.elements.len()
        ),
        &svg,
        &format!(
            "<div class=\"metrics\">
                <article><span>Maximum displacement</span><strong>{:.4e} m</strong></article>
                <article><span>Maximum axial stress</span><strong>{:.4e} Pa</strong></article>
                <article><span>Maximum member force</span><strong>{:.4e} N</strong></article>
                <article><span>CG iterations</span><strong>{}</strong></article>
            </div>
            <section class=\"panel\"><h2>Numerical diagnostics</h2><dl>
                <dt>Matrix non-zero entries</dt><dd>{}</dd>
                <dt>Relative residual</dt><dd>{:.6e}</dd>
                <dt>Strain energy</dt><dd>{:.6e} J</dd>
                <dt>External work</dt><dd>{:.6e} J</dd>
                <dt>Energy balance residual</dt><dd>{:.6e} J</dd>
                <dt>Relative energy error</dt><dd>{:.6e}</dd>
                <dt>Deformation display scale</dt><dd>{:.4e} × (visual only)</dd>
            </dl></section>
            <section class=\"panel\"><h2>Member results</h2><div class=\"table-wrap\"><table><thead><tr><th>Member</th><th>Axial force (N)</th><th>Axial stress (Pa)</th><th>Strain</th></tr></thead><tbody>{}</tbody></table></div></section>",
            result.max_displacement_m,
            result.max_absolute_stress_pa,
            result.max_absolute_axial_force_n,
            result.diagnostics.iterations,
            result.diagnostics.matrix_nonzero_count,
            result.diagnostics.relative_residual,
            result.diagnostics.strain_energy_j,
            result.diagnostics.external_work_j,
            result.diagnostics.energy_balance_error_j,
            result.diagnostics.energy_balance_relative_error,
            deformation_scale(result),
            truss_member_table(result),
        ),
        csv_path.file_name().and_then(|name| name.to_str()).unwrap_or("truss-results.csv"),
        &result.metadata.input_fingerprint,
        &result.metadata.solver_id,
        &result.metadata.solver_version,
    );
    let html_path = write_unique(directory, &format!("{stem}_visualization"), "html", html.as_bytes())?;
    Ok(VisualizationFiles {
        html_path,
        data_path: csv_path,
    })
}

fn validate_truss_result(result: &TrussResult) -> Result<(), VisualizationError> {
    if result.config.nodes.len() < 2 || result.config.elements.is_empty() {
        return Err(VisualizationError::InvalidResult(
            "a structural plot requires at least two nodes and one member".into(),
        ));
    }
    if result.config.nodes.len() != result.nodes.len() || result.config.elements.len() != result.elements.len() {
        return Err(VisualizationError::InvalidResult(
            "input geometry and result arrays have different lengths".into(),
        ));
    }
    let node_ids: HashMap<u32, ()> = result.nodes.iter().map(|node| (node.node_id, ())).collect();
    let element_ids: HashMap<u32, ()> = result.elements.iter().map(|element| (element.element_id, ())).collect();
    let input_node_ids: HashSet<u32> = result.config.nodes.iter().map(|node| node.id).collect();
    let input_element_ids: HashSet<u32> = result.config.elements.iter().map(|element| element.id).collect();
    if node_ids.len() != result.nodes.len()
        || element_ids.len() != result.elements.len()
        || input_node_ids.len() != result.config.nodes.len()
        || input_element_ids.len() != result.config.elements.len()
        || result.config.nodes.iter().any(|node| !node_ids.contains_key(&node.id))
        || result
            .config
            .elements
            .iter()
            .any(|element| !element_ids.contains_key(&element.id))
        || result
            .config
            .elements
            .iter()
            .any(|element| !input_node_ids.contains(&element.node_a) || !input_node_ids.contains(&element.node_b))
        || result
            .config
            .supports
            .iter()
            .any(|support| !input_node_ids.contains(&support.node_id))
        || result
            .config
            .loads
            .iter()
            .any(|load| !input_node_ids.contains(&load.node_id))
    {
        return Err(VisualizationError::InvalidResult(
            "result identifiers do not match the input geometry".into(),
        ));
    }
    let finite_input_nodes = result
        .config
        .nodes
        .iter()
        .all(|node| node.x_m.is_finite() && node.y_m.is_finite());
    let finite_loads = result
        .config
        .loads
        .iter()
        .all(|load| load.force_x_n.is_finite() && load.force_y_n.is_finite());
    let finite_nodes = result.nodes.iter().all(|node| {
        [
            node.displacement_x_m,
            node.displacement_y_m,
            node.reaction_x_n,
            node.reaction_y_n,
        ]
        .iter()
        .all(|value| value.is_finite())
    });
    let finite_elements = result.elements.iter().all(|element| {
        [
            element.length_m,
            element.axial_force_n,
            element.axial_stress_pa,
            element.axial_strain,
        ]
        .iter()
        .all(|value| value.is_finite())
    });
    if !finite_input_nodes
        || !finite_loads
        || !finite_nodes
        || !finite_elements
        || !result.max_displacement_m.is_finite()
        || !result.max_absolute_stress_pa.is_finite()
        || !result.max_absolute_axial_force_n.is_finite()
        || result.max_displacement_m < 0.0
        || result.max_absolute_stress_pa < 0.0
        || result.max_absolute_axial_force_n < 0.0
    {
        return Err(VisualizationError::InvalidResult(
            "structural results contain a non-finite value".into(),
        ));
    }
    Ok(())
}

fn thermal_svg(result: &ThermalResult) -> String {
    let bins_x = result.width.min(THERMAL_MAX_BINS);
    let bins_y = result.height.min(THERMAL_MAX_BINS);
    let plot_x = 72.0;
    let plot_y = 54.0;
    let plot_w = 760.0;
    let plot_h = 500.0;
    let tile_w = plot_w / bins_x as f64;
    let tile_h = plot_h / bins_y as f64;
    let range = (result.max_temperature_k - result.min_temperature_k).max(1.0e-12);
    let mut cells = String::with_capacity(bins_x * bins_y * 100);

    for by in 0..bins_y {
        let y0 = by * result.height / bins_y;
        let y1 = ((by + 1) * result.height / bins_y).max(y0 + 1);
        for bx in 0..bins_x {
            let x0 = bx * result.width / bins_x;
            let x1 = ((bx + 1) * result.width / bins_x).max(x0 + 1);
            let mut sum = 0.0;
            let mut count = 0usize;
            for y in y0..y1 {
                for x in x0..x1 {
                    sum += result.final_temperature_k[y * result.width + x];
                    count += 1;
                }
            }
            let average = sum / count as f64;
            let normalized = ((average - result.min_temperature_k) / range).clamp(0.0, 1.0);
            let rx = plot_x + bx as f64 * tile_w;
            let ry = plot_y + by as f64 * tile_h;
            cells.push_str(&format!(
                "<rect x=\"{rx:.3}\" y=\"{ry:.3}\" width=\"{:.3}\" height=\"{:.3}\" fill=\"{}\"><title>Block ({x0}..{x1}, {y0}..{y1}) · mean {average:.4} K</title></rect>",
                tile_w + 0.1,
                tile_h + 0.1,
                temperature_color(normalized),
            ));
        }
    }

    format!(
        "<svg class=\"plot\" viewBox=\"0 0 980 640\" role=\"img\" aria-label=\"Downsampled thermal field heatmap\">
            <defs><linearGradient id=\"thermal-scale\" x1=\"0\" x2=\"1\"><stop offset=\"0%\" stop-color=\"#183c8c\"/><stop offset=\"35%\" stop-color=\"#16a6c7\"/><stop offset=\"68%\" stop-color=\"#ffe066\"/><stop offset=\"100%\" stop-color=\"#c0392b\"/></linearGradient></defs>
            <rect x=\"{plot_x}\" y=\"{plot_y}\" width=\"{plot_w}\" height=\"{plot_h}\" fill=\"#101a27\" stroke=\"#65778c\" stroke-width=\"1\"/>{cells}
            <text x=\"452\" y=\"600\" text-anchor=\"middle\" class=\"axis-label\">X position · {x_extent:.4} m</text>
            <text x=\"24\" y=\"304\" text-anchor=\"middle\" transform=\"rotate(-90 24 304)\" class=\"axis-label\">Y position · {y_extent:.4} m</text>
            <rect x=\"860\" y=\"100\" width=\"24\" height=\"380\" fill=\"url(#thermal-scale)\"/>
            <text x=\"895\" y=\"108\" class=\"legend-label\">{max:.2} K</text>
            <text x=\"895\" y=\"484\" class=\"legend-label\">{min:.2} K</text>
            <text x=\"872\" y=\"520\" text-anchor=\"middle\" class=\"legend-label\">TEMP</text>
            <text x=\"72\" y=\"628\" class=\"small-note\">Rendered at {bins_x} × {bins_y} blocks by averaging. Full-resolution field is available in the CSV.</text>
        </svg>",
        cells = cells,
        x_extent = result.width as f64 * result.config.cell_size_x_m,
        y_extent = result.height as f64 * result.config.cell_size_y_m,
        max = result.max_temperature_k,
        min = result.min_temperature_k,
        bins_x = bins_x,
        bins_y = bins_y,
    )
}

fn temperature_color(value: f64) -> String {
    const STOPS: [(f64, [f64; 3]); 4] = [
        (0.0, [24.0, 60.0, 140.0]),
        (0.35, [22.0, 166.0, 199.0]),
        (0.68, [255.0, 224.0, 102.0]),
        (1.0, [192.0, 57.0, 43.0]),
    ];
    let value = value.clamp(0.0, 1.0);
    for pair in STOPS.windows(2) {
        let (start, first) = pair[0];
        let (end, second) = pair[1];
        if value <= end {
            let t = ((value - start) / (end - start)).clamp(0.0, 1.0);
            let channel = |index: usize| (first[index] + (second[index] - first[index]) * t).round() as u8;
            return format!("#{:02x}{:02x}{:02x}", channel(0), channel(1), channel(2));
        }
    }
    "#c0392b".to_string()
}

fn thermal_csv(result: &ThermalResult) -> String {
    let mut csv = String::from("x_index,y_index,x_center_m,y_center_m,temperature_k\n");
    for y in 0..result.height {
        for x in 0..result.width {
            let temperature = result.final_temperature_k[y * result.width + x];
            csv.push_str(&format!(
                "{x},{y},{:.9},{:.9},{temperature:.12}\n",
                (x as f64 + 0.5) * result.config.cell_size_x_m,
                (y as f64 + 0.5) * result.config.cell_size_y_m,
            ));
        }
    }
    csv
}

fn truss_svg(result: &TrussResult) -> String {
    let x_min = result
        .config
        .nodes
        .iter()
        .map(|node| node.x_m)
        .fold(f64::INFINITY, f64::min);
    let x_max = result
        .config
        .nodes
        .iter()
        .map(|node| node.x_m)
        .fold(f64::NEG_INFINITY, f64::max);
    let y_min = result
        .config
        .nodes
        .iter()
        .map(|node| node.y_m)
        .fold(f64::INFINITY, f64::min);
    let y_max = result
        .config
        .nodes
        .iter()
        .map(|node| node.y_m)
        .fold(f64::NEG_INFINITY, f64::max);
    let span_x = (x_max - x_min).max(1.0e-6);
    let span_y = (y_max - y_min).max(1.0e-6);
    let plot_x = 80.0;
    let plot_y = 70.0;
    let plot_w = 820.0;
    let plot_h = 470.0;
    let scale = (plot_w / span_x).min(plot_h / span_y);
    let centre_x = (x_min + x_max) * 0.5;
    let centre_y = (y_min + y_max) * 0.5;
    let map_point = |x: f64, y: f64| -> (f64, f64) {
        (
            plot_x + plot_w * 0.5 + (x - centre_x) * scale,
            plot_y + plot_h * 0.5 - (y - centre_y) * scale,
        )
    };
    let max_displacement = result.max_displacement_m;
    let geometric_diagonal = span_x.hypot(span_y);
    let exaggeration = if max_displacement > 0.0 {
        0.12 * geometric_diagonal / max_displacement
    } else {
        1.0
    };
    let node_results: HashMap<u32, _> = result.nodes.iter().map(|node| (node.node_id, node)).collect();
    let element_results: HashMap<u32, _> = result
        .elements
        .iter()
        .map(|element| (element.element_id, element))
        .collect();
    let node_inputs: HashMap<u32, _> = result.config.nodes.iter().map(|node| (node.id, node)).collect();
    let mut members = String::new();

    for element in &result.config.elements {
        let Some(a) = node_inputs.get(&element.node_a) else {
            continue;
        };
        let Some(b) = node_inputs.get(&element.node_b) else {
            continue;
        };
        let original_a = map_point(a.x_m, a.y_m);
        let original_b = map_point(b.x_m, b.y_m);
        let result_a = node_results[&a.id];
        let result_b = node_results[&b.id];
        let deformed_a = map_point(
            a.x_m + result_a.displacement_x_m * exaggeration,
            a.y_m + result_a.displacement_y_m * exaggeration,
        );
        let deformed_b = map_point(
            b.x_m + result_b.displacement_x_m * exaggeration,
            b.y_m + result_b.displacement_y_m * exaggeration,
        );
        let member = element_results[&element.id];
        let color = stress_color(member.axial_stress_pa, result.max_absolute_stress_pa);
        members.push_str(&format!(
            "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" class=\"original-member\"/><line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{color}\" stroke-width=\"5\" stroke-linecap=\"round\"><title>Member {} · force {:.4e} N · stress {:.4e} Pa · {} </title></line>",
            original_a.0, original_a.1, original_b.0, original_b.1,
            deformed_a.0, deformed_a.1, deformed_b.0, deformed_b.1,
            element.id, member.axial_force_n, member.axial_stress_pa,
            if member.axial_force_n >= 0.0 { "tension" } else { "compression" },
        ));
    }

    let mut node_marks = String::new();
    for node in &result.config.nodes {
        let displacement = node_results[&node.id];
        let point = map_point(
            node.x_m + displacement.displacement_x_m * exaggeration,
            node.y_m + displacement.displacement_y_m * exaggeration,
        );
        node_marks.push_str(&format!(
            "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"6\" class=\"node-point\"/><text x=\"{:.2}\" y=\"{:.2}\" class=\"node-label\">N{}</text>",
            point.0, point.1, point.0 + 8.0, point.1 - 10.0, node.id
        ));
    }

    let mut supports = String::new();
    for support in &result.config.supports {
        if let Some(node) = node_inputs.get(&support.node_id) {
            let (x, y) = map_point(node.x_m, node.y_m);
            supports.push_str(&format!(
                "<path d=\"M {:.2} {:.2} l -12 20 h 24 z\" class=\"support\"/><line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" class=\"support-ground\"/>",
                x, y + 8.0, x - 17.0, y + 29.0, x + 17.0, y + 29.0
            ));
        }
    }

    let mut loads = String::new();
    for load in &result.config.loads {
        if let Some(node) = node_inputs.get(&load.node_id) {
            let (x, y) = map_point(node.x_m, node.y_m);
            let fx = load.force_x_n;
            let fy = load.force_y_n;
            let magnitude = fx.hypot(fy);
            if magnitude > 0.0 {
                let dx = fx / magnitude * 42.0;
                let dy = -fy / magnitude * 42.0;
                let end_x = x + dx;
                let end_y = y + dy;
                loads.push_str(&format!(
                    "<line x1=\"{x:.2}\" y1=\"{y:.2}\" x2=\"{end_x:.2}\" y2=\"{end_y:.2}\" class=\"load-arrow\"/><text x=\"{:.2}\" y=\"{:.2}\" class=\"load-label\">{:.3e} N</text>",
                    end_x + 4.0, end_y - 5.0, magnitude
                ));
            }
        }
    }

    format!(
        "<svg class=\"plot\" viewBox=\"0 0 980 650\" role=\"img\" aria-label=\"Civil truss undeformed and deformed geometry with member stress coloring\">
            <defs><marker id=\"arrow-head\" markerWidth=\"8\" markerHeight=\"8\" refX=\"5\" refY=\"3\" orient=\"auto\"><path d=\"M0,0 L0,6 L6,3 z\" fill=\"#f5c451\"/></marker></defs>
            <rect x=\"{plot_x}\" y=\"{plot_y}\" width=\"{plot_w}\" height=\"{plot_h}\" fill=\"#101a27\" stroke=\"#65778c\"/>
            {members}{supports}{loads}{node_marks}
            <line x1=\"90\" y1=\"585\" x2=\"140\" y2=\"585\" class=\"original-member\"/><text x=\"150\" y=\"590\" class=\"legend-label\">Undeformed</text>
            <line x1=\"280\" y1=\"585\" x2=\"330\" y2=\"585\" stroke=\"#df665b\" stroke-width=\"5\"/><text x=\"340\" y=\"590\" class=\"legend-label\">Tension</text>
            <line x1=\"440\" y1=\"585\" x2=\"490\" y2=\"585\" stroke=\"#4f9bd8\" stroke-width=\"5\"/><text x=\"500\" y=\"590\" class=\"legend-label\">Compression</text>
            <line x1=\"650\" y1=\"585\" x2=\"700\" y2=\"585\" class=\"load-arrow\"/><text x=\"710\" y=\"590\" class=\"legend-label\">Applied load</text>
            <text x=\"80\" y=\"630\" class=\"small-note\">Deformation display scale: {exaggeration:.4e}×. Geometry is exaggerated for visibility; numerical exports remain in SI units.</text>
        </svg>",
        members = members,
        supports = supports,
        loads = loads,
        node_marks = node_marks,
        plot_x = plot_x,
        plot_y = plot_y,
        plot_w = plot_w,
        plot_h = plot_h,
        exaggeration = exaggeration,
    )
}

fn deformation_scale(result: &TrussResult) -> f64 {
    let x_min = result
        .config
        .nodes
        .iter()
        .map(|node| node.x_m)
        .fold(f64::INFINITY, f64::min);
    let x_max = result
        .config
        .nodes
        .iter()
        .map(|node| node.x_m)
        .fold(f64::NEG_INFINITY, f64::max);
    let y_min = result
        .config
        .nodes
        .iter()
        .map(|node| node.y_m)
        .fold(f64::INFINITY, f64::min);
    let y_max = result
        .config
        .nodes
        .iter()
        .map(|node| node.y_m)
        .fold(f64::NEG_INFINITY, f64::max);
    let diagonal = (x_max - x_min).hypot(y_max - y_min);
    if result.max_displacement_m > 0.0 {
        0.12 * diagonal.max(1.0e-6) / result.max_displacement_m
    } else {
        1.0
    }
}

fn stress_color(stress: f64, maximum: f64) -> &'static str {
    if maximum <= f64::EPSILON || stress.abs() <= maximum * 1.0e-8 {
        "#9aa8b8"
    } else if stress > 0.0 {
        "#df665b"
    } else {
        "#4f9bd8"
    }
}

fn truss_member_table(result: &TrussResult) -> String {
    let mut rows = String::new();
    for member in &result.elements {
        rows.push_str(&format!(
            "<tr><td>{}</td><td>{:.5e}</td><td>{:.5e}</td><td>{:.5e}</td></tr>",
            member.element_id, member.axial_force_n, member.axial_stress_pa, member.axial_strain
        ));
    }
    rows
}

fn truss_csv(result: &TrussResult) -> String {
    let node_results: HashMap<u32, _> = result.nodes.iter().map(|node| (node.node_id, node)).collect();
    let mut csv = String::from(
        "record_type,id,x_m,y_m,displacement_x_m,displacement_y_m,reaction_x_n,reaction_y_n,length_m,axial_force_n,axial_stress_pa,axial_strain\n",
    );
    for node in &result.config.nodes {
        let value = node_results[&node.id];
        csv.push_str(&format!(
            "node,{},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},,,,\n",
            node.id,
            node.x_m,
            node.y_m,
            value.displacement_x_m,
            value.displacement_y_m,
            value.reaction_x_n,
            value.reaction_y_n
        ));
    }
    for element in &result.elements {
        csv.push_str(&format!(
            "member,{id},,,,,,,{length:.12},{force:.12},{stress:.12},{strain:.12}\n",
            id = element.element_id,
            length = element.length_m,
            force = element.axial_force_n,
            stress = element.axial_stress_pa,
            strain = element.axial_strain,
        ));
    }
    csv
}

fn report_html(
    title: &str,
    heading: &str,
    subtitle: &str,
    svg: &str,
    metrics: &str,
    data_filename: &str,
    fingerprint: &str,
    solver: &str,
    solver_version: &str,
) -> String {
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{}</title>
        <style>
        :root{{color-scheme:dark;font-family:Inter,ui-sans-serif,system-ui,sans-serif;background:#09111b;color:#e8eef5}}
        *{{box-sizing:border-box}}body{{margin:0;padding:28px;background:radial-gradient(circle at 15% 0%,#172b43 0,#09111b 48%)}}
        main{{max-width:1180px;margin:0 auto}}header{{margin:0 0 20px}}.eyebrow{{color:#70c6d7;font-size:12px;letter-spacing:.16em;text-transform:uppercase}}
        h1{{font-size:clamp(25px,4vw,38px);margin:7px 0}}.subtitle{{color:#9cafc2}}.panel{{background:#111e2c;border:1px solid #2b3d50;border-radius:14px;padding:18px;margin:16px 0}}
        .plot{{width:100%;height:auto;display:block}}.metrics{{display:grid;grid-template-columns:repeat(auto-fit,minmax(170px,1fr));gap:12px;margin:14px 0}}
        .metrics article{{background:#111e2c;border:1px solid #2b3d50;border-radius:12px;padding:16px}}.metrics span{{display:block;color:#96a9bd;font-size:12px}}
        .metrics strong{{display:block;margin-top:7px;font-size:20px;font-variant-numeric:tabular-nums}}h2{{font-size:17px;margin:0 0 14px}}
        dl{{display:grid;grid-template-columns:minmax(180px,1fr) 1.2fr;gap:9px 18px;margin:0}}dt{{color:#96a9bd}}dd{{margin:0;font-variant-numeric:tabular-nums;overflow-wrap:anywhere}}
        .table-wrap{{overflow-x:auto}}table{{width:100%;border-collapse:collapse;font-size:13px}}th,td{{text-align:right;padding:9px;border-bottom:1px solid #2b3d50;font-variant-numeric:tabular-nums}}th:first-child,td:first-child{{text-align:left}}
        .foot{{color:#8799ab;font-size:12px;line-height:1.7;overflow-wrap:anywhere}}.download{{display:inline-block;color:#75d2e2;margin-bottom:12px}}.axis-label,.legend-label{{fill:#d5dfeb;font-size:13px}}.small-note{{fill:#8fa2b5;font-size:11px}}
        .original-member{{stroke:#8493a3;stroke-width:2.5;stroke-dasharray:7 6;fill:none}}.node-point{{fill:#eff5fb;stroke:#09111b;stroke-width:2}}
        .node-label{{fill:#eff5fb;font-size:12px;font-weight:700}}.support{{fill:#607387;stroke:#c4d2df;stroke-width:1.5}}.support-ground{{stroke:#c4d2df;stroke-width:2}}
        .load-arrow{{stroke:#f5c451;stroke-width:3;marker-end:url(#arrow-head)}}.load-label{{fill:#f5c451;font-size:12px}}
        @media(max-width:650px){{body{{padding:12px}}dl{{grid-template-columns:1fr;gap:4px}}dd{{margin-bottom:8px}}}}
        </style></head><body><main><header><div class=\"eyebrow\">OMEGA · Scientific report</div><h1>{}</h1><div class=\"subtitle\">{}</div></header>
        <section class=\"panel\">{}</section>{}<section class=\"panel foot\"><a class=\"download\" href=\"{}\" download>Download full-resolution CSV data</a><br><strong>Provenance</strong><br>Solver: {} v{}<br>Input fingerprint: {}<br>Generated from solver output. This visualization is a diagnostic aid, not a certification report.</section></main></body></html>",
        html_escape(title), html_escape(heading), html_escape(subtitle), svg, metrics,
        html_escape(data_filename), html_escape(solver), html_escape(solver_version), html_escape(fingerprint),
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn write_unique(directory: &Path, stem: &str, extension: &str, contents: &[u8]) -> Result<PathBuf, VisualizationError> {
    for collision in 0_u32..10_000 {
        let suffix = if collision == 0 {
            String::new()
        } else {
            format!("_{collision}")
        };
        let path = directory.join(format!("{stem}{suffix}.{extension}"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(contents) {
                    let _ = fs::remove_file(&path);
                    return Err(VisualizationError::Io(error));
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(VisualizationError::Io(error)),
        }
    }
    Err(VisualizationError::InvalidResult(
        "could not allocate a unique report filename".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TrussConfig, solve_truss};
    use rayon::ThreadPoolBuilder;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("omega-viz-{label}-{nonce}"))
    }

    #[test]
    fn thermal_visualization_exports_html_and_full_resolution_csv() {
        let pool = ThreadPoolBuilder::new().num_threads(2).build().unwrap();
        let config = crate::ThermalConfig {
            width: 8,
            height: 8,
            time_step_s: 0.001,
            steps: 2,
            heat_patch: None,
            boundary_conditions: crate::BoundaryConditions::insulated(),
            ..Default::default()
        };
        let result = crate::simulate(&config, &pool, |_| {}).unwrap();
        let directory = temp_dir("thermal");
        let files = export_thermal_visualization(&result, &directory).unwrap();
        let html = fs::read_to_string(&files.html_path).unwrap();
        let csv = fs::read_to_string(&files.data_path).unwrap();
        assert!(html.contains("<svg"));
        assert!(html.contains("Energy balance residual"));
        assert!(html.contains(files.data_path.file_name().unwrap().to_str().unwrap()));
        assert_eq!(csv.lines().count(), 65);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn mesh_thermal_visualization_exports_mesh_svg_and_source_provenance() {
        let pool = ThreadPoolBuilder::new().num_threads(2).build().unwrap();
        let config = crate::MeshThermalConfig {
            steps: 3,
            time_step_s: 0.01,
            ..Default::default()
        };
        let result = crate::simulate_mesh_thermal(&config, &pool).unwrap();
        let directory = temp_dir("mesh-thermal");
        let files = export_mesh_thermal_visualization(&result, &directory).unwrap();
        let html = fs::read_to_string(&files.html_path).unwrap();
        let csv = fs::read_to_string(&files.data_path).unwrap();
        assert!(html.contains("triangular mesh"));
        assert!(html.contains("<polygon"));
        assert!(html.contains("Boundary nodes"));
        assert!(html.contains("Mesh quality"));
        assert!(html.contains("Minimum interior angle"));
        assert!(csv.contains("source_part_id"));
        assert!(csv.contains("reference-plate"));
        assert_eq!(
            csv.lines().count(),
            result.config.mesh.nodes.len() + result.config.mesh.triangles.len() + 1
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn truss_visualization_exports_deformed_shape_and_tables() {
        let pool = ThreadPoolBuilder::new().num_threads(2).build().unwrap();
        let result = solve_truss(&TrussConfig::default(), &pool, |_| {}).unwrap();
        let directory = temp_dir("truss");
        let files = export_truss_visualization(&result, &directory).unwrap();
        let html = fs::read_to_string(&files.html_path).unwrap();
        let csv = fs::read_to_string(&files.data_path).unwrap();
        assert!(html.contains("Deformation display scale"));
        assert!(html.contains("Member results"));
        assert!(html.contains(files.data_path.file_name().unwrap().to_str().unwrap()));
        assert!(csv.contains("record_type,id"));
        assert!(csv.lines().count() > result.config.nodes.len());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn empty_truss_geometry_is_rejected() {
        let pool = ThreadPoolBuilder::new().num_threads(2).build().unwrap();
        let mut result = solve_truss(&TrussConfig::default(), &pool, |_| {}).unwrap();
        result.config.nodes.clear();
        result.nodes.clear();
        let directory = temp_dir("empty-truss");
        assert!(matches!(
            export_truss_visualization(&result, &directory),
            Err(VisualizationError::InvalidResult(_))
        ));
    }

    #[test]
    fn invalid_thermal_field_is_rejected() {
        let pool = ThreadPoolBuilder::new().num_threads(1).build().unwrap();
        let config = crate::ThermalConfig {
            width: 8,
            height: 8,
            time_step_s: 0.001,
            steps: 1,
            heat_patch: None,
            ..Default::default()
        };
        let mut result = crate::simulate(&config, &pool, |_| {}).unwrap();
        result.final_temperature_k.pop();
        assert!(matches!(
            export_thermal_visualization(&result, temp_dir("invalid")),
            Err(VisualizationError::InvalidResult(_))
        ));
    }
}
