//! Model download & conversion job manager
//!
//! Downloads models from Hugging Face and (if needed) converts them to GGUF.
//! Jobs are processed asynchronously in the background and progress can be queried via API.

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, Mutex};
use tokio::task;
use uuid::Uuid;

use crate::api::models::normalize_quantization_label;
use crate::registry::models::{
    generate_model_id, model_name_to_dir, router_models_dir, ModelInfo, ModelSource,
};
use crate::registry::NodeRegistry;
use llm_router_common::error::RouterError;

// ===== GGUF Validation =====

/// GGUFファイルの検証結果
#[derive(Debug)]
pub struct GgufValidation {
    /// マジックバイト ("GGUF")
    pub magic: [u8; 4],
    /// GGUFバージョン
    pub version: u32,
    /// テンソル数
    pub tensor_count: u64,
    /// メタデータKV数
    pub kv_count: u64,
    /// ファイルサイズ（バイト）
    pub file_size: u64,
}

/// GGUFファイルを検証し、ヘッダ情報を返す
pub fn validate_gguf_file(path: &Path) -> Result<GgufValidation, RouterError> {
    use std::fs::File;
    use std::io::Read as StdRead;

    // ファイル存在確認
    if !path.exists() {
        return Err(RouterError::Internal(format!(
            "GGUF file not found: {}",
            path.display()
        )));
    }

    let mut file =
        File::open(path).map_err(|e| RouterError::Internal(format!("Cannot open GGUF: {}", e)))?;

    // ファイルサイズ取得
    let file_size = file
        .metadata()
        .map(|m| m.len())
        .map_err(|e| RouterError::Internal(format!("Cannot get file size: {}", e)))?;

    // 最小ヘッダサイズチェック（magic 4 + version 4 + tensor_count 8 + kv_count 8 = 24）
    if file_size < 24 {
        return Err(RouterError::Internal(format!(
            "GGUF file too small: {} bytes (minimum 24 bytes for header)",
            file_size
        )));
    }

    // ヘッダ読み取り（24バイト）
    let mut header = [0u8; 24];
    file.read_exact(&mut header)
        .map_err(|e| RouterError::Internal(format!("Cannot read GGUF header: {}", e)))?;

    // マジックバイト検証 ("GGUF" = 0x46554747 little-endian)
    let magic: [u8; 4] = header[0..4].try_into().unwrap();
    if &magic != b"GGUF" {
        return Err(RouterError::Internal(format!(
            "Invalid GGUF magic: expected 'GGUF', got {:?}",
            magic
        )));
    }

    // バージョン（u32 little-endian）
    let version = u32::from_le_bytes(header[4..8].try_into().unwrap());

    // テンソル数（u64 little-endian）
    let tensor_count = u64::from_le_bytes(header[8..16].try_into().unwrap());

    // KV数（u64 little-endian）
    let kv_count = u64::from_le_bytes(header[16..24].try_into().unwrap());

    Ok(GgufValidation {
        magic,
        version,
        tensor_count,
        kv_count,
        file_size,
    })
}
use llm_router_common::types::NodeStatus;

// ===== Push Notification Context (SPEC-dcaeaec4 FR-7) =====

/// プッシュ通知用のコンテキスト
struct NotificationContext {
    registry: NodeRegistry,
    http_client: reqwest::Client,
}

/// グローバルな通知コンテキスト
static NOTIFICATION_CONTEXT: Lazy<RwLock<Option<NotificationContext>>> =
    Lazy::new(|| RwLock::new(None));

/// 通知コンテキストを設定（main.rsから呼び出し）
pub fn set_notification_context(registry: NodeRegistry, http_client: reqwest::Client) {
    let mut ctx = NOTIFICATION_CONTEXT.write().unwrap();
    *ctx = Some(NotificationContext {
        registry,
        http_client,
    });
    tracing::info!("Push notification context initialized");
}

/// オンラインノードに新しいモデルの通知を送信
/// SPEC-dcaeaec4 FR-7: 無限リトライ、指数バックオフ（1s, 2s, 4s, ... 最大60s）
async fn notify_nodes_of_new_model(model_name: &str) {
    let (registry, http_client) = {
        let ctx = NOTIFICATION_CONTEXT.read().unwrap();
        match ctx.as_ref() {
            Some(c) => (c.registry.clone(), c.http_client.clone()),
            None => {
                tracing::warn!("Notification context not set, skipping push notification");
                return;
            }
        }
    };

    let nodes = registry.list().await;
    let online_nodes: Vec<_> = nodes
        .into_iter()
        .filter(|n| n.status == NodeStatus::Online)
        .collect();

    if online_nodes.is_empty() {
        tracing::debug!("No online nodes to notify about model: {}", model_name);
        return;
    }

    tracing::info!(
        model = %model_name,
        node_count = online_nodes.len(),
        "Sending push notifications to online nodes"
    );

    // 各ノードへの通知を非同期で実行（メインフローをブロックしない）
    for node in online_nodes {
        let model_name = model_name.to_string();
        let http_client = http_client.clone();
        let node_id = node.id;
        // ノードAPIは runtime_port + 1 で提供される
        let node_api_port = node.runtime_port + 1;
        let node_addr = format!("http://{}:{}", node.ip_address, node_api_port);

        tokio::spawn(async move {
            notify_single_node_with_retry(&http_client, &node_addr, &model_name, node_id).await;
        });
    }
}

/// 単一ノードへの通知（指数バックオフ付きリトライ）
async fn notify_single_node_with_retry(
    http_client: &reqwest::Client,
    node_addr: &str,
    model_name: &str,
    node_id: Uuid,
) {
    const MAX_BACKOFF_SECS: u64 = 60;
    const MAX_ATTEMPTS: u32 = 20; // 無限ではなく実用的な上限を設定

    let url = format!("{}/api/models/pull", node_addr);
    let body = serde_json::json!({
        "model": model_name
    });

    let mut attempt = 0u32;
    let mut backoff_secs = 1u64;

    loop {
        attempt += 1;

        match http_client
            .post(&url)
            .json(&body)
            .timeout(Duration::from_secs(30))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!(
                    node_id = %node_id,
                    model = %model_name,
                    attempt = attempt,
                    "Push notification sent successfully"
                );
                return;
            }
            Ok(resp) => {
                tracing::warn!(
                    node_id = %node_id,
                    model = %model_name,
                    status = %resp.status(),
                    attempt = attempt,
                    "Push notification failed with status"
                );
            }
            Err(e) => {
                tracing::warn!(
                    node_id = %node_id,
                    model = %model_name,
                    error = %e,
                    attempt = attempt,
                    "Push notification request failed"
                );
            }
        }

        if attempt >= MAX_ATTEMPTS {
            tracing::error!(
                node_id = %node_id,
                model = %model_name,
                "Push notification failed after {} attempts, giving up",
                MAX_ATTEMPTS
            );
            return;
        }

        // 指数バックオフ（最大60秒）
        tracing::debug!(
            node_id = %node_id,
            backoff_secs = backoff_secs,
            "Retrying push notification after backoff"
        );
        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
        backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF_SECS);
    }
}

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

/// gguf_new_metadata.py スクリプトのパスを取得
fn find_gguf_metadata_script() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("node/third_party/llama.cpp/gguf-py/gguf/scripts/gguf_new_metadata.py"),
        PathBuf::from("third_party/llama.cpp/gguf-py/gguf/scripts/gguf_new_metadata.py"),
        PathBuf::from("../node/third_party/llama.cpp/gguf-py/gguf/scripts/gguf_new_metadata.py"),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return Some(candidate);
        }
    }

    // 実行ファイルの相対パスから探す
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let relative = exe_dir
                .join("../node/third_party/llama.cpp/gguf-py/gguf/scripts/gguf_new_metadata.py");
            if relative.exists() {
                return Some(relative);
            }
        }
    }

    None
}

/// gguf-py ライブラリのパスを取得（PYTHONPATH用）
fn find_gguf_py_path() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("node/third_party/llama.cpp/gguf-py"),
        PathBuf::from("third_party/llama.cpp/gguf-py"),
        PathBuf::from("../node/third_party/llama.cpp/gguf-py"),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return Some(candidate);
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let relative = exe_dir.join("../node/third_party/llama.cpp/gguf-py");
            if relative.exists() {
                return Some(relative);
            }
        }
    }

    None
}

/// GGUFファイルにchat_templateを追加
async fn add_chat_template_to_gguf(
    gguf_path: &Path,
    chat_template: &str,
) -> Result<(), RouterError> {
    let script = find_gguf_metadata_script()
        .ok_or_else(|| RouterError::Internal("gguf_new_metadata.py not found".into()))?;
    let gguf_py_path = find_gguf_py_path()
        .ok_or_else(|| RouterError::Internal("gguf-py library not found".into()))?;

    let python_bin = std::env::var("LLM_CONVERT_PYTHON").unwrap_or_else(|_| "python3".into());

    // 一時ファイルに出力し、成功したら置換
    let temp_path = gguf_path.with_extension("gguf.tmp");

    let script_clone = script.clone();
    let gguf_path_clone = gguf_path.to_path_buf();
    let temp_path_clone = temp_path.clone();
    let chat_template_clone = chat_template.to_string();
    let gguf_py_path_clone = gguf_py_path.clone();

    let result = task::spawn_blocking(move || {
        let output = Command::new(&python_bin)
            .env("PYTHONPATH", &gguf_py_path_clone)
            .arg(&script_clone)
            .arg(&gguf_path_clone)
            .arg(&temp_path_clone)
            .arg("--chat-template")
            .arg(&chat_template_clone)
            .arg("--force")
            .output();

        match output {
            Ok(out) => {
                if out.status.success() {
                    Ok(())
                } else {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    Err(format!("gguf_new_metadata.py failed: {}", stderr))
                }
            }
            Err(e) => Err(format!("Failed to run gguf_new_metadata.py: {}", e)),
        }
    })
    .await
    .map_err(|e| RouterError::Internal(e.to_string()))?;

    result.map_err(RouterError::Internal)?;

    // 成功したら置換
    tokio::fs::rename(&temp_path, gguf_path)
        .await
        .map_err(|e| RouterError::Internal(format!("Failed to replace GGUF file: {}", e)))?;

    tracing::info!(path=?gguf_path, template_len=chat_template.len(), "Added chat_template to GGUF");

    Ok(())
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
    /// concurrency: 同時に実行可能な変換タスクの最大数
    /// db_pool: SQLiteプール（モデル登録の永続化に使用）
    pub fn new(concurrency: usize, db_pool: sqlx::SqlitePool) -> Self {
        let concurrency = concurrency.max(1); // 最低1つは確保
        let (tx, mut rx) = mpsc::channel::<Uuid>(128);
        let tasks = Arc::new(Mutex::new(HashMap::new()));
        let tasks_clone = tasks.clone();
        let pool_clone = db_pool.clone();

        // Semaphoreで同時実行数を制限
        let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));

        tokio::spawn(async move {
            while let Some(task_id) = rx.recv().await {
                let tasks = tasks_clone.clone();
                let sem = semaphore.clone();
                let pool = pool_clone.clone();

                // Semaphoreのpermitを取得してから並列実行
                tokio::spawn(async move {
                    // acquire_owned()でpermitを取得（タスク完了まで保持）
                    let _permit = match sem.acquire_owned().await {
                        Ok(permit) => permit,
                        Err(_) => {
                            tracing::error!(task_id=?task_id, "Semaphore closed");
                            return;
                        }
                    };

                    if let Err(e) = Self::process_task(tasks, task_id, &pool).await {
                        tracing::error!(task_id=?task_id, error=?e, "convert_task_failed");
                    }
                    // _permit is dropped here, releasing the semaphore
                });
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

    /// 全てのタスクを取得
    pub async fn list_tasks(&self) -> Vec<ConvertTask> {
        self.tasks.lock().await.values().cloned().collect()
    }

    async fn process_task(
        tasks: Arc<Mutex<HashMap<Uuid, ConvertTask>>>,
        task_id: Uuid,
        db_pool: &sqlx::SqlitePool,
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
            db_pool,
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
    quantization: Option<&str>,
    chat_template: Option<String>,
    progress_callback: F,
    db_pool: &sqlx::SqlitePool,
) -> Result<String, RouterError>
where
    F: Fn(f32) + Send + Sync + Clone + 'static,
{
    let is_gguf = filename.to_ascii_lowercase().ends_with(".gguf");
    // モデルIDは階層形式（リポジトリ名）を使用 (SPEC-dcaeaec4 FR-2)
    let model_name = generate_model_id(repo);
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
        let valid = match tokio::fs::metadata(&target).await {
            Ok(meta) => meta.is_file() && meta.len() > 0,
            Err(_) => false,
        };
        if valid {
            progress_callback(1.0);
            finalize_model_registration(
                &model_name,
                repo,
                filename,
                &url,
                &target,
                chat_template.clone(),
                db_pool,
            )
            .await;
            return Ok(target.to_string_lossy().to_string());
        }
        let _ = tokio::fs::remove_file(&target).await;
    }

    if is_gguf {
        download_file(&url, &target).await?;
        progress_callback(1.0);
    } else {
        convert_non_gguf(
            repo,
            revision,
            quantization,
            &target,
            progress_callback.clone(),
        )
        .await?;
    }

    // chat_templateが指定されている場合、GGUFに埋め込む
    if let Some(ref template) = chat_template {
        if !template.is_empty() {
            tracing::info!(
                template_len = template.len(),
                "Embedding chat_template into GGUF"
            );
            if let Err(e) = add_chat_template_to_gguf(&target, template).await {
                // 埋め込み失敗は警告のみ（モデル自体は使用可能）
                tracing::warn!(error=%e, "Failed to embed chat_template, model may work with fallback detection");
            }
        }
    }

    finalize_model_registration(
        &model_name,
        repo,
        filename,
        &url,
        &target,
        chat_template,
        db_pool,
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

/// リトライ可能なエラーかどうかを判定
fn is_retryable_error(error_msg: &str) -> bool {
    let retryable_patterns = [
        "connection reset",
        "connection refused",
        "timeout",
        "timed out",
        "temporary failure",
        "network",
        "ECONNRESET",
        "ETIMEDOUT",
        "ECONNREFUSED",
        "no space left",
        "out of memory",
        "OOM",
        "CUDA",
        "cannot allocate",
    ];

    let error_lower = error_msg.to_lowercase();
    retryable_patterns
        .iter()
        .any(|pattern| error_lower.contains(&pattern.to_lowercase()))
}

fn quantization_outtype(label: &str) -> Option<&'static str> {
    match label {
        "F32" => Some("f32"),
        "F16" => Some("f16"),
        "BF16" => Some("bf16"),
        "Q8_0" => Some("q8_0"),
        _ => None,
    }
}

fn resolve_llama_quantize_bin() -> Result<PathBuf, RouterError> {
    if let Ok(path) = std::env::var("LLM_QUANTIZE_BIN") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
        return Err(RouterError::Internal(
            "LLM_QUANTIZE_BIN is set but the binary was not found".into(),
        ));
    }

    let candidates = [
        PathBuf::from("node/third_party/llama.cpp/build/bin/llama-quantize"),
        PathBuf::from("node/third_party/llama.cpp/llama-quantize"),
        PathBuf::from("third_party/llama.cpp/build/bin/llama-quantize"),
        PathBuf::from("third_party/llama.cpp/llama-quantize"),
    ];

    for cand in candidates {
        if cand.exists() {
            return Ok(cand);
        }
    }

    Ok(PathBuf::from("llama-quantize"))
}

fn run_llama_quantize(
    input_path: &Path,
    output_path: &Path,
    quantization: &str,
) -> Result<(), RouterError> {
    let bin = resolve_llama_quantize_bin()?;
    let output = Command::new(&bin)
        .arg(input_path)
        .arg(output_path)
        .arg(quantization)
        .output()
        .map_err(|e| {
            RouterError::Internal(format!(
                "Failed to run llama-quantize (set LLM_QUANTIZE_BIN): {}",
                e
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(RouterError::Internal(format!(
            "llama-quantize failed: {}{}{}",
            output
                .status
                .code()
                .map(|c| format!("exit code {}", c))
                .unwrap_or_else(|| "terminated".into()),
            if stderr.is_empty() {
                "".into()
            } else {
                format!(" stderr: {}", stderr)
            },
            if stdout.is_empty() {
                "".into()
            } else {
                format!(" stdout: {}", stdout)
            }
        )));
    }

    Ok(())
}

/// 非GGUFをGGUFへコンバート（sync heavy → blocking thread）
/// progress_callback: プログレス更新用のコールバック（0.0〜1.0）
/// リトライ機能付き: 一時的なエラーの場合は最大3回リトライ（指数バックオフ）
async fn convert_non_gguf<F>(
    repo: &str,
    revision: Option<&str>,
    quantization: Option<&str>,
    target: &Path,
    progress_callback: F,
) -> Result<(), RouterError>
where
    F: Fn(f32) + Send + Sync + Clone + 'static,
{
    const MAX_ATTEMPTS: u32 = 3;
    const INITIAL_BACKOFF_SECS: u64 = 5;

    let normalized_quantization = quantization.and_then(normalize_quantization_label);
    if quantization.is_some() && normalized_quantization.is_none() {
        return Err(RouterError::Internal("Invalid quantization label".into()));
    }

    let (convert_outtype, needs_post_quantize) = match normalized_quantization.as_deref() {
        Some(label) => match quantization_outtype(label) {
            Some(outtype) => (Some(outtype), false),
            None => (Some("f16"), true),
        },
        None => (None, false),
    };

    let convert_target = if needs_post_quantize {
        target.with_extension("gguf.tmp")
    } else {
        target.to_path_buf()
    };

    if should_use_fake_convert() {
        progress_callback(0.5);
        // Test delay for concurrency testing (LLM_CONVERT_FAKE_DELAY_MS)
        if let Ok(delay_ms) = std::env::var("LLM_CONVERT_FAKE_DELAY_MS") {
            if let Ok(ms) = delay_ms.parse::<u64>() {
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
            }
        }
        write_dummy_gguf(target).await?;
        progress_callback(1.0);
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

    if let Some(parent) = convert_target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| RouterError::Internal(e.to_string()))?;
    }

    let repo_with_rev = if let Some(rev) = revision {
        format!("{}@{}", repo, rev)
    } else {
        repo.to_string()
    };

    let mut attempt = 0u32;
    let mut backoff_secs = INITIAL_BACKOFF_SECS;
    let mut last_error: Option<String> = None;

    while attempt < MAX_ATTEMPTS {
        attempt += 1;

        // 既存のターゲットファイルを削除（リトライ時にも必要）
        if convert_target.exists() {
            let _ = tokio::fs::remove_file(&convert_target).await;
        }
        if needs_post_quantize && target.exists() {
            let _ = tokio::fs::remove_file(target).await;
        }

        let script_clone = script.clone();
        let target_path = convert_target.to_path_buf();
        let cmd_repo = repo_with_rev.clone();
        let python_bin_clone = python_bin.clone();
        let hf_token_clone = hf_token.clone();
        let progress_callback_clone = progress_callback.clone();
        let progress_scale: f32 = if needs_post_quantize { 0.9 } else { 1.0 };
        let outtype = convert_outtype;

        if attempt > 1 {
            tracing::info!(
                repo = %repo,
                attempt = attempt,
                max_attempts = MAX_ATTEMPTS,
                "Retrying conversion"
            );
        }

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
            if let Some(outtype) = outtype {
                cmd.arg("--outtype").arg(outtype);
            }

            if let Some(token) = hf_token_clone {
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
                                progress_callback_clone(progress * progress_scale);
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
                        progress_callback_clone(progress * progress_scale);
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

        // Handle the result and check for retryable errors
        match result {
            Ok(()) => {
                // Phase 1: GGUF検証 - 変換後のファイルを検証
                let validation = validate_gguf_file(&convert_target)?;

                // テンソル数が0の場合はエラー（空のGGUFファイル）
                if validation.tensor_count == 0 {
                    let error_msg = "Conversion produced empty GGUF file (0 tensors)";
                    // Empty GGUF is typically not a retryable error, but check anyway
                    if attempt < MAX_ATTEMPTS && is_retryable_error(error_msg) {
                        last_error = Some(error_msg.to_string());
                        tracing::warn!(
                            repo = %repo,
                            attempt = attempt,
                            error = %error_msg,
                            backoff_secs = backoff_secs,
                            "Conversion failed with retryable error, will retry"
                        );
                        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                        backoff_secs *= 2;
                        continue;
                    }
                    return Err(RouterError::Internal(error_msg.into()));
                }

                let file_size_mb = validation.file_size / 1_000_000;

                // Phase 3: サイズ妥当性警告（500MB未満は警告）
                if file_size_mb < 500 {
                    tracing::warn!(
                        path = %target.display(),
                        file_size_mb = file_size_mb,
                        tensor_count = validation.tensor_count,
                        "GGUF file is unusually small - conversion may be incomplete"
                    );
                }

                tracing::info!(
                    path = %convert_target.display(),
                    version = validation.version,
                    tensor_count = validation.tensor_count,
                    kv_count = validation.kv_count,
                    file_size_mb = file_size_mb,
                    "GGUF validation passed"
                );

                if needs_post_quantize {
                    let quantization_label =
                        normalized_quantization.as_deref().ok_or_else(|| {
                            RouterError::Internal("Missing quantization label".into())
                        })?;
                    if let Err(e) = run_llama_quantize(&convert_target, target, quantization_label)
                    {
                        let _ = tokio::fs::remove_file(&convert_target).await;
                        let _ = tokio::fs::remove_file(target).await;
                        return Err(e);
                    }
                    let _ = tokio::fs::remove_file(&convert_target).await;
                    let validation = validate_gguf_file(target)?;
                    if validation.tensor_count == 0 {
                        return Err(RouterError::Internal("Quantized GGUF file is empty".into()));
                    }
                    progress_callback(1.0);
                }

                return Ok(());
            }
            Err(error_msg) => {
                // Check if this is a retryable error
                if attempt < MAX_ATTEMPTS && is_retryable_error(&error_msg) {
                    last_error = Some(error_msg.clone());
                    tracing::warn!(
                        repo = %repo,
                        attempt = attempt,
                        error = %error_msg,
                        backoff_secs = backoff_secs,
                        "Conversion failed with retryable error, will retry"
                    );
                    tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                    backoff_secs *= 2;
                    continue;
                }
                // Non-retryable error or max attempts reached
                return Err(RouterError::Internal(error_msg));
            }
        }
    }

    // All retries exhausted
    Err(RouterError::Internal(format!(
        "Conversion failed after {} attempts: {}",
        MAX_ATTEMPTS,
        last_error.unwrap_or_else(|| "Unknown error".to_string())
    )))
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
    db_pool: &sqlx::SqlitePool,
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
    persist_registered_models(db_pool).await;

    // SPEC-dcaeaec4 FR-7: オンラインノードにプッシュ通知を送信
    notify_nodes_of_new_model(model_name).await;
}

/// 非GGUF形式のHFモデルをGGUFに変換
#[allow(dead_code)]
async fn convert_to_gguf(
    repo: &str,
    revision: Option<&str>,
    _quantization: Option<&str>,
    db_pool: &sqlx::SqlitePool,
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
    // モデルIDは階層形式（リポジトリ名）を使用 (SPEC-dcaeaec4 FR-2)
    let model_name = generate_model_id(repo);
    let dir = base.join(model_name_to_dir(&model_name));
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
        repo.to_string(), // モデル名 = リポジトリ名
        size,
        repo.to_string(),
        0,
        vec!["gguf".into(), "converted".into()],
    );
    model.path = Some(target.to_string_lossy().to_string());
    model.source = ModelSource::HfGguf;
    let _ = crate::api::models::add_registered_model(model.clone());
    crate::api::models::persist_registered_models(db_pool).await;

    Ok(target.to_string_lossy().to_string())
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
    use serial_test::serial;
    use sqlx::SqlitePool;
    use std::{env, time::Duration};

    /// テスト用のインメモリSQLiteプールを作成
    async fn create_test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create test pool");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("Failed to run migrations");

        pool
    }

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

        let pool = create_test_pool().await;
        let manager = ConvertTaskManager::new(1, pool);

        let mut pending = ModelInfo::new("gpt-oss-20b".into(), 0, "desc".into(), 0, vec![]);
        pending.repo = Some("openai/gpt-oss-20b".into());
        pending.filename = Some("metal/model.bin".into());
        pending.status = Some("pending_conversion".into());
        pending.source = ModelSource::HfPendingConversion;

        let mut cached = ModelInfo::new("other".into(), 0, "done".into(), 0, vec![]);
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
    #[serial]
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
    #[serial]
    fn verify_convert_ready_errors_when_missing_script() {
        env::remove_var("LLM_CONVERT_SCRIPT");
        let res = verify_convert_ready();
        assert!(res.is_err());
    }

    // ===== GGUF Validation Tests =====

    #[test]
    fn validate_gguf_file_valid_header() {
        let tmp = tempfile::tempdir().unwrap();
        let gguf_path = tmp.path().join("test.gguf");

        // Create a valid GGUF header (24 bytes minimum)
        // Magic: "GGUF" (4 bytes)
        // Version: 3 (4 bytes, little-endian)
        // Tensor count: 100 (8 bytes, little-endian)
        // KV count: 10 (8 bytes, little-endian)
        let mut header = Vec::with_capacity(24);
        header.extend_from_slice(b"GGUF");
        header.extend_from_slice(&3u32.to_le_bytes()); // version
        header.extend_from_slice(&100u64.to_le_bytes()); // tensor_count
        header.extend_from_slice(&10u64.to_le_bytes()); // kv_count

        std::fs::write(&gguf_path, &header).unwrap();

        let result = validate_gguf_file(&gguf_path);
        assert!(result.is_ok());

        let validation = result.unwrap();
        assert_eq!(&validation.magic, b"GGUF");
        assert_eq!(validation.version, 3);
        assert_eq!(validation.tensor_count, 100);
        assert_eq!(validation.kv_count, 10);
        assert_eq!(validation.file_size, 24);
    }

    #[test]
    fn validate_gguf_file_invalid_magic() {
        let tmp = tempfile::tempdir().unwrap();
        let gguf_path = tmp.path().join("invalid.gguf");

        // Create header with invalid magic
        let mut header = Vec::with_capacity(24);
        header.extend_from_slice(b"XXXX"); // Invalid magic
        header.extend_from_slice(&3u32.to_le_bytes());
        header.extend_from_slice(&100u64.to_le_bytes());
        header.extend_from_slice(&10u64.to_le_bytes());

        std::fs::write(&gguf_path, &header).unwrap();

        let result = validate_gguf_file(&gguf_path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid GGUF magic"));
    }

    #[test]
    fn validate_gguf_file_too_small() {
        let tmp = tempfile::tempdir().unwrap();
        let gguf_path = tmp.path().join("small.gguf");

        // Create file smaller than minimum header size (24 bytes)
        std::fs::write(&gguf_path, b"GGUF123").unwrap(); // Only 7 bytes

        let result = validate_gguf_file(&gguf_path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("too small"));
    }

    #[test]
    fn validate_gguf_file_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let gguf_path = tmp.path().join("nonexistent.gguf");

        let result = validate_gguf_file(&gguf_path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"));
    }

    #[test]
    fn validate_gguf_file_zero_tensors() {
        let tmp = tempfile::tempdir().unwrap();
        let gguf_path = tmp.path().join("empty.gguf");

        // Create header with 0 tensors
        let mut header = Vec::with_capacity(24);
        header.extend_from_slice(b"GGUF");
        header.extend_from_slice(&3u32.to_le_bytes());
        header.extend_from_slice(&0u64.to_le_bytes()); // 0 tensors
        header.extend_from_slice(&10u64.to_le_bytes());

        std::fs::write(&gguf_path, &header).unwrap();

        let result = validate_gguf_file(&gguf_path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().tensor_count, 0);
    }

    // ===== Retry Logic Tests =====

    #[test]
    fn is_retryable_error_network_errors() {
        // Network-related errors should be retryable
        assert!(is_retryable_error("connection reset by peer"));
        assert!(is_retryable_error("Connection refused"));
        assert!(is_retryable_error("request timeout"));
        assert!(is_retryable_error("operation timed out"));
        assert!(is_retryable_error("temporary failure in name resolution"));
        assert!(is_retryable_error("network is unreachable"));
        assert!(is_retryable_error("ECONNRESET"));
        assert!(is_retryable_error("ETIMEDOUT"));
        assert!(is_retryable_error("ECONNREFUSED"));
    }

    #[test]
    fn is_retryable_error_resource_errors() {
        // Resource-related errors should be retryable
        assert!(is_retryable_error("no space left on device"));
        assert!(is_retryable_error("out of memory"));
        assert!(is_retryable_error("OOM killed"));
        assert!(is_retryable_error("CUDA out of memory"));
        assert!(is_retryable_error("cannot allocate memory"));
    }

    #[test]
    fn is_retryable_error_non_retryable() {
        // Non-retryable errors
        assert!(!is_retryable_error("file not found"));
        assert!(!is_retryable_error("permission denied"));
        assert!(!is_retryable_error("invalid argument"));
        assert!(!is_retryable_error("syntax error in script"));
        assert!(!is_retryable_error("model format not supported"));
        assert!(!is_retryable_error(""));
    }

    #[test]
    fn is_retryable_error_case_insensitive() {
        // Should be case-insensitive
        assert!(is_retryable_error("CONNECTION RESET"));
        assert!(is_retryable_error("Timeout"));
        assert!(is_retryable_error("OUT OF MEMORY"));
    }

    // ===== Concurrency Tests =====

    #[tokio::test]
    async fn convert_task_manager_processes_tasks_concurrently() {
        // Setup: use fake conversion with a delay
        let tmp = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", tmp.path());
        std::env::set_var("LLM_CONVERT_FAKE", "1");
        std::env::set_var("LLM_CONVERT_FAKE_DELAY_MS", "100"); // 100ms per task

        // Create manager with concurrency=2
        let pool = create_test_pool().await;
        let manager = ConvertTaskManager::new(2, pool);

        // Enqueue 2 tasks
        let start = std::time::Instant::now();
        let _task1 = manager
            .enqueue(
                "test/model1".into(),
                "model.safetensors".into(),
                None,
                None,
                None,
            )
            .await;
        let _task2 = manager
            .enqueue(
                "test/model2".into(),
                "model.safetensors".into(),
                None,
                None,
                None,
            )
            .await;

        // Wait for both tasks to complete (with timeout)
        let timeout = Duration::from_millis(500);
        let poll_interval = Duration::from_millis(20);
        let deadline = std::time::Instant::now() + timeout;

        loop {
            let tasks = manager.list().await;
            let completed = tasks
                .iter()
                .filter(|t| matches!(t.status, ConvertStatus::Completed | ConvertStatus::Failed))
                .count();

            if completed == 2 {
                break;
            }

            if std::time::Instant::now() > deadline {
                panic!("Timeout waiting for tasks to complete");
            }

            tokio::time::sleep(poll_interval).await;
        }

        let elapsed = start.elapsed();

        // With concurrency=2 and 100ms delay each, both tasks should complete
        // in approximately 100ms (running in parallel), not 200ms (sequential).
        // Allow some overhead, but should be under 180ms if truly concurrent.
        assert!(
            elapsed.as_millis() < 180,
            "Expected concurrent execution (< 180ms), but took {}ms. \
             This suggests tasks ran sequentially.",
            elapsed.as_millis()
        );

        // Cleanup
        std::env::remove_var("LLM_CONVERT_FAKE_DELAY_MS");
    }
}
