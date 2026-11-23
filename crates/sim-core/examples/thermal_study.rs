use omega_sim_core::{ThermalConfig, export_thermal_visualization, simulate};
use rayon::ThreadPoolBuilder;
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let detected = std::thread::available_parallelism().map(usize::from).unwrap_or(2);
    let threads = std::env::var("OMEGA_WORKER_THREADS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(|| detected.saturating_sub(1).max(1))
        .clamp(1, 32);
    let pool = ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|index| format!("omega-sim-{index}"))
        .build()?;
    let config_path = std::env::args_os().nth(1).map(PathBuf::from);
    let config = if let Some(path) = &config_path {
        let file = File::open(path)?;
        serde_json::from_reader::<_, ThermalConfig>(file)?
    } else {
        ThermalConfig::default()
    };

    println!("OMEGA Scientific Core | 2D finite-volume heat conduction");
    if let Some(path) = &config_path {
        println!("Input deck: {}", path.display());
    } else {
        println!("Input deck: built-in default");
    }
    println!(
        "Grid: {}x{} | cells: {} | workers: {} | requested dt: {:.5} s | steps: {}",
        config.width,
        config.height,
        config.width.saturating_mul(config.height),
        threads,
        config.time_step_s,
        config.steps
    );

    let result = simulate(&config, &pool, |fraction| {
        let percent = (fraction * 100.0).round() as u32;
        if percent.is_multiple_of(10) {
            println!("Progress: {percent:3}%");
        }
    })?;

    println!("\nCompleted in {:.3} s wall time", result.elapsed_ms as f64 / 1_000.0);
    println!("Simulated time: {:.3} s", result.simulated_time_s);
    println!(
        "Stable time-step limit: {:.6} s",
        result.diagnostics.stable_time_step_limit_s
    );
    println!(
        "Temperature: min {:.3} K | mean {:.3} K | max {:.3} K",
        result.min_temperature_k, result.mean_temperature_k, result.max_temperature_k
    );
    println!(
        "Energy balance: absolute {:.6e} J | relative {:.6e}",
        result.diagnostics.energy_balance_error_j, result.diagnostics.energy_balance_relative_error
    );
    println!("Input fingerprint: {}", result.metadata.input_fingerprint);

    let report_directory = PathBuf::from("results");
    fs::create_dir_all(&report_directory)?;
    let timestamp = result.metadata.created_unix_ms;
    let mut collision = 0_u32;
    let report_path = loop {
        let suffix = if collision == 0 {
            String::new()
        } else {
            format!("_{collision}")
        };
        let path = report_directory.join(format!("thermal_study_{timestamp}{suffix}.json"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => {
                serde_json::to_writer_pretty(file, &result)?;
                break path;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                collision = collision.saturating_add(1);
            }
            Err(error) => return Err(error.into()),
        }
    };
    println!("Full field and diagnostics saved to {}", report_path.display());
    let visualizations = export_thermal_visualization(&result, &report_directory)?;
    println!("Browser visualization saved to {}", visualizations.html_path.display());
    println!("Full-resolution CSV saved to {}", visualizations.data_path.display());
    Ok(())
}
