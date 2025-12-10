//! Model download & conversion job manager
//!
//! Downloads models from Hugging Face and (if needed) converts them to GGUF.
//! Jobs are processed asynchronously in the background and progress can be queried via API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, Mutex};
use tokio::task;
use uuid::Uuid;

use crate::registry::models::{model_name_to_dir, router_models_dir, ModelInfo, ModelSource};
use llm_router_common::error::RouterError;

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
    _quantization: Option<&str>,
    chat_template: Option<String>,
) -> Result<String, RouterError> {
    let is_gguf = filename.to_ascii_lowercase().ends_with(".gguf");
    let model_name = format!("hf/{}/{}", repo, filename);
    let url = format!(
        "https://huggingface.co/{}/resolve/{}/{}",
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

    ensure_python_deps().await?;

    let script = locate_convert_script()
        .ok_or_else(|| RouterError::Internal("convert_hf_to_gguf.py not found".into()))?;

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
    let output = task::spawn_blocking(move || {
        std::process::Command::new("python3")
            .arg(script_clone)
            .arg("--remote")
            .arg("--outfile")
            .arg(&target_path)
            .arg(&cmd_repo)
            .output()
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
    let script = "import importlib,sys;missing=[m for m in ['transformers','torch','sentencepiece'] if importlib.util.find_spec(m) is None];\n\
if missing:\n print(','.join(missing)); sys.exit(1)\n";

    let output =
        task::spawn_blocking(move || Command::new(&python_bin).arg("-c").arg(script).output())
            .await
            .map_err(|e| RouterError::Internal(e.to_string()))?
            .map_err(|e| RouterError::Internal(e.to_string()))?;

    if output.status.success() {
        return Ok(());
    }

    let missing = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let deps = if missing.is_empty() {
        "transformers, torch, sentencepiece".to_string()
    } else {
        missing
    };
    Err(RouterError::Internal(format!(
        "Missing python deps for HF convert: {}. Install with: python3 -m pip install -r node/third_party/llama.cpp/requirements/requirements-convert_hf_to_gguf.txt",
        deps
    )))
}

fn should_use_fake_convert() -> bool {
    std::env::var("LLM_CONVERT_FAKE")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE"))
        .unwrap_or(false)
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
