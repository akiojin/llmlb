//! Self-update manager.
//!
//! This module implements:
//! - Update discovery via GitHub Releases
//! - Background download of the preferred payload for the current platform
//! - User-approved apply flow: drain inference requests, then restart into the new version
//! - Internal helper modes (`__internal`) to safely replace binaries / run installers

use crate::{inference_gate::InferenceGate, shutdown::ShutdownController};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{Notify, RwLock};

const DEFAULT_OWNER: &str = "akiojin";
const DEFAULT_REPO: &str = "llmlb";
const DEFAULT_TTL: Duration = Duration::from_secs(60 * 60 * 24);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UpdateCacheFile {
    last_checked_at: DateTime<Utc>,
    latest_version: Option<String>,
    release_url: Option<String>,
    portable_asset_url: Option<String>,
    installer_asset_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case")]
/// Current self-update state exposed to the dashboard/tray.
///
/// This state is intentionally serializable (snake_case) to make it easy to consume from the UI.
pub enum UpdateState {
    /// No update is available (or not yet checked).
    UpToDate {
        /// When the last update check completed (if known).
        checked_at: Option<DateTime<Utc>>,
    },
    /// A newer version is available on GitHub Releases.
    Available {
        /// Current running version.
        current: String,
        /// Latest available version.
        latest: String,
        /// Release page URL.
        release_url: String,
        /// Preferred portable payload URL for this platform, if present.
        portable_asset_url: Option<String>,
        /// Preferred installer payload URL for this platform, if present.
        installer_asset_url: Option<String>,
        /// Current payload download/preparation status.
        payload: PayloadState,
        /// When this update was last checked.
        checked_at: DateTime<Utc>,
    },
    /// Apply was requested; new inference requests are rejected while in-flight requests drain.
    Draining {
        /// Latest version being applied.
        latest: String,
        /// Current in-flight inference request count.
        in_flight: usize,
        /// When apply was requested.
        requested_at: DateTime<Utc>,
    },
    /// Update is being applied by an internal helper process.
    Applying {
        /// Latest version being applied.
        latest: String,
        /// Apply method chosen for this platform/install.
        method: ApplyMethod,
    },
    /// Update check/download/apply failed (best-effort; the server should keep running).
    Failed {
        /// Latest version (if known).
        latest: Option<String>,
        /// Release page URL (if known).
        release_url: Option<String>,
        /// Human-readable failure message.
        message: String,
        /// When the failure was recorded.
        failed_at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "payload", rename_all = "snake_case")]
/// Status of the update payload (portable archive or installer).
pub enum PayloadState {
    /// Payload is not downloaded/prepared yet.
    NotReady,
    /// Payload is being downloaded/extracted.
    Downloading {
        /// When the download/extraction started.
        started_at: DateTime<Utc>,
    },
    /// Payload is ready to apply.
    Ready {
        /// Prepared payload kind.
        kind: PayloadKind,
    },
    /// Payload download/extraction failed.
    Error {
        /// Human-readable error message.
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Prepared update payload kind.
pub enum PayloadKind {
    /// Portable archive extracted; `binary_path` points to the new executable.
    Portable {
        /// Path to the extracted new executable.
        binary_path: String,
    },
    /// Installer downloaded; `installer_path` points to the installer file.
    Installer {
        /// Path to the downloaded installer file.
        installer_path: String,
        /// Installer kind (OS-dependent).
        kind: InstallerKind,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Installer kind (OS-dependent).
pub enum InstallerKind {
    /// macOS `.pkg`.
    MacPkg,
    /// Windows `.msi`.
    WindowsMsi,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Apply method used for the current update.
pub enum ApplyMethod {
    /// Replace the running executable with the extracted portable binary.
    PortableReplace,
    /// Run a macOS `.pkg` installer.
    MacPkg,
    /// Run a Windows `.msi` installer.
    WindowsMsi,
}

#[derive(Clone)]
/// Self-update manager.
///
/// This component checks GitHub Releases in the background, prepares the preferred payload,
/// and applies the update after explicit user approval (drain inference requests, then restart).
pub struct UpdateManager {
    inner: Arc<UpdateManagerInner>,
}

struct UpdateManagerInner {
    started: AtomicBool,
    apply_requested: AtomicBool,
    apply_notify: Notify,

    current_version: Version,
    http_client: reqwest::Client,
    gate: InferenceGate,
    shutdown: ShutdownController,

    owner: String,
    repo: String,
    ttl: Duration,

    cache_path: PathBuf,
    updates_dir: PathBuf,

    state: RwLock<UpdateState>,

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    tray_proxy: RwLock<Option<crate::gui::tray::TrayEventProxy>>,
}

impl std::fmt::Debug for UpdateManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpdateManager").finish()
    }
}

impl UpdateManager {
    /// Create a new update manager for the current running version.
    ///
    /// This does not start background tasks; call [`UpdateManager::start_background_tasks`].
    pub fn new(
        http_client: reqwest::Client,
        gate: InferenceGate,
        shutdown: ShutdownController,
    ) -> Result<Self> {
        let current_version = Version::parse(env!("CARGO_PKG_VERSION"))
            .context("Failed to parse CARGO_PKG_VERSION as semver")?;

        let (cache_path, updates_dir) = default_paths()?;

        Ok(Self {
            inner: Arc::new(UpdateManagerInner {
                started: AtomicBool::new(false),
                apply_requested: AtomicBool::new(false),
                apply_notify: Notify::new(),
                current_version,
                http_client,
                gate,
                shutdown,
                owner: DEFAULT_OWNER.to_string(),
                repo: DEFAULT_REPO.to_string(),
                ttl: DEFAULT_TTL,
                cache_path,
                updates_dir,
                state: RwLock::new(UpdateState::UpToDate { checked_at: None }),
                #[cfg(any(target_os = "windows", target_os = "macos"))]
                tray_proxy: RwLock::new(None),
            }),
        })
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    /// Attach a tray event proxy to publish update state (best-effort).
    pub async fn set_tray_proxy(&self, proxy: crate::gui::tray::TrayEventProxy) {
        *self.inner.tray_proxy.write().await = Some(proxy);
    }

    /// Return the current update state snapshot.
    pub async fn state(&self) -> UpdateState {
        self.inner.state.read().await.clone()
    }

    /// Force an update check now (ignores TTL cache).
    ///
    /// Intended for the dashboard "Check for updates" button.
    pub async fn check_now(&self) -> Result<UpdateState> {
        match self.check_and_maybe_download(true).await {
            Ok(()) => Ok(self.state().await),
            Err(err) => {
                self.record_check_failure(err.to_string()).await;
                Err(err)
            }
        }
    }

    /// Return the current in-flight inference request count.
    pub async fn in_flight(&self) -> usize {
        self.inner.gate.in_flight()
    }

    /// Request applying the update as soon as it is safe.
    ///
    /// The background task will:
    /// - (Re)check GitHub Releases
    /// - Ensure the payload is downloaded/prepared
    /// - Start rejecting new inference requests and drain in-flight requests
    /// - Spawn an internal helper to apply the update, then request shutdown
    pub fn request_apply(&self) {
        self.inner.apply_requested.store(true, Ordering::SeqCst);
        self.inner.apply_notify.notify_waiters();
    }

    /// Start background update check loop and apply loop (idempotent).
    pub fn start_background_tasks(&self) {
        if self.inner.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let mgr = self.clone();
        tokio::spawn(async move {
            if let Err(e) = mgr.check_and_maybe_download(false).await {
                tracing::warn!("update check failed: {e}");
            }

            let mut interval = tokio::time::interval(Duration::from_secs(60 * 60));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            // The first tick completes immediately; consume it since we already checked on startup.
            interval.tick().await;

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = mgr.check_and_maybe_download(false).await {
                            tracing::warn!("update check failed: {e}");
                        }
                    }
                    _ = mgr.inner.apply_notify.notified() => {
                        let should_apply = mgr.inner.apply_requested.swap(false, Ordering::SeqCst);
                        if !should_apply {
                            continue;
                        }

                        // Refresh state before applying (e.g., first click immediately after boot, or retry after failure).
                        if let Err(e) = mgr.check_and_maybe_download(true).await {
                            tracing::warn!("update check failed before apply: {e}");

                            // If GitHub is temporarily unreachable, fall back to the cached state so we can still
                            // apply a previously discovered update.
                            let already_available = {
                                let st = mgr.inner.state.read().await;
                                matches!(&*st, UpdateState::Available { .. })
                            };
                            if !already_available {
                                if let Some(cache) =
                                    load_cache(&mgr.inner.cache_path).ok().flatten()
                                {
                                    if let Err(err) = mgr.apply_cache(cache).await {
                                        tracing::warn!(
                                            "update cache apply failed before apply: {err}"
                                        );
                                    }
                                }
                            }
                        }

                        let is_available = {
                            let st = mgr.inner.state.read().await;
                            matches!(&*st, UpdateState::Available { .. })
                        };
                        if !is_available {
                            continue;
                        }

                        if let Err(err) = mgr.apply_flow().await {
                            tracing::warn!("update apply failed: {err}");
                            mgr.inner.gate.stop_rejecting();
                            let mut st = mgr.inner.state.write().await;
                            let (latest, release_url) = match &*st {
                                UpdateState::Available { latest, release_url, .. } => {
                                    (Some(latest.clone()), Some(release_url.clone()))
                                }
                                UpdateState::Draining { latest, .. } => (Some(latest.clone()), None),
                                UpdateState::Applying { latest, .. } => (Some(latest.clone()), None),
                                _ => (None, None),
                            };
                            *st = UpdateState::Failed {
                                latest,
                                release_url,
                                message: err.to_string(),
                                failed_at: Utc::now(),
                            };
                            #[cfg(any(target_os = "windows", target_os = "macos"))]
                            notify_tray_failed(&mgr.inner.tray_proxy, err.to_string()).await;
                        }
                    }
                }
            }
        });
    }

    async fn check_and_maybe_download(&self, force: bool) -> Result<()> {
        if !force {
            if let Some(cache) = load_cache(&self.inner.cache_path).ok().flatten() {
                let age = Utc::now().signed_duration_since(cache.last_checked_at);
                if age.to_std().unwrap_or(Duration::MAX) < self.inner.ttl {
                    self.apply_cache(cache).await?;
                    #[cfg(any(target_os = "windows", target_os = "macos"))]
                    {
                        match &*self.inner.state.read().await {
                            UpdateState::Available { latest, .. } => {
                                notify_tray_available(&self.inner.tray_proxy, latest.clone()).await;
                            }
                            UpdateState::UpToDate { .. } => {
                                notify_tray_up_to_date(&self.inner.tray_proxy).await;
                            }
                            _ => {}
                        }
                    }
                    // Start download if update is available.
                    if matches!(
                        self.inner.state.read().await.clone(),
                        UpdateState::Available { .. }
                    ) {
                        let _ = self.ensure_payload_ready().await;
                    }
                    return Ok(());
                }
            }
        }

        let timeout = if force {
            Duration::from_secs(10)
        } else {
            Duration::from_secs(2)
        };
        let release = fetch_latest_release(
            &self.inner.http_client,
            &self.inner.owner,
            &self.inner.repo,
            timeout,
        )
        .await?;
        let latest = parse_tag_to_version(&release.tag_name)?;
        if latest <= self.inner.current_version {
            *self.inner.state.write().await = UpdateState::UpToDate {
                checked_at: Some(Utc::now()),
            };
            save_cache(
                &self.inner.cache_path,
                UpdateCacheFile {
                    last_checked_at: Utc::now(),
                    latest_version: Some(latest.to_string()),
                    release_url: Some(release.html_url.clone()),
                    portable_asset_url: None,
                    installer_asset_url: None,
                },
            )?;
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            notify_tray_up_to_date(&self.inner.tray_proxy).await;
            return Ok(());
        }

        let platform = Platform::detect()?;
        let (portable_asset, installer_asset) = select_assets(&release, &platform);

        let cache = UpdateCacheFile {
            last_checked_at: Utc::now(),
            latest_version: Some(latest.to_string()),
            release_url: Some(release.html_url.clone()),
            portable_asset_url: portable_asset
                .as_ref()
                .map(|a| a.browser_download_url.clone()),
            installer_asset_url: installer_asset
                .as_ref()
                .map(|a| a.browser_download_url.clone()),
        };
        save_cache(&self.inner.cache_path, cache.clone())?;

        let mut st = self.inner.state.write().await;
        *st = UpdateState::Available {
            current: self.inner.current_version.to_string(),
            latest: latest.to_string(),
            release_url: release.html_url,
            portable_asset_url: cache.portable_asset_url.clone(),
            installer_asset_url: cache.installer_asset_url.clone(),
            payload: PayloadState::NotReady,
            checked_at: cache.last_checked_at,
        };

        #[cfg(any(target_os = "windows", target_os = "macos"))]
        notify_tray_available(&self.inner.tray_proxy, latest.to_string()).await;

        // Background download (best-effort).
        let _ = self.ensure_payload_ready().await;
        Ok(())
    }

    async fn apply_cache(&self, cache: UpdateCacheFile) -> Result<()> {
        let latest_version = cache.latest_version.clone().unwrap_or_default();
        if latest_version.is_empty() {
            *self.inner.state.write().await = UpdateState::UpToDate {
                checked_at: Some(cache.last_checked_at),
            };
            return Ok(());
        }
        let latest = Version::parse(&latest_version).context("cached latest_version is invalid")?;
        if latest <= self.inner.current_version {
            *self.inner.state.write().await = UpdateState::UpToDate {
                checked_at: Some(cache.last_checked_at),
            };
            return Ok(());
        }
        let release_url = cache.release_url.clone().unwrap_or_else(|| {
            format!(
                "https://github.com/{}/{}/releases/latest",
                self.inner.owner, self.inner.repo
            )
        });
        *self.inner.state.write().await = UpdateState::Available {
            current: self.inner.current_version.to_string(),
            latest: latest.to_string(),
            release_url,
            portable_asset_url: cache.portable_asset_url.clone(),
            installer_asset_url: cache.installer_asset_url.clone(),
            payload: PayloadState::NotReady,
            checked_at: cache.last_checked_at,
        };
        Ok(())
    }

    async fn record_check_failure(&self, message: String) {
        let mut st = self.inner.state.write().await;
        // Keep an already discovered update actionable even if a subsequent
        // manual check temporarily fails (e.g., transient GitHub outage).
        if matches!(&*st, UpdateState::Available { .. }) {
            return;
        }

        let (latest, release_url) = match &*st {
            UpdateState::Draining { latest, .. } => (Some(latest.clone()), None),
            UpdateState::Applying { latest, .. } => (Some(latest.clone()), None),
            UpdateState::Failed {
                latest,
                release_url,
                ..
            } => (latest.clone(), release_url.clone()),
            _ => (None, None),
        };

        *st = UpdateState::Failed {
            latest,
            release_url,
            message: message.clone(),
            failed_at: Utc::now(),
        };

        #[cfg(any(target_os = "windows", target_os = "macos"))]
        notify_tray_failed(&self.inner.tray_proxy, message).await;
    }

    async fn ensure_payload_ready(&self) -> Result<PayloadKind> {
        let (latest, release_url, portable, installer) = {
            let st = self.inner.state.read().await;
            match &*st {
                UpdateState::Available {
                    latest,
                    release_url,
                    portable_asset_url,
                    installer_asset_url,
                    ..
                } => (
                    latest.clone(),
                    release_url.clone(),
                    portable_asset_url.clone(),
                    installer_asset_url.clone(),
                ),
                _ => return Err(anyhow!("No update is available")),
            }
        };

        {
            let mut st = self.inner.state.write().await;
            if let UpdateState::Available {
                payload: PayloadState::Ready { kind },
                ..
            } = &*st
            {
                return Ok(kind.clone());
            }
            if let UpdateState::Available { payload, .. } = &mut *st {
                *payload = PayloadState::Downloading {
                    started_at: Utc::now(),
                };
            }
        }

        let platform = Platform::detect()?;
        let current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("llmlb"));
        let plan = choose_apply_plan(
            &platform,
            &current_exe,
            portable.as_deref(),
            installer.as_deref(),
        );

        let Some(plan) = plan else {
            let dir = current_exe.parent().unwrap_or_else(|| Path::new("."));
            let writable = is_dir_writable(dir).unwrap_or(false);
            let msg = if !writable && installer.is_none() {
                format!(
                    "Automatic update is not supported because '{}' is not writable. Please reinstall from: {}",
                    dir.display(),
                    release_url
                )
            } else {
                format!(
                    "No suitable update asset found for this platform. Please download from: {}",
                    release_url
                )
            };
            self.set_payload_error(msg.clone()).await;
            return Err(anyhow!(msg));
        };

        let update_dir = self.inner.updates_dir.join(&latest);
        fs::create_dir_all(&update_dir).ok();

        let kind = match plan {
            ApplyPlan::Portable { url } => {
                let asset_name =
                    asset_name_from_url(&url).unwrap_or_else(|| "llmlb-update".to_string());
                let archive_path = update_dir.join(&asset_name);
                download_to_path(&self.inner.http_client, &url, &archive_path).await?;
                let extract_dir = update_dir.join("extract");
                if extract_dir.exists() {
                    fs::remove_dir_all(&extract_dir).ok();
                }
                fs::create_dir_all(&extract_dir)?;
                extract_archive(&archive_path, &extract_dir)?;
                let binary_name = platform.binary_name();
                let binary_path = find_extracted_binary(&extract_dir, &binary_name)?
                    .ok_or_else(|| anyhow!("Extracted archive did not contain {binary_name}"))?;
                PayloadKind::Portable {
                    binary_path: binary_path.to_string_lossy().to_string(),
                }
            }
            ApplyPlan::Installer { url, kind } => {
                let asset_name =
                    asset_name_from_url(&url).unwrap_or_else(|| "llmlb-installer".to_string());
                let installer_path = update_dir.join(&asset_name);
                download_to_path(&self.inner.http_client, &url, &installer_path).await?;
                PayloadKind::Installer {
                    installer_path: installer_path.to_string_lossy().to_string(),
                    kind,
                }
            }
        };

        {
            let mut st = self.inner.state.write().await;
            if let UpdateState::Available { payload, .. } = &mut *st {
                *payload = PayloadState::Ready { kind: kind.clone() };
            }
        }

        #[cfg(any(target_os = "windows", target_os = "macos"))]
        notify_tray_ready(&self.inner.tray_proxy).await;

        Ok(kind)
    }

    async fn set_payload_error(&self, msg: String) {
        let mut st = self.inner.state.write().await;
        if let UpdateState::Available { payload, .. } = &mut *st {
            *payload = PayloadState::Error { message: msg };
        }
    }

    async fn apply_flow(&self) -> Result<()> {
        let payload = self.ensure_payload_ready().await?;
        let latest = {
            let st = self.inner.state.read().await;
            match &*st {
                UpdateState::Available { latest, .. } => latest.clone(),
                _ => return Err(anyhow!("No update is available")),
            }
        };

        // Start draining after payload is ready to minimize downtime.
        self.inner.gate.start_rejecting();

        let requested_at = Utc::now();
        loop {
            let in_flight = self.inner.gate.in_flight();
            {
                *self.inner.state.write().await = UpdateState::Draining {
                    latest: latest.clone(),
                    in_flight,
                    requested_at,
                };
            }
            if in_flight == 0 {
                break;
            }
            self.inner.gate.wait_for_idle().await;
        }

        let current_exe =
            std::env::current_exe().context("Failed to resolve current executable path")?;
        let args_file = write_restart_args_file(&self.inner.updates_dir.join(&latest))?;

        match payload {
            PayloadKind::Portable { binary_path } => {
                *self.inner.state.write().await = UpdateState::Applying {
                    latest: latest.clone(),
                    method: ApplyMethod::PortableReplace,
                };
                spawn_internal_apply_update(&current_exe, &binary_path, &args_file)?;
                self.inner.shutdown.request_shutdown();
                Ok(())
            }
            PayloadKind::Installer {
                installer_path,
                kind,
            } => {
                let method = match kind {
                    InstallerKind::MacPkg => ApplyMethod::MacPkg,
                    InstallerKind::WindowsMsi => ApplyMethod::WindowsMsi,
                };
                *self.inner.state.write().await = UpdateState::Applying {
                    latest: latest.clone(),
                    method,
                };
                spawn_internal_run_installer(&current_exe, &installer_path, kind, &args_file)?;
                self.inner.shutdown.request_shutdown();
                Ok(())
            }
        }
    }
}

fn default_paths() -> Result<(PathBuf, PathBuf)> {
    let data_dir = if let Ok(dir) = std::env::var("LLMLB_DATA_DIR") {
        PathBuf::from(dir)
    } else {
        match std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
            Ok(home) => PathBuf::from(home).join(".llmlb"),
            // Some environments (systemd services, minimal containers) may not set HOME/USERPROFILE.
            // Self-update should not prevent startup, so use a best-effort temporary directory.
            Err(_) => std::env::temp_dir().join("llmlb"),
        }
    };
    Ok((data_dir.join("update-check.json"), data_dir.join("updates")))
}

fn load_cache(path: &Path) -> Result<Option<UpdateCacheFile>> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    let cache: UpdateCacheFile = serde_json::from_str(&content)?;
    Ok(Some(cache))
}

fn save_cache(path: &Path, cache: UpdateCacheFile) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(&cache)?)?;
    fs::rename(tmp, path)?;
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubReleaseResponse {
    tag_name: String,
    html_url: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GitHubAsset>,
}

async fn fetch_latest_release(
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
    timeout: Duration,
) -> Result<GitHubRelease> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let user_agent = format!("llmlb/{}", env!("CARGO_PKG_VERSION"));
    let res = client
        .get(url)
        .header("accept", "application/vnd.github+json")
        .header("user-agent", user_agent)
        .timeout(timeout)
        .send()
        .await
        .context("Failed to call GitHub Releases API")?;
    if !res.status().is_success() {
        return Err(anyhow!("GitHub API returned {}", res.status().as_u16()));
    }
    let parsed: GitHubReleaseResponse = res
        .json()
        .await
        .context("Failed to parse GitHub release JSON")?;
    Ok(GitHubRelease {
        tag_name: parsed.tag_name,
        html_url: parsed.html_url,
        assets: parsed.assets,
    })
}

fn parse_tag_to_version(tag: &str) -> Result<Version> {
    let normalized = tag.strip_prefix('v').unwrap_or(tag);
    Version::parse(normalized).map_err(|e| anyhow!("Invalid tag semver: {e}"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Platform {
    os: String,
    arch: String,
}

impl Platform {
    fn detect() -> Result<Self> {
        Ok(Self {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        })
    }

    fn artifact(&self) -> Option<&'static str> {
        match (self.os.as_str(), self.arch.as_str()) {
            ("linux", "x86_64") => Some("linux-x86_64"),
            ("linux", "aarch64") => Some("linux-arm64"),
            ("macos", "x86_64") => Some("macos-x86_64"),
            ("macos", "aarch64") => Some("macos-arm64"),
            ("windows", "x86_64") => Some("windows-x86_64"),
            _ => None,
        }
    }

    fn binary_name(&self) -> String {
        if self.os == "windows" {
            "llmlb.exe".to_string()
        } else {
            "llmlb".to_string()
        }
    }

    fn portable_asset_name(&self) -> Option<String> {
        self.artifact().map(|a| {
            if self.os == "windows" {
                format!("llmlb-{a}.zip")
            } else {
                format!("llmlb-{a}.tar.gz")
            }
        })
    }

    fn installer_asset_name(&self) -> Option<(String, InstallerKind)> {
        let artifact = self.artifact()?;
        match self.os.as_str() {
            "macos" => Some((format!("llmlb-{artifact}.pkg"), InstallerKind::MacPkg)),
            "windows" => Some((format!("llmlb-{artifact}.msi"), InstallerKind::WindowsMsi)),
            _ => None,
        }
    }
}

fn select_assets(
    release: &GitHubRelease,
    platform: &Platform,
) -> (Option<GitHubAsset>, Option<GitHubAsset>) {
    let portable_name = platform.portable_asset_name();
    let installer = platform.installer_asset_name();

    let portable_asset =
        portable_name.and_then(|name| release.assets.iter().find(|a| a.name == name).cloned());

    let installer_asset =
        installer.and_then(|(name, _kind)| release.assets.iter().find(|a| a.name == name).cloned());

    (portable_asset, installer_asset)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ApplyPlan {
    Portable { url: String },
    Installer { url: String, kind: InstallerKind },
}

fn choose_apply_plan(
    platform: &Platform,
    current_exe: &Path,
    portable_url: Option<&str>,
    installer_url: Option<&str>,
) -> Option<ApplyPlan> {
    let dir = current_exe.parent().unwrap_or_else(|| Path::new("."));
    let writable = is_dir_writable(dir).unwrap_or(false);

    // If we cannot replace the current executable in-place, prefer installer when available.
    if !writable {
        if let Some(url) = installer_url {
            let kind = platform.installer_asset_name().map(|(_, k)| k)?;
            return Some(ApplyPlan::Installer {
                url: url.to_string(),
                kind,
            });
        }
        // No installer available and we cannot replace in-place.
        return None;
    }

    if let Some(url) = portable_url {
        return Some(ApplyPlan::Portable {
            url: url.to_string(),
        });
    }

    if let Some(url) = installer_url {
        let kind = platform.installer_asset_name().map(|(_, k)| k)?;
        return Some(ApplyPlan::Installer {
            url: url.to_string(),
            kind,
        });
    }

    None
}

fn is_dir_writable(dir: &Path) -> Result<bool> {
    fs::create_dir_all(dir).ok();
    let probe = dir.join(".llmlb_write_probe");
    let result = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map(|_| true)
        .or_else(|e| {
            if matches!(e.kind(), io::ErrorKind::PermissionDenied) {
                Ok(false)
            } else {
                Err(e)
            }
        })?;
    if result {
        let _ = fs::remove_file(&probe);
    }
    Ok(result)
}

fn asset_name_from_url(url: &str) -> Option<String> {
    url.split('/').next_back().map(|s| s.to_string())
}

async fn download_to_path(client: &reqwest::Client, url: &str, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }
    let res = client
        .get(url)
        .timeout(Duration::from_secs(30))
        .send()
        .await?;
    if !res.status().is_success() {
        return Err(anyhow!("download failed with status {}", res.status()));
    }
    let bytes = res.bytes().await?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, &bytes)?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn extract_archive(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let name = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();

    if name.ends_with(".tar.gz") {
        let file = fs::File::open(archive_path)?;
        let decoder = GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        archive.unpack(dest_dir)?;
        return Ok(());
    }

    if name.ends_with(".zip") {
        let file = fs::File::open(archive_path)?;
        let mut zip = zip::ZipArchive::new(file)?;
        zip.extract(dest_dir)?;
        return Ok(());
    }

    Err(anyhow!("unsupported archive format: {name}"))
}

fn find_extracted_binary(extract_dir: &Path, binary_name: &str) -> Result<Option<PathBuf>> {
    // Expected layout: dist/llmlb-<artifact>/<binary>
    let mut candidates = Vec::<PathBuf>::new();
    for entry in fs::read_dir(extract_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            candidates.push(path.join(binary_name));
        } else if path.file_name().and_then(|n| n.to_str()) == Some(binary_name) {
            candidates.push(path);
        }
    }

    for c in candidates {
        if c.exists() {
            return Ok(Some(c));
        }
    }

    // Fallback: deep search.
    let mut stack = vec![extract_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)
            .unwrap_or_else(|_| fs::read_dir(extract_dir).unwrap())
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|n| n.to_str()) == Some(binary_name) {
                return Ok(Some(path));
            }
        }
    }

    Ok(None)
}

#[derive(Debug, Serialize, Deserialize)]
struct RestartArgsFile {
    args: Vec<String>,
    cwd: String,
}

fn write_restart_args_file(update_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(update_dir).ok();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cwd = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .to_string_lossy()
        .to_string();
    let payload = RestartArgsFile { args, cwd };
    let path = update_dir.join("restart_args.json");
    let tmp = update_dir.join("restart_args.json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(&payload)?)?;
    fs::rename(tmp, &path)?;
    Ok(path)
}

fn spawn_internal_apply_update(
    current_exe: &Path,
    new_binary_path: &str,
    args_file: &Path,
) -> Result<()> {
    let pid = std::process::id().to_string();
    let target = current_exe.to_string_lossy().to_string();
    Command::new(current_exe)
        .arg("__internal")
        .arg("apply-update")
        .arg("--old-pid")
        .arg(pid)
        .arg("--target")
        .arg(target)
        .arg("--new-binary")
        .arg(new_binary_path)
        .arg("--args-file")
        .arg(args_file)
        .spawn()
        .context("Failed to spawn internal apply-update")?;
    Ok(())
}

fn spawn_internal_run_installer(
    current_exe: &Path,
    installer_path: &str,
    kind: InstallerKind,
    args_file: &Path,
) -> Result<()> {
    let pid = std::process::id().to_string();
    let target = current_exe.to_string_lossy().to_string();

    // Windows: helper runs non-elevated and triggers UAC for msiexec itself.
    // Other platforms: best-effort (may fail due to missing privileges).
    Command::new(current_exe)
        .arg("__internal")
        .arg("run-installer")
        .arg("--old-pid")
        .arg(pid)
        .arg("--target")
        .arg(target)
        .arg("--installer")
        .arg(installer_path)
        .arg("--installer-kind")
        .arg(match kind {
            InstallerKind::MacPkg => "mac_pkg",
            InstallerKind::WindowsMsi => "windows_msi",
        })
        .arg("--args-file")
        .arg(args_file)
        .spawn()
        .context("Failed to spawn internal run-installer")?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn sh_single_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let escaped = s.replace('\'', "'\\''");
    format!("'{escaped}'")
}

#[cfg(target_os = "macos")]
fn escape_applescript_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn wait_for_pid_exit(pid: u32, timeout: Duration) -> Result<()> {
    let started = std::time::Instant::now();
    while crate::lock::is_process_running(pid) {
        if started.elapsed() > timeout {
            return Err(anyhow!("Timed out waiting for process {pid} to exit"));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

pub(crate) fn internal_apply_update(
    old_pid: u32,
    target: PathBuf,
    new_binary: PathBuf,
    args_file: PathBuf,
) -> Result<()> {
    wait_for_pid_exit(old_pid, Duration::from_secs(300))?;

    // Backup target (best-effort).
    let backup = target.with_extension("bak");
    if backup.exists() {
        let _ = fs::remove_file(&backup);
    }
    if target.exists() {
        let _ = fs::rename(&target, &backup);
    }

    if let Err(e) = fs::rename(&new_binary, &target) {
        // Cross-device rename fallback.
        if e.kind() == io::ErrorKind::CrossesDevices {
            fs::copy(&new_binary, &target)?;
            let _ = fs::remove_file(&new_binary);
        } else {
            return Err(e).context("Failed to replace target executable");
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&target)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&target, perms).ok();
    }

    restart_from_args_file(&target, &args_file)?;
    Ok(())
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(crate) fn internal_run_installer(
    old_pid: u32,
    target: PathBuf,
    installer: PathBuf,
    installer_kind: InstallerKind,
    args_file: PathBuf,
) -> Result<()> {
    wait_for_pid_exit(old_pid, Duration::from_secs(300))?;

    match installer_kind {
        InstallerKind::MacPkg => {
            #[cfg(target_os = "macos")]
            {
                run_macos_pkg_installer_with_privileges(&installer)?;
            }
            #[cfg(not(target_os = "macos"))]
            {
                return Err(anyhow!("mac_pkg installer can only run on macOS"));
            }
        }
        InstallerKind::WindowsMsi => {
            #[cfg(target_os = "windows")]
            {
                // Trigger UAC for msiexec via PowerShell.
                let msi = installer.to_string_lossy().to_string();
                let args = format!(
                    "Start-Process msiexec.exe -Verb RunAs -Wait -ArgumentList @('/i', '{}', '/passive')",
                    msi.replace('\'', "''")
                );
                let status = Command::new("powershell")
                    .arg("-NoProfile")
                    .arg("-Command")
                    .arg(args)
                    .status()
                    .context("Failed to run msiexec")?;
                if !status.success() {
                    return Err(anyhow!("msiexec exited with {}", status));
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                return Err(anyhow!("windows_msi installer can only run on Windows"));
            }
        }
    }

    restart_from_args_file(&target, &args_file)?;
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub(crate) fn internal_run_installer(
    _old_pid: u32,
    _target: PathBuf,
    _installer: PathBuf,
    _installer_kind: InstallerKind,
    _args_file: PathBuf,
) -> Result<()> {
    Err(anyhow!(
        "installer updates are only supported on macOS/Windows"
    ))
}

fn restart_from_args_file(target: &Path, args_file: &Path) -> Result<()> {
    let content = fs::read_to_string(args_file).context("Failed to read args-file")?;
    let parsed: RestartArgsFile =
        serde_json::from_str(&content).context("Invalid args-file JSON")?;

    let mut cmd = Command::new(target);
    cmd.args(parsed.args);
    if !parsed.cwd.is_empty() {
        cmd.current_dir(parsed.cwd);
    }
    cmd.spawn().context("Failed to spawn restarted process")?;
    Ok(())
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
async fn notify_tray_available(
    tray: &RwLock<Option<crate::gui::tray::TrayEventProxy>>,
    latest: String,
) {
    if let Some(proxy) = tray.read().await.clone() {
        proxy.notify_update_available(latest);
    }
}

#[cfg(target_os = "macos")]
fn run_macos_pkg_installer_with_privileges(installer: &Path) -> Result<()> {
    let installer_path = installer.to_string_lossy().to_string();
    // Run /usr/sbin/installer as admin via AppleScript. Keep this helper process non-root so restart
    // happens under the invoking user account.
    let shell_cmd = format!(
        "/usr/sbin/installer -pkg {} -target /",
        sh_single_quote(&installer_path)
    );
    let applescript_cmd = format!(
        "do shell script \"{}\" with administrator privileges",
        escape_applescript_string(&shell_cmd)
    );
    let status = Command::new("osascript")
        .arg("-e")
        .arg(applescript_cmd)
        .status()
        .context("Failed to run macOS installer via osascript")?;
    if !status.success() {
        return Err(anyhow!("osascript installer exited with {}", status));
    }
    Ok(())
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
async fn notify_tray_ready(tray: &RwLock<Option<crate::gui::tray::TrayEventProxy>>) {
    if let Some(proxy) = tray.read().await.clone() {
        proxy.notify_update_ready();
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
async fn notify_tray_failed(
    tray: &RwLock<Option<crate::gui::tray::TrayEventProxy>>,
    message: String,
) {
    if let Some(proxy) = tray.read().await.clone() {
        proxy.notify_update_failed(message);
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
async fn notify_tray_up_to_date(tray: &RwLock<Option<crate::gui::tray::TrayEventProxy>>) {
    if let Some(proxy) = tray.read().await.clone() {
        proxy.notify_update_up_to_date();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_asset_names() {
        let p = Platform {
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
        };
        assert_eq!(
            p.portable_asset_name(),
            Some("llmlb-linux-x86_64.tar.gz".to_string())
        );
        assert_eq!(p.installer_asset_name(), None);

        let p = Platform {
            os: "windows".to_string(),
            arch: "x86_64".to_string(),
        };
        assert_eq!(
            p.portable_asset_name(),
            Some("llmlb-windows-x86_64.zip".to_string())
        );
        assert_eq!(
            p.installer_asset_name(),
            Some((
                "llmlb-windows-x86_64.msi".to_string(),
                InstallerKind::WindowsMsi
            ))
        );
    }

    #[test]
    fn test_parse_tag_to_version() {
        assert_eq!(
            parse_tag_to_version("v3.1.0").unwrap(),
            Version::parse("3.1.0").unwrap()
        );
        assert_eq!(
            parse_tag_to_version("3.1.0").unwrap(),
            Version::parse("3.1.0").unwrap()
        );
    }

    #[tokio::test]
    async fn record_check_failure_preserves_available_payload() {
        let manager = UpdateManager::new(
            reqwest::Client::new(),
            InferenceGate::default(),
            ShutdownController::default(),
        )
        .expect("create update manager");

        let ready_payload = PayloadState::Ready {
            kind: PayloadKind::Portable {
                binary_path: "/tmp/llmlb-new".to_string(),
            },
        };

        {
            *manager.inner.state.write().await = UpdateState::Available {
                current: "4.5.0".to_string(),
                latest: "4.5.1".to_string(),
                release_url: "https://example.com/release".to_string(),
                portable_asset_url: Some("https://example.com/portable.tar.gz".to_string()),
                installer_asset_url: None,
                payload: ready_payload.clone(),
                checked_at: Utc::now(),
            };
        }

        manager
            .record_check_failure("temporary network outage".to_string())
            .await;

        match manager.state().await {
            UpdateState::Available {
                latest, payload, ..
            } => {
                assert_eq!(latest, "4.5.1");
                assert_eq!(payload, ready_payload);
            }
            other => panic!("expected available state, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn record_check_failure_transitions_non_available_to_failed() {
        let manager = UpdateManager::new(
            reqwest::Client::new(),
            InferenceGate::default(),
            ShutdownController::default(),
        )
        .expect("create update manager");

        {
            *manager.inner.state.write().await = UpdateState::UpToDate { checked_at: None };
        }

        manager
            .record_check_failure("check failed".to_string())
            .await;

        match manager.state().await {
            UpdateState::Failed {
                latest,
                release_url,
                message,
                ..
            } => {
                assert_eq!(latest, None);
                assert_eq!(release_url, None);
                assert_eq!(message, "check failed");
            }
            other => panic!("expected failed state, got {other:?}"),
        }
    }
}
