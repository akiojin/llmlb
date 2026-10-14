//! Windows installer mode preservation and bounded privilege elevation.

use anyhow::{anyhow, Result};
use std::{
    ffi::OsString,
    fmt, io,
    path::Path,
    time::{Duration, Instant},
};

#[cfg(windows)]
pub(crate) use native::run_windows_installer_worker;
#[cfg(windows)]
pub(super) use native::{installer_mode, requires_elevation, run_windows_installer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InstallerMode {
    CurrentUser,
    AllUsers,
}

fn select_installer_mode(
    current_user_registered: bool,
    all_users_registered: bool,
    in_program_files: bool,
    writable: bool,
) -> Result<InstallerMode> {
    match (current_user_registered, all_users_registered) {
        (true, true) => Err(anyhow!(
            "The target directory is registered for both per-user and all-users installations. Run the installer manually and select the existing installation."
        )),
        (true, false) => Ok(InstallerMode::CurrentUser),
        (false, true) => Ok(InstallerMode::AllUsers),
        (false, false) if in_program_files => Ok(InstallerMode::AllUsers),
        (false, false) if writable => Ok(InstallerMode::CurrentUser),
        _ => Err(anyhow!(
            "Cannot determine the installation mode for this protected directory. Run the installer manually and select the existing installation."
        )),
    }
}

fn elevation_required(mode: InstallerMode, writable: bool, elevated: bool) -> Result<bool> {
    match mode {
        InstallerMode::AllUsers => Ok(!elevated),
        InstallerMode::CurrentUser if writable => Ok(false),
        InstallerMode::CurrentUser => Err(anyhow!(
            "The per-user installation directory is not writable. Check its permissions or run the installer manually as the original user."
        )),
    }
}

fn installer_parameters(target: &Path, mode: InstallerMode) -> Result<Vec<OsString>> {
    let directory = target
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| anyhow!("Windows update target has no installation directory"))?;
    let mut destination = OsString::from("/DIR=");
    destination.push(directory);
    Ok(vec![
        OsString::from("/VERYSILENT"),
        OsString::from("/CLOSEAPPLICATIONS"),
        OsString::from("/SUPPRESSMSGBOXES"),
        OsString::from("/NORESTART"),
        OsString::from(match mode {
            InstallerMode::CurrentUser => "/CURRENTUSER",
            InstallerMode::AllUsers => "/ALLUSERS",
        }),
        destination,
    ])
}

fn installer_launch_error(error: io::Error) -> anyhow::Error {
    match error.raw_os_error() {
        Some(1223) => anyhow!(
            "Administrator elevation was declined (Windows error 1223). Run the installer manually and approve the UAC prompt. {error}"
        ),
        Some(5 | 740) => anyhow!(
            "Administrator privileges are required or unavailable. Run the installer manually as administrator in an interactive Windows session. {error}"
        ),
        _ => anyhow!(
            "Failed to start the Windows installer. Run the downloaded installer manually. {error}"
        ),
    }
}

fn setup_exit_result(code: u32) -> Result<()> {
    if code == 0 {
        return Ok(());
    }
    let detail = if code == 8 {
        " Windows must be restarted before installation can proceed."
    } else {
        ""
    };
    Err(anyhow!(
        "Windows setup installer failed with exit code {code}.{detail} Run the installer manually to inspect the failure."
    ))
}

#[derive(Debug)]
struct WaitDeadline {
    phase: &'static str,
    timeout: Duration,
}

impl fmt::Display for WaitDeadline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} timed out after {} seconds. Check the Windows session and run the installer manually.",
            self.phase,
            self.timeout.as_secs()
        )
    }
}

impl std::error::Error for WaitDeadline {}

fn bounded_wait<T>(
    phase: &'static str,
    timeout: Duration,
    mut poll: impl FnMut() -> Result<Option<T>>,
) -> Result<T> {
    let started = Instant::now();
    loop {
        if let Some(value) = poll()? {
            return Ok(value);
        }
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(WaitDeadline { phase, timeout }.into());
        }
        std::thread::sleep(remaining.min(Duration::from_millis(50)));
    }
}

#[derive(Debug)]
struct InstallerMayStillRun(String);

impl fmt::Display for InstallerMayStillRun {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} The installer may still be running; do not restart or retry the update until it finishes. Check the Windows session and run the installer manually if necessary.",
            self.0
        )
    }
}

impl std::error::Error for InstallerMayStillRun {}

fn installer_still_running_error(detail: impl Into<String>) -> anyhow::Error {
    InstallerMayStillRun(detail.into()).into()
}

pub(super) fn installer_may_still_run(error: &anyhow::Error) -> bool {
    error.downcast_ref::<InstallerMayStillRun>().is_some()
}

/// Quote UTF-16 command-line units without losing non-Unicode Windows paths.
fn quote_windows_units(argument: &[u16]) -> Vec<u16> {
    let mut quoted = vec![b'"' as u16];
    let mut backslashes = 0;
    for &unit in argument {
        if unit == b'\\' as u16 {
            backslashes += 1;
            continue;
        }
        let count = if unit == b'"' as u16 {
            backslashes * 2 + 1
        } else {
            backslashes
        };
        quoted.extend(std::iter::repeat_n(b'\\' as u16, count));
        backslashes = 0;
        quoted.push(unit);
    }
    quoted.extend(std::iter::repeat_n(b'\\' as u16, backslashes * 2));
    quoted.push(b'"' as u16);
    quoted
}

#[cfg(windows)]
mod native {
    use super::*;
    use anyhow::Context;
    use serde::{Deserialize, Serialize};
    use std::{
        fs,
        io::Read,
        os::windows::{
            ffi::OsStrExt,
            io::{AsRawHandle, FromRawHandle, OwnedHandle},
        },
        path::PathBuf,
        process::{Child, Command, ExitStatus, Stdio},
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::{
            ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS, HANDLE, WAIT_FAILED,
            WAIT_OBJECT_0, WAIT_TIMEOUT,
        },
        Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY},
        System::{
            Com::{
                CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
            },
            Registry::{
                RegGetValueW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ,
                RRF_SUBKEY_WOW6432KEY, RRF_SUBKEY_WOW6464KEY,
            },
            Threading::{
                GetCurrentProcess, GetExitCodeProcess, OpenProcessToken, TerminateProcess,
                WaitForSingleObject,
            },
        },
        UI::{
            Shell::{
                ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS,
                SHELLEXECUTEINFOW,
            },
            WindowsAndMessaging::SW_SHOWNORMAL,
        },
    };

    const LAUNCH_TIMEOUT: Duration = Duration::from_secs(60);
    const INSTALL_TIMEOUT: Duration = Duration::from_secs(300);
    const STOP_TIMEOUT: Duration = Duration::from_secs(5);
    const UNINSTALL_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{B5064C44-1AB8-4AEA-86A1-0B36FEC7B6B7}_is1";

    fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
        value.encode_wide().chain(Some(0)).collect()
    }

    fn normalized_path(path: &Path) -> Vec<u16> {
        let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let mut units: Vec<_> = path
            .as_os_str()
            .encode_wide()
            .map(|unit| match unit {
                47 => 92,
                65..=90 => unit + 32,
                _ => unit,
            })
            .collect();
        if units.starts_with(&[92, 92, 63, 92]) {
            units.drain(..4);
        }
        while units.last() == Some(&92) {
            units.pop();
        }
        units
    }

    fn in_environment_root(directory: &Path, name: &str) -> bool {
        let Some(root) = std::env::var_os(name) else {
            return false;
        };
        let directory = normalized_path(directory);
        let mut root = normalized_path(Path::new(&root));
        if root.is_empty() {
            return false;
        }
        if directory == root {
            return true;
        }
        root.push(92);
        directory.starts_with(&root)
    }

    fn registered_location(hive: HKEY, view: u32) -> Result<Option<PathBuf>> {
        let key = wide(std::ffi::OsStr::new(UNINSTALL_KEY));
        let value = wide(std::ffi::OsStr::new("InstallLocation"));
        let flags = RRF_RT_REG_SZ | view;
        let mut byte_count = 0u32;
        // SAFETY: the nul-terminated input buffers outlive the call, and the size output is valid.
        let result = unsafe {
            RegGetValueW(
                hive,
                key.as_ptr(),
                value.as_ptr(),
                flags,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut byte_count,
            )
        };
        if matches!(result, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) {
            return Ok(None);
        }
        if result != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(result as i32)).context(
                "Cannot read the existing Windows installation mode; run the installer manually",
            );
        }
        if byte_count == 0 || byte_count > 65_536 || !byte_count.is_multiple_of(2) {
            return Err(anyhow!(
                "Invalid Windows installer registration; run the installer manually"
            ));
        }
        let mut buffer = vec![0u16; byte_count as usize / 2];
        // SAFETY: the buffer is sized from the preceding call and all pointers remain valid.
        let result = unsafe {
            RegGetValueW(
                hive,
                key.as_ptr(),
                value.as_ptr(),
                flags,
                ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut byte_count,
            )
        };
        if result != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(result as i32))
                .context("Cannot read the existing Windows installation directory; run the installer manually");
        }
        let end = buffer
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(buffer.len());
        if end == 0 {
            return Ok(None);
        }
        use std::os::windows::ffi::OsStringExt;
        Ok(Some(PathBuf::from(OsString::from_wide(&buffer[..end]))))
    }

    fn registration_matches(hive: HKEY, directory: &Path) -> Result<bool> {
        for view in [RRF_SUBKEY_WOW6464KEY, RRF_SUBKEY_WOW6432KEY] {
            if registered_location(hive, view)?
                .is_some_and(|location| normalized_path(&location) == normalized_path(directory))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(in crate::update) fn installer_mode(target: &Path) -> Result<InstallerMode> {
        let directory = target
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| anyhow!("Windows update target has no installation directory"))?;
        let current_user = registration_matches(HKEY_CURRENT_USER, directory)?;
        let all_users = registration_matches(HKEY_LOCAL_MACHINE, directory)?;
        let program_files = ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"]
            .iter()
            .any(|name| in_environment_root(directory, name));
        let writable = super::super::platform::is_dir_writable(directory)?;
        let local_app_data = in_environment_root(directory, "LOCALAPPDATA");
        let mode = if !current_user && !all_users && local_app_data && !program_files {
            InstallerMode::CurrentUser
        } else {
            select_installer_mode(current_user, all_users, program_files, writable)?
        };
        let reason = if current_user || all_users {
            "matching uninstall InstallLocation registration"
        } else if program_files {
            "Program Files directory boundary"
        } else if local_app_data {
            "current user's LOCALAPPDATA directory boundary"
        } else {
            "unregistered writable directory; preserve per-user default"
        };
        tracing::info!(?mode, reason, target = %target.display(), "Selected Windows installer mode");
        Ok(mode)
    }

    fn process_is_elevated() -> Result<bool> {
        let mut token = ptr::null_mut();
        // SAFETY: the process pseudo-handle is valid and token points to initialized storage.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(installer_launch_error(io::Error::last_os_error()));
        }
        // SAFETY: OpenProcessToken returned an owned handle on success.
        let token = unsafe { OwnedHandle::from_raw_handle(token) };
        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0;
        // SAFETY: the token is live and the output buffer has the requested size.
        let result = unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                TokenElevation,
                (&mut elevation as *mut TOKEN_ELEVATION).cast(),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut returned,
            )
        };
        if result == 0 {
            return Err(installer_launch_error(io::Error::last_os_error()));
        }
        Ok(elevation.TokenIsElevated != 0)
    }

    pub(in crate::update) fn requires_elevation(target: &Path) -> Result<bool> {
        let mode = super::installer_mode(target)?;
        let directory = target
            .parent()
            .ok_or_else(|| anyhow!("Windows update target has no installation directory"))?;
        let elevated = mode == InstallerMode::AllUsers && process_is_elevated()?;
        elevation_required(
            mode,
            super::super::platform::is_dir_writable(directory)?,
            elevated,
        )
    }

    #[derive(Serialize, Deserialize)]
    struct WorkerSignal {
        complete: bool,
        may_still_run: bool,
        error: Option<String>,
    }

    fn write_signal(path: &Path, signal: &WorkerSignal) -> Result<()> {
        let temporary = path.with_extension("ready.tmp");
        fs::write(&temporary, serde_json::to_vec(signal)?)?;
        fs::rename(temporary, path)?;
        Ok(())
    }

    fn stop_child(child: &mut Child) -> Result<()> {
        if child.try_wait()?.is_some() {
            return Ok(());
        }
        child.kill()?;
        bounded_wait("Stopping Windows installer worker", STOP_TIMEOUT, || {
            Ok(child.try_wait()?)
        })?;
        Ok(())
    }

    fn worker_failure(ready_file: &Path, status: ExitStatus) -> anyhow::Error {
        if let Some(signal) = fs::read(ready_file)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<WorkerSignal>(&bytes).ok())
        {
            if signal.may_still_run || !signal.complete {
                return installer_still_running_error(signal.error.unwrap_or_else(|| {
                    format!("Windows installer worker exited unexpectedly with {status}.")
                }));
            }
            if let Some(message) = signal.error {
                return anyhow!(message);
            }
        }
        let mut message = String::new();
        if let Ok(stderr) = fs::File::open(ready_file.with_extension("stderr")) {
            let _ = stderr.take(65_536).read_to_string(&mut message);
        }
        installer_still_running_error(format!(
            "Windows installer worker exited with {status}: {} Run the installer manually.",
            message.trim()
        ))
    }

    fn run_elevation_worker(installer: &Path, target: &Path) -> Result<()> {
        let ready_file = installer
            .parent()
            .ok_or_else(|| anyhow!("Installer has no parent directory"))?
            .join(format!(
                ".windows-installer-{}.ready",
                uuid::Uuid::new_v4().simple()
            ));
        let mut child = Command::new(std::env::current_exe()?)
            .args(["__internal", "windows-installer-worker", "--installer"])
            .arg(installer)
            .arg("--target")
            .arg(target)
            .arg("--ready-file")
            .arg(&ready_file)
            .stdout(Stdio::null())
            .stderr(Stdio::from(fs::File::create(
                ready_file.with_extension("stderr"),
            )?))
            .spawn()
            .map_err(installer_launch_error)?;
        let result = (|| {
            let launched = bounded_wait("UAC elevation", LAUNCH_TIMEOUT, || {
                if ready_file.exists() {
                    return Ok(Some(()));
                }
                if let Some(status) = child.try_wait()? {
                    return Err(worker_failure(&ready_file, status));
                }
                Ok(None)
            });
            if let Err(error) = launched {
                let timed_out = error.downcast_ref::<WaitDeadline>().is_some();
                if let Err(stop_error) = stop_child(&mut child) {
                    return Err(installer_still_running_error(format!(
                        "{error:#} Failed to stop the elevation worker: {stop_error:#}"
                    )));
                }
                // A successful launch can race the ready-file write. Never restart over a possible installer.
                return if timed_out {
                    Err(installer_still_running_error(error.to_string()))
                } else {
                    Err(error)
                };
            }
            let status = bounded_wait(
                "Windows installer worker",
                INSTALL_TIMEOUT + STOP_TIMEOUT + STOP_TIMEOUT,
                || Ok(child.try_wait()?),
            );
            let status = match status {
                Ok(status) => status,
                Err(error) => {
                    let stop = stop_child(&mut child);
                    return Err(installer_still_running_error(format!(
                        "{error:#} Elevation worker stop result: {stop:?}"
                    )));
                }
            };
            if !status.success() {
                return Err(worker_failure(&ready_file, status));
            }
            let signal: WorkerSignal = serde_json::from_slice(&fs::read(&ready_file)?)?;
            if !signal.complete || signal.may_still_run {
                return Err(installer_still_running_error(
                    "Windows installer worker returned without a confirmed installer result.",
                ));
            }
            if let Some(error) = signal.error {
                return Err(anyhow!(error));
            }
            Ok(())
        })();
        let _ = fs::remove_file(&ready_file);
        let _ = fs::remove_file(ready_file.with_extension("ready.tmp"));
        let _ = fs::remove_file(ready_file.with_extension("stderr"));
        result
    }

    fn run_direct(installer: &Path, parameters: &[OsString]) -> Result<()> {
        let mut child = Command::new(installer)
            .args(parameters)
            .spawn()
            .map_err(installer_launch_error)?;
        let status = match bounded_wait("Windows setup installer", INSTALL_TIMEOUT, || {
            Ok(child.try_wait()?)
        }) {
            Ok(status) => status,
            Err(error) => {
                let stop = stop_child(&mut child);
                // Inno Setup can delegate to a second process. Stopping its launcher alone does
                // not prove that every installer process finished.
                return Err(installer_still_running_error(format!(
                    "{error:#} Installer launcher stop result: {stop:?}"
                )));
            }
        };
        setup_exit_result(status.code().unwrap_or(-1) as u32)
    }

    pub(in crate::update) fn run_windows_installer(installer: &Path, target: &Path) -> Result<()> {
        let mode = super::installer_mode(target)?;
        let parameters = installer_parameters(target, mode)?;
        if requires_elevation(target)? {
            run_elevation_worker(installer, target)
        } else {
            run_direct(installer, &parameters)
        }
    }

    struct ComApartment;
    impl Drop for ComApartment {
        fn drop(&mut self) {
            // SAFETY: this guard exists only after a successful CoInitializeEx on this thread.
            unsafe { CoUninitialize() };
        }
    }

    fn launch_elevated(installer: &Path, parameters: &[OsString]) -> Result<OwnedHandle> {
        // SAFETY: initialization is paired with the guard on the same thread.
        let result = unsafe {
            CoInitializeEx(
                ptr::null(),
                (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
            )
        };
        if result < 0 {
            return Err(anyhow!("Failed to initialize Windows elevation (HRESULT {result:#x}); run the installer manually"));
        }
        let _apartment = ComApartment;
        let verb = wide(std::ffi::OsStr::new("runas"));
        let file = wide(installer.as_os_str());
        let mut command_line = Vec::new();
        for argument in parameters {
            if !command_line.is_empty() {
                command_line.push(b' ' as u16);
            }
            command_line.extend(quote_windows_units(
                &argument.encode_wide().collect::<Vec<_>>(),
            ));
        }
        command_line.push(0);
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
            lpVerb: verb.as_ptr(),
            lpFile: file.as_ptr(),
            lpParameters: command_line.as_ptr(),
            nShow: SW_SHOWNORMAL,
            ..Default::default()
        };
        // SAFETY: every referenced nul-terminated buffer remains alive through the call.
        if unsafe { ShellExecuteExW(&mut info) } == 0 {
            return Err(installer_launch_error(io::Error::last_os_error()));
        }
        if info.hProcess.is_null() {
            return Err(installer_still_running_error(
                "Windows elevation returned no installer process handle.",
            ));
        }
        // SAFETY: SEE_MASK_NOCLOSEPROCESS transfers a live process handle to this caller.
        Ok(unsafe { OwnedHandle::from_raw_handle(info.hProcess) })
    }

    fn poll_native(handle: HANDLE) -> Result<Option<u32>> {
        // SAFETY: callers retain ownership of this live process handle throughout polling.
        match unsafe { WaitForSingleObject(handle, 0) } {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let mut exit_code = 0;
                // SAFETY: the process handle is signaled and exit_code is valid output storage.
                if unsafe { GetExitCodeProcess(handle, &mut exit_code) } == 0 {
                    return Err(io::Error::last_os_error().into());
                }
                Ok(Some(exit_code))
            }
            WAIT_FAILED => Err(io::Error::last_os_error().into()),
            value => Err(anyhow!("Unexpected installer wait result {value}")),
        }
    }

    fn stop_native(handle: HANDLE) -> Result<()> {
        if poll_native(handle)?.is_some() {
            return Ok(());
        }
        // SAFETY: this is exactly the installer process handle returned by this worker's launch.
        if unsafe { TerminateProcess(handle, 1) } == 0 {
            return Err(io::Error::last_os_error().into());
        }
        bounded_wait("Stopping Windows installer", STOP_TIMEOUT, || {
            poll_native(handle)
        })?;
        Ok(())
    }

    fn run_worker_inner(installer: &Path, target: &Path, ready_file: &Path) -> Result<()> {
        let mode = installer_mode(target)?;
        if mode != InstallerMode::AllUsers {
            return Err(anyhow!("A per-user installer must not request elevation; run the installer manually as the original user"));
        }
        let handle = launch_elevated(installer, &installer_parameters(target, mode)?)?;
        let started = WorkerSignal {
            complete: false,
            may_still_run: false,
            error: None,
        };
        if let Err(error) = write_signal(ready_file, &started) {
            return match stop_native(handle.as_raw_handle()) {
                Ok(()) => Err(error.context("Failed to acknowledge Windows installer launch")),
                Err(stop) => Err(installer_still_running_error(format!("Failed to acknowledge installer launch: {error:#}. Failed to stop installer: {stop:#}"))),
            };
        }
        let result = match bounded_wait("Windows setup installer", INSTALL_TIMEOUT, || {
            poll_native(handle.as_raw_handle())
        }) {
            Ok(code) => setup_exit_result(code),
            Err(error) => {
                let stop = stop_native(handle.as_raw_handle());
                Err(installer_still_running_error(format!(
                    "{error:#} Installer launcher stop result: {stop:?}"
                )))
            }
        };
        let signal = WorkerSignal {
            complete: true,
            may_still_run: result.as_ref().err().is_some_and(installer_may_still_run),
            error: result.as_ref().err().map(|error| format!("{error:#}")),
        };
        write_signal(ready_file, &signal).context("Failed to record Windows installer outcome")?;
        result
    }

    pub(crate) fn run_windows_installer_worker(
        installer: &Path,
        target: &Path,
        ready_file: &Path,
    ) -> Result<()> {
        let result = run_worker_inner(installer, target, ready_file);
        // Also publish pre-launch errors so an unexpected worker crash can remain fail-closed.
        if let Err(error) = &result {
            let signal = WorkerSignal {
                complete: true,
                may_still_run: installer_may_still_run(error),
                error: Some(format!("{error:#}")),
            };
            write_signal(ready_file, &signal)
                .context("Failed to record Windows installer failure")?;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsString, io, path::Path, time::Duration};

    #[test]
    fn registered_per_user_install_does_not_change_mode_in_protected_directory() {
        assert_eq!(
            select_installer_mode(true, false, true, false).unwrap(),
            InstallerMode::CurrentUser
        );
    }

    #[test]
    fn registered_all_users_install_stays_all_users_when_directory_is_writable() {
        assert_eq!(
            select_installer_mode(false, true, false, true).unwrap(),
            InstallerMode::AllUsers
        );
    }

    #[test]
    fn conflicting_registered_modes_fail_without_guessing() {
        let err = select_installer_mode(true, true, false, true).unwrap_err();
        assert!(err.to_string().contains("both per-user and all-users"));
    }

    #[test]
    fn program_files_install_remains_all_users_even_for_administrator() {
        assert_eq!(
            select_installer_mode(false, false, true, true).unwrap(),
            InstallerMode::AllUsers
        );
    }

    #[test]
    fn unregistered_writable_portable_install_uses_current_user() {
        assert_eq!(
            select_installer_mode(false, false, false, true).unwrap(),
            InstallerMode::CurrentUser
        );
    }

    #[test]
    fn unregistered_protected_custom_directory_fails_closed() {
        let err = select_installer_mode(false, false, false, false).unwrap_err();
        assert!(err.to_string().contains("Cannot determine"));
        assert!(err.to_string().contains("manually"));
    }

    #[test]
    fn per_user_update_never_requests_elevation() {
        assert!(!elevation_required(InstallerMode::CurrentUser, true, false).unwrap());
        let err = elevation_required(InstallerMode::CurrentUser, false, false).unwrap_err();
        assert!(err.to_string().contains("per-user"));
        assert!(err.to_string().contains("manually"));
    }

    #[test]
    fn all_users_update_requires_elevation_unless_already_elevated() {
        assert!(elevation_required(InstallerMode::AllUsers, true, false).unwrap());
        assert!(elevation_required(InstallerMode::AllUsers, false, false).unwrap());
        assert!(!elevation_required(InstallerMode::AllUsers, true, true).unwrap());
    }

    #[test]
    fn setup_parameters_pin_current_user_and_existing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir
            .path()
            .join("directory with spaces 日本語")
            .join("llmlb.exe");
        let args = installer_parameters(&target, InstallerMode::CurrentUser).unwrap();
        assert!(args.contains(&OsString::from("/CURRENTUSER")));
        assert!(!args.contains(&OsString::from("/ALLUSERS")));
        assert!(args.contains(&OsString::from("/NORESTART")));
        assert!(args.contains(&OsString::from("/VERYSILENT")));
        let mut directory = OsString::from("/DIR=");
        directory.push(target.parent().unwrap());
        assert!(args.contains(&directory));
    }

    #[test]
    fn setup_parameters_pin_all_users() {
        let dir = tempfile::tempdir().unwrap();
        let args =
            installer_parameters(&dir.path().join("llmlb.exe"), InstallerMode::AllUsers).unwrap();
        assert!(args.contains(&OsString::from("/ALLUSERS")));
        assert!(!args.contains(&OsString::from("/CURRENTUSER")));
    }

    #[test]
    fn setup_parameters_reject_missing_target_directory() {
        assert!(installer_parameters(Path::new("llmlb.exe"), InstallerMode::CurrentUser).is_err());
    }

    #[test]
    fn uac_decline_has_actionable_error() {
        let err = installer_launch_error(io::Error::from_raw_os_error(1223));
        let message = err.to_string();
        assert!(message.contains("declined"));
        assert!(message.contains("manually"));
        assert!(message.contains("1223"));
    }

    #[test]
    fn elevation_unavailable_has_actionable_error() {
        for code in [5, 740] {
            let message = installer_launch_error(io::Error::from_raw_os_error(code)).to_string();
            assert!(message.contains("Administrator privileges"));
            assert!(message.contains("manually"));
        }
    }

    #[test]
    fn setup_failure_includes_exit_code_and_manual_recovery() {
        assert!(setup_exit_result(0).is_ok());
        for code in [1, 2, 4, 5, 7, 8, 123] {
            let message = setup_exit_result(code).unwrap_err().to_string();
            assert!(message.contains(&format!("exit code {code}")));
            assert!(message.contains("manually"));
        }
    }

    #[test]
    fn completed_process_returns_without_waiting_for_deadline() {
        let result = bounded_wait("installer", Duration::ZERO, || Ok(Some(42))).unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn unanswered_uac_request_expires_with_recovery_guidance() {
        let message = bounded_wait("UAC elevation", Duration::ZERO, || Ok(None::<u32>))
            .unwrap_err()
            .to_string();
        assert!(message.contains("UAC elevation timed out"));
        assert!(message.contains("manually"));
    }

    #[test]
    fn process_wait_errors_are_not_retried_until_deadline() {
        let mut attempts = 0;
        let result = bounded_wait("installer", Duration::from_secs(300), || {
            attempts += 1;
            Err::<Option<u32>, _>(anyhow::anyhow!("lost process handle"))
        });
        assert_eq!(attempts, 1);
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("lost process handle"));
    }

    #[test]
    fn uncertain_installer_state_remains_visible_through_context() {
        let err = installer_still_running_error("process termination was denied")
            .context("installer update failed");
        assert!(installer_may_still_run(&err));
        assert!(!installer_may_still_run(&anyhow::anyhow!("UAC declined")));
    }

    #[test]
    fn windows_argument_quoting_preserves_spaces_quotes_and_trailing_slashes() {
        let quote = |value: &str| {
            String::from_utf16(&quote_windows_units(
                &value.encode_utf16().collect::<Vec<_>>(),
            ))
            .unwrap()
        };
        assert_eq!(quote("/CURRENTUSER"), "\"/CURRENTUSER\"");
        assert_eq!(
            quote("/DIR=C:\\Program Files\\日本語"),
            "\"/DIR=C:\\Program Files\\日本語\""
        );
        assert_eq!(quote("/DIR=C:\\"), "\"/DIR=C:\\\\\"");
        assert_eq!(quote("a\"b"), "\"a\\\"b\"");
    }
}
