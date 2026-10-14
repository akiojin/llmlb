//! Bounded Windows file operations. Never discard the original on apply failure.
use anyhow::{Context, Result};
use std::{fs, io, path::Path, time::Duration};

fn retry(operation: &mut impl FnMut() -> io::Result<()>) -> io::Result<()> {
    for attempt in 0..16 {
        match operation() {
            Err(error) if matches!(error.raw_os_error(), Some(5 | 32 | 33)) && attempt < 15 => {
                std::thread::sleep(Duration::from_millis(200));
            }
            result => return result,
        }
    }
    unreachable!()
}

fn replace_using(
    target: &Path,
    new_binary: &Path,
    rename: &mut impl FnMut(&Path, &Path) -> io::Result<()>,
) -> Result<()> {
    let backup = target.with_extension("bak");
    if target.exists() {
        if backup.exists() {
            retry(&mut || fs::remove_file(&backup)).context("Failed to remove previous backup")?;
        }
        retry(&mut || rename(target, &backup)).context("Failed to back up target executable")?;
    }
    let install = retry(&mut || rename(new_binary, target));
    if let Err(error) = install {
        if backup.exists() {
            retry(&mut || match fs::remove_file(target) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(e),
            })
            .context("Failed to remove incomplete update; original remains in .bak")?;
            retry(&mut || rename(&backup, target))
                .with_context(|| format!("Failed to restore original executable after {error}"))?;
        }
        return Err(error).context("Failed to replace target executable; original restored");
    }
    Ok(())
}

pub(super) fn replace_executable(target: &Path, new_binary: &Path) -> Result<()> {
    replace_using(target, new_binary, &mut |source, destination| {
        match fs::rename(source, destination) {
            Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
                // Copy beside the target first; a failed copy must never expose a partial EXE.
                let staged = destination.with_extension(format!("update-{}", uuid::Uuid::new_v4()));
                let result =
                    fs::copy(source, &staged).and_then(|_| fs::rename(&staged, destination));
                let _ = fs::remove_file(&staged);
                if result.is_ok() {
                    let _ = fs::remove_file(source);
                }
                result
            }
            result => result,
        }
    })
}

#[cfg(any(windows, test))]
pub(super) fn replace_and_start<T>(
    target: &Path,
    new_binary: &Path,
    start: impl FnOnce() -> Result<T>,
) -> Result<T> {
    replace_executable(target, new_binary)?;
    match start() {
        Ok(child) => Ok(child),
        Err(error) => {
            retry(&mut || match fs::remove_file(target) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
            })
            .context("Failed to remove unstartable update; original remains in .bak")?;
            retry(&mut || fs::rename(target.with_extension("bak"), target)).with_context(|| {
                format!("Failed to restore original executable after {error:#}")
            })?;
            Err(error).context("Failed to start update; original executable restored")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_windows_locks_retry_before_replacing() {
        for code in [5, 32, 33] {
            let dir = tempfile::tempdir().unwrap();
            let target = dir.path().join("llmlb.exe");
            let new = dir.path().join("new.exe");
            fs::write(&target, "old").unwrap();
            fs::write(&new, "new").unwrap();
            let mut attempts = 0;
            replace_using(&target, &new, &mut |from, to| {
                if from == target {
                    attempts += 1;
                    if attempts < 2 {
                        return Err(io::Error::from_raw_os_error(code));
                    }
                }
                fs::rename(from, to)
            })
            .unwrap();
            assert_eq!(attempts, 2);
            assert_eq!(fs::read_to_string(&target).unwrap(), "new");
            assert_eq!(
                fs::read_to_string(target.with_extension("bak")).unwrap(),
                "old"
            );
        }
    }

    #[test]
    fn permanent_lock_preserves_original_and_bounds_retries() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("llmlb.exe");
        let new = dir.path().join("new.exe");
        fs::write(&target, "old").unwrap();
        fs::write(&new, "new").unwrap();
        let mut attempts = 0;
        let error = replace_using(&target, &new, &mut |_, _| {
            attempts += 1;
            Err(io::Error::from_raw_os_error(32))
        })
        .unwrap_err();
        assert_eq!(attempts, 16);
        assert!(format!("{error:#}").contains("back up"));
        assert_eq!(fs::read_to_string(target).unwrap(), "old");
        assert_eq!(fs::read_to_string(new).unwrap(), "new");
    }

    #[test]
    fn failed_install_restores_original() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("llmlb.exe");
        let new = dir.path().join("missing.exe");
        fs::write(&target, "old").unwrap();
        assert!(replace_executable(&target, &new).is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), "old");
    }

    #[test]
    fn failed_restart_restores_previous_executable() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("llmlb.exe");
        let new = dir.path().join("new.exe");
        fs::write(&target, "old").unwrap();
        fs::write(&new, "broken-new").unwrap();
        let result: Result<()> = replace_and_start(&target, &new, || {
            anyhow::bail!("Failed to spawn restarted process")
        });
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), "old");
    }

    #[test]
    fn disappeared_update_still_restores_original_after_spawn_failure() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("llmlb.exe");
        let new = dir.path().join("new.exe");
        fs::write(&target, "old").unwrap();
        fs::write(&new, "quarantined-update").unwrap();
        let result: Result<()> = replace_and_start(&target, &new, || {
            fs::remove_file(&target)?;
            anyhow::bail!("Failed to spawn restarted process")
        });
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), "old");
    }

    #[test]
    fn unrelated_errors_are_not_retried() {
        let mut attempts = 0;
        assert!(retry(&mut || {
            attempts += 1;
            Err(io::Error::from_raw_os_error(2))
        })
        .is_err());
        assert_eq!(attempts, 1);
    }

    #[cfg(windows)]
    #[test]
    fn actual_windows_lock_releases_before_retry_limit() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("llmlb.exe");
        let new = dir.path().join("new.exe");
        fs::write(&target, "old").unwrap();
        fs::write(&new, "new").unwrap();
        let mut locked = Some(
            fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(&target)
                .unwrap(),
        );
        let mut observed_lock = false;
        replace_using(&target, &new, &mut |source, destination| {
            let result = fs::rename(source, destination);
            if source == target && result.is_err() {
                observed_lock = true;
                // Release only after a real sharing violation, never after an arbitrary wait.
                drop(locked.take());
            }
            result
        })
        .unwrap();
        assert!(observed_lock);
        assert_eq!(fs::read_to_string(target).unwrap(), "new");
    }
}
