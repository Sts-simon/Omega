use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

/// Write `bytes` to `path` so that a crash never leaves a half-written file:
/// write a sibling temp file, flush and fsync it, then rename over the target.
/// An existing target is first copied to `<name>.bak` (one previous generation).
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("invalid path {}", path.display()))?;
    let tmp = path.with_file_name(format!("{file_name}.tmp"));
    {
        let mut file = File::create(&tmp).map_err(|e| format!("cannot create {}: {e}", tmp.display()))?;
        file.write_all(bytes).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
        file.flush().map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| format!("cannot sync {}: {e}", tmp.display()))?;
    }
    if path.exists() {
        let bak = path.with_file_name(format!("{file_name}.bak"));
        // Best effort: a failed backup must not block saving the new state.
        let _ = fs::copy(path, bak);
    }
    fs::rename(&tmp, path).map_err(|e| format!("cannot replace {}: {e}", path.display()))
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
        assert!(!path.with_file_name("a.json.tmp").exists());
        let _ = fs::remove_dir_all(dir);
    }
}
