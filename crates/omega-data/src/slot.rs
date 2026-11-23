use crate::atomic::write_atomic;
use crate::hash::ContentHash;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const CURRENT_SLOT_SCHEMA: u32 = 2;
const MANIFEST_FILE: &str = "manifest.json";

/// Files of the 0.21 layout and where they live inside a v2 slot.
pub const LEGACY_FILES: [(&str, &str); 4] = [
    ("campaign.json", "campaign.json"),
    ("assembly.json", "designs/assembly.json"),
    ("polygon-sketch.json", "designs/polygon-sketch.json"),
    ("lab_sequence.json", "programs/lab_sequence.json"),
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub path: String,
    pub hash: ContentHash,
    pub bytes: u64,
}

/// Index of a save slot. It is written **last**, so a crash before it is replaced leaves the
/// previous manifest (and therefore the previous consistent state) authoritative.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub game_version: String,
    /// Monotonic save counter; avoids wall-clock dependence in tests.
    pub revision: u64,
    pub files: Vec<ManifestEntry>,
}

#[derive(Debug)]
pub enum SlotError {
    Io(String),
    Parse(String),
    UnsafePath(String),
    MissingFile(String),
    Corrupt { path: String, expected: ContentHash, actual: ContentHash },
    UnsupportedSchema(u32),
    AlreadyExists,
    NothingToImport,
}

impl fmt::Display for SlotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(m) | Self::Parse(m) => write!(f, "{m}"),
            Self::UnsafePath(p) => write!(f, "unsafe slot path: {p}"),
            Self::MissingFile(p) => write!(f, "file listed in manifest is missing: {p}"),
            Self::Corrupt { path, expected, actual } => {
                write!(f, "{path} does not match the manifest (expected {}, found {})", expected.as_str(), actual.as_str())
            }
            Self::UnsupportedSchema(v) => write!(f, "unsupported save schema {v}"),
            Self::AlreadyExists => write!(f, "slot already contains a manifest"),
            Self::NothingToImport => write!(f, "no legacy 0.21 save files were found"),
        }
    }
}

impl std::error::Error for SlotError {}

pub struct SaveSlot {
    root: PathBuf,
}

fn check_relative(rel: &str) -> Result<(), SlotError> {
    let path = Path::new(rel);
    let safe = !rel.is_empty()
        && path.components().all(|c| matches!(c, Component::Normal(_)))
        && rel != MANIFEST_FILE;
    if safe { Ok(()) } else { Err(SlotError::UnsafePath(rel.to_string())) }
}

impl SaveSlot {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn read_manifest(&self) -> Result<Option<Manifest>, SlotError> {
        let path = self.root.join(MANIFEST_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(&path).map_err(|e| SlotError::Io(format!("cannot read {}: {e}", path.display())))?;
        let manifest: Manifest =
            serde_json::from_slice(&bytes).map_err(|e| SlotError::Parse(format!("invalid manifest: {e}")))?;
        if manifest.schema_version > CURRENT_SLOT_SCHEMA {
            return Err(SlotError::UnsupportedSchema(manifest.schema_version));
        }
        Ok(Some(manifest))
    }

    /// Write every file atomically, then commit the manifest. Files not mentioned in `files`
    /// but present in the previous manifest are carried over unchanged.
    pub fn commit(&self, game_version: &str, files: &[(&str, Vec<u8>)]) -> Result<Manifest, SlotError> {
        for (rel, _) in files {
            check_relative(rel)?;
        }
        let previous = self.read_manifest()?;
        let mut entries: Vec<ManifestEntry> = previous
            .as_ref()
            .map(|m| m.files.iter().filter(|e| !files.iter().any(|(r, _)| *r == e.path)).cloned().collect())
            .unwrap_or_default();
        for (rel, bytes) in files {
            write_atomic(&self.root.join(rel), bytes).map_err(SlotError::Io)?;
            entries.push(ManifestEntry {
                path: (*rel).to_string(),
                hash: ContentHash::of_bytes(bytes),
                bytes: bytes.len() as u64,
            });
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let manifest = Manifest {
            schema_version: CURRENT_SLOT_SCHEMA,
            game_version: game_version.to_string(),
            revision: previous.map(|m| m.revision + 1).unwrap_or(1),
            files: entries,
        };
        let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| SlotError::Parse(e.to_string()))?;
        write_atomic(&self.root.join(MANIFEST_FILE), &bytes).map_err(SlotError::Io)?;
        Ok(manifest)
    }

    /// Read one file and verify it against the manifest.
    pub fn read_verified(&self, rel: &str) -> Result<Vec<u8>, SlotError> {
        check_relative(rel)?;
        let manifest = self.read_manifest()?.ok_or_else(|| SlotError::MissingFile(MANIFEST_FILE.to_string()))?;
        let entry = manifest
            .files
            .iter()
            .find(|e| e.path == rel)
            .ok_or_else(|| SlotError::MissingFile(rel.to_string()))?;
        let bytes = fs::read(self.root.join(rel)).map_err(|_| SlotError::MissingFile(rel.to_string()))?;
        let actual = ContentHash::of_bytes(&bytes);
        if actual != entry.hash {
            return Err(SlotError::Corrupt { path: rel.to_string(), expected: entry.hash.clone(), actual });
        }
        Ok(bytes)
    }

    /// Check every file listed in the manifest; returns the list of problems (empty = healthy).
    pub fn verify_all(&self) -> Result<Vec<SlotError>, SlotError> {
        let Some(manifest) = self.read_manifest()? else {
            return Ok(Vec::new());
        };
        let mut problems = Vec::new();
        for entry in &manifest.files {
            if let Err(error) = self.read_verified(&entry.path) {
                problems.push(error);
            }
        }
        Ok(problems)
    }

    /// One-time migration of the four fixed-path 0.21 save files into this slot.
    /// The legacy directory is only read, never modified.
    pub fn import_legacy(&self, legacy_dir: &Path, game_version: &str) -> Result<Manifest, SlotError> {
        if self.read_manifest()?.is_some() {
            return Err(SlotError::AlreadyExists);
        }
        let mut files: Vec<(&str, Vec<u8>)> = Vec::new();
        for (legacy_name, target) in LEGACY_FILES {
            let path = legacy_dir.join(legacy_name);
            if path.exists() {
                let bytes = fs::read(&path).map_err(|e| SlotError::Io(format!("cannot read {}: {e}", path.display())))?;
                files.push((target, bytes));
            }
        }
        if files.is_empty() {
            return Err(SlotError::NothingToImport);
        }
        self.commit(game_version, &files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("omega-slot-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn commit_then_read_verified_round_trips() {
        let dir = temp_dir("roundtrip");
        let slot = SaveSlot::new(dir.join("slot_1"));
        let manifest = slot.commit("0.22.0", &[("campaign.json", b"{\"a\":1}".to_vec())]).unwrap();
        assert_eq!(manifest.revision, 1);
        assert_eq!(slot.read_verified("campaign.json").unwrap(), b"{\"a\":1}");
        let second = slot.commit("0.22.0", &[("designs/a.json", b"[]".to_vec())]).unwrap();
        assert_eq!(second.revision, 2);
        assert_eq!(second.files.len(), 2, "unrelated files are carried over");
        assert!(slot.verify_all().unwrap().is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn tampered_file_is_detected() {
        let dir = temp_dir("tamper");
        let slot = SaveSlot::new(dir.join("s"));
        slot.commit("0.22.0", &[("campaign.json", b"good".to_vec())]).unwrap();
        fs::write(slot.root().join("campaign.json"), b"evil").unwrap();
        assert!(matches!(slot.read_verified("campaign.json"), Err(SlotError::Corrupt { .. })));
        assert_eq!(slot.verify_all().unwrap().len(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn crash_before_manifest_commit_keeps_previous_state_authoritative() {
        let dir = temp_dir("crash");
        let slot = SaveSlot::new(dir.join("s"));
        slot.commit("0.22.0", &[("campaign.json", b"v1".to_vec())]).unwrap();
        // Simulate a crash: the data file is replaced but the manifest is not updated.
        write_atomic(&slot.root().join("campaign.json"), b"v2-partial").unwrap();
        assert!(matches!(slot.read_verified("campaign.json"), Err(SlotError::Corrupt { .. })));
        // The previous generation is still on disk for recovery.
        assert_eq!(fs::read(slot.root().join("campaign.json.bak")).unwrap(), b"v1");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn unsafe_paths_are_rejected() {
        let slot = SaveSlot::new(temp_dir("unsafe").join("s"));
        for bad in ["../x.json", "/etc/passwd", "a/../../b", "", "manifest.json"] {
            assert!(matches!(slot.commit("0.22.0", &[(bad, vec![])]), Err(SlotError::UnsafePath(_))), "{bad}");
        }
    }

    #[test]
    fn legacy_021_files_are_imported_without_touching_the_source() {
        let dir = temp_dir("legacy");
        let legacy = dir.join("saves");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("campaign.json"), b"{\"credits\":1}").unwrap();
        fs::write(legacy.join("assembly.json"), b"{\"parts\":[]}").unwrap();
        let slot = SaveSlot::new(dir.join("slot_1"));
        let manifest = slot.import_legacy(&legacy, "0.22.0").unwrap();
        assert_eq!(manifest.files.len(), 2);
        assert_eq!(slot.read_verified("designs/assembly.json").unwrap(), b"{\"parts\":[]}");
        assert!(legacy.join("assembly.json").exists());
        assert!(matches!(slot.import_legacy(&legacy, "0.22.0"), Err(SlotError::AlreadyExists)));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn empty_legacy_directory_is_reported() {
        let dir = temp_dir("legacy-empty");
        fs::create_dir_all(dir.join("saves")).unwrap();
        let slot = SaveSlot::new(dir.join("slot_1"));
        assert!(matches!(slot.import_legacy(&dir.join("saves"), "0.22.0"), Err(SlotError::NothingToImport)));
        let _ = fs::remove_dir_all(dir);
    }
}
