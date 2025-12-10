//! Model download & conversion job manager
//!
//! Downloads models from Hugging Face and (if needed) converts them to GGUF.
//! Jobs are processed asynchronously in the background and progress can be queried via API.

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
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

/// 変換スクリプトのパスを取得
#[allow(dead_code)]
fn find_convert_script() -> Option<PathBuf> {
    // 環境変数でオーバーライド
    if let Ok(custom) = std::env::var("LLM_CONVERT_SCRIPT") {
        return Some(PathBuf::from(custom));
    }

    // node/third_party/llama.cpp/convert_hf_to_gguf.py を探す
    let candidates = [
        PathBuf::from("node/third_party/llama.cpp/convert_hf_to_gguf.py"),
        PathBuf::from("third_party/llama.cpp/convert_hf_to_gguf.py"),
        PathBuf::from("../node/third_party/llama.cpp/convert_hf_to_gguf.py"),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return Some(candidate);
        }
    }

    // 実行ファイルの相対パスから探す
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let relative = exe_dir.join("../node/third_party/llama.cpp/convert_hf_to_gguf.py");
            if relative.exists() {
                return Some(relative);
            }
        }
    }

    None
}

/// requirementsファイルのパスを取得
fn find_requirements_file() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from(
            "node/third_party/llama.cpp/requirements/requirements-convert_hf_to_gguf.txt",
        ),
        PathBuf::from("third_party/llama.cpp/requirements/requirements-convert_hf_to_gguf.txt"),
        PathBuf::from(
            "../node/third_party/llama.cpp/requirements/requirements-convert_hf_to_gguf.txt",
        ),
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
        let task = ConvertTask::new(repo, filename, revision, quantization, chat_template);
        let id = task.id;
        {
            let mut guard = self.tasks.lock().await;
            guard.insert(id, task);
        }
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

    async fn process_task(
        tasks: Arc<Mutex<HashMap<Uuid, ConvertTask>>>,
        task_id: Uuid,
    ) -> Result<(), RouterError> {
        let (repo, filename, revision, quantization, chat_template) = {
            let mut guard = tasks.lock().await;
            let task = guard
                .get_mut(&task_id)
                .ok_or_else(|| RouterError::Internal("Task not found".into()))?;
            task.status = ConvertStatus::InProgress;
            task.updated_at = Utc::now();
            (
                task.repo.clone(),
                task.filename.clone(),
                task.revision.clone(),
                task.quantization.clone(),
                task.chat_template.clone(),
            )
        };

        // execute download/convert
        let res = download_and_maybe_convert(
            &repo,
            &filename,
            revision.as_deref(),
            quantization.as_deref(),
            chat_template.clone(),
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
async fn download_and_maybe_convert(
    repo: &str,
    filename: &str,
    revision: Option<&str>,
    _quantization: Option<&str>,
    chat_template: Option<String>,
) -> Result<String, RouterError> {
    let is_gguf = filename.to_ascii_lowercase().ends_with(".gguf");
    let model_name = format!("hf/{}/{}", repo, filename);
    let base_url = std::env::var("HF_BASE_URL")
        .unwrap_or_else(|_| "https://huggingface.co".to_string())
        .trim_end_matches('/')
        .to_string();
    let url = format!(
        "{}/{}/resolve/{}/{}",
        base_url,
        repo,
        revision.unwrap_or("main"),
        filename
    );

    let base = router_models_dir().ok_or_else(|| RouterError::Internal("HOME not set".into()))?;
    let dir = base.join(model_name_to_dir(&model_name));
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| RouterError::Internal(e.to_string()))?;
    let target = dir.join("model.gguf");

    // skip if already present but make sure metadata is up-to-date
    if target.exists() {
        finalize_model_registration(
            &model_name,
            repo,
            filename,
            &url,
            &target,
            chat_template.clone(),
        )
        .await;
        return Ok(target.to_string_lossy().to_string());
    }

    if is_gguf {
        download_file(&url, &target).await?;
    } else {
        convert_non_gguf(repo, revision, &target).await?;
    }

    finalize_model_registration(&model_name, repo, filename, &url, &target, chat_template).await;

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

/// 非GGUFをGGUFへコンバート（sync heavy → blocking thread）
async fn convert_non_gguf(
    repo: &str,
    revision: Option<&str>,
    target: &Path,
) -> Result<(), RouterError> {
    if should_use_fake_convert() {
        write_dummy_gguf(target).await?;
        return Ok(());
    }

    let script = locate_convert_script()
        .ok_or_else(|| RouterError::Internal("convert_hf_to_gguf.py not found".into()))?;
    // デフォルトスクリプトを使う場合のみ依存チェックを行う。カスタムスクリプトは自己完結を想定。
    if script
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.contains("convert_hf_to_gguf.py"))
        .unwrap_or(false)
    {
        ensure_python_deps().await?;
    }
    let python_bin = std::env::var("LLM_CONVERT_PYTHON").unwrap_or_else(|_| "python3".into());
    let hf_token = std::env::var("HF_TOKEN").ok();

    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| RouterError::Internal(e.to_string()))?;
    }
    if target.exists() {
        let _ = tokio::fs::remove_file(target).await;
    }

    let repo_with_rev = if let Some(rev) = revision {
        format!("{}@{}", repo, rev)
    } else {
        repo.to_string()
    };

    let script_clone = script.clone();
    let target_path = target.to_path_buf();
    let cmd_repo = repo_with_rev.clone();
    let python_bin_clone = python_bin.clone();
    let output = task::spawn_blocking(move || {
        let mut cmd = std::process::Command::new(&python_bin_clone);
        cmd.arg(script_clone)
            .arg("--remote")
            .arg("--outfile")
            .arg(&target_path)
            .arg(&cmd_repo);
        if let Some(token) = hf_token {
            cmd.env("HF_TOKEN", token);
        }
        cmd.output()
    })
    .await
    .map_err(|e| RouterError::Internal(e.to_string()))?
    .map_err(|e| RouterError::Internal(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(RouterError::Internal(format!(
            "convert failed: {}{}{}",
            output
                .status
                .code()
                .map(|c| format!("exit code {}", c))
                .unwrap_or_else(|| "terminated".into()),
            if !stderr.trim().is_empty() {
                format!(" stderr: {}", stderr.trim())
            } else {
                "".into()
            },
            if !stdout.trim().is_empty() {
                format!(" stdout: {}", stdout.trim())
            } else {
                "".into()
            },
        )));
    }

    Ok(())
}

/// python依存が無いときは事前にエラーにする
async fn ensure_python_deps() -> Result<(), RouterError> {
    let python_bin = std::env::var("LLM_CONVERT_PYTHON").unwrap_or_else(|_| "python3".into());
    let script = "import importlib, importlib.util, sys;missing=[m for m in ['transformers','torch','sentencepiece'] if importlib.util.find_spec(m) is None];\n\
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
        "transformers, torch, sentencepiece".to_string()
    } else {
        missing
    };
    Err(RouterError::Internal(format!(
        "Missing python deps for HF convert: {}. Install with: python3 -m pip install -r node/third_party/llama.cpp/requirements/requirements-convert_hf_to_gguf.txt (python_bin={}, stderr={})",
        deps,
        python_bin,
        stderr
    )))
}

fn ensure_python_deps_sync() -> Result<(), RouterError> {
    let python_bin = std::env::var("LLM_CONVERT_PYTHON").unwrap_or_else(|_| "python3".into());
    let script = "import importlib, importlib.util, sys;missing=[m for m in ['transformers','torch','sentencepiece'] if importlib.util.find_spec(m) is None];\n\
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
        "transformers, torch, sentencepiece".to_string()
    } else {
        missing
    };
    Err(RouterError::Internal(format!(
        "Missing python deps for HF convert: {}. Install with: python3 -m pip install -r node/third_party/llama.cpp/requirements/requirements-convert_hf_to_gguf.txt (python_bin={}, stderr={})",
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
        tracing::warn!("LLM_CONVERT_FAKE is enabled - using dummy GGUF output (for testing only)");
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

async fn write_dummy_gguf(target: &Path) -> Result<(), RouterError> {
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| RouterError::Internal(e.to_string()))?;
    }
    let mut file = tokio::fs::File::create(target)
        .await
        .map_err(|e| RouterError::Internal(e.to_string()))?;
    file.write_all(b"gguf dummy")
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
        PathBuf::from("node/third_party/llama.cpp/convert_hf_to_gguf.py"),
        PathBuf::from("third_party/llama.cpp/convert_hf_to_gguf.py"),
    ];

    for cand in candidates {
        if cand.exists() {
            return Some(cand);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let cand = dir
                .join("..")
                .join("third_party")
                .join("llama.cpp")
                .join("convert_hf_to_gguf.py");
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
        .map(|n| n.contains("convert_hf_to_gguf.py"))
        .unwrap_or(false)
}

/// 変換に必要なスクリプト・依存が利用可能かを起動時に検証する。
/// - デフォルトスクリプトを使う場合のみ Python 依存チェックを行う。
/// - カスタムスクリプト指定時は存在確認のみ。
pub fn verify_convert_ready() -> Result<(), RouterError> {
    let script = locate_convert_script()
        .ok_or_else(|| RouterError::Internal("convert_hf_to_gguf.py not found".into()))?;
    if is_default_convert_script(&script) {
        // 起動前チェックは同期で実行し、依存不足なら即エラー
        ensure_python_deps_sync()?;
    }
    Ok(())
}

async fn finalize_model_registration(
    model_name: &str,
    repo: &str,
    filename: &str,
    download_url: &str,
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
    model.tags = vec!["gguf".into()];
    model.source = ModelSource::HfGguf;
    model.path = Some(target.to_string_lossy().to_string());
    model.download_url = Some(download_url.to_string());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::models::{ModelInfo, ModelSource};
    use std::{env, time::Duration};

    #[tokio::test]
    async fn resume_pending_converts_enqueues_pending_only() {
        // avoid touching real HOME / conversions
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", tmp.path());
        std::env::set_var("LLM_CONVERT_FAKE", "1");

        let manager = ConvertTaskManager::new(1);

        let mut pending = ModelInfo::new(
            "hf/openai/gpt-oss-20b/metal/model.bin".into(),
            0,
            "desc".into(),
            0,
            vec![],
        );
        pending.repo = Some("openai/gpt-oss-20b".into());
        pending.filename = Some("metal/model.bin".into());
        pending.status = Some("pending_conversion".into());
        pending.source = ModelSource::HfPendingConversion;

        let mut cached = ModelInfo::new("hf/other/model.gguf".into(), 0, "done".into(), 0, vec![]);
        cached.repo = Some("other/model".into());
        cached.filename = Some("model.gguf".into());
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
    fn verify_convert_ready_errors_when_missing_script() {
        env::remove_var("LLM_CONVERT_SCRIPT");
        let res = verify_convert_ready();
        assert!(res.is_err());
    }
}

/// 非GGUF形式のHFモデルをGGUFに変換
#[allow(dead_code)]
async fn convert_to_gguf(
    repo: &str,
    revision: Option<&str>,
    quantization: Option<&str>,
) -> Result<String, RouterError> {
    // venvが準備完了か確認
    if !is_venv_ready() {
        return Err(RouterError::Internal(
            "venv is not ready. Run setup_venv() first or set LLM_CONVERT_SKIP_VENV=1".into(),
        ));
    }

    // Python実行ファイルを取得
    let python = get_venv_python()
        .ok_or_else(|| RouterError::Internal("Cannot find Python executable".into()))?;

    // 変換スクリプトを取得
    let script = find_convert_script().ok_or_else(|| {
        RouterError::Internal(
            "convert_hf_to_gguf.py not found. Ensure llama.cpp is available at node/third_party/llama.cpp/".into(),
        )
    })?;

    // 出力ディレクトリを準備
    let base = router_models_dir().ok_or_else(|| RouterError::Internal("HOME not set".into()))?;
    let model_name_safe = repo.replace('/', "_");
    let quant = quantization.unwrap_or("Q4_K_M");
    let output_filename = format!("{}-{}.gguf", model_name_safe, quant);
    let dir = base.join(model_name_to_dir(&format!(
        "hf/{}/{}",
        repo, output_filename
    )));
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| RouterError::Internal(e.to_string()))?;
    let target = dir.join("model.gguf");

    // 既に変換済みならスキップ
    if target.exists() {
        return Ok(target.to_string_lossy().to_string());
    }

    // HFリポジトリ指定
    let repo_spec = if let Some(rev) = revision {
        format!("{}@{}", repo, rev)
    } else {
        repo.to_string()
    };

    tracing::info!("Starting conversion: {} -> {:?}", repo_spec, target);

    // 変換コマンドを実行（spawn_blockingで非同期に）
    let python_clone = python.clone();
    let script_clone = script.clone();
    let target_clone = target.clone();
    let repo_spec_clone = repo_spec.clone();

    let result = tokio::task::spawn_blocking(move || {
        let mut cmd = Command::new(&python_clone);
        cmd.arg(&script_clone)
            .arg("--remote")
            .arg(&repo_spec_clone)
            .arg("--outfile")
            .arg(&target_clone);

        // HF_TOKENがあれば環境変数として渡す
        if let Ok(token) = std::env::var("HF_TOKEN") {
            cmd.env("HF_TOKEN", token);
        }

        tracing::debug!("Running: {:?}", cmd);
        cmd.output()
    })
    .await
    .map_err(|e| RouterError::Internal(format!("Task join error: {}", e)))?
    .map_err(|e| RouterError::Internal(format!("Failed to run convert script: {}", e)))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        let stdout = String::from_utf8_lossy(&result.stdout);
        return Err(RouterError::Internal(format!(
            "Conversion failed:\nstdout: {}\nstderr: {}",
            stdout, stderr
        )));
    }

    // 変換結果を確認
    if !target.exists() {
        return Err(RouterError::Internal(
            "Conversion completed but output file not found".into(),
        ));
    }

    tracing::info!("Conversion completed: {:?}", target);

    // モデルを登録
    let size = tokio::fs::metadata(&target)
        .await
        .map(|m| m.len())
        .unwrap_or(0);

    let mut model = ModelInfo::new(
        format!("hf/{}/{}", repo, output_filename),
        size,
        repo.to_string(),
        0,
        vec!["gguf".into(), "converted".into()],
    );
    model.path = Some(target.to_string_lossy().to_string());
    model.source = ModelSource::HfGguf;
    let _ = crate::api::models::add_registered_model(model.clone());
    crate::api::models::persist_registered_models().await;

    Ok(target.to_string_lossy().to_string())
}
