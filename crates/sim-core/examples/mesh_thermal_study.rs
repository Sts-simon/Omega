use omega_sim_core::{MeshThermalConfig, export_mesh_thermal_visualization, simulate_mesh_thermal};
use rayon::ThreadPoolBuilder;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = if let Some(path) = std::env::args().nth(1) {
        let bytes = fs::read(&path)?;
        serde_json::from_slice::<MeshThermalConfig>(&bytes)?
    } else {
        MeshThermalConfig::default()
    };
    let threads = std::env::var("OMEGA_WORKER_THREADS")
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
    println!(
        "Mesh: {} nodes, {} triangles, {} materials",
        config.mesh.nodes.len(),
        config.mesh.triangles.len(),
        config.materials.len()
    );
    println!("Worker threads: {threads}");

    let result = simulate_mesh_thermal(&config, &pool)?;
    fs::create_dir_all("results")?;
    let json_path = unique_path(
        "results",
        &format!("mesh_thermal_{}", result.metadata.created_unix_ms),
        "json",
    )?;
    let bytes = serde_json::to_vec_pretty(&result)?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(&json_path)?;
    file.write_all(&bytes)?;
    let visualizations = export_mesh_thermal_visualization(&result, "results")?;

    println!(
        "Temperature range: {:.4} .. {:.4} K (mean {:.4} K)",
        result.min_temperature_k, result.max_temperature_k, result.mean_temperature_k
    );
    println!(
        "Area: {:.6} m² | volume: {:.6} m³",
        result.diagnostics.total_area_m2, result.diagnostics.total_volume_m3
    );
    println!(
        "Stable dt: {:.6e} s | requested dt: {:.6e} s",
        result.diagnostics.stable_time_step_limit_s, result.config.time_step_s
    );
    println!(
        "Energy balance relative error: {:.6e}",
        result.diagnostics.energy_balance_relative_error
    );
    println!("JSON: {}", json_path.display());
    println!("HTML: {}", visualizations.html_path.display());
    println!("CSV: {}", visualizations.data_path.display());
    Ok(())
}

fn unique_path(directory: &str, stem: &str, extension: &str) -> Result<PathBuf, std::io::Error> {
    for index in 0_u32..10_000 {
        let suffix = if index == 0 { String::new() } else { format!("_{index}") };
        let path = PathBuf::from(directory).join(format!("{stem}{suffix}.{extension}"));
        if !path.exists() {
            return Ok(path);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate output filename",
    ))
}
