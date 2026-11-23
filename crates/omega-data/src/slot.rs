use crate::atomic::write_atomic;
use crate::hash::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const CURRENT_SLOT_SCHEMA: u32 = 2;
const MANIFEST_FILE: &str = "manifest.json";

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub game_version: String,
    pub revision: u64,
    pub files: Vec<ManifestEntry>,
}

#[derive(Debug)]
pub enum SlotError {
    Io(String),
    Parse(String),
    InvalidManifest(String),
    UnsafePath(String),
    DuplicatePath(String),
    MissingFile(String),
    SizeMismatch {
        path: String,
        expected: u64,
        actual: u64,
    },
    Corrupt {
        path: String,
        expected: ContentHash,
        actual: ContentHash,
    },
    UnsupportedSchema(u32),
    RevisionOverflow,
    AlreadyExists,
    NothingToImport,
}

impl fmt::Display for SlotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) | Self::Parse(message) => write!(f, "{message}"),
            Self::InvalidManifest(message) => write!(f, "invalid save manifest: {message}"),
            Self::UnsafePath(path) => write!(f, "unsafe slot path: {path}"),
            Self::DuplicatePath(path) => write!(f, "duplicate path in save manifest: {path}"),
            Self::MissingFile(path) => write!(f, "file listed in manifest is missing: {path}"),
            Self::SizeMismatch { path, expected, actual } => {
                write!(f, "{path} has {actual} bytes; manifest records {expected}")
            }
            Self::Corrupt { path, expected, actual } => write!(
                f,
                "{path} does not match the manifest (expected {}, found {})",
                expected.as_str(),
                actual.as_str()
            ),
            Self::UnsupportedSchema(version) => write!(f, "unsupported save schema {version}"),
            Self::RevisionOverflow => write!(f, "save revision counter overflow"),
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
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && rel != MANIFEST_FILE;
    if safe {
        Ok(())
    } else {
        Err(SlotError::UnsafePath(rel.to_string()))
    }
}

fn validate_manifest(manifest: &Manifest) -> Result<(), SlotError> {
    if manifest.schema_version != CURRENT_SLOT_SCHEMA {
        return Err(SlotError::UnsupportedSchema(manifest.schema_version));
    }
    if manifest.game_version.trim().is_empty() {
        return Err(SlotError::InvalidManifest("game_version is empty".to_string()));
    }
    if manifest.revision == 0 {
        return Err(SlotError::InvalidManifest(
            "revision must be greater than zero".to_string(),
        ));
    }

    let mut paths = HashSet::with_capacity(manifest.files.len());
    for entry in &manifest.files {
        check_relative(&entry.path)?;
        if !paths.insert(entry.path.as_str()) {
            return Err(SlotError::DuplicatePath(entry.path.clone()));
        }
        let hash = entry.hash.as_str();
        if hash.len() != 16 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(SlotError::InvalidManifest(format!(
                "invalid content hash for {}",
                entry.path
            )));
        }
    }
    Ok(())
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
        let bytes =
            fs::read(&path).map_err(|error| SlotError::Io(format!("cannot read {}: {error}", path.display())))?;
        let manifest: Manifest =
            serde_json::from_slice(&bytes).map_err(|error| SlotError::Parse(format!("invalid manifest: {error}")))?;
        validate_manifest(&manifest)?;
        Ok(Some(manifest))
    }

    pub fn commit(&self, game_version: &str, files: &[(&str, Vec<u8>)]) -> Result<Manifest, SlotError> {
        let mut input_paths = HashSet::with_capacity(files.len());
        for (rel, _) in files {
            check_relative(rel)?;
            if !input_paths.insert(*rel) {
                return Err(SlotError::DuplicatePath((*rel).to_string()));
            }
        }
        if game_version.trim().is_empty() {
            return Err(SlotError::InvalidManifest("game_version is empty".to_string()));
        }

        let previous = self.read_manifest()?;
        if previous.is_some() {
            if let Some(problem) = self.verify_all()?.into_iter().next() {
                return Err(problem);
            }
        }
        let revision = match previous.as_ref() {
            Some(manifest) => manifest.revision.checked_add(1).ok_or(SlotError::RevisionOverflow)?,
            None => 1,
        };
        let mut entries: Vec<ManifestEntry> = previous
            .as_ref()
            .map(|manifest| {
                manifest
                    .files
                    .iter()
                    .filter(|entry| !input_paths.contains(entry.path.as_str()))
                    .cloned()
                    .collect()
            })
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
            revision,
            files: entries,
        };
        let bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| SlotError::Parse(error.to_string()))?;
        write_atomic(&self.root.join(MANIFEST_FILE), &bytes).map_err(SlotError::Io)?;
        Ok(manifest)
    }

    pub fn read_verified(&self, rel: &str) -> Result<Vec<u8>, SlotError> {
        check_relative(rel)?;
        let manifest = self
            .read_manifest()?
            .ok_or_else(|| SlotError::MissingFile(MANIFEST_FILE.to_string()))?;
        let entry = manifest
            .files
            .iter()
            .find(|entry| entry.path == rel)
            .ok_or_else(|| SlotError::MissingFile(rel.to_string()))?;
        let path = self.root.join(rel);
        let bytes = fs::read(&path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                SlotError::MissingFile(rel.to_string())
            } else {
                SlotError::Io(format!("cannot read {}: {error}", path.display()))
            }
        })?;
        if bytes.len() as u64 != entry.bytes {
            return Err(SlotError::SizeMismatch {
                path: rel.to_string(),
                expected: entry.bytes,
                actual: bytes.len() as u64,
            });
        }
        let actual = ContentHash::of_bytes(&bytes);
        if actual != entry.hash {
            return Err(SlotError::Corrupt {
                path: rel.to_string(),
                expected: entry.hash.clone(),
                actual,
            });
        }
        Ok(bytes)
    }

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

    pub fn import_legacy(&self, legacy_dir: &Path, game_version: &str) -> Result<Manifest, SlotError> {
        if self.read_manifest()?.is_some() {
            return Err(SlotError::AlreadyExists);
        }
        let mut files: Vec<(&str, Vec<u8>)> = Vec::new();
        for (legacy_name, target) in LEGACY_FILES {
            let path = legacy_dir.join(legacy_name);
            if path.exists() {
                let bytes = fs::read(&path)
                    .map_err(|error| SlotError::Io(format!("cannot read {}: {error}", path.display())))?;
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

    fn write_manifest(slot: &SaveSlot, manifest: &Manifest) {
        fs::create_dir_all(slot.root()).unwrap();
        fs::write(slot.root().join(MANIFEST_FILE), serde_json::to_vec(manifest).unwrap()).unwrap();
    }

    fn valid_manifest() -> Manifest {
        Manifest {
            schema_version: CURRENT_SLOT_SCHEMA,
            game_version: "0.30.1".to_string(),
            revision: 1,
            files: vec![ManifestEntry {
                path: "campaign.json".to_string(),
                hash: ContentHash::of_bytes(b"data"),
                bytes: 4,
            }],
        }
    }

    #[test]
    fn commit_then_read_verified_round_trips() {
        let dir = temp_dir("roundtrip");
        let slot = SaveSlot::new(dir.join("slot_1"));
        let manifest = slot
            .commit("0.30.1", &[("campaign.json", b"{\"a\":1}".to_vec())])
            .unwrap();
        assert_eq!(manifest.revision, 1);
        assert_eq!(slot.read_verified("campaign.json").unwrap(), b"{\"a\":1}");
        let second = slot.commit("0.30.1", &[("designs/a.json", b"[]".to_vec())]).unwrap();
        assert_eq!(second.revision, 2);
        assert_eq!(second.files.len(), 2);
        assert!(slot.verify_all().unwrap().is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn tampered_file_is_detected() {
        let dir = temp_dir("tamper");
        let slot = SaveSlot::new(dir.join("s"));
        slot.commit("0.30.1", &[("campaign.json", b"good".to_vec())]).unwrap();
        fs::write(slot.root().join("campaign.json"), b"evil").unwrap();
        assert!(matches!(
            slot.read_verified("campaign.json"),
            Err(SlotError::Corrupt { .. })
        ));
        assert_eq!(slot.verify_all().unwrap().len(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_commit_refuses_to_overwrite_a_corrupt_slot_or_its_backup() {
        let dir = temp_dir("corrupt-commit");
        let slot = SaveSlot::new(dir.join("s"));
        slot.commit("0.30.1", &[("campaign.json", b"v1".to_vec())]).unwrap();
        slot.commit("0.30.1", &[("campaign.json", b"v2".to_vec())]).unwrap();
        fs::write(slot.root().join("campaign.json"), b"tampered").unwrap();

        let result = slot.commit("0.30.1", &[("campaign.json", b"v3".to_vec())]);
        assert!(matches!(result, Err(SlotError::Corrupt { .. })));
        assert_eq!(fs::read(slot.root().join("campaign.json")).unwrap(), b"tampered");
        assert_eq!(fs::read(slot.root().join("campaign.json.bak")).unwrap(), b"v1");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn manifest_rejects_unsupported_schema_versions() {
        let dir = temp_dir("schema");
        let slot = SaveSlot::new(dir.join("s"));
        let mut manifest = valid_manifest();
        manifest.schema_version = CURRENT_SLOT_SCHEMA + 1;
        write_manifest(&slot, &manifest);
        assert!(matches!(slot.read_manifest(), Err(SlotError::UnsupportedSchema(_))));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn manifest_rejects_older_unmigrated_schema_versions() {
        let dir = temp_dir("old-schema");
        let slot = SaveSlot::new(dir.join("s"));
        let mut manifest = valid_manifest();
        manifest.schema_version = 1;
        write_manifest(&slot, &manifest);
        assert!(matches!(slot.read_manifest(), Err(SlotError::UnsupportedSchema(1))));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn manifest_rejects_unsafe_and_duplicate_paths() {
        let dir = temp_dir("manifest-paths");
        let slot = SaveSlot::new(dir.join("s"));
        let mut manifest = valid_manifest();
        manifest.files[0].path = "../outside.json".to_string();
        write_manifest(&slot, &manifest);
        assert!(matches!(slot.read_manifest(), Err(SlotError::UnsafePath(_))));

        let mut manifest = valid_manifest();
        manifest.files.push(manifest.files[0].clone());
        write_manifest(&slot, &manifest);
        assert!(matches!(slot.read_manifest(), Err(SlotError::DuplicatePath(_))));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn manifest_rejects_invalid_hashes_and_zero_revision() {
        let dir = temp_dir("manifest-invalid");
        let slot = SaveSlot::new(dir.join("s"));
        let mut manifest = valid_manifest();
        manifest.files[0].hash = ContentHash("not-a-hash".to_string());
        write_manifest(&slot, &manifest);
        assert!(matches!(slot.read_manifest(), Err(SlotError::InvalidManifest(_))));

        let mut manifest = valid_manifest();
        manifest.revision = 0;
        write_manifest(&slot, &manifest);
        assert!(matches!(slot.read_manifest(), Err(SlotError::InvalidManifest(_))));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn duplicate_paths_in_a_commit_are_rejected_before_writing() {
        let dir = temp_dir("duplicate-input");
        let slot = SaveSlot::new(dir.join("s"));
        let result = slot.commit(
            "0.30.1",
            &[("campaign.json", b"one".to_vec()), ("campaign.json", b"two".to_vec())],
        );
        assert!(matches!(result, Err(SlotError::DuplicatePath(_))));
        assert!(!slot.root().join("campaign.json").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn file_size_is_checked_before_hash() {
        let dir = temp_dir("size");
        let slot = SaveSlot::new(dir.join("s"));
        slot.commit("0.30.1", &[("campaign.json", b"longer".to_vec())]).unwrap();
        fs::write(slot.root().join("campaign.json"), b"x").unwrap();
        assert!(matches!(
            slot.read_verified("campaign.json"),
            Err(SlotError::SizeMismatch { .. })
        ));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn crash_before_manifest_commit_keeps_previous_state_authoritative() {
        let dir = temp_dir("crash");
        let slot = SaveSlot::new(dir.join("s"));
        slot.commit("0.30.1", &[("campaign.json", b"v1".to_vec())]).unwrap();
        write_atomic(&slot.root().join("campaign.json"), b"v2-partial").unwrap();
        assert!(matches!(
            slot.read_verified("campaign.json"),
            Err(SlotError::Corrupt { .. })
        ));
        assert_eq!(fs::read(slot.root().join("campaign.json.bak")).unwrap(), b"v1");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn unsafe_paths_are_rejected() {
        let slot = SaveSlot::new(temp_dir("unsafe").join("s"));
        for bad in ["../x.json", "/etc/passwd", "a/../../b", "", "manifest.json"] {
            assert!(
                matches!(slot.commit("0.30.1", &[(bad, vec![])]), Err(SlotError::UnsafePath(_))),
                "{bad}"
            );
        }
        let _ = fs::remove_dir_all(slot.root().parent().unwrap());
    }

    #[test]
    fn legacy_021_files_are_imported_without_touching_the_source() {
        let dir = temp_dir("legacy");
        let legacy = dir.join("saves");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("campaign.json"), b"{\"credits\":1}").unwrap();
        fs::write(legacy.join("assembly.json"), b"{\"parts\":[]}").unwrap();
        let slot = SaveSlot::new(dir.join("slot_1"));
        let manifest = slot.import_legacy(&legacy, "0.30.1").unwrap();
        assert_eq!(manifest.files.len(), 2);
        assert_eq!(slot.read_verified("designs/assembly.json").unwrap(), b"{\"parts\":[]}");
        assert!(legacy.join("assembly.json").exists());
        assert!(matches!(
            slot.import_legacy(&legacy, "0.30.1"),
            Err(SlotError::AlreadyExists)
        ));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn empty_legacy_directory_is_reported() {
        let dir = temp_dir("legacy-empty");
        fs::create_dir_all(dir.join("saves")).unwrap();
        let slot = SaveSlot::new(dir.join("slot_1"));
        assert!(matches!(
            slot.import_legacy(&dir.join("saves"), "0.30.1"),
            Err(SlotError::NothingToImport)
        ));
        let _ = fs::remove_dir_all(dir);
    }
}
