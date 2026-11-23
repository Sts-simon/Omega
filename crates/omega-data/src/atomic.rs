use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn unique_sibling(path: &Path, suffix: &str) -> Result<PathBuf, String> {
    let file_name = path.file_name().and_then(|name| name.to_str())
        .ok_or_else(|| format!("invalid path {}", path.display()))?;
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    Ok(path.with_file_name(format!("{file_name}.{}.{}.{}", std::process::id(), sequence, suffix)))
}

fn sync_parent(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        File::open(parent).and_then(|directory| directory.sync_all())
            .map_err(|error| format!("cannot sync directory {}: {error}", parent.display()))?;
    }
    Ok(())
}

fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
    file.write_all(bytes).map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    file.flush().map_err(|error| format!("cannot flush {}: {error}", path.display()))?;
    file.sync_all().map_err(|error| format!("cannot sync {}: {error}", path.display()))
}

/// Replace a file through a unique sibling temporary file.
///
/// The new contents and previous-generation backup are synced before their renames. If preserving
/// the backup fails, the replacement is aborted rather than silently discarding the previous copy.
/// On Unix, the containing directory is synced after each rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }

    let temp = unique_sibling(path, "tmp")?;
    let backup_temp = unique_sibling(path, "bak.tmp")?;
    let file_name = path.file_name().and_then(|name| name.to_str())
        .ok_or_else(|| format!("invalid path {}", path.display()))?;
    let backup = path.with_file_name(format!("{file_name}.bak"));

    let result = (|| {
        write_new_synced(&temp, bytes)?;

        if path.exists() {
            fs::copy(path, &backup_temp)
                .map_err(|error| format!("cannot back up {}: {error}", path.display()))?;
            File::open(&backup_temp).and_then(|file| file.sync_all())
                .map_err(|error| format!("cannot sync backup for {}: {error}", path.display()))?;
            fs::rename(&backup_temp, &backup)
                .map_err(|error| format!("cannot replace backup {}: {error}", backup.display()))?;
            sync_parent(&backup)?;
        }

        fs::rename(&temp, path).map_err(|error| format!("cannot replace {}: {error}", path.display()))?;
        sync_parent(path)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp);
        let _ = fs::remove_file(&backup_temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("omega-data-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn writes_new_file_and_keeps_previous_generation() {
        let dir = temp_dir("atomic");
        let path = dir.join("nested").join("a.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        assert_eq!(fs::read(path.with_file_name("a.json.bak")).unwrap(), b"one");
        let leftovers = fs::read_dir(path.parent().unwrap()).unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn failed_write_does_not_replace_the_previous_file() {
        let dir = temp_dir("atomic-failure");
        let path = dir.join("state.json");
        write_atomic(&path, b"original").unwrap();
        let invalid_parent = dir.join("not-a-directory");
        fs::write(&invalid_parent, b"file").unwrap();
        let result = write_atomic(&invalid_parent.join("state.json"), b"replacement");
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        let _ = fs::remove_dir_all(dir);
    }
}
