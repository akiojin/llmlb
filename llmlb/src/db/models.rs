//! モデル情報の永続化 (SQLite)

use crate::common::error::{LbError, RouterResult};
use crate::types::ModelCapability;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::registry::models::{ModelInfo, ModelSource};

/// SQLiteベースのモデルストレージ
#[derive(Clone)]
pub struct ModelStorage {
    pool: SqlitePool,
}

/// データベース行からの読み取り用構造体
#[derive(Debug, sqlx::FromRow)]
struct ModelRow {
    name: String,
    size: i64,
    description: String,
    required_memory: i64,
    source: String,
    chat_template: Option<String>,
    repo: Option<String>,
    filename: Option<String>,
    last_modified: Option<String>,
    status: Option<String>,
}

impl ModelStorage {
    /// 新しいModelStorageを作成
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// モデルを保存（UPSERT）
    pub async fn save_model(&self, model: &ModelInfo) -> RouterResult<()> {
        let source_str = match model.source {
            ModelSource::Predefined => "predefined",
            ModelSource::HfGguf => "hf_gguf",
            ModelSource::HfSafetensors => "hf_safetensors",
            ModelSource::HfOnnx => "hf_onnx",
        };

        let last_modified_str = model.last_modified.map(|dt| dt.to_rfc3339());

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| LbError::Database(format!("Failed to begin transaction: {}", e)))?;

        // メインモデルをUPSERT
        sqlx::query(
            r#"
            INSERT INTO models (name, size, description, required_memory, source,
                               chat_template, repo, filename,
                               last_modified, status)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(name) DO UPDATE SET
                size = excluded.size,
                description = excluded.description,
                required_memory = excluded.required_memory,
                source = excluded.source,
                chat_template = excluded.chat_template,
                repo = excluded.repo,
                filename = excluded.filename,
                last_modified = excluded.last_modified,
                status = excluded.status
            "#,
        )
        .bind(&model.name)
        .bind(model.size as i64)
        .bind(&model.description)
        .bind(model.required_memory as i64)
        .bind(source_str)
        .bind(&model.chat_template)
        .bind(&model.repo)
        .bind(&model.filename)
        .bind(&last_modified_str)
        .bind(&model.status)
        .execute(&mut *tx)
        .await
        .map_err(|e| LbError::Database(format!("Failed to upsert model: {}", e)))?;

        // タグを更新
        self.clear_and_insert_tags(&mut tx, &model.name, &model.tags)
            .await?;

        // 能力を更新
        self.clear_and_insert_capabilities(&mut tx, &model.name, &model.capabilities)
            .await?;

        tx.commit()
            .await
            .map_err(|e| LbError::Database(format!("Failed to commit transaction: {}", e)))?;

        Ok(())
    }

    /// タグをクリアして再挿入
    async fn clear_and_insert_tags(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        model_name: &str,
        tags: &[String],
    ) -> RouterResult<()> {
        sqlx::query("DELETE FROM model_tags WHERE model_name = ?")
            .bind(model_name)
            .execute(&mut **tx)
            .await
            .map_err(|e| LbError::Database(format!("Failed to delete tags: {}", e)))?;

        for tag in tags {
            sqlx::query("INSERT INTO model_tags (model_name, tag) VALUES (?, ?)")
                .bind(model_name)
                .bind(tag)
                .execute(&mut **tx)
                .await
                .map_err(|e| LbError::Database(format!("Failed to insert tag: {}", e)))?;
        }

        Ok(())
    }

    /// 能力をクリアして再挿入
    async fn clear_and_insert_capabilities(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        model_name: &str,
        capabilities: &[ModelCapability],
    ) -> RouterResult<()> {
        sqlx::query("DELETE FROM model_capabilities WHERE model_name = ?")
            .bind(model_name)
            .execute(&mut **tx)
            .await
            .map_err(|e| LbError::Database(format!("Failed to delete capabilities: {}", e)))?;

        for cap in capabilities {
            let cap_str = format!("{:?}", cap);
            sqlx::query("INSERT INTO model_capabilities (model_name, capability) VALUES (?, ?)")
                .bind(model_name)
                .bind(&cap_str)
                .execute(&mut **tx)
                .await
                .map_err(|e| LbError::Database(format!("Failed to insert capability: {}", e)))?;
        }

        Ok(())
    }

    /// 全モデルを読み込み
    pub async fn load_models(&self) -> RouterResult<Vec<ModelInfo>> {
        let rows: Vec<ModelRow> = sqlx::query_as("SELECT * FROM models")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| LbError::Database(format!("Failed to load models: {}", e)))?;

        let mut models = Vec::with_capacity(rows.len());

        for row in rows {
            let tags = self.load_tags(&row.name).await?;
            let capabilities = self.load_capabilities(&row.name).await?;

            let source = match row.source.as_str() {
                "hf_gguf" => ModelSource::HfGguf,
                "hf_safetensors" => ModelSource::HfSafetensors,
                "hf_onnx" => ModelSource::HfOnnx,
                "hf_pending_conversion" => ModelSource::HfSafetensors,
                _ => ModelSource::Predefined,
            };

            let last_modified = row.last_modified.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            });

            models.push(ModelInfo {
                name: row.name,
                size: row.size as u64,
                description: row.description,
                required_memory: row.required_memory as u64,
                tags,
                capabilities,
                source,
                chat_template: row.chat_template,
                repo: row.repo,
                filename: row.filename,
                last_modified,
                status: row.status,
            });
        }

        Ok(models)
    }

    /// 特定のモデルを読み込み
    pub async fn load_model(&self, name: &str) -> RouterResult<Option<ModelInfo>> {
        let row: Option<ModelRow> = sqlx::query_as("SELECT * FROM models WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| LbError::Database(format!("Failed to load model: {}", e)))?;

        match row {
            Some(row) => {
                let tags = self.load_tags(&row.name).await?;
                let capabilities = self.load_capabilities(&row.name).await?;

                let source = match row.source.as_str() {
                    "hf_gguf" => ModelSource::HfGguf,
                    "hf_safetensors" => ModelSource::HfSafetensors,
                    "hf_onnx" => ModelSource::HfOnnx,
                    "hf_pending_conversion" => ModelSource::HfSafetensors,
                    _ => ModelSource::Predefined,
                };

                let last_modified = row.last_modified.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&Utc))
                        .ok()
                });

                Ok(Some(ModelInfo {
                    name: row.name,
                    size: row.size as u64,
                    description: row.description,
                    required_memory: row.required_memory as u64,
                    tags,
                    capabilities,
                    source,
                    chat_template: row.chat_template,
                    repo: row.repo,
                    filename: row.filename,
                    last_modified,
                    status: row.status,
                }))
            }
            None => Ok(None),
        }
    }

    /// モデルを削除
    pub async fn delete_model(&self, name: &str) -> RouterResult<()> {
        sqlx::query("DELETE FROM models WHERE name = ?")
            .bind(name)
            .execute(&self.pool)
            .await
            .map_err(|e| LbError::Database(format!("Failed to delete model: {}", e)))?;

        Ok(())
    }

    /// タグを読み込み
    async fn load_tags(&self, model_name: &str) -> RouterResult<Vec<String>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT tag FROM model_tags WHERE model_name = ?")
                .bind(model_name)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| LbError::Database(format!("Failed to load tags: {}", e)))?;

        Ok(rows.into_iter().map(|(tag,)| tag).collect())
    }

    /// 能力を読み込み
    async fn load_capabilities(&self, model_name: &str) -> RouterResult<Vec<ModelCapability>> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT capability FROM model_capabilities WHERE model_name = ?")
                .bind(model_name)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| LbError::Database(format!("Failed to load capabilities: {}", e)))?;

        let capabilities: Vec<ModelCapability> = rows
            .into_iter()
            .filter_map(|(cap_str,)| match cap_str.as_str() {
                "TextGeneration" => Some(ModelCapability::TextGeneration),
                "TextToSpeech" => Some(ModelCapability::TextToSpeech),
                "SpeechToText" => Some(ModelCapability::SpeechToText),
                "ImageGeneration" => Some(ModelCapability::ImageGeneration),
                "Embedding" => Some(ModelCapability::Embedding),
                _ => None,
            })
            .collect();

        Ok(capabilities)
    }

    /// 複数モデルを一括保存
    pub async fn save_models(&self, models: &[ModelInfo]) -> RouterResult<()> {
        for model in models {
            self.save_model(model).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_pool() -> SqlitePool {
        crate::db::test_utils::test_db_pool().await
    }

    #[tokio::test]
    async fn test_save_and_load_model() {
        let pool = create_test_pool().await;
        let storage = ModelStorage::new(pool);

        let model = ModelInfo {
            name: "test-model".to_string(),
            size: 1000000,
            description: "Test model".to_string(),
            required_memory: 2000000,
            tags: vec!["llm".to_string(), "test".to_string()],
            capabilities: vec![ModelCapability::TextGeneration],
            source: ModelSource::Predefined,
            chat_template: None,
            repo: Some("test/repo".to_string()),
            filename: Some("model.gguf".to_string()),
            last_modified: Some(Utc::now()),
            status: Some("available".to_string()),
        };

        storage.save_model(&model).await.unwrap();

        let loaded = storage.load_model("test-model").await.unwrap();
        assert!(loaded.is_some());

        let loaded = loaded.unwrap();
        assert_eq!(loaded.name, "test-model");
        assert_eq!(loaded.size, 1000000);
        assert_eq!(loaded.tags, vec!["llm", "test"]);
        assert_eq!(loaded.capabilities, vec![ModelCapability::TextGeneration]);
    }

    #[tokio::test]
    async fn test_load_models() {
        let pool = create_test_pool().await;
        let storage = ModelStorage::new(pool);

        let model1 = ModelInfo::new(
            "model1".to_string(),
            1000,
            "Model 1".to_string(),
            2000,
            vec!["tag1".to_string()],
        );
        let model2 = ModelInfo::new(
            "model2".to_string(),
            2000,
            "Model 2".to_string(),
            4000,
            vec!["tag2".to_string()],
        );

        storage.save_model(&model1).await.unwrap();
        storage.save_model(&model2).await.unwrap();

        let models = storage.load_models().await.unwrap();
        assert_eq!(models.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_model() {
        let pool = create_test_pool().await;
        let storage = ModelStorage::new(pool);

        let model = ModelInfo::new(
            "to-delete".to_string(),
            1000,
            "To delete".to_string(),
            2000,
            vec![],
        );

        storage.save_model(&model).await.unwrap();
        assert!(storage.load_model("to-delete").await.unwrap().is_some());

        storage.delete_model("to-delete").await.unwrap();
        assert!(storage.load_model("to-delete").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_update_model() {
        let pool = create_test_pool().await;
        let storage = ModelStorage::new(pool);

        let mut model = ModelInfo::new(
            "updatable".to_string(),
            1000,
            "Original".to_string(),
            2000,
            vec!["original".to_string()],
        );

        storage.save_model(&model).await.unwrap();

        // Update the model
        model.description = "Updated".to_string();
        model.tags = vec!["updated".to_string()];
        model.capabilities = vec![ModelCapability::TextGeneration];

        storage.save_model(&model).await.unwrap();

        let loaded = storage.load_model("updatable").await.unwrap().unwrap();
        assert_eq!(loaded.description, "Updated");
        assert_eq!(loaded.tags, vec!["updated"]);
        assert_eq!(loaded.capabilities, vec![ModelCapability::TextGeneration]);
    }
}
