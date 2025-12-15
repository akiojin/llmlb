//! Model download & conversion job manager
//!
//! Downloads models from Hugging Face and (if needed) exports them to ONNX.
//! Jobs are processed asynchronously in the background and progress can be queried via API.

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, Mutex};
use tokio::task;
use uuid::Uuid;

use crate::registry::models::{model_name_to_dir, router_models_dir, ModelInfo, ModelSource};
use llm_router_common::error::RouterError;

// ===== venv Auto-Setup =====

/// venvのセットアップ状態
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VenvStatus {
    /// 未チェック
    Unknown,
    /// セットアップ中
    Setting,
    /// 準備完了
    Ready,
    /// セットアップ失敗
    Failed(String),
}

static VENV_STATUS: Lazy<RwLock<VenvStatus>> = Lazy::new(|| RwLock::new(VenvStatus::Unknown));

/// venvディレクトリのパスを取得
pub fn get_venv_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(|h| PathBuf::from(h).join(".llm-router").join("venv"))
}

/// venv内のPython実行ファイルのパスを取得
pub fn get_venv_python() -> Option<PathBuf> {
    // 環境変数でオーバーライド可能
    if let Ok(custom) = std::env::var("LLM_CONVERT_PYTHON") {
        return Some(PathBuf::from(custom));
    }

    get_venv_dir().map(|venv| {
        if cfg!(windows) {
            venv.join("Scripts").join("python.exe")
        } else {
            venv.join("bin").join("python3")
        }
    })
}

/// requirementsファイルのパスを取得
fn find_requirements_file() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("scripts/requirements-export-hf-to-onnx.txt"),
        PathBuf::from("../scripts/requirements-export-hf-to-onnx.txt"),
    ];

    candidates.into_iter().find(|candidate| candidate.exists())
}

/// venv環境をセットアップ（同期実行）
pub fn setup_venv() -> Result<(), RouterError> {
    // スキップフラグチェック
    if std::env::var("LLM_CONVERT_SKIP_VENV").is_ok() {
        tracing::info!("Skipping venv setup (LLM_CONVERT_SKIP_VENV set)");
        *VENV_STATUS.write().unwrap() = VenvStatus::Ready;
        return Ok(());
    }

    // カスタムPythonが指定されている場合はスキップ
    if std::env::var("LLM_CONVERT_PYTHON").is_ok() {
        tracing::info!("Using custom Python (LLM_CONVERT_PYTHON set)");
        *VENV_STATUS.write().unwrap() = VenvStatus::Ready;
        return Ok(());
    }

    let venv_dir =
        get_venv_dir().ok_or_else(|| RouterError::Internal("HOME directory not set".into()))?;

    let venv_python = get_venv_python()
        .ok_or_else(|| RouterError::Internal("Cannot determine venv python path".into()))?;

    // 既にvenvが存在し、pythonが実行可能なら完了
    if venv_python.exists() {
        let check = Command::new(&venv_python).arg("--version").output();
        if let Ok(output) = check {
            if output.status.success() {
                tracing::info!("venv already exists at {:?}", venv_dir);
                *VENV_STATUS.write().unwrap() = VenvStatus::Ready;
                return Ok(());
            }
        }
    }

    {
        let mut status = VENV_STATUS.write().unwrap();
        *status = VenvStatus::Setting;
    }

    tracing::info!("Creating venv at {:?}", venv_dir);

    // venvディレクトリを作成
    if let Some(parent) = venv_dir.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| RouterError::Internal(format!("Failed to create dir: {}", e)))?;
    }

    // python3 -m venv を実行
    let venv_output = Command::new("python3")
        .args(["-m", "venv", &venv_dir.to_string_lossy()])
        .output()
        .map_err(|e| RouterError::Internal(format!("Failed to create venv: {}", e)))?;

    if !venv_output.status.success() {
        let err = String::from_utf8_lossy(&venv_output.stderr);
        let msg = format!("venv creation failed: {}", err);
        *VENV_STATUS.write().unwrap() = VenvStatus::Failed(msg.clone());
        return Err(RouterError::Internal(msg));
    }

    // requirements.txtがあればインストール
    if let Some(requirements) = find_requirements_file() {
        tracing::info!("Installing dependencies from {:?}", requirements);

        let pip_output = Command::new(&venv_python)
            .args([
                "-m",
                "pip",
                "install",
                "-q",
                "-r",
                &requirements.to_string_lossy(),
            ])
            .output()
            .map_err(|e| RouterError::Internal(format!("Failed to run pip: {}", e)))?;

        if !pip_output.status.success() {
            let err = String::from_utf8_lossy(&pip_output.stderr);
            let msg = format!("pip install failed: {}", err);
            *VENV_STATUS.write().unwrap() = VenvStatus::Failed(msg.clone());
            return Err(RouterError::Internal(msg));
        }
    } else {
        tracing::warn!("requirements file not found, skipping pip install");
    }

    tracing::info!("venv setup completed at {:?}", venv_dir);
    *VENV_STATUS.write().unwrap() = VenvStatus::Ready;
    Ok(())
}

/// venvが準備完了かどうか
pub fn is_venv_ready() -> bool {
    matches!(*VENV_STATUS.read().unwrap(), VenvStatus::Ready)
}

/// venvステータスを取得
pub fn get_venv_status() -> VenvStatus {
    VENV_STATUS.read().unwrap().clone()
}

/// ジョブ状態
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConvertStatus {
    /// キュー待ち
    Queued,
    /// 実行中
    InProgress,
    /// 正常終了
    Completed,
    /// 失敗
    Failed,
}

/// 変換ジョブ
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertTask {
    /// タスクID
    pub id: Uuid,
    /// HFリポジトリ
    pub repo: String,
    /// 対象ファイル名
    pub filename: String,
    /// リビジョン（任意）
    pub revision: Option<String>,
    /// 量子化指定（未使用）
    pub quantization: Option<String>,
    /// chat_template
    pub chat_template: Option<String>,
    /// ステータス
    pub status: ConvertStatus,
    /// 進捗 (0-1)
    pub progress: f32,
    /// エラーメッセージ
    pub error: Option<String>,
    /// 出力パス
    pub path: Option<String>,
    /// 作成時刻
    pub created_at: DateTime<Utc>,
    /// 更新時刻
    pub updated_at: DateTime<Utc>,
}

impl ConvertTask {
    fn new(
        repo: String,
        filename: String,
        revision: Option<String>,
        quantization: Option<String>,
        chat_template: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            repo,
            filename,
            revision,
            quantization,
            chat_template,
            status: ConvertStatus::Queued,
            progress: 0.0,
            error: None,
            path: None,
            created_at: now,
            updated_at: now,
        }
    }
}

/// 変換タスクマネージャー
#[derive(Clone)]
pub struct ConvertTaskManager {
    tasks: Arc<Mutex<HashMap<Uuid, ConvertTask>>>,
    queue_tx: mpsc::Sender<Uuid>,
}

impl ConvertTaskManager {
    /// 新しいマネージャーを生成し、ワーカーを起動
    pub fn new(_concurrency: usize) -> Self {
        let (tx, mut rx) = mpsc::channel::<Uuid>(128);
        let tasks = Arc::new(Mutex::new(HashMap::new()));
        let tasks_clone = tasks.clone();

        tokio::spawn(async move {
            while let Some(task_id) = rx.recv().await {
                if let Err(e) = Self::process_task(tasks_clone.clone(), task_id).await {
                    tracing::error!(task_id=?task_id, error=?e, "convert_task_failed");
                }
            }
        });

        Self {
            tasks,
            queue_tx: tx,
        }
    }

    /// ジョブ作成しキュー投入
    pub async fn enqueue(
        &self,
        repo: String,
        filename: String,
        revision: Option<String>,
        quantization: Option<String>,
        chat_template: Option<String>,
    ) -> ConvertTask {
        let task = ConvertTask::new(
            repo.clone(),
            filename.clone(),
            revision,
            quantization,
            chat_template,
        );
        let id = task.id;
        tracing::info!(task_id=?id, repo=%repo, filename=%filename, "convert_task_enqueued");
        {
            let mut guard = self.tasks.lock().await;
            guard.insert(id, task);
        }
        tracing::info!(task_id=?id, "convert_task_sending_to_queue");
        if let Err(e) = self.queue_tx.send(id).await {
            tracing::error!("Failed to enqueue convert task {}: {}", id, e);
            // タスクをFailed状態に更新
            if let Some(task) = self.tasks.lock().await.get_mut(&id) {
                task.status = ConvertStatus::Failed;
                task.error = Some(format!("Failed to enqueue: {}", e));
                task.updated_at = Utc::now();
            }
        }
        self.tasks.lock().await.get(&id).cloned().unwrap()
    }

    /// ジョブ一覧を取得（更新時刻降順）
    pub async fn list(&self) -> Vec<ConvertTask> {
        let guard = self.tasks.lock().await;
        let mut list: Vec<_> = guard.values().cloned().collect();
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        list
    }

    /// 単一ジョブを取得
    pub async fn get(&self, id: Uuid) -> Option<ConvertTask> {
        self.tasks.lock().await.get(&id).cloned()
    }

    /// ジョブを削除
    pub async fn delete(&self, id: Uuid) -> bool {
        self.tasks.lock().await.remove(&id).is_some()
    }

    /// 指定されたリポジトリのタスクが存在するかチェック（失敗状態を除く）
    pub async fn has_task_for_repo(&self, repo: &str) -> bool {
        let guard = self.tasks.lock().await;
        guard
            .values()
            .any(|task| task.repo == repo && task.status != ConvertStatus::Failed)
    }

    async fn process_task(
        tasks: Arc<Mutex<HashMap<Uuid, ConvertTask>>>,
        task_id: Uuid,
    ) -> Result<(), RouterError> {
        tracing::info!(task_id=?task_id, "convert_task_started");
        let (repo, filename, revision, quantization, chat_template) = {
            let mut guard = tasks.lock().await;
            let task = guard
                .get_mut(&task_id)
                .ok_or_else(|| RouterError::Internal("Task not found".into()))?;
            task.status = ConvertStatus::InProgress;
            task.updated_at = Utc::now();
            tracing::info!(task_id=?task_id, repo=%task.repo, "convert_task_in_progress");
            (
                task.repo.clone(),
                task.filename.clone(),
                task.revision.clone(),
                task.quantization.clone(),
                task.chat_template.clone(),
            )
        };

        // Create progress callback that updates the task
        // Get runtime handle here so it can be used inside spawn_blocking
        let runtime_handle = tokio::runtime::Handle::current();
        let tasks_for_callback = tasks.clone();
        let progress_callback = move |progress: f32| {
            // Use runtime handle to spawn async task from sync context
            let tasks = tasks_for_callback.clone();
            let task_id = task_id;
            runtime_handle.spawn(async move {
                if let Some(task) = tasks.lock().await.get_mut(&task_id) {
                    task.progress = progress.clamp(0.0, 1.0);
                    task.updated_at = Utc::now();
                    tracing::debug!(task_id=?task_id, progress=progress, "convert_progress_updated");
                }
            });
        };

        // execute download/convert with progress callback
        let res = download_and_maybe_convert(
            &repo,
            &filename,
            revision.as_deref(),
            quantization.as_deref(),
            chat_template.clone(),
            progress_callback,
        )
        .await;

        let mut guard = tasks.lock().await;
        let task = guard
            .get_mut(&task_id)
            .ok_or_else(|| RouterError::Internal("Task not found".into()))?;
        match res {
            Ok(path) => {
                task.status = ConvertStatus::Completed;
                task.progress = 1.0;
                task.path = Some(path);
                task.error = None;
            }
            Err(err) => {
                task.status = ConvertStatus::Failed;
                task.error = Some(err.to_string());
            }
        }
        task.updated_at = Utc::now();
        Ok(())
    }
}

/// ダウンロードして必要なら変換する。
/// progress_callback: プログレス更新用のコールバック（0.0〜1.0）
async fn download_and_maybe_convert<F>(
    repo: &str,
    filename: &str,
    revision: Option<&str>,
    _quantization: Option<&str>,
    chat_template: Option<String>,
    progress_callback: F,
) -> Result<String, RouterError>
where
    F: Fn(f32) + Send + Sync + Clone + 'static,
{
    let is_onnx = filename.to_ascii_lowercase().ends_with(".onnx");
    // モデル名 = リポジトリ名（例: openai/gpt-oss-20b）
    let model_name = repo.to_string();
    let base_url = std::env::var("HF_BASE_URL")
        .unwrap_or_else(|_| "https://huggingface.co".to_string())
        .trim_end_matches('/')
        .to_string();
    let url = if filename.is_empty() {
        String::new()
    } else {
        format!(
            "{}/{}/resolve/{}/{}",
            base_url,
            repo,
            revision.unwrap_or("main"),
            filename
        )
    };

    let base = router_models_dir().ok_or_else(|| RouterError::Internal("HOME not set".into()))?;
    let dir = base.join(model_name_to_dir(&model_name));
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| RouterError::Internal(e.to_string()))?;
    let target = dir.join("model.onnx");

    // skip if already present but make sure metadata is up-to-date
    if target.exists() {
        progress_callback(1.0);
        finalize_model_registration(
            &model_name,
            repo,
            filename,
            if url.is_empty() {
                None
            } else {
                Some(url.as_str())
            },
            &target,
            chat_template.clone(),
        )
        .await;
        if let Err(e) = ensure_manifest(&dir, &model_name).await {
            tracing::warn!(error=%e, model=%model_name, "failed_to_write_manifest");
        }
        return Ok(target.to_string_lossy().to_string());
    }

    if is_onnx {
        if url.is_empty() {
            return Err(RouterError::Internal("ONNX filename is empty".into()));
        }
        download_file(&url, &target).await?;
        progress_callback(1.0);
    } else {
        export_non_onnx(repo, revision, &target, progress_callback.clone()).await?;
    }

    if let Err(e) = ensure_manifest(&dir, &model_name).await {
        tracing::warn!(error=%e, model=%model_name, "failed_to_write_manifest");
    }

    finalize_model_registration(
        &model_name,
        repo,
        filename,
        if url.is_empty() {
            None
        } else {
            Some(url.as_str())
        },
        &target,
        chat_template,
    )
    .await;

    Ok(target.to_string_lossy().to_string())
}

/// HTTP GET to file and stream to path
async fn download_file(url: &str, target: &Path) -> Result<(), RouterError> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| RouterError::Http(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(RouterError::Http(resp.status().to_string()));
    }
    let mut file = tokio::fs::File::create(target)
        .await
        .map_err(|e| RouterError::Internal(e.to_string()))?;
    let mut stream = resp.bytes_stream();
    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| RouterError::Http(e.to_string()))?;
        file.write_all(&bytes)
            .await
            .map_err(|e| RouterError::Internal(e.to_string()))?;
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct ModelManifest {
    model: String,
    files: Vec<ModelManifestFile>,
}

#[derive(Debug, Serialize)]
struct ModelManifestFile {
    name: String,
    digest: String,
}

async fn ensure_manifest(model_dir: &Path, model_name: &str) -> Result<(), RouterError> {
    let dir = model_dir.to_path_buf();
    let model = model_name.to_string();
    task::spawn_blocking(move || write_manifest_sync(&dir, &model))
        .await
        .map_err(|e| RouterError::Internal(e.to_string()))?
}

fn write_manifest_sync(model_dir: &Path, model_name: &str) -> Result<(), RouterError> {
    let manifest_path = model_dir.join("manifest.json");

    let mut rel_files: Vec<PathBuf> = Vec::new();
    collect_files_recursive(model_dir, model_dir, &mut rel_files)
        .map_err(|e| RouterError::Internal(e.to_string()))?;

    rel_files.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));

    let mut files: Vec<ModelManifestFile> = Vec::with_capacity(rel_files.len());
    for rel in rel_files {
        let full = model_dir.join(&rel);
        let digest = sha256_hex_of_file(&full).map_err(|e| RouterError::Internal(e.to_string()))?;
        files.push(ModelManifestFile {
            name: rel.to_string_lossy().to_string(),
            digest,
        });
    }

    let manifest = ModelManifest {
        model: model_name.to_string(),
        files,
    };

    let temp_path = manifest_path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| RouterError::Internal(format!("Failed to serialize manifest: {}", e)))?;
    std::fs::write(&temp_path, json)
        .map_err(|e| RouterError::Internal(format!("Failed to write manifest: {}", e)))?;
    std::fs::rename(&temp_path, &manifest_path)
        .map_err(|e| RouterError::Internal(format!("Failed to finalize manifest: {}", e)))?;

    Ok(())
}

fn collect_files_recursive(
    base_dir: &Path,
    dir: &Path,
    out: &mut Vec<PathBuf>,
) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(base_dir, &path, out)?;
            continue;
        }
        if path.file_name().and_then(|n| n.to_str()) == Some("manifest.json") {
            continue;
        }
        let rel = path.strip_prefix(base_dir).unwrap_or(&path).to_path_buf();
        out.push(rel);
    }
    Ok(())
}

fn sha256_hex_of_file(path: &Path) -> Result<String, std::io::Error> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 8 * 1024 * 1024];
    loop {
        let n = std::io::Read::read(&mut file, &mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let digest = hasher.finalize();
    Ok(hex_lower(&digest))
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(&mut out, "{:02x}", b);
    }
    out
}

/// 非ONNXをONNXへエクスポート（sync heavy → blocking thread）
/// progress_callback: プログレス更新用のコールバック（0.0〜1.0）
async fn export_non_onnx<F>(
    repo: &str,
    revision: Option<&str>,
    target: &Path,
    progress_callback: F,
) -> Result<(), RouterError>
where
    F: Fn(f32) + Send + Sync + 'static,
{
    if should_use_fake_convert() {
        progress_callback(0.5);
        write_dummy_onnx(target).await?;
        progress_callback(1.0);
        return Ok(());
    }

    let script = locate_convert_script()
        .ok_or_else(|| RouterError::Internal("export_hf_to_onnx.py not found".into()))?;
    // デフォルトスクリプトを使う場合のみ依存チェックを行う。カスタムスクリプトは自己完結を想定。
    if script
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.contains("export_hf_to_onnx.py"))
        .unwrap_or(false)
    {
        ensure_python_deps().await?;
    }
    let python_bin = get_venv_python()
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "python3".into());
    let hf_token = std::env::var("HF_TOKEN").ok();

    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| RouterError::Internal(e.to_string()))?;

        // Best-effort cleanup of previous ONNX artifacts to avoid stale external data.
        let _ = tokio::fs::remove_file(parent.join("manifest.json")).await;
        if let Ok(mut rd) = tokio::fs::read_dir(parent).await {
            while let Ok(Some(entry)) = rd.next_entry().await {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if name == "model.onnx" || name.starts_with("model.onnx.") {
                    let _ = tokio::fs::remove_file(&path).await;
                }
            }
        }
    }
    let _ = tokio::fs::remove_file(target).await;

    let repo_with_rev = if let Some(rev) = revision {
        format!("{}@{}", repo, rev)
    } else {
        repo.to_string()
    };

    let script_clone = script.clone();
    let target_path = target.to_path_buf();
    let cmd_repo = repo_with_rev.clone();
    let python_bin_clone = python_bin.clone();

    // Use spawn() with piped stderr to capture progress output
    let result = task::spawn_blocking(move || {
        let mut cmd = Command::new(&python_bin_clone);
        cmd.arg(&script_clone)
            .arg("--remote")
            .arg("--outfile")
            .arg(&target_path)
            .arg(&cmd_repo)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Force unbuffered output from Python so tqdm progress lines are flushed immediately
            .env("PYTHONUNBUFFERED", "1");

        if let Some(token) = hf_token {
            cmd.env("HF_TOKEN", token);
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return Err(format!("Failed to spawn convert process: {}", e)),
        };

        // tqdm outputs to stderr, so we read from there
        let stderr = child.stderr.take();
        let stdout = child.stdout.take();

        // Collect stderr output while parsing progress
        // tqdm uses \r (carriage return) to update progress lines, not \n
        // So we need to read byte-by-byte and split on either \r or \n
        let mut stderr_output = String::new();
        if let Some(stderr_reader) = stderr {
            let mut reader = BufReader::new(stderr_reader);
            let mut line_buffer = String::new();
            let mut byte = [0u8; 1];

            while reader.read(&mut byte).unwrap_or(0) > 0 {
                let ch = byte[0] as char;
                if ch == '\r' || ch == '\n' {
                    if !line_buffer.is_empty() {
                        tracing::debug!(line=%line_buffer, "convert_stderr_line");
                        // Try to parse progress from tqdm output
                        if let Some(progress) = parse_tqdm_progress(&line_buffer) {
                            tracing::debug!(progress, "convert_progress_parsed");
                            progress_callback(progress);
                        }
                        stderr_output.push_str(&line_buffer);
                        stderr_output.push('\n');
                        line_buffer.clear();
                    }
                } else {
                    line_buffer.push(ch);
                }
            }
            // Handle any remaining content
            if !line_buffer.is_empty() {
                if let Some(progress) = parse_tqdm_progress(&line_buffer) {
                    progress_callback(progress);
                }
                stderr_output.push_str(&line_buffer);
            }
        }

        // Collect stdout
        let mut stdout_output = String::new();
        if let Some(stdout_reader) = stdout {
            let reader = BufReader::new(stdout_reader);
            for line in reader.lines().map_while(Result::ok) {
                stdout_output.push_str(&line);
                stdout_output.push('\n');
            }
        }

        // Wait for process to complete
        let status = match child.wait() {
            Ok(s) => s,
            Err(e) => return Err(format!("Failed to wait for convert process: {}", e)),
        };

        if !status.success() {
            return Err(format!(
                "convert failed: {}{}{}",
                status
                    .code()
                    .map(|c| format!("exit code {}", c))
                    .unwrap_or_else(|| "terminated".into()),
                if !stderr_output.trim().is_empty() {
                    format!(" stderr: {}", stderr_output.trim())
                } else {
                    "".into()
                },
                if !stdout_output.trim().is_empty() {
                    format!(" stdout: {}", stdout_output.trim())
                } else {
                    "".into()
                },
            ));
        }

        Ok(())
    })
    .await
    .map_err(|e| RouterError::Internal(e.to_string()))?;

    result.map_err(RouterError::Internal)
}

/// python依存が無いときは事前にエラーにする
async fn ensure_python_deps() -> Result<(), RouterError> {
    let python_bin = get_venv_python()
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "python3".into());
    let script = "import importlib, importlib.util, sys;missing=[m for m in ['transformers','torch','onnx','huggingface_hub'] if importlib.util.find_spec(m) is None];\n\
if missing:\n print(','.join(missing)); sys.exit(1)\n";

    let python_bin_for_cmd = python_bin.clone();
    let output = task::spawn_blocking(move || {
        Command::new(&python_bin_for_cmd)
            .arg("-c")
            .arg(script)
            .output()
    })
    .await
    .map_err(|e| RouterError::Internal(e.to_string()))?
    .map_err(|e| RouterError::Internal(e.to_string()))?;

    if output.status.success() {
        return Ok(());
    }

    let missing = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let deps = if missing.is_empty() {
        "transformers, torch, onnx, huggingface_hub".to_string()
    } else {
        missing
    };
    Err(RouterError::Internal(format!(
        "Missing python deps for HF ONNX export: {}. Install with: python3 -m pip install -r scripts/requirements-export-hf-to-onnx.txt (python_bin={}, stderr={})",
        deps,
        python_bin,
        stderr
    )))
}

fn ensure_python_deps_sync() -> Result<(), RouterError> {
    let python_bin = get_venv_python()
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "python3".into());
    let script = "import importlib, importlib.util, sys;missing=[m for m in ['transformers','torch','onnx','huggingface_hub'] if importlib.util.find_spec(m) is None];\n\
if missing:\n print(','.join(missing)); sys.exit(1)\n";
    let output = std::process::Command::new(&python_bin)
        .arg("-c")
        .arg(script)
        .output()
        .map_err(|e| RouterError::Internal(e.to_string()))?;

    if output.status.success() {
        return Ok(());
    }

    let missing = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let deps = if missing.is_empty() {
        "transformers, torch, onnx, huggingface_hub".to_string()
    } else {
        missing
    };
    Err(RouterError::Internal(format!(
        "Missing python deps for HF ONNX export: {}. Install with: python3 -m pip install -r scripts/requirements-export-hf-to-onnx.txt (python_bin={}, stderr={})",
        deps,
        python_bin,
        stderr
    )))
}

fn should_use_fake_convert() -> bool {
    let enabled = std::env::var("LLM_CONVERT_FAKE")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE"))
        .unwrap_or(false);
    if enabled {
        tracing::warn!("LLM_CONVERT_FAKE is enabled - using dummy ONNX output (for testing only)");
    }
    enabled
}

/// ルーター再起動時に pending_conversion の登録済みモデルを再キューする
pub async fn resume_pending_converts(manager: &ConvertTaskManager, models: Vec<ModelInfo>) {
    for model in models {
        if model.status.as_deref() == Some("pending_conversion") {
            if let (Some(repo), Some(filename)) = (model.repo.clone(), model.filename.clone()) {
                let chat_template = model.chat_template.clone();
                manager
                    .enqueue(repo, filename, None, None, chat_template)
                    .await;
            }
        }
    }
}

async fn write_dummy_onnx(target: &Path) -> Result<(), RouterError> {
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| RouterError::Internal(e.to_string()))?;
    }
    let mut file = tokio::fs::File::create(target)
        .await
        .map_err(|e| RouterError::Internal(e.to_string()))?;
    file.write_all(b"onnx dummy")
        .await
        .map_err(|e| RouterError::Internal(e.to_string()))?;
    Ok(())
}

fn locate_convert_script() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("LLM_CONVERT_SCRIPT") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    let candidates = vec![
        PathBuf::from("scripts/export_hf_to_onnx.py"),
        PathBuf::from("../scripts/export_hf_to_onnx.py"),
    ];

    for cand in candidates {
        if cand.exists() {
            return Some(cand);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let cand = dir.join("../scripts/export_hf_to_onnx.py");
            if cand.exists() {
                return Some(cand);
            }
        }
    }

    None
}

fn is_default_convert_script(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.contains("export_hf_to_onnx.py"))
        .unwrap_or(false)
}

/// 変換に必要なスクリプト・依存が利用可能かを起動時に検証する。
/// - デフォルトスクリプトを使う場合のみ Python 依存チェックを行う。
/// - カスタムスクリプト指定時は存在確認のみ。
pub fn verify_convert_ready() -> Result<(), RouterError> {
    if should_use_fake_convert() {
        return Ok(());
    }
    let script = locate_convert_script()
        .ok_or_else(|| RouterError::Internal("export_hf_to_onnx.py not found".into()))?;
    if is_default_convert_script(&script) {
        // 起動前の依存チェックはベストエフォート（不足しても起動自体は許可）。
        // 実際の変換時に ensure_python_deps() がエラーを返す。
        if let Err(e) = ensure_python_deps_sync() {
            tracing::warn!(
                "ONNX export deps check failed (conversion may not work): {}",
                e
            );
        }
    }
    Ok(())
}

async fn finalize_model_registration(
    model_name: &str,
    repo: &str,
    filename: &str,
    download_url: Option<&str>,
    target: &Path,
    chat_template: Option<String>,
) {
    use crate::api::models::{
        list_registered_models, persist_registered_models, upsert_registered_model,
    };

    let size = tokio::fs::metadata(target)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    const REQUIRED_MEMORY_RATIO: f64 = 1.5;
    let required_memory = ((size as f64) * REQUIRED_MEMORY_RATIO).ceil() as u64;

    let mut model = list_registered_models()
        .into_iter()
        .find(|m| m.name == model_name)
        .unwrap_or_else(|| ModelInfo::new(model_name.to_string(), 0, repo.to_string(), 0, vec![]));

    model.size = size;
    model.required_memory = required_memory;
    model.tags = vec!["onnx".into()];
    model.source = ModelSource::HfOnnx;
    model.path = Some(target.to_string_lossy().to_string());
    model.download_url = download_url
        .map(|u| u.to_string())
        .filter(|u| !u.is_empty());
    model.repo = Some(repo.to_string());
    model.filename = Some(filename.to_string());
    model.status = Some("cached".into());
    if model.description.is_empty() {
        model.description = repo.to_string();
    }
    if chat_template.is_some() || model.chat_template.is_none() {
        model.chat_template = chat_template;
    }

    upsert_registered_model(model);
    persist_registered_models().await;
}

// ===== Progress Parsing =====

/// Lazy-compiled regex patterns for parsing tqdm/progress output
static PERCENT_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(\d+(?:\.\d+)?)\s*%").expect("Invalid percent regex"));

static FRACTION_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(\d+)/(\d+)\b").expect("Invalid fraction regex"));

/// Parse progress from tqdm-style output
/// Supports formats:
/// - Percentage: "45%", "99.5%"
/// - Fraction: "45/100", "Writing: 1024/2048"
fn parse_tqdm_progress(line: &str) -> Option<f32> {
    // Pattern 1: Percentage (e.g., "45%" or "Writing: 99.5%|████░░░|")
    if let Some(caps) = PERCENT_REGEX.captures(line) {
        if let Some(percent_str) = caps.get(1) {
            if let Ok(percent) = percent_str.as_str().parse::<f32>() {
                return Some((percent / 100.0).clamp(0.0, 1.0));
            }
        }
    }

    // Pattern 2: Fraction (e.g., "45/100" or "Writing: 1024/2048 MB")
    if let Some(caps) = FRACTION_REGEX.captures(line) {
        if let (Some(current_str), Some(total_str)) = (caps.get(1), caps.get(2)) {
            if let (Ok(current), Ok(total)) = (
                current_str.as_str().parse::<f64>(),
                total_str.as_str().parse::<f64>(),
            ) {
                if total > 0.0 {
                    return Some(((current / total) as f32).clamp(0.0, 1.0));
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::models::{ModelInfo, ModelSource};
    use std::{env, time::Duration};

    // ===== Progress Parser Tests =====

    #[test]
    fn parse_tqdm_progress_percentage() {
        // Simple percentage
        assert!((parse_tqdm_progress("45%").unwrap() - 0.45).abs() < 0.001);
        assert!((parse_tqdm_progress("100%").unwrap() - 1.0).abs() < 0.001);
        assert!((parse_tqdm_progress("0%").unwrap() - 0.0).abs() < 0.001);

        // Percentage with decimals
        assert!((parse_tqdm_progress("99.5%").unwrap() - 0.995).abs() < 0.001);

        // Percentage in tqdm output format
        assert!((parse_tqdm_progress("Writing: 45%|████░░░░░░|").unwrap() - 0.45).abs() < 0.001);
    }

    #[test]
    fn parse_tqdm_progress_fraction() {
        // Simple fraction
        assert!((parse_tqdm_progress("45/100").unwrap() - 0.45).abs() < 0.001);
        assert!((parse_tqdm_progress("100/100").unwrap() - 1.0).abs() < 0.001);
        assert!((parse_tqdm_progress("0/100").unwrap() - 0.0).abs() < 0.001);

        // Fraction with context
        assert!((parse_tqdm_progress("Writing: 1024/2048 MB").unwrap() - 0.5).abs() < 0.001);
    }

    #[test]
    fn parse_tqdm_progress_no_match() {
        assert!(parse_tqdm_progress("No progress here").is_none());
        assert!(parse_tqdm_progress("").is_none());
        assert!(parse_tqdm_progress("Loading model...").is_none());
    }

    #[test]
    fn parse_tqdm_progress_edge_cases() {
        // Values > 100% should clamp to 1.0
        assert!((parse_tqdm_progress("150%").unwrap() - 1.0).abs() < 0.001);

        // Zero total should return None (avoid division by zero)
        assert!(parse_tqdm_progress("5/0").is_none());
    }

    // ===== Existing Tests =====

    #[tokio::test]
    async fn resume_pending_converts_enqueues_pending_only() {
        // avoid touching real HOME / conversions
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", tmp.path());
        std::env::set_var("LLM_CONVERT_FAKE", "1");

        let manager = ConvertTaskManager::new(1);

        let mut pending = ModelInfo::new("openai/gpt-oss-20b".into(), 0, "desc".into(), 0, vec![]);
        pending.repo = Some("openai/gpt-oss-20b".into());
        pending.filename = Some("".into());
        pending.status = Some("pending_conversion".into());
        pending.source = ModelSource::HfPendingConversion;

        let mut cached = ModelInfo::new("other/model".into(), 0, "done".into(), 0, vec![]);
        cached.repo = Some("other/model".into());
        cached.filename = Some("model.onnx".into());
        cached.status = Some("cached".into());

        resume_pending_converts(&manager, vec![pending.clone(), cached]).await;

        // give worker thread a moment to pick up the job
        tokio::time::sleep(Duration::from_millis(50)).await;

        let tasks = manager.list().await;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].repo, pending.repo.unwrap());
        assert_eq!(tasks[0].filename, pending.filename.unwrap());
        assert!(matches!(
            tasks[0].status,
            ConvertStatus::Queued | ConvertStatus::InProgress | ConvertStatus::Completed
        ));
    }

    #[test]
    fn verify_convert_ready_allows_custom_script_without_deps() {
        let tmp = tempfile::tempdir().unwrap();
        let script_path = tmp.path().join("mock_script.py");
        std::fs::write(&script_path, "print('ok')").unwrap();
        env::set_var("LLM_CONVERT_SCRIPT", &script_path);
        let res = verify_convert_ready();
        env::remove_var("LLM_CONVERT_SCRIPT");
        assert!(res.is_ok());
    }

    #[test]
    fn verify_convert_ready_falls_back_to_default_script_when_custom_missing() {
        // When the custom script path is invalid, fall back to the default script.
        env::set_var("LLM_CONVERT_SCRIPT", "/nonexistent/script.py");
        let res = verify_convert_ready();
        env::remove_var("LLM_CONVERT_SCRIPT");
        assert!(res.is_ok());
    }
}
