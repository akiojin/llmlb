//! Update schedule management.
//!
//! Provides schedule persistence (JSON file) and the `UpdateSchedule` type used by
//! the schedule API and the background scheduler loop.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Schedule mode for an update.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleMode {
    /// Apply immediately.
    Immediate,
    /// Apply when inference is idle (in_flight == 0).
    Idle,
    /// Apply at a specific time.
    Scheduled,
}

/// A persisted update schedule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateSchedule {
    /// Schedule mode.
    pub mode: ScheduleMode,
    /// Target time (only meaningful for `Scheduled` mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_at: Option<DateTime<Utc>>,
    /// Who created the schedule.
    pub scheduled_by: String,
    /// Target version to apply.
    pub target_version: String,
    /// When the schedule was created.
    pub created_at: DateTime<Utc>,
}

/// Manages the `update-schedule.json` file.
#[derive(Debug, Clone)]
pub struct ScheduleStore {
    path: PathBuf,
}

impl ScheduleStore {
    /// Create a new schedule store writing to `update-schedule.json` in `data_dir`.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join("update-schedule.json"),
        }
    }

    /// Load the current schedule (if any).
    pub fn load(&self) -> Result<Option<UpdateSchedule>> {
        if !self.path.exists() {
            return Ok(None);
        }
        let content =
            fs::read_to_string(&self.path).context("Failed to read update-schedule.json")?;
        if content.trim().is_empty() {
            return Ok(None);
        }
        let schedule: UpdateSchedule =
            serde_json::from_str(&content).context("Failed to parse update-schedule.json")?;
        Ok(Some(schedule))
    }

    /// Save a schedule (overwrites any existing one).
    pub fn save(&self, schedule: &UpdateSchedule) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(schedule)?)?;
        fs::rename(tmp, &self.path)?;
        Ok(())
    }

    /// Remove the current schedule.
    pub fn remove(&self) -> Result<bool> {
        if self.path.exists() {
            fs::remove_file(&self.path).context("Failed to remove update-schedule.json")?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = ScheduleStore::new(dir.path());

        // Initially empty.
        assert!(store.load().unwrap().is_none());

        let schedule = UpdateSchedule {
            mode: ScheduleMode::Scheduled,
            scheduled_at: Some(Utc::now()),
            scheduled_by: "admin".to_string(),
            target_version: "5.1.0".to_string(),
            created_at: Utc::now(),
        };

        store.save(&schedule).unwrap();
        let loaded = store.load().unwrap().expect("should have schedule");
        assert_eq!(loaded.mode, ScheduleMode::Scheduled);
        assert_eq!(loaded.target_version, "5.1.0");
        assert_eq!(loaded.scheduled_by, "admin");
        assert!(loaded.scheduled_at.is_some());

        // Remove
        assert!(store.remove().unwrap());
        assert!(store.load().unwrap().is_none());
        assert!(!store.remove().unwrap());
    }

    #[test]
    fn schedule_immediate_mode() {
        let dir = tempfile::tempdir().unwrap();
        let store = ScheduleStore::new(dir.path());

        let schedule = UpdateSchedule {
            mode: ScheduleMode::Immediate,
            scheduled_at: None,
            scheduled_by: "admin".to_string(),
            target_version: "5.1.0".to_string(),
            created_at: Utc::now(),
        };
        store.save(&schedule).unwrap();
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.mode, ScheduleMode::Immediate);
        assert!(loaded.scheduled_at.is_none());
    }

    #[test]
    fn schedule_idle_mode() {
        let dir = tempfile::tempdir().unwrap();
        let store = ScheduleStore::new(dir.path());

        let schedule = UpdateSchedule {
            mode: ScheduleMode::Idle,
            scheduled_at: None,
            scheduled_by: "user1".to_string(),
            target_version: "5.2.0".to_string(),
            created_at: Utc::now(),
        };
        store.save(&schedule).unwrap();
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.mode, ScheduleMode::Idle);
    }

    #[test]
    fn schedule_json_serialization() {
        let schedule = UpdateSchedule {
            mode: ScheduleMode::Scheduled,
            scheduled_at: Some(Utc::now()),
            scheduled_by: "admin".to_string(),
            target_version: "5.1.0".to_string(),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&schedule).unwrap();
        assert!(json.contains("\"mode\":\"scheduled\""));
        assert!(json.contains("\"scheduled_at\""));
        let deserialized: UpdateSchedule = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, schedule);
    }
}
