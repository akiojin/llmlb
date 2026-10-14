//! Durable helper diagnostics in the same data directory as update history.
use anyhow::Result;
use chrono::Utc;
use std::{fs, io::Write, path::Path};

pub(super) fn phase(args_file: &Path, message: &str) {
    let Some(dir) = args_file.parent() else {
        return;
    };
    let result = (|| -> std::io::Result<()> {
        fs::create_dir_all(dir)?;
        let mut log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("helper.log"))?;
        writeln!(log, "{} {message}", Utc::now().to_rfc3339())
    })();
    if let Err(error) = result {
        eprintln!("Failed to write update helper log: {error}");
    }
}

pub(super) fn failure(args_file: &Path, error: &anyhow::Error) {
    let locked = error
        .chain()
        .filter_map(|e| e.downcast_ref::<std::io::Error>())
        .any(|e| matches!(e.raw_os_error(), Some(5 | 32 | 33)));
    let hint = if locked {
        "The executable was locked or access was denied. Close other llmlb instances and retry, or manually run the installer on the server."
    } else {
        "Check the server and manually run the installer if necessary."
    };
    let message = format!("{error:#}. {hint}");
    phase(args_file, &format!("failed: {message}"));
    let data_dir = args_file
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new("."));
    let version = args_file
        .parent()
        .and_then(Path::file_name)
        .and_then(|s| s.to_str())
        .unwrap_or(env!("CARGO_PKG_VERSION"))
        .to_string();
    if let Err(error) =
        super::history::HistoryStore::new(data_dir).append(super::history::HistoryEntry {
            kind: super::history::HistoryEventKind::Failed,
            version,
            message: Some(message),
            timestamp: Utc::now(),
        })
    {
        eprintln!("Failed to record update failure: {error}");
    }
}

pub(super) fn run(args_file: &Path, operation: impl FnOnce() -> Result<()>) -> Result<()> {
    let result = operation();
    if let Err(error) = &result {
        failure(args_file, error);
    }
    result
}

#[cfg(test)]
fn apply_windows_payload(target: &Path, new_binary: &Path, args_file: &Path) -> Result<()> {
    run(args_file, || {
        super::file_replace::replace_executable(target, new_binary)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io};

    #[test]
    fn helper_failure_records_actionable_history_and_stage_log() {
        let dir = tempfile::tempdir().unwrap();
        let update_dir = dir.path().join("updates/6.3.1");
        fs::create_dir_all(&update_dir).unwrap();
        let args_file = update_dir.join("restart_args.json");
        phase(&args_file, "waiting_old_process_exit");
        failure(&args_file, &io::Error::from_raw_os_error(32).into());
        let entries = crate::update::history::HistoryStore::new(dir.path())
            .load()
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].kind,
            crate::update::history::HistoryEventKind::Failed
        );
        assert_eq!(entries[0].version, "6.3.1");
        let message = entries[0].message.as_deref().unwrap();
        assert!(message.contains("locked"), "{message}");
        assert!(message.contains("manually"), "{message}");
        let log = fs::read_to_string(update_dir.join("helper.log")).unwrap();
        assert!(log.contains("waiting_old_process_exit"));
        assert!(log.contains("failed"));
    }

    #[test]
    fn replacement_failure_restores_before_recording_history() {
        let dir = tempfile::tempdir().unwrap();
        let update_dir = dir.path().join("updates/6.3.1");
        fs::create_dir_all(&update_dir).unwrap();
        let target = dir.path().join("llmlb.exe");
        fs::write(&target, "old").unwrap();
        assert!(apply_windows_payload(
            &target,
            &update_dir.join("missing"),
            &update_dir.join("restart_args.json")
        )
        .is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), "old");
        assert_eq!(
            crate::update::history::HistoryStore::new(dir.path())
                .load()
                .unwrap()
                .len(),
            1
        );
    }

    #[cfg(windows)]
    #[test]
    fn permanent_native_lock_preserves_exe_and_records_failure() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let update_dir = dir.path().join("updates/6.3.1");
        fs::create_dir_all(&update_dir).unwrap();
        let target = dir.path().join("llmlb.exe");
        let new = update_dir.join("llmlb.exe");
        fs::write(&target, "old").unwrap();
        fs::write(&new, "new").unwrap();
        let _locked = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&target)
            .unwrap();
        assert!(
            apply_windows_payload(&target, &new, &update_dir.join("restart_args.json")).is_err()
        );
        drop(_locked);
        assert_eq!(fs::read_to_string(target).unwrap(), "old");
        assert_eq!(
            crate::update::history::HistoryStore::new(dir.path())
                .load()
                .unwrap()
                .len(),
            1
        );
    }
}
