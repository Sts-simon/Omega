use super::thermal::{self, ThermalConfig, ThermalError, ThermalResult};
use aerolab_sim_core::{
    MeshThermalConfig, MeshThermalError, MeshThermalResult, TrussConfig, TrussError,
    TrussResult, export_mesh_thermal_visualization, export_thermal_visualization,
    export_truss_visualization, simulate_mesh_thermal_cancellable, solve_truss_cancellable,
};
use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use rayon::ThreadPoolBuilder;
use crate::campus::{CampusState, CampusView};
use crate::sequence::SequenceWorkspace;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone, Debug)]
enum JobCommand {
    ThermalPlate {
        id: u64,
        config: ThermalConfig,
        cancel: Arc<AtomicBool>,
    },
    CivilTruss {
        id: u64,
        config: TrussConfig,
        cancel: Arc<AtomicBool>,
    },
    MeshThermal {
        id: u64,
        config: MeshThermalConfig,
        cancel: Arc<AtomicBool>,
    },
}

#[derive(Clone, Debug)]
enum JobEvent {
    Started { id: u64 },
    Progress { id: u64, fraction: f32 },
    Completed { id: u64, result: ThermalResult, report_path: PathBuf },
    TrussCompleted { id: u64, result: TrussResult, report_path: PathBuf },
    MeshThermalCompleted { id: u64, result: MeshThermalResult, report_path: PathBuf },
    Cancelled { id: u64 },
    Failed { id: u64, message: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobState {
    Queued,
    Running,
    Cancelling,
    Completed,
    Cancelled,
    Failed,
}

impl JobState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Queued => "QUEUED",
            Self::Running => "RUNNING",
            Self::Cancelling => "CANCELLING",
            Self::Completed => "COMPLETED",
            Self::Cancelled => "CANCELLED",
            Self::Failed => "FAILED",
        }
    }
}

#[derive(Clone, Debug)]
pub struct JobRecord {
    pub id: u64,
    pub title: String,
    /// Contract ID when this task is a qualification step for an accepted project.
    pub linked_contract_id: Option<u32>,
    pub state: JobState,
    pub progress: f32,
    pub summary: Option<String>,
    pub report_path: Option<PathBuf>,
}

#[derive(Resource)]
pub struct JobManager {
    command_tx: Sender<JobCommand>,
    event_rx: Receiver<JobEvent>,
    cancel_tokens: HashMap<u64, Arc<AtomicBool>>,
    next_id: u64,
    worker_threads: usize,
    pub records: Vec<JobRecord>,
}

impl Default for JobManager {
    fn default() -> Self {
        let (command_tx, command_rx) = unbounded::<JobCommand>();
        let (event_tx, event_rx) = unbounded::<JobEvent>();
        let detected_threads = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(4);
        // Leave one logical CPU available for Bevy/UI by default. The environment
        // variable lets users explicitly spend the full CPU budget on research.
        let default_budget = detected_threads.saturating_sub(1).max(1);
        let worker_threads = std::env::var("AEROLAB_WORKER_THREADS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(default_budget)
            .clamp(1, 16);

        std::thread::Builder::new()
            .name("aerolab-research-worker".to_string())
            .spawn(move || {
                let pool = ThreadPoolBuilder::new()
                    .num_threads(worker_threads)
                    .thread_name(|index| format!("aerolab-cpu-{index}"))
                    .build()
                    .expect("Could not create Rayon thread pool");

                while let Ok(command) = command_rx.recv() {
                    match command {
                        JobCommand::ThermalPlate { id, config, cancel } => {
                            if cancel.load(Ordering::Relaxed) {
                                let _ = event_tx.send(JobEvent::Cancelled { id });
                                continue;
                            }

                            let _ = event_tx.send(JobEvent::Started { id });
                            let progress_tx = event_tx.clone();
                            let result = thermal::simulate_cancellable(
                                &config,
                                &pool,
                                |fraction| {
                                    let _ = progress_tx.send(JobEvent::Progress { id, fraction });
                                },
                                &cancel,
                            );

                            match result {
                                Ok(result) => {
                                    if cancel.load(Ordering::Relaxed) {
                                        let _ = event_tx.send(JobEvent::Cancelled { id });
                                    } else {
                                        match write_report(id, &result) {
                                            Ok(report_path) => {
                                                let _ = event_tx.send(JobEvent::Completed {
                                                    id,
                                                    result,
                                                    report_path,
                                                });
                                            }
                                            Err(message) => {
                                                let _ = event_tx.send(JobEvent::Failed { id, message });
                                            }
                                        }
                                    }
                                }
                                Err(ThermalError::Cancelled) => {
                                    let _ = event_tx.send(JobEvent::Cancelled { id });
                                }
                                Err(error) => {
                                    let _ = event_tx.send(JobEvent::Failed {
                                        id,
                                        message: error.to_string(),
                                    });
                                }
                            }
                        }
                        JobCommand::CivilTruss { id, config, cancel } => {
                            if cancel.load(Ordering::Relaxed) {
                                let _ = event_tx.send(JobEvent::Cancelled { id });
                                continue;
                            }

                            let _ = event_tx.send(JobEvent::Started { id });
                            let progress_tx = event_tx.clone();
                            let result = solve_truss_cancellable(
                                &config,
                                &pool,
                                |fraction| {
                                    let _ = progress_tx.send(JobEvent::Progress {
                                        id,
                                        fraction: fraction as f32,
                                    });
                                },
                                &cancel,
                            );

                            match result {
                                Ok(result) => {
                                    if cancel.load(Ordering::Relaxed) {
                                        let _ = event_tx.send(JobEvent::Cancelled { id });
                                    } else {
                                        match write_truss_report(id, &result) {
                                            Ok(report_path) => {
                                                let _ = event_tx.send(JobEvent::TrussCompleted {
                                                    id,
                                                    result,
                                                    report_path,
                                                });
                                            }
                                            Err(message) => {
                                                let _ = event_tx.send(JobEvent::Failed { id, message });
                                            }
                                        }
                                    }
                                }
                                Err(TrussError::Cancelled) => {
                                    let _ = event_tx.send(JobEvent::Cancelled { id });
                                }
                                Err(error) => {
                                    let _ = event_tx.send(JobEvent::Failed {
                                        id,
                                        message: error.to_string(),
                                    });
                                }
                            }
                        }
                        JobCommand::MeshThermal { id, config, cancel } => {
                            if cancel.load(Ordering::Relaxed) {
                                let _ = event_tx.send(JobEvent::Cancelled { id });
                                continue;
                            }

                            let _ = event_tx.send(JobEvent::Started { id });
                            let progress_tx = event_tx.clone();
                            let result = simulate_mesh_thermal_cancellable(
                                &config,
                                &pool,
                                |fraction| {
                                    let _ = progress_tx.send(JobEvent::Progress {
                                        id,
                                        fraction: fraction as f32,
                                    });
                                },
                                &cancel,
                            );

                            match result {
                                Ok(result) => {
                                    if cancel.load(Ordering::Relaxed) {
                                        let _ = event_tx.send(JobEvent::Cancelled { id });
                                    } else {
                                        match write_mesh_thermal_report(id, &result) {
                                            Ok(report_path) => {
                                                let _ = event_tx.send(JobEvent::MeshThermalCompleted {
                                                    id,
                                                    result,
                                                    report_path,
                                                });
                                            }
                                            Err(message) => {
                                                let _ = event_tx.send(JobEvent::Failed { id, message });
                                            }
                                        }
                                    }
                                }
                                Err(MeshThermalError::Cancelled) => {
                                    let _ = event_tx.send(JobEvent::Cancelled { id });
                                }
                                Err(error) => {
                                    let _ = event_tx.send(JobEvent::Failed {
                                        id,
                                        message: error.to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            })
            .expect("Could not start research worker thread");

        Self {
            command_tx,
            event_rx,
            cancel_tokens: HashMap::new(),
            next_id: 1,
            worker_threads,
            records: Vec::new(),
        }
    }
}

impl JobManager {
    pub fn worker_threads(&self) -> usize {
        self.worker_threads
    }

    pub fn enqueue_thermal_demo(&mut self) -> u64 {
        self.enqueue_thermal_task(None)
    }

    /// Queues the generic finite-volume material study and links the result to a
    /// civilian instrument contract in the campaign. This is not a geometry-level
    /// qualification of the submitted assembly.
    pub fn enqueue_contract_thermal_demo(&mut self, contract_id: u32) -> u64 {
        self.enqueue_thermal_task(Some(contract_id))
    }

    fn enqueue_thermal_task(&mut self, linked_contract_id: Option<u32>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel_tokens.insert(id, cancel.clone());
        let title = match linked_contract_id {
            Some(contract_id) => format!("Contract #{contract_id} material thermal screening"),
            None => "2D finite-volume material heat-conduction study".to_string(),
        };
        self.records.push(JobRecord {
            id,
            title,
            linked_contract_id,
            state: JobState::Queued,
            progress: 0.0,
            summary: None,
            report_path: None,
        });

        if self
            .command_tx
            .send(JobCommand::ThermalPlate {
                id,
                config: ThermalConfig::default(),
                cancel,
            })
            .is_err()
        {
            if let Some(record) = self.find_record_mut(id) {
                record.state = JobState::Failed;
                record.summary = Some("Research worker is unavailable".to_string());
            }
            self.cancel_tokens.remove(&id);
        }
        id
    }

    /// Queues a small civil truss reference study for structural visualization.
    pub fn enqueue_truss_demo(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel_tokens.insert(id, cancel.clone());
        self.records.push(JobRecord {
            id,
            title: "2D civil truss static structural study".to_string(),
            linked_contract_id: None,
            state: JobState::Queued,
            progress: 0.0,
            summary: None,
            report_path: None,
        });

        if self.command_tx.send(JobCommand::CivilTruss {
            id,
            config: TrussConfig::default(),
            cancel,
        }).is_err() {
            if let Some(record) = self.find_record_mut(id) {
                record.state = JobState::Failed;
                record.summary = Some("Research worker is unavailable".to_string());
            }
            self.cancel_tokens.remove(&id);
        }
        id
    }

    /// Queues a meshed thermal study generated from supported flat workshop parts.
    pub fn enqueue_mesh_thermal(&mut self, config: MeshThermalConfig) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel_tokens.insert(id, cancel.clone());
        self.records.push(JobRecord {
            id,
            title: format!("Triangular mesh thermal study | {}", config.name),
            linked_contract_id: None,
            state: JobState::Queued,
            progress: 0.0,
            summary: None,
            report_path: None,
        });
        if self.command_tx.send(JobCommand::MeshThermal { id, config, cancel }).is_err() {
            if let Some(record) = self.find_record_mut(id) {
                record.state = JobState::Failed;
                record.summary = Some("Research worker is unavailable".to_string());
            }
            self.cancel_tokens.remove(&id);
        }
        id
    }

    pub fn open_latest_report(&self) -> Result<(), String> {
        let path = self.records.iter().rev().find_map(|record| record.report_path.as_ref())
            .ok_or_else(|| "No visualization report has been generated yet".to_string())?;
        let absolute = std::fs::canonicalize(path).map_err(|error| error.to_string())?;

        #[cfg(target_os = "linux")]
        let result = std::process::Command::new("xdg-open").arg(&absolute).spawn();
        #[cfg(target_os = "windows")]
        let result = std::process::Command::new("cmd").args(["/C", "start", ""]).arg(&absolute).spawn();
        #[cfg(target_os = "macos")]
        let result = std::process::Command::new("open").arg(&absolute).spawn();
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        let result: Result<std::process::Child, std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Opening reports is not configured for this operating system",
        ));

        result.map(|_| ()).map_err(|error| format!("Could not open {}: {error}", absolute.display()))
    }

    /// Requests cancellation of the newest queued or running task.
    /// Cancellation is cooperative and is checked between solver iterations.
    pub fn cancel_latest(&mut self) -> Option<u64> {
        let id = self
            .records
            .iter()
            .rev()
            .find(|record| matches!(&record.state, JobState::Queued | JobState::Running))
            .map(|record| record.id)?;
        self.cancel_job(id).then_some(id)
    }

    pub fn cancel_job(&mut self, id: u64) -> bool {
        let Some(token) = self.cancel_tokens.get(&id).cloned() else { return false };
        let Some(record) = self.records.iter_mut().find(|record| record.id == id) else {
            return false;
        };
        if !matches!(&record.state, JobState::Queued | JobState::Running) {
            return false;
        }
        token.store(true, Ordering::Relaxed);
        record.state = JobState::Cancelling;
        true
    }

    pub fn collect_updates(&mut self) -> Vec<String> {
        let mut messages = Vec::new();

        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                JobEvent::Started { id } => {
                    if let Some(record) = self.find_record_mut(id) {
                        // Do not overwrite a cancellation request made just before this event.
                        if record.state != JobState::Cancelling {
                            record.state = JobState::Running;
                        }
                    }
                    messages.push(format!("Job #{id}: started"));
                }
                JobEvent::Progress { id, fraction } => {
                    let mut milestone = None;
                    if let Some(record) = self.find_record_mut(id) {
                        let previous = (record.progress * 10.0).floor() as u8;
                        record.progress = fraction.clamp(0.0, 1.0);
                        let current = (record.progress * 10.0).floor() as u8;
                        if current > previous {
                            milestone = Some(current.min(10) * 10);
                        }
                    }
                    if let Some(percent) = milestone {
                        messages.push(format!("Job #{id}: {percent}%"));
                    }
                }
                JobEvent::Completed { id, result, report_path } => {
                    if let Some(record) = self.find_record_mut(id) {
                        record.state = JobState::Completed;
                        record.progress = 1.0;
                        record.summary = Some(format!(
                            "{}x{} grid, {} steps, {:.2} simulated s in {:.2} wall s; max {:.2} K, mean {:.2} K; energy balance {:.2e}; input {}",
                            result.width,
                            result.height,
                            result.steps_completed,
                            result.simulated_time_s,
                            result.elapsed_ms as f64 / 1_000.0,
                            result.max_temperature_k,
                            result.mean_temperature_k,
                            result.diagnostics.energy_balance_relative_error,
                            result.metadata.input_fingerprint,
                        ));
                        record.report_path = Some(report_path.clone());
                    }
                    self.cancel_tokens.remove(&id);
                    messages.push(format!("Job #{id}: completed. Browser report: {}", report_path.display()));
                }
                JobEvent::TrussCompleted { id, result, report_path } => {
                    if let Some(record) = self.find_record_mut(id) {
                        record.state = JobState::Completed;
                        record.progress = 1.0;
                        record.summary = Some(format!(
                            "{} nodes, {} members; max displacement {:.3e} m; max stress {:.3e} Pa; residual {:.2e}",
                            result.nodes.len(),
                            result.elements.len(),
                            result.max_displacement_m,
                            result.max_absolute_stress_pa,
                            result.diagnostics.relative_residual,
                        ));
                        record.report_path = Some(report_path.clone());
                    }
                    self.cancel_tokens.remove(&id);
                    messages.push(format!("Job #{id}: structural study completed. Browser report: {}", report_path.display()));
                }
                JobEvent::MeshThermalCompleted { id, result, report_path } => {
                    if let Some(record) = self.find_record_mut(id) {
                        record.state = JobState::Completed;
                        record.progress = 1.0;
                        record.summary = Some(format!(
                            "{} nodes, {} triangles; {:.4}..{:.4} K; {:.3e} s stable-step limit; energy residual {:.2e}",
                            result.config.mesh.nodes.len(),
                            result.config.mesh.triangles.len(),
                            result.min_temperature_k,
                            result.max_temperature_k,
                            result.diagnostics.stable_time_step_limit_s,
                            result.diagnostics.energy_balance_relative_error,
                        ));
                        record.report_path = Some(report_path.clone());
                    }
                    self.cancel_tokens.remove(&id);
                    messages.push(format!("Job #{id}: workshop mesh thermal study completed. Browser report: {}", report_path.display()));
                }
                JobEvent::Cancelled { id } => {
                    if let Some(record) = self.find_record_mut(id) {
                        record.state = JobState::Cancelled;
                        record.summary = Some("Cancelled by user".to_string());
                    }
                    self.cancel_tokens.remove(&id);
                    messages.push(format!("Job #{id}: cancelled"));
                }
                JobEvent::Failed { id, message } => {
                    if let Some(record) = self.find_record_mut(id) {
                        record.state = JobState::Failed;
                        record.summary = Some(message.clone());
                    }
                    self.cancel_tokens.remove(&id);
                    messages.push(format!("Job #{id}: failed: {message}"));
                }
            }
        }

        messages
    }

    fn find_record_mut(&mut self, id: u64) -> Option<&mut JobRecord> {
        self.records.iter_mut().find(|record| record.id == id)
    }
}

#[derive(Component)]
pub struct ComputationHud;

pub fn setup_ui(mut commands: Commands) {
    commands.spawn((
        Text::new("AEROLAB | Computation queue\nWaiting for jobs..."),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::srgb(0.91, 0.95, 0.98)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(78.0),
            right: Val::Px(14.0),
            max_width: Val::Px(430.0),
            max_height: Val::Percent(82.0),
            padding: bevy::prelude::UiRect::all(Val::Px(12.0)),
            overflow: bevy::prelude::Overflow::scroll_y(),
            ..default()
        },
        Interaction::None,
        ScrollPosition::default(),
        bevy::prelude::BackgroundColor(Color::srgba(0.025, 0.04, 0.055, 0.88)),
        ComputationHud,
    ));
}

pub fn update_hud(
    jobs: Res<JobManager>,
    campus: Res<CampusState>,
    sequence: Res<SequenceWorkspace>,
    mut hud: Query<(&mut Text, &mut Node), bevy::prelude::With<ComputationHud>>,
) {
    let Ok((mut text, mut node)) = hud.single_mut() else { return };
    node.display = if campus.view == CampusView::Computation && !sequence.graph_is_open() {
        bevy::prelude::Display::Flex
    } else {
        bevy::prelude::Display::None
    };
    if campus.view != CampusView::Computation || sequence.graph_is_open() {
        return;
    }
    let mut lines = vec![format!(
        "AEROLAB 0.26.0 | COMPUTATION CENTRE\n{}\n\nCPU worker threads: {}\nR: thermal study | G: civil truss | M: workshop mesh | P: polygon mesh | Shift+M: local refinement | T: field test | C: cancel | O: latest report",
        campus.ui_computation_summary(),
        jobs.worker_threads()
    )];

    if jobs.records.is_empty() {
        lines.push("\nNo computation tasks submitted.".to_string());
    } else {
        lines.push("\nRecent tasks:".to_string());
        for record in jobs.records.iter().rev().take(6).rev() {
            lines.push(format!(
                "#{:03} {:<10} {:>3.0}%  {}",
                record.id,
                record.state.label(),
                record.progress * 100.0,
                record.title
            ));
            if let Some(summary) = &record.summary {
                lines.push(format!("    {summary}"));
            }
            if let Some(path) = &record.report_path {
                lines.push(format!("    HTML: {}", path.display()));
            }
        }
    }
    text.0 = lines.join("\n");
}

fn write_report(id: u64, result: &ThermalResult) -> Result<PathBuf, String> {
    let directory = PathBuf::from("results");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let json = serde_json::to_vec_pretty(result).map_err(|error| error.to_string())?;
    let timestamp = result.metadata.created_unix_ms;
    let mut collision = 0_u32;

    loop {
        let suffix = if collision == 0 {
            String::new()
        } else {
            format!("_{collision}")
        };
        let path = directory.join(format!("thermal_fvm_{id:04}_{timestamp}{suffix}.json"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(&json) {
                    let _ = fs::remove_file(&path);
                    return Err(error.to_string());
                }
                let visualizations = export_thermal_visualization(result, &directory)
                    .map_err(|error| error.to_string())?;
                return Ok(visualizations.html_path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                collision = collision.saturating_add(1);
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}


fn write_mesh_thermal_report(id: u64, result: &MeshThermalResult) -> Result<PathBuf, String> {
    let directory = PathBuf::from("results");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let json = serde_json::to_vec_pretty(result).map_err(|error| error.to_string())?;
    let timestamp = result.metadata.created_unix_ms;
    for collision in 0_u32..10_000 {
        let suffix = if collision == 0 { String::new() } else { format!("_{collision}") };
        let path = directory.join(format!("mesh_thermal_{id:04}_{timestamp}{suffix}.json"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(&json) {
                    let _ = fs::remove_file(&path);
                    return Err(error.to_string());
                }
                let visualizations = export_mesh_thermal_visualization(result, &directory)
                    .map_err(|error| error.to_string())?;
                return Ok(visualizations.html_path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("could not allocate a unique meshed thermal report filename".into())
}

fn write_truss_report(id: u64, result: &TrussResult) -> Result<PathBuf, String> {
    let directory = PathBuf::from("results");
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let json = serde_json::to_vec_pretty(result).map_err(|error| error.to_string())?;
    let timestamp = result.metadata.created_unix_ms;
    let mut collision = 0_u32;
    loop {
        let suffix = if collision == 0 { String::new() } else { format!("_{collision}") };
        let path = directory.join(format!("truss_static_{id:04}_{timestamp}{suffix}.json"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(&json) {
                    let _ = fs::remove_file(&path);
                    return Err(error.to_string());
                }
                let visualizations = export_truss_visualization(result, &directory)
                    .map_err(|error| error.to_string())?;
                return Ok(visualizations.html_path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                collision = collision.saturating_add(1);
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}
