//! 更新の適用（drain→apply）実行状態機械。
//!
//! ペイロード準備完了を確認し、ドレイン待機を経て内部適用/インストーラ実行へ
//! 遷移させる。状態遷移とタイムアウト処理を担う。

use super::dto::*;
use super::helper::{
    spawn_internal_apply_update, spawn_internal_run_installer, write_restart_args_file,
};
use super::manager::{ApplyRequestMode, UpdateManager, DEFAULT_DRAIN_TIMEOUT_SECS};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use std::time::Duration;

impl UpdateManager {
    /// Persist failures that happen before a helper can record its own outcome.
    pub async fn record_apply_failure(&self, error: &anyhow::Error) {
        self.inner.gate.stop_rejecting();
        let message =
            format!("{error:#}. Check the server logs and retry, or install the release manually.");
        let version = {
            let mut state = self.inner.state.write().await;
            let (latest, release_url) = match &*state {
                UpdateState::Available {
                    latest,
                    release_url,
                    ..
                } => (Some(latest.clone()), Some(release_url.clone())),
                UpdateState::Draining { latest, .. } | UpdateState::Applying { latest, .. } => {
                    (Some(latest.clone()), None)
                }
                UpdateState::Failed {
                    latest,
                    release_url,
                    ..
                } => (latest.clone(), release_url.clone()),
                _ => (None, None),
            };
            let version = latest
                .clone()
                .unwrap_or_else(|| env!("CARGO_PKG_VERSION").into());
            *state = UpdateState::Failed {
                latest,
                release_url,
                message: message.clone(),
                failed_at: Utc::now(),
            };
            version
        };
        self.record_history(super::history::HistoryEntry {
            kind: super::history::HistoryEventKind::Failed,
            version: version.clone(),
            message: Some(message.clone()),
            timestamp: Utc::now(),
        });
        #[cfg(windows)]
        super::helper_log::phase(
            &self
                .inner
                .updates_dir
                .join(version)
                .join("restart_args.json"),
            &format!("failed_before_helper: {message}"),
        );
        self.notify_state_changed();
    }

    /// Fail before draining/shutdown when a remote browser cannot approve UAC.
    pub async fn validate_apply_environment(&self) -> Result<()> {
        #[cfg(windows)]
        {
            let payload = {
                let state = self.inner.state.read().await;
                match &*state {
                    UpdateState::Available {
                        payload: PayloadState::Ready { kind },
                        ..
                    } => kind.clone(),
                    _ => return Ok(()),
                }
            };
            let target =
                std::env::current_exe().context("Failed to resolve current executable path")?;
            super::installer_preflight::check(
                &payload,
                &target,
                super::windows_installer::requires_elevation,
            )?;
        }
        Ok(())
    }

    pub(super) async fn require_ready_payload(&self) -> Result<PayloadKind> {
        let st = self.inner.state.read().await;
        match &*st {
            UpdateState::Available {
                payload: PayloadState::Ready { kind },
                ..
            } => Ok(kind.clone()),
            UpdateState::Available { .. } => Err(anyhow!("Update payload is not ready")),
            UpdateState::Draining { .. } | UpdateState::Applying { .. } => {
                Err(anyhow!("Update is already in progress"))
            }
            _ => Err(anyhow!("No update is available")),
        }
    }

    pub(super) async fn set_applying_state(
        &self,
        latest: &str,
        method: ApplyMethod,
        phase: ApplyPhase,
        started_at: DateTime<Utc>,
        timeout_at: Option<DateTime<Utc>>,
    ) {
        *self.inner.state.write().await = UpdateState::Applying {
            latest: latest.to_string(),
            method,
            phase: phase.clone(),
            phase_message: phase.message().to_string(),
            started_at,
            timeout_at,
        };
        self.notify_state_changed();
    }

    pub(super) async fn apply_flow(&self, mode: ApplyRequestMode) -> Result<()> {
        let payload = match mode {
            ApplyRequestMode::Normal | ApplyRequestMode::InteractiveNormal => {
                self.ensure_payload_ready().await?
            }
            ApplyRequestMode::Force => self.require_ready_payload().await?,
            ApplyRequestMode::None => return Err(anyhow!("No apply request mode")),
        };
        if mode != ApplyRequestMode::InteractiveNormal {
            self.validate_apply_environment().await?;
        }
        let apply_method = match &payload {
            PayloadKind::Portable { .. } => ApplyMethod::PortableReplace,
            PayloadKind::Installer { kind, .. } => match kind {
                InstallerKind::MacPkg => ApplyMethod::MacPkg,
                InstallerKind::WindowsSetup => ApplyMethod::WindowsSetup,
            },
        };
        let latest = {
            let st = self.inner.state.read().await;
            match &*st {
                UpdateState::Available { latest, .. } => latest.clone(),
                _ => return Err(anyhow!("No update is available")),
            }
        };

        // Start draining after payload is ready to minimize downtime.
        self.inner.gate.start_rejecting();
        let applying_started_at = Utc::now();

        if mode == ApplyRequestMode::Force {
            // Mark as in-progress before waiting so UI/API cannot trigger duplicate apply actions.
            self.set_applying_state(
                &latest,
                apply_method.clone(),
                ApplyPhase::Starting,
                applying_started_at,
                None,
            )
            .await;
            // Force mode cancels active in-flight work instead of waiting for drain completion.
            self.inner.gate.abort_in_flight();
            if tokio::time::timeout(Duration::from_secs(3), self.inner.gate.wait_for_idle())
                .await
                .is_err()
            {
                tracing::warn!(
                    "force apply proceeding while in-flight requests are still unwinding"
                );
            }
        }

        if matches!(
            mode,
            ApplyRequestMode::Normal | ApplyRequestMode::InteractiveNormal
        ) {
            let requested_at = Utc::now();
            let drain_timeout = Duration::from_secs(DEFAULT_DRAIN_TIMEOUT_SECS);
            let timeout_at =
                requested_at + chrono::Duration::seconds(DEFAULT_DRAIN_TIMEOUT_SECS as i64);
            let deadline = tokio::time::Instant::now() + drain_timeout;

            loop {
                let in_flight = self.inner.gate.in_flight();
                if in_flight == 0 {
                    break;
                }
                {
                    *self.inner.state.write().await = UpdateState::Draining {
                        latest: latest.clone(),
                        in_flight,
                        requested_at,
                        timeout_at,
                    };
                    self.notify_state_changed();
                }
                if tokio::time::timeout_at(deadline, self.inner.gate.wait_for_idle())
                    .await
                    .is_err()
                {
                    // Drain timed out — cancel and restore normal operation.
                    tracing::warn!(
                        "drain timed out after {}s with {} in-flight requests",
                        DEFAULT_DRAIN_TIMEOUT_SECS,
                        self.inner.gate.in_flight()
                    );
                    self.inner.gate.stop_rejecting();
                    *self.inner.state.write().await = UpdateState::Failed {
                        latest: Some(latest.clone()),
                        release_url: None,
                        message: format!("Drain timed out after {}s", DEFAULT_DRAIN_TIMEOUT_SECS),
                        failed_at: Utc::now(),
                    };
                    self.notify_state_changed();
                    return Err(anyhow!(
                        "Drain timed out after {}s",
                        DEFAULT_DRAIN_TIMEOUT_SECS
                    ));
                }
            }
            self.set_applying_state(
                &latest,
                apply_method.clone(),
                ApplyPhase::Starting,
                applying_started_at,
                None,
            )
            .await;
        }

        let current_exe =
            std::env::current_exe().context("Failed to resolve current executable path")?;
        let args_file = write_restart_args_file(&self.inner.updates_dir.join(&latest))?;

        match payload {
            PayloadKind::Portable { binary_path } => {
                self.set_applying_state(
                    &latest,
                    apply_method.clone(),
                    ApplyPhase::Restarting,
                    applying_started_at,
                    None,
                )
                .await;
                spawn_internal_apply_update(&current_exe, &binary_path, &args_file)?;
                self.inner.shutdown.request_shutdown();
                Ok(())
            }
            PayloadKind::Installer {
                installer_path,
                kind,
            } => {
                self.set_applying_state(
                    &latest,
                    apply_method.clone(),
                    ApplyPhase::RunningInstaller,
                    applying_started_at,
                    None,
                )
                .await;
                spawn_internal_run_installer(&current_exe, &installer_path, kind, &args_file)?;
                self.inner.shutdown.request_shutdown();
                Ok(())
            }
        }
    }
}
