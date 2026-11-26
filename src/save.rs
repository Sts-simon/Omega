//! Application-level access to the six versioned campaign slots (`saves/slot_1/` to `saves/slot_6/`).
//!
//! Every persistent file (campaign, vehicle, missile program, revisions, contracts) goes through
//! here so that writes are atomic, listed in a manifest committed last, and verified by hash when
//! read. The title screen selects the active slot; gameplay refuses to write before a selection.
//! Saves from 0.21 (`saves/*.json`) are imported once into slot 1, without being modified or deleted.

use bevy::log::{info, warn};
use omega_data::{SaveSlot, SlotError};
use omega_rules::ResearchState;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

const SAVES_DIR: &str = "saves";
const GAME_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Number of campaign slots offered by the main menu.
pub const SLOT_COUNT: u32 = 6;

/// `0` means no campaign is open yet (the main menu is showing).
static ACTIVE_SLOT: AtomicU32 = AtomicU32::new(0);

pub fn slot_dir(id: u32) -> PathBuf {
    Path::new(SAVES_DIR).join(format!("slot_{id}"))
}

pub fn active_slot() -> Option<u32> {
    match ACTIVE_SLOT.load(Ordering::Relaxed) {
        0 => None,
        id => Some(id),
    }
}

pub fn select_slot(id: u32) -> Result<(), String> {
    if !(1..=SLOT_COUNT).contains(&id) {
        return Err(format!("invalid save slot {id}"));
    }
    ACTIVE_SLOT.store(id, Ordering::Relaxed);
    Ok(())
}

/// A menu-friendly snapshot of one slot. Reading a slot never changes it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SaveSlotSummary {
    pub id: u32,
    pub exists: bool,
    pub day: u32,
    pub credits: u64,
    pub technologies: usize,
    pub active_contracts: usize,
    pub completed_contracts: usize,
    pub error: Option<String>,
}

#[derive(Deserialize, Default)]
struct CampaignPreview {
    #[serde(default)]
    credits: u64,
    #[serde(default)]
    day: u32,
    #[serde(default)]
    research: ResearchState,
}

#[derive(Deserialize, Default)]
struct ProcurementPreview {
    #[serde(default)]
    active: Vec<serde_json::Value>,
    #[serde(default)]
    completed: Vec<serde_json::Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveFile {
    Campaign,
    MissileProgram,
    ProgramLibrary,
    Subassemblies,
    Revisions,
    Vehicle,
    Procurement,
    TrialHistory,
}

impl SaveFile {
    pub fn rel(self) -> &'static str {
        match self {
            Self::Campaign => "campaign.json",
            Self::MissileProgram => "programs/missile.json",
            Self::ProgramLibrary => "programs/library.json",
            Self::Subassemblies => "designs/subassemblies.json",
            Self::Revisions => "designs/revisions.json",
            Self::Vehicle => "designs/vehicle.json",
            Self::Procurement => "contracts/missile.json",
            Self::TrialHistory => "trials/history.json",
        }
    }
}

fn migrate(slot: &SaveSlot, legacy_dir: &Path) {
    match slot.import_legacy(legacy_dir, GAME_VERSION) {
        Ok(manifest) => info!(
            "Imported {} file(s) from the 0.21 save layout into {}",
            manifest.files.len(),
            slot.root().display()
        ),
        Err(SlotError::AlreadyExists | SlotError::NothingToImport) => {}
        Err(error) => warn!("Could not import 0.21 saves: {error}"),
    }
}

fn write_slot(slot: &SaveSlot, file: SaveFile, bytes: &[u8]) -> Result<(), String> {
    slot.commit(GAME_VERSION, &[(file.rel(), bytes.to_vec())])
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn read_slot(slot: &SaveSlot, file: SaveFile) -> Result<Option<Vec<u8>>, String> {
    match slot.read_manifest().map_err(|error| error.to_string())? {
        None => Ok(None),
        Some(manifest) if !manifest.files.iter().any(|entry| entry.path == file.rel()) => Ok(None),
        Some(_) => match slot.read_verified(file.rel()) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) => match slot.recover_backup(file.rel()) {
                Ok(bytes) => {
                    warn!("Recovered {} from its previous generation after: {error}", file.rel());
                    Ok(Some(bytes))
                }
                Err(_) => Err(error.to_string()),
            },
        },
    }
}

pub fn write_to(slot: &SaveSlot, legacy_dir: &Path, file: SaveFile, bytes: &[u8]) -> Result<(), String> {
    migrate(slot, legacy_dir);
    write_slot(slot, file, bytes)
}

/// `Ok(None)` means "nothing saved yet"; `Err` means the slot is unreadable or the file no
/// longer matches its manifest entry (the previous generation stays on disk as `<name>.bak`).
#[cfg(test)]
pub fn read_from(slot: &SaveSlot, legacy_dir: &Path, file: SaveFile) -> Result<Option<Vec<u8>>, String> {
    migrate(slot, legacy_dir);
    read_slot(slot, file)
}

/// Import the old 0.21 flat files once, into the first slot, so they remain visible in the menu.
pub fn ensure_legacy_import() {
    let slot = SaveSlot::new(slot_dir(1));
    migrate(&slot, Path::new(SAVES_DIR));
}

pub fn write(file: SaveFile, bytes: &[u8]) -> Result<(), String> {
    let id = active_slot().ok_or_else(|| "no save slot selected".to_string())?;
    let slot = SaveSlot::new(slot_dir(id));
    if id == 1 {
        migrate(&slot, Path::new(SAVES_DIR));
    }
    write_slot(&slot, file, bytes)
}

pub fn read(file: SaveFile) -> Result<Option<Vec<u8>>, String> {
    let Some(id) = active_slot() else {
        return Ok(None);
    };
    let slot = SaveSlot::new(slot_dir(id));
    if id == 1 {
        migrate(&slot, Path::new(SAVES_DIR));
    }
    read_slot(&slot, file)
}

fn list_slot(id: u32) -> SaveSlotSummary {
    let slot = SaveSlot::new(slot_dir(id));
    let mut summary = SaveSlotSummary {
        id,
        ..Default::default()
    };
    match slot.read_manifest() {
        Ok(None) => return summary,
        Err(error) => {
            summary.exists = true;
            summary.error = Some(error.to_string());
            return summary;
        }
        Ok(Some(_)) => summary.exists = true,
    }
    match read_slot(&slot, SaveFile::Campaign) {
        Ok(Some(bytes)) => match serde_json::from_slice::<CampaignPreview>(&bytes) {
            Ok(campaign) => {
                summary.day = campaign.day.max(1);
                summary.credits = campaign.credits;
                summary.technologies = campaign.research.completed.len();
            }
            Err(error) => summary.error = Some(format!("campagne illisible : {error}")),
        },
        Ok(None) => summary.error = Some("campagne absente".to_string()),
        Err(error) => summary.error = Some(error),
    }
    if let Ok(Some(bytes)) = read_slot(&slot, SaveFile::Procurement) {
        match serde_json::from_slice::<ProcurementPreview>(&bytes) {
            Ok(office) => {
                summary.active_contracts = office.active.len();
                summary.completed_contracts = office.completed.len();
            }
            Err(error) => summary.error = Some(format!("contrats illisibles : {error}")),
        }
    }
    summary
}

/// The six slots in menu order, including empty ones.
pub fn list_slots() -> Vec<SaveSlotSummary> {
    (1..=SLOT_COUNT).map(list_slot).collect()
}

pub fn delete_slot(id: u32) -> Result<(), String> {
    if !(1..=SLOT_COUNT).contains(&id) {
        return Err(format!("invalid save slot {id}"));
    }
    let path = slot_dir(id);
    if !path.exists() {
        return Err(format!("campaign {id} is already empty"));
    }
    std::fs::remove_dir_all(&path).map_err(|error| format!("cannot delete {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("omega-save-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn nothing_saved_reads_as_none_and_written_data_round_trips() {
        let dir = temp_dir("roundtrip");
        let slot = SaveSlot::new(dir.join("slot_1"));
        let legacy = dir.join("saves");
        assert_eq!(read_from(&slot, &legacy, SaveFile::Campaign).unwrap(), None);
        write_to(&slot, &legacy, SaveFile::Campaign, b"{\"credits\":5}").unwrap();
        assert_eq!(
            read_from(&slot, &legacy, SaveFile::Campaign).unwrap().unwrap(),
            b"{\"credits\":5}"
        );
        assert_eq!(read_from(&slot, &legacy, SaveFile::Vehicle).unwrap(), None);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn legacy_files_are_imported_on_first_access_and_left_in_place() {
        let dir = temp_dir("legacy");
        let legacy = dir.join("saves");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("campaign.json"), b"{\"credits\":9}").unwrap();
        let slot = SaveSlot::new(dir.join("slot_1"));
        assert_eq!(
            read_from(&slot, &legacy, SaveFile::Campaign).unwrap().unwrap(),
            b"{\"credits\":9}"
        );
        assert!(legacy.join("campaign.json").exists());
        // Later changes to the legacy file are ignored once the slot exists.
        fs::write(legacy.join("campaign.json"), b"{\"credits\":11}").unwrap();
        assert_eq!(
            read_from(&slot, &legacy, SaveFile::Campaign).unwrap().unwrap(),
            b"{\"credits\":9}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn writing_before_any_read_still_imports_legacy_files_first() {
        let dir = temp_dir("write-first");
        let legacy = dir.join("saves");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("campaign.json"), b"{\"credits\":1}").unwrap();
        let slot = SaveSlot::new(dir.join("slot_1"));
        write_to(&slot, &legacy, SaveFile::Vehicle, b"{\"a\":1}").unwrap();
        assert_eq!(
            read_from(&slot, &legacy, SaveFile::Campaign).unwrap().unwrap(),
            b"{\"credits\":1}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_damaged_file_is_reported_instead_of_loaded() {
        let dir = temp_dir("damaged");
        let slot = SaveSlot::new(dir.join("slot_1"));
        let legacy = dir.join("saves");
        write_to(&slot, &legacy, SaveFile::Campaign, b"good").unwrap();
        fs::write(slot.root().join("campaign.json"), b"bad").unwrap();
        let error = read_from(&slot, &legacy, SaveFile::Campaign).unwrap_err();
        assert!(error.contains("manifest"), "{error}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_corrupt_primary_file_is_restored_from_its_backup() {
        let dir = temp_dir("recovery");
        let slot = SaveSlot::new(dir.join("slot_1"));
        let legacy = dir.join("saves");
        write_to(&slot, &legacy, SaveFile::Campaign, b"v1").unwrap();
        write_to(&slot, &legacy, SaveFile::Campaign, b"v2").unwrap();
        fs::write(slot.root().join("campaign.json"), b"interrupted").unwrap();
        assert_eq!(
            read_from(&slot, &legacy, SaveFile::Campaign).unwrap().unwrap(),
            b"v1",
            "the previous generation is restored instead of failing the campaign"
        );
        assert_eq!(slot.read_verified("campaign.json").unwrap(), b"v1");
        let _ = fs::remove_dir_all(dir);
    }
}
