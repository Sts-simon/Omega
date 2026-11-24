//! Screenless trial runner: `omega --trial <slot-dir> [--seed N] [--air]` loads the design and the
//! program stored in a save slot, flies one seeded trial and prints a JSON report. No window is
//! opened, so a lot's flight acceptance criteria can be checked over SSH or in CI.

use omega_rules::flight::{FlightConfig, LaunchMode, simulate};
use omega_rules::{Program, Vehicle};
use std::path::Path;

const USAGE: &str = "usage: omega --trial <slot-dir> [--seed N] [--air]";

/// Runs the headless mode when `--trial` is present. `None` means "start the game normally".
pub(crate) fn main() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.iter().any(|arg| arg == "--trial") {
        return None;
    }
    match run(&args) {
        Ok(report) => {
            println!("{report}");
            Some(0)
        }
        Err(error) => {
            eprintln!("omega: {error}");
            Some(1)
        }
    }
}

fn value_of<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let index = args.iter().position(|arg| arg == flag)?;
    args.get(index + 1).map(String::as_str)
}

fn run(args: &[String]) -> Result<String, String> {
    let dir = value_of(args, "--trial").ok_or_else(|| USAGE.to_string())?;
    let config = config(args)?;
    let slot = Path::new(dir);
    let vehicle_path = slot.join("designs/vehicle.json");
    let program_path = slot.join("programs/missile.json");
    let vehicle = std::fs::read(&vehicle_path).map_err(|error| format!("{}: {error}", vehicle_path.display()))?;
    let program = std::fs::read(&program_path).map_err(|error| format!("{}: {error}", program_path.display()))?;
    trial_json(&vehicle, &program, config)
}

fn config(args: &[String]) -> Result<FlightConfig, String> {
    let seed = match value_of(args, "--seed") {
        Some(value) => value
            .parse::<u64>()
            .map_err(|_| format!("--seed needs a whole number\n{USAGE}"))?,
        None => 1,
    };
    Ok(FlightConfig {
        launch: if args.iter().any(|arg| arg == "--air") {
            LaunchMode::Air {
                speed_ms: 200.0,
                altitude_m: 0.0,
            }
        } else {
            LaunchMode::Vertical
        },
        seed,
        ..FlightConfig::default()
    })
}

fn trial_json(vehicle_bytes: &[u8], program_bytes: &[u8], config: FlightConfig) -> Result<String, String> {
    let vehicle: Vehicle = serde_json::from_slice(vehicle_bytes).map_err(|error| format!("design: {error}"))?;
    let program: Program = serde_json::from_slice(program_bytes).map_err(|error| format!("program: {error}"))?;
    let figures = omega_rules::aero::figures(&vehicle);
    let report = simulate(&vehicle, &program, config).map_err(|issues| issues.join("; "))?;
    let lock = report.lock.map(|lock| {
        serde_json::json!({
            "success": lock.success,
            "probability": lock.probability,
            "speed_ms": lock.speed_ms,
        })
    });
    let json = serde_json::json!({
        "design": vehicle.name,
        "seed": config.seed,
        "launch": if matches!(config.launch, LaunchMode::Air { .. }) { "air" } else { "vertical" },
        "aero": {
            "surface_count": figures.surface_count,
            "stability": figures.stability,
            "lift": figures.lift,
            "control_authority": figures.control_authority,
            "drag_index": figures.drag_index,
            "reliability": figures.reliability,
            "mass_kg": figures.mass_kg,
            "cg_y_m": figures.cg_y_m,
            "ca_y_m": figures.ca_y_m,
            "margin_m": figures.margin_m,
        },
        "flight": {
            "lifted_off": report.lifted_off,
            "max_speed_ms": report.max_speed_ms,
            "path_length_m": report.path_length_m,
            "flight_time_s": report.flight_time_s,
            "final_speed_ms": report.final_speed_ms,
            "first_ignition_twr": report.first_ignition_twr,
            "samples": report.samples.len(),
        },
        "lock": lock,
        "events": report.events,
        "anomalies": report.anomalies,
    });
    serde_json::to_string_pretty(&json).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_rules::Block;

    fn program() -> Vec<u8> {
        serde_json::to_vec(&Program::single(vec![Block::IgniteStage { stage: 0 }])).unwrap()
    }

    #[test]
    fn the_json_report_quotes_the_aero_and_the_flight() {
        let vehicle = crate::missile::MissileSession::new_default().to_bytes().unwrap();
        let json = trial_json(&vehicle, &program(), FlightConfig::default()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value["aero"]["surface_count"].is_number());
        assert!(value["flight"]["max_speed_ms"].is_number());
        assert!(value["flight"]["samples"].as_u64().unwrap() > 0);
        assert_eq!(value["seed"], 1);
        assert_eq!(value["launch"], "vertical");
    }

    #[test]
    fn the_seed_and_the_air_launch_reach_the_configuration() {
        let args = ["--trial", "slot", "--seed", "7", "--air"].map(String::from);
        let parsed = config(&args).unwrap();
        assert_eq!(parsed.seed, 7);
        assert!(matches!(parsed.launch, LaunchMode::Air { .. }));
        let broken = ["--trial", "slot", "--seed", "seven"].map(String::from);
        assert!(config(&broken).unwrap_err().contains("--seed"));
    }

    #[test]
    fn a_broken_design_is_reported_instead_of_panicking() {
        let error = trial_json(b"{", &program(), FlightConfig::default()).unwrap_err();
        assert!(error.contains("design"), "{error}");
        let empty = br#"{"schema_version":4,"name":"x","parts":[]}"#;
        let error = trial_json(empty, b"{}", FlightConfig::default()).unwrap_err();
        assert!(error.contains("program") || error.contains("no parts"), "{error}");
    }
}
