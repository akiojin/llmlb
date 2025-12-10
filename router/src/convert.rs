//! Model download & conversion job manager
//!
//! Downloads models from Hugging Face and (if needed) converts them to GGUF.
//! Jobs are processed asynchronously in the background and progress can be queried via API.

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, RwLock};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, Mutex};
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
        let _ = self.queue_tx.send(id).await;
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
    quantization: Option<&str>,
    chat_template: Option<String>,
) -> Result<String, RouterError> {
    let is_gguf = filename.to_ascii_lowercase().ends_with(".gguf");

    // 非GGUF形式の場合は変換を試みる
    if !is_gguf {
        return convert_to_gguf(repo, revision, quantization).await;
    }

    let url = format!(
        "https://huggingface.co/{}/resolve/{}/{}",
        repo,
        revision.unwrap_or("main"),
        filename
    );

    let base = router_models_dir().ok_or_else(|| RouterError::Internal("HOME not set".into()))?;
    let dir = base.join(model_name_to_dir(&format!("hf/{}/{}", repo, filename)));
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| RouterError::Internal(e.to_string()))?;
    let target = dir.join("model.gguf");

    // skip if already present
    if target.exists() {
        return Ok(target.to_string_lossy().to_string());
    }

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| RouterError::Http(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(RouterError::Http(resp.status().to_string()));
    }
    let mut file = tokio::fs::File::create(&target)
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

    // register model info
    let mut model = ModelInfo::new(
        format!("hf/{}/{}", repo, filename),
        0,
        repo.to_string(),
        0,
        vec!["gguf".into()],
    );
    model.download_url = Some(url);
    model.path = Some(target.to_string_lossy().to_string());
    model.chat_template = chat_template;
    model.source = ModelSource::HfGguf;
    if let Ok(meta) = tokio::fs::metadata(&target).await {
        model.size = meta.len();
    }
    let _ = crate::api::models::add_registered_model(model.clone());
    crate::api::models::persist_registered_models().await;

    Ok(target.to_string_lossy().to_string())
}

/// 非GGUF形式のHFモデルをGGUFに変換
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
