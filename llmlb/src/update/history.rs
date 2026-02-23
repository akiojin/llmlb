//! Update history management.
//!
//! Records update events (check, apply, rollback) in a JSON file.
//! Only the most recent 100 entries are retained.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_HISTORY_ENTRIES: usize = 100;

/// Kind of update history event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HistoryEventKind {
    /// Update applied successfully.
    Applied,
    /// Update apply failed.
    Failed,
    /// Rollback performed.
    Rollback,
}

/// A single update history entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    /// Event kind.
    pub kind: HistoryEventKind,
    /// Version involved.
    pub version: String,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
}

/// Persisted history file wrapper.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct HistoryFile {
    entries: Vec<HistoryEntry>,
}

/// Manages the `update-history.json` file.
#[derive(Debug, Clone)]
pub struct HistoryStore {
    path: PathBuf,
}

impl HistoryStore {
    /// Create a new history store writing to `update-history.json` in `data_dir`.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join("update-history.json"),
        }
    }

    /// Load all history entries.
    pub fn load(&self) -> Result<Vec<HistoryEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content =
            fs::read_to_string(&self.path).context("Failed to read update-history.json")?;
        if content.trim().is_empty() {
            return Ok(Vec::new());
        }
        let file: HistoryFile =
            serde_json::from_str(&content).context("Failed to parse update-history.json")?;
        Ok(file.entries)
    }

    /// Append an entry, keeping only the most recent 100.
    pub fn append(&self, entry: HistoryEntry) -> Result<()> {
        let mut entries = self.load().unwrap_or_default();
        entries.push(entry);
        // Truncate to most recent MAX_HISTORY_ENTRIES.
        if entries.len() > MAX_HISTORY_ENTRIES {
            let start = entries.len() - MAX_HISTORY_ENTRIES;
            entries = entries[start..].to_vec();
        }
        self.save_entries(&entries)
    }

    fn save_entries(&self, entries: &[HistoryEntry]) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let file = HistoryFile {
            entries: entries.to_vec(),
        };
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(&file)?)?;
        fs::rename(tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_append_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::new(dir.path());

        assert!(store.load().unwrap().is_empty());

        store
            .append(HistoryEntry {
                kind: HistoryEventKind::Applied,
                version: "5.1.0".to_string(),
                message: Some("Update applied successfully".to_string()),
                timestamp: Utc::now(),
            })
            .unwrap();

        let entries = store.load().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, HistoryEventKind::Applied);
        assert_eq!(entries[0].version, "5.1.0");
    }

    #[test]
    fn history_truncates_at_100() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::new(dir.path());

        for i in 0..110 {
            store
                .append(HistoryEntry {
                    kind: HistoryEventKind::Applied,
                    version: format!("1.0.{i}"),
                    message: None,
                    timestamp: Utc::now(),
                })
                .unwrap();
        }

        let entries = store.load().unwrap();
        assert_eq!(entries.len(), MAX_HISTORY_ENTRIES);
        // The oldest entries should have been trimmed.
        assert_eq!(entries[0].version, "1.0.10");
        assert_eq!(entries[99].version, "1.0.109");
    }

    #[test]
    fn history_rollback_entry() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::new(dir.path());

        store
            .append(HistoryEntry {
                kind: HistoryEventKind::Rollback,
                version: "5.1.0".to_string(),
                message: Some("Rolled back to 5.0.1".to_string()),
                timestamp: Utc::now(),
            })
            .unwrap();

        let entries = store.load().unwrap();
        assert_eq!(entries[0].kind, HistoryEventKind::Rollback);
    }

    #[test]
    fn history_json_serialization() {
        let entry = HistoryEntry {
            kind: HistoryEventKind::Failed,
            version: "5.1.0".to_string(),
            message: Some("Download failed".to_string()),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"kind\":\"failed\""));
        let deserialized: HistoryEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, entry);
    }
}
