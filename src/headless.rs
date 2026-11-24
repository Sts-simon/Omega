//! Screenless trial runner used by `omega` command-line flags:
//!
//! - `omega --trial <slot-dir> [--seed N] [--air]` flies the saved design and program once and
//!   prints a JSON report.
//! - `omega --revisions <slot-dir> [--seed N] [--air]` flies the working copy and the last ten
//!   recorded revisions with the same seed and prints them with the deltas between neighbours, so
//!   a design change can be compared without editing anything in the game.
//! - `omega --export-design <slot-dir> <out.json>` and `--import-design <slot-dir> <in.json>`
//!   move one design between saves or people, refusing anything that does not validate.
//!
//! Neither mode opens a window, so a lot's flight acceptance criteria can be checked over SSH or
//! in CI.

use omega_rules::flight::{FlightConfig, LaunchMode, simulate};
use omega_rules::{Program, Vehicle};
use std::path::Path;

const USAGE: &str = "usage: omega --trial <slot-dir> [--seed N] [--air] | \
                     omega --revisions <slot-dir> [--seed N] [--air] | omega --aero-report | \
                     omega --export-design <slot-dir> <out.json> | \
                     omega --import-design <slot-dir> <in.json>";
/// How many recorded revisions the comparison mode replays.
const HISTORY_LIMIT: usize = 10;

/// Runs the headless modes when one of their flags is present. `None` means "start the game".
pub(crate) fn main() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--aero-report") {
        println!("{}", omega_rules::aero::balance_report());
        return Some(0);
    }
    let mode = if args.iter().any(|arg| arg == "--trial") {
        run_trial(&args)
    } else if args.iter().any(|arg| arg == "--revisions") {
        run_revisions(&args)
    } else if args.iter().any(|arg| arg == "--export-design") {
        run_export(&args)
    } else if args.iter().any(|arg| arg == "--import-design") {
        run_import(&args)
    } else {
        return None;
    };
    match mode {
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

fn pair_of<'a>(args: &'a [String], flag: &str) -> Option<(&'a str, &'a str)> {
    let index = args.iter().position(|arg| arg == flag)?;
    Some((args.get(index + 1)?.as_str(), args.get(index + 2)?.as_str()))
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))
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

fn run_trial(args: &[String]) -> Result<String, String> {
    let dir = value_of(args, "--trial").ok_or_else(|| USAGE.to_string())?;
    let config = config(args)?;
    let slot = Path::new(dir);
    let vehicle = read(&slot.join("designs/vehicle.json"))?;
    let program = read(&slot.join("programs/missile.json"))?;
    trial_json(&vehicle, &program, config)
}

fn trial_json(vehicle_bytes: &[u8], program_bytes: &[u8], config: FlightConfig) -> Result<String, String> {
    let vehicle: Vehicle = serde_json::from_slice(vehicle_bytes).map_err(|error| format!("design: {error}"))?;
    let program: Program = serde_json::from_slice(program_bytes).map_err(|error| format!("program: {error}"))?;
    let flight = flight_json(&vehicle, &program, config)?;
    let anomalies = flight["anomalies"].clone();
    let json = serde_json::json!({
        "design": vehicle.name,
        "seed": config.seed,
        "launch": launch_label(config.launch),
        "aero": figures_json(&vehicle),
        "flight": flight,
        "anomalies": anomalies,
    });
    serde_json::to_string_pretty(&json).map_err(|error| error.to_string())
}

#[derive(serde::Deserialize)]
struct RawLog {
    #[serde(default)]
    revisions: Vec<RawRevision>,
}

#[derive(serde::Deserialize)]
struct RawRevision {
    id: u32,
    #[serde(default)]
    label: String,
    #[serde(default)]
    hash: String,
    vehicle: Vehicle,
}

/// One entry of the comparison: the working copy first, then the recorded revisions, newest first.
struct HistoryEntry {
    name: String,
    hash: String,
    vehicle: Vehicle,
}

fn run_revisions(args: &[String]) -> Result<String, String> {
    let dir = value_of(args, "--revisions").ok_or_else(|| USAGE.to_string())?;
    let config = config(args)?;
    let slot = Path::new(dir);
    let working: Vehicle = serde_json::from_slice(&read(&slot.join("designs/vehicle.json"))?)
        .map_err(|error| format!("design: {error}"))?;
    let program: Program = serde_json::from_slice(&read(&slot.join("programs/missile.json"))?)
        .map_err(|error| format!("program: {error}"))?;
    let log = match read(&slot.join("designs/revisions.json")) {
        Ok(bytes) => serde_json::from_slice::<RawLog>(&bytes).map_err(|error| format!("revisions: {error}"))?,
        Err(_) => RawLog { revisions: Vec::new() },
    };
    let history = history_entries(&working, log);
    history_json(&history, &program, config)
}

fn history_entries(working: &Vehicle, log: RawLog) -> Vec<HistoryEntry> {
    let hash = omega_data::hash_serializable(working)
        .map(|hash| hash.as_str().to_string())
        .unwrap_or_default();
    let mut entries = vec![HistoryEntry {
        name: "working copy".to_string(),
        hash,
        vehicle: working.clone(),
    }];
    entries.extend(
        log.revisions
            .into_iter()
            .rev()
            .take(HISTORY_LIMIT)
            .map(|revision| HistoryEntry {
                name: if revision.label.is_empty() {
                    format!("revision {}", revision.id)
                } else {
                    format!("revision {} ({})", revision.id, revision.label)
                },
                hash: revision.hash,
                vehicle: revision.vehicle,
            }),
    );
    entries
}

fn history_json(history: &[HistoryEntry], program: &Program, config: FlightConfig) -> Result<String, String> {
    let mut entries = Vec::new();
    for entry in history {
        entries.push(serde_json::json!({
            "design": entry.name,
            "hash": entry.hash,
            "aero": figures_json(&entry.vehicle),
            "flight": flight_json(&entry.vehicle, program, config)?,
        }));
    }
    let deltas: Vec<serde_json::Value> = entries
        .windows(2)
        .map(|pair| {
            let (now, before) = (&pair[0], &pair[1]);
            let change = |path: &[&str]| {
                let field = |entry: &serde_json::Value| {
                    path.iter()
                        .fold(entry, |value, key| &value[key])
                        .as_f64()
                        .unwrap_or(0.0)
                };
                field(now) - field(before)
            };
            serde_json::json!({
                "from": before["design"],
                "to": now["design"],
                "stability": change(&["aero", "stability"]),
                "control_authority": change(&["aero", "control_authority"]),
                "lift": change(&["aero", "lift"]),
                "drag_index": change(&["aero", "drag_index"]),
                "max_speed_ms": change(&["flight", "max_speed_ms"]),
                "path_length_m": change(&["flight", "path_length_m"]),
            })
        })
        .collect();
    let json = serde_json::json!({
        "seed": config.seed,
        "launch": launch_label(config.launch),
        "entries": entries,
        "deltas": deltas,
    });
    serde_json::to_string_pretty(&json).map_err(|error| error.to_string())
}

/// A design is exported as the schema-4 vehicle itself, so it can be read back by any copy of the
/// game; the export refuses a design the game would refuse.
fn export_bytes(vehicle_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let vehicle: Vehicle = serde_json::from_slice(vehicle_bytes).map_err(|error| format!("design: {error}"))?;
    vehicle
        .validate()
        .map_err(|issues| format!("design refused: {}", issues.join("; ")))?;
    serde_json::to_vec_pretty(&vehicle).map_err(|error| error.to_string())
}

/// Import accepts a schema-4 export or any older design file the game can migrate, validates it and
/// returns the schema-4 bytes to save. A refused design never reaches the save slot.
fn import_bytes(input: &[u8]) -> Result<Vec<u8>, String> {
    let vehicle: Vehicle = serde_json::from_slice(input).map_err(|error| format!("design: {error}"))?;
    vehicle
        .validate()
        .map_err(|issues| format!("design refused: {}", issues.join("; ")))?;
    serde_json::to_vec_pretty(&vehicle).map_err(|error| error.to_string())
}

fn run_export(args: &[String]) -> Result<String, String> {
    let (slot_dir, out_path) = pair_of(args, "--export-design").ok_or_else(|| USAGE.to_string())?;
    let vehicle = read(&Path::new(slot_dir).join("designs/vehicle.json"))?;
    let portable = export_bytes(&vehicle)?;
    std::fs::write(out_path, &portable).map_err(|error| format!("{out_path}: {error}"))?;
    Ok(format!("Exported the design to {out_path} ({} bytes)", portable.len()))
}

fn run_import(args: &[String]) -> Result<String, String> {
    let (slot_dir, in_path) = pair_of(args, "--import-design").ok_or_else(|| USAGE.to_string())?;
    let input = read(Path::new(in_path))?;
    let bytes = import_bytes(&input)?;
    let slot = omega_data::SaveSlot::new(slot_dir);
    // The 0.21 files, if any, sit next to the slot, not in the process working directory.
    let legacy = Path::new(slot_dir)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("saves"));
    crate::save::write_to(&slot, legacy, crate::save::SaveFile::Vehicle, &bytes)?;
    Ok(format!("Imported the design into {slot_dir}"))
}

fn launch_label(launch: LaunchMode) -> &'static str {
    if matches!(launch, LaunchMode::Air { .. }) {
        "air"
    } else {
        "vertical"
    }
}

fn figures_json(vehicle: &Vehicle) -> serde_json::Value {
    let figures = omega_rules::aero::figures(vehicle);
    serde_json::json!({
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
    })
}

fn flight_json(vehicle: &Vehicle, program: &Program, config: FlightConfig) -> Result<serde_json::Value, String> {
    let report = simulate(vehicle, program, config).map_err(|issues| issues.join("; "))?;
    let lock = report.lock.map(|lock| {
        serde_json::json!({
            "success": lock.success,
            "probability": lock.probability,
            "speed_ms": lock.speed_ms,
        })
    });
    Ok(serde_json::json!({
        "lifted_off": report.lifted_off,
        "max_speed_ms": report.max_speed_ms,
        "path_length_m": report.path_length_m,
        "flight_time_s": report.flight_time_s,
        "final_speed_ms": report.final_speed_ms,
        "first_ignition_twr": report.first_ignition_twr,
        "samples": report.samples.len(),
        "lock": lock,
        "events": report.events,
        "anomalies": report.anomalies,
    }))
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

    #[test]
    fn a_design_export_round_trips_and_a_broken_import_is_refused() {
        let session = crate::missile::MissileSession::new_default();
        let original = session.to_bytes().unwrap();
        let exported = export_bytes(&original).unwrap();
        let imported = import_bytes(&exported).unwrap();
        assert_eq!(imported, exported, "an export imports byte for byte");
        let before: Vehicle = serde_json::from_slice(&original).unwrap();
        let after: Vehicle = serde_json::from_slice(&imported).unwrap();
        assert_eq!(before, after);
        assert!(import_bytes(b"{}").unwrap_err().contains("design"));
        let empty = br#"{"schema_version":4,"name":"x","parts":[]}"#;
        assert!(import_bytes(empty).unwrap_err().contains("refused"));
    }

    #[test]
    fn the_revision_history_starts_with_the_working_copy_and_compares_neighbours() {
        let working = crate::missile::MissileSession::new_default().vehicle.clone();
        let log = RawLog {
            revisions: vec![RawRevision {
                id: 4,
                label: "Missile revision 4".to_string(),
                hash: "abc".to_string(),
                vehicle: working.clone(),
            }],
        };
        let history = history_entries(&working, log);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].name, "working copy");
        assert!(history[1].name.contains("revision 4"));
        let program = Program::single(vec![Block::IgniteStage { stage: 0 }]);
        let json = history_json(&history, &program, FlightConfig::default()).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["entries"].as_array().unwrap().len(), 2);
        let deltas = value["deltas"].as_array().unwrap();
        assert_eq!(deltas.len(), 1);
        assert!(deltas[0]["stability"].as_f64().unwrap().abs() < 1e-9);
        assert!(deltas[0]["from"].as_str().unwrap().contains("revision 4"));
    }
}
