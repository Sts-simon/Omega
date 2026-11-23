use aerolab_sim_core::{TrussConfig, export_truss_visualization, solve_truss};
use rayon::ThreadPoolBuilder;
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let detected = std::thread::available_parallelism().map(usize::from).unwrap_or(2);
    let threads = std::env::var("AEROLAB_WORKER_THREADS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(|| detected.saturating_sub(1).max(1))
        .clamp(1, 32);
    let pool = ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|index| format!("aerolab-truss-{index}"))
        .build()?;
    let config_path = std::env::args_os().nth(1).map(PathBuf::from);
    let config = if let Some(path) = &config_path {
        serde_json::from_reader::<_, TrussConfig>(File::open(path)?)?
    } else {
        TrussConfig::default()
    };

    println!("AEROLAB Scientific Core | 2D civil truss statics");
    println!("Study: {}", config.name);
    println!("Nodes: {} | members: {} | worker threads: {}", config.nodes.len(), config.elements.len(), threads);
    let result = solve_truss(&config, &pool, |fraction| {
        let percent = (fraction * 100.0).round() as u32;
        if percent == 0 || percent == 85 || percent == 100 {
            println!("Progress: {percent}%");
        }
    })?;

    println!("\nCompleted in {:.3} s wall time", result.elapsed_ms as f64 / 1_000.0);
    println!("Maximum nodal displacement: {:.6e} m", result.max_displacement_m);
    println!("Maximum absolute axial stress: {:.6e} Pa", result.max_absolute_stress_pa);
    println!("Maximum absolute member force: {:.6e} N", result.max_absolute_axial_force_n);
    println!("CG iterations: {} | relative residual: {:.3e}", result.diagnostics.iterations, result.diagnostics.relative_residual);
    println!("Strain energy: {:.6e} J | external work: {:.6e} J", result.diagnostics.strain_energy_j, result.diagnostics.external_work_j);
    println!("Energy consistency residual: {:.3e}", result.diagnostics.energy_balance_relative_error);
    println!("Input fingerprint: {}", result.metadata.input_fingerprint);

    let report_directory = PathBuf::from("results");
    fs::create_dir_all(&report_directory)?;
    let timestamp = result.metadata.created_unix_ms;
    let mut collision = 0_u32;
    let report_path = loop {
        let suffix = if collision == 0 { String::new() } else { format!("_{collision}") };
        let path = report_directory.join(format!("truss_study_{timestamp}{suffix}.json"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => {
                serde_json::to_writer_pretty(file, &result)?;
                break path;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => collision += 1,
            Err(error) => return Err(error.into()),
        }
    };
    println!("Full report saved to {}", report_path.display());
    let visualizations = export_truss_visualization(&result, &report_directory)?;
    println!("Browser visualization saved to {}", visualizations.html_path.display());
    println!("Full-resolution CSV saved to {}", visualizations.data_path.display());
    Ok(())
}
