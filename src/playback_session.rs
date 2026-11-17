//! Local playback checkpoints contain track metadata, never resolved stream URLs.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, io::{Read, Write}, path::{Path, PathBuf}};

const MAX_BYTES: u64 = 16 * 1024 * 1024;
const MAX_QUEUE: usize = 10_000;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Checkpoint {
    pub queue: Vec<Value>,
    pub index: usize,
    pub position: f64,
}

impl Checkpoint {
    pub fn validate(&self) -> Result<(), String> {
        if self.queue.is_empty() || self.queue.len() > MAX_QUEUE || self.index >= self.queue.len()
            || !self.position.is_finite() || self.position < 0.0 || self.position > 604800.0 {
            return Err("Недійсний стан прослуховування".into());
        }
        for track in &self.queue {
            crate::library::validated_track(track)?;
        }
        Ok(())
    }
}

fn path(root: &Path, user: &str) -> PathBuf {
    root.join(format!("{:x}.json", Sha256::digest(user.as_bytes())))
}

pub(crate) fn load(root: &Path, user: &str) -> Option<Checkpoint> {
    let target = path(root, user);
    let metadata = fs::symlink_metadata(&target).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_BYTES {
        return None;
    }
    let root = root.canonicalize().ok()?;
    if target.canonicalize().ok()?.parent() != Some(root.as_path()) {
        return None;
    }
    let file = fs::File::open(target).ok()?;
    if file.metadata().ok()?.len() > MAX_BYTES { return None; }
    let checkpoint: Checkpoint = serde_json::from_reader(file.take(MAX_BYTES)).ok()?;
    checkpoint.validate().ok()?;
    Some(checkpoint)
}

pub(crate) fn load_for_run(root: &Path, user: &str, run: &str) -> Option<Checkpoint> {
    let target = path(root, user);
    let metadata = fs::symlink_metadata(&target).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_BYTES { return None; }
    let value: Value = serde_json::from_reader(fs::File::open(target).ok()?.take(MAX_BYTES)).ok()?;
    if value["run_id"].as_str() != Some(run) { return None; }
    load(root, user)
}

pub(crate) fn save(root: &Path, user: &str, checkpoint: Option<&Checkpoint>) -> Result<(), String> {
    save_for_run(root, user, checkpoint, "")
}

pub(crate) fn save_for_run(root: &Path, user: &str, checkpoint: Option<&Checkpoint>, run: &str) -> Result<(), String> {
    let target = path(root, user);
    let Some(checkpoint) = checkpoint else {
        return match fs::remove_file(target) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        };
    };
    checkpoint.validate()?;
    let mut value = serde_json::to_value(checkpoint).map_err(|e| e.to_string())?;
    value["run_id"] = Value::String(run.to_owned());
    let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES { return Err("Черга завелика для збереження".into()); }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(root).map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(target).map_err(|e| e.error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn preserves_duplicate_queue_occurrences_and_isolates_profiles() {
        let dir = tempfile::tempdir().unwrap();
        let track = json!({"id":"1", "title":"Song", "source":"soundcloud"});
        let checkpoint = Checkpoint { queue: vec![track.clone(), track], index: 1, position: 42.5 };
        save(dir.path(), "../profile", Some(&checkpoint)).unwrap();
        let restored = load(dir.path(), "../profile").unwrap();
        assert_eq!(restored.index, 1);
        assert_eq!(restored.position, 42.5);
        assert_eq!(restored.queue.len(), 2);
        assert!(load(dir.path(), "other").is_none());
        save(dir.path(), "../profile", None).unwrap();
        assert!(load(dir.path(), "../profile").is_none());
    }
    #[test]
    fn new_run_ignores_legacy_and_previous_run_but_keeps_profile_switch() {
        let dir = tempfile::tempdir().unwrap();
        let checkpoint = Checkpoint { queue: vec![json!({"id":"old", "title":"Yesterday"})], index:0, position:42.0 };
        save(dir.path(), "guest", Some(&checkpoint)).unwrap();
        assert!(load_for_run(dir.path(), "guest", "today").is_none());
        save_for_run(dir.path(), "guest", Some(&checkpoint), "yesterday").unwrap();
        assert!(load_for_run(dir.path(), "guest", "today").is_none());
        assert!(load_for_run(dir.path(), "guest", "yesterday").is_some());
        save_for_run(dir.path(), "guest", Some(&checkpoint), "today").unwrap();
        assert_eq!(load_for_run(dir.path(), "guest", "today").unwrap().position, 42.0);
    }
    #[test]
    fn rejects_corruption_and_invalid_positions() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(path(dir.path(), "guest"), b"broken").unwrap();
        assert!(load(dir.path(), "guest").is_none());
        let mut checkpoint = Checkpoint { queue: vec![json!({"id":"1","title":"Song"})], index: 2, position: 0.0 };
        assert!(checkpoint.validate().is_err());
        checkpoint.index = 0;
        checkpoint.position = f64::NAN;
        assert!(checkpoint.validate().is_err());
    }
}
