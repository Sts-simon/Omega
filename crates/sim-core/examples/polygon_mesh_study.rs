use aerolab_sim_core::{
    Material, MeshThermalConfig, PlanarPolygonRegion, export_mesh_thermal_visualization,
    mesh_planar_polygon, simulate_mesh_thermal,
};
use rayon::ThreadPoolBuilder;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let region = if let Some(path) = std::env::args().nth(1) {
        serde_json::from_slice::<PlanarPolygonRegion>(&fs::read(path)?)?
    } else {
        serde_json::from_str::<PlanarPolygonRegion>(include_str!("perforated-panel-polygon.json"))?
    };
    let mesh = mesh_planar_polygon(&region)?;
    let stats = mesh.validate(usize::from(region.material_id) + 1)?;
    let quality = mesh.quality_stats()?;
    let config = MeshThermalConfig {
        name: format!("{} | polygon-with-holes thermal study", region.source_part_id),
        mesh,
        materials: vec![Material::aluminium()],
        initial_temperature_k: 293.15,
        // The existing slab solver applies this value to every mesh boundary,
        // including the boundary of each hole.
        fixed_boundary_temperature_k: Some(293.15),
        time_step_s: 0.01,
        steps: 300,
        volumetric_heat_source_w_m3: 10_000.0,
    };
    let threads = std::env::var("AEROLAB_WORKER_THREADS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1)
                .saturating_sub(1)
                .max(1)
        })
        .clamp(1, 16);
    let pool = ThreadPoolBuilder::new().num_threads(threads).build()?;

    println!("Study: {}", config.name);
    println!("Polygon: {} outer vertices, {} holes", region.outer_boundary_m.len(), region.holes_m.len());
    println!("Mesh: {} nodes, {} triangles, {} boundary edges", stats.node_count, stats.triangle_count, stats.boundary_edge_count);
    println!("Area: {:.6} m² | volume: {:.6} m³", stats.total_area_m2, stats.total_volume_m3);
    println!("Mesh quality: minimum angle {:.2}°, maximum radius ratio {:.3}", quality.minimum_angle_deg, quality.maximum_radius_ratio);
    println!("Worker threads: {threads}");

    let result = simulate_mesh_thermal(&config, &pool)?;
    fs::create_dir_all("results")?;
    let json_path = unique_path("results", &format!("polygon_thermal_{}", result.metadata.created_unix_ms), "json")?;
    let bytes = serde_json::to_vec_pretty(&result)?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(&json_path)?;
    file.write_all(&bytes)?;
    let visualizations = export_mesh_thermal_visualization(&result, "results")?;

    println!("Temperature range: {:.4} .. {:.4} K (mean {:.4} K)", result.min_temperature_k, result.max_temperature_k, result.mean_temperature_k);
    println!("Energy balance relative error: {:.6e}", result.diagnostics.energy_balance_relative_error);
    println!("JSON: {}", json_path.display());
    println!("HTML: {}", visualizations.html_path.display());
    println!("CSV: {}", visualizations.data_path.display());
    Ok(())
}

fn unique_path(directory: &str, stem: &str, extension: &str) -> Result<PathBuf, std::io::Error> {
    for index in 0_u32..10_000 {
        let suffix = if index == 0 { String::new() } else { format!("_{index}") };
        let path = PathBuf::from(directory).join(format!("{stem}{suffix}.{extension}"));
        if !path.exists() { return Ok(path); }
    }
    Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, "could not allocate output filename"))
}
