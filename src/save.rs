//! Application-level access to the versioned save slot (`saves/slot_1/`).
//!
//! Every persistent file (campaign, assembly, polygon sketch, program) goes through here so that
//! writes are atomic, listed in a manifest committed last, and verified by hash when read.
//! Saves from 0.21 (`saves/*.json`) are imported once, without being modified or deleted.

use bevy::log::{info, warn};
use omega_data::{SaveSlot, SlotError};
use std::path::Path;

const SAVES_DIR: &str = "saves";
const SLOT_DIR: &str = "saves/slot_1";
const GAME_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveFile {
    Campaign,
    Assembly,
    PolygonSketch,
    Program,
    Revisions,
    Vehicle,
}

impl SaveFile {
    pub fn rel(self) -> &'static str {
        match self {
            Self::Campaign => "campaign.json",
            Self::Assembly => "designs/assembly.json",
            Self::PolygonSketch => "designs/polygon-sketch.json",
            Self::Program => "programs/lab_sequence.json",
            Self::Revisions => "designs/revisions.json",
            Self::Vehicle => "designs/vehicle.json",
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

pub fn write_to(slot: &SaveSlot, legacy_dir: &Path, file: SaveFile, bytes: &[u8]) -> Result<(), String> {
    migrate(slot, legacy_dir);
    slot.commit(GAME_VERSION, &[(file.rel(), bytes.to_vec())])
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// `Ok(None)` means "nothing saved yet"; `Err` means the slot is unreadable or the file no
/// longer matches its manifest entry (the previous generation stays on disk as `<name>.bak`).
pub fn read_from(slot: &SaveSlot, legacy_dir: &Path, file: SaveFile) -> Result<Option<Vec<u8>>, String> {
    migrate(slot, legacy_dir);
    match slot.read_manifest().map_err(|error| error.to_string())? {
        None => Ok(None),
        Some(manifest) if !manifest.files.iter().any(|entry| entry.path == file.rel()) => Ok(None),
        Some(_) => slot
            .read_verified(file.rel())
            .map(Some)
            .map_err(|error| error.to_string()),
    }
}

pub fn write(file: SaveFile, bytes: &[u8]) -> Result<(), String> {
    write_to(&SaveSlot::new(SLOT_DIR), Path::new(SAVES_DIR), file, bytes)
}

pub fn read(file: SaveFile) -> Result<Option<Vec<u8>>, String> {
    read_from(&SaveSlot::new(SLOT_DIR), Path::new(SAVES_DIR), file)
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
        assert_eq!(read_from(&slot, &legacy, SaveFile::Assembly).unwrap(), None);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn legacy_files_are_imported_on_first_access_and_left_in_place() {
        let dir = temp_dir("legacy");
        let legacy = dir.join("saves");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("assembly.json"), b"{\"old\":true}").unwrap();
        fs::write(legacy.join("lab_sequence.json"), b"{\"prog\":1}").unwrap();
        let slot = SaveSlot::new(dir.join("slot_1"));
        assert_eq!(
            read_from(&slot, &legacy, SaveFile::Assembly).unwrap().unwrap(),
            b"{\"old\":true}"
        );
        assert_eq!(
            read_from(&slot, &legacy, SaveFile::Program).unwrap().unwrap(),
            b"{\"prog\":1}"
        );
        assert!(legacy.join("assembly.json").exists());
        // Later changes to the legacy file are ignored once the slot exists.
        fs::write(legacy.join("assembly.json"), b"{\"old\":false}").unwrap();
        assert_eq!(
            read_from(&slot, &legacy, SaveFile::Assembly).unwrap().unwrap(),
            b"{\"old\":true}"
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
        write_to(&slot, &legacy, SaveFile::Assembly, b"{\"a\":1}").unwrap();
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
        assert!(error.contains("does not match the manifest"), "{error}");
        let _ = fs::remove_dir_all(dir);
    }
}
