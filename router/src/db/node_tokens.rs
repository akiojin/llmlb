// ノードトークンCRUD操作とトークン生成

use chrono::{DateTime, Utc};
use llm_router_common::auth::{NodeToken, NodeTokenWithPlaintext};
use llm_router_common::error::RouterError;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

/// ノードトークンを生成
pub async fn create(
    pool: &SqlitePool,
    node_id: Uuid,
) -> Result<NodeTokenWithPlaintext, RouterError> {
    let token = generate_node_token();
    let token_hash = hash_with_sha256(&token);
    let created_at = Utc::now();

    sqlx::query(
        "INSERT INTO node_tokens (node_id, token_hash, created_at)
         VALUES (?, ?, ?)",
    )
    .bind(node_id.to_string())
    .bind(&token_hash)
    .bind(created_at.to_rfc3339())
    .execute(pool)
    .await
    .map_err(|e| RouterError::Database(format!("Failed to create node token: {}", e)))?;

    Ok(NodeTokenWithPlaintext {
        node_id,
        token,
        created_at,
    })
}

/// ハッシュ値でノードトークンを検索
pub async fn find_by_hash(
    pool: &SqlitePool,
    token_hash: &str,
) -> Result<Option<NodeToken>, RouterError> {
    let row = sqlx::query_as::<_, NodeTokenRow>(
        "SELECT node_id, token_hash, created_at FROM node_tokens WHERE token_hash = ?",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .map_err(|e| RouterError::Database(format!("Failed to find node token: {}", e)))?;

    Ok(row.map(|r| r.into_node_token()))
}

/// ノードIDでトークンを検索
pub async fn find_by_node_id(
    pool: &SqlitePool,
    node_id: Uuid,
) -> Result<Option<NodeToken>, RouterError> {
    let row = sqlx::query_as::<_, NodeTokenRow>(
        "SELECT node_id, token_hash, created_at FROM node_tokens WHERE node_id = ?",
    )
    .bind(node_id.to_string())
    .fetch_optional(pool)
    .await
    .map_err(|e| RouterError::Database(format!("Failed to find node token: {}", e)))?;

    Ok(row.map(|r| r.into_node_token()))
}

/// ノードトークンを削除
pub async fn delete(pool: &SqlitePool, node_id: Uuid) -> Result<(), RouterError> {
    sqlx::query("DELETE FROM node_tokens WHERE node_id = ?")
        .bind(node_id.to_string())
        .execute(pool)
        .await
        .map_err(|e| RouterError::Database(format!("Failed to delete node token: {}", e)))?;

    Ok(())
}

/// ノードトークンを生成（`nt_` + UUID）
fn generate_node_token() -> String {
    let uuid = Uuid::new_v4();
    format!("nt_{}", uuid)
}

/// SHA-256ハッシュ化ヘルパー関数
fn hash_with_sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}

// SQLiteからの行取得用の内部型
#[derive(sqlx::FromRow)]
struct NodeTokenRow {
    node_id: String,
    token_hash: String,
    created_at: String,
}

impl NodeTokenRow {
    fn into_node_token(self) -> NodeToken {
        let node_id = Uuid::parse_str(&self.node_id).unwrap();
        let created_at = DateTime::parse_from_rfc3339(&self.created_at)
            .unwrap()
            .with_timezone(&Utc);

        NodeToken {
            node_id,
            token_hash: self.token_hash,
            created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::initialize_database;

    async fn setup_test_db() -> SqlitePool {
        initialize_database("sqlite::memory:")
            .await
            .expect("Failed to initialize test database")
    }

    #[tokio::test]
    async fn test_generate_node_token() {
        let token = generate_node_token();
        assert!(token.starts_with("nt_"));
        // "nt_" + UUID（36文字）
        assert_eq!(token.len(), 3 + 36);
    }

    #[tokio::test]
    async fn test_create_and_find_node_token() {
        let pool = setup_test_db().await;

        let node_id = Uuid::new_v4();
        let token_with_plaintext = create(&pool, node_id)
            .await
            .expect("Failed to create node token");

        assert!(token_with_plaintext.token.starts_with("nt_"));
        assert_eq!(token_with_plaintext.node_id, node_id);

        // ハッシュで検索
        let token_hash = hash_with_sha256(&token_with_plaintext.token);
        let found = find_by_hash(&pool, &token_hash)
            .await
            .expect("Failed to find node token");

        assert!(found.is_some());
        let found_token = found.unwrap();
        assert_eq!(found_token.node_id, node_id);
    }

    #[tokio::test]
    async fn test_delete_node_token() {
        let pool = setup_test_db().await;
        let node_id = Uuid::new_v4();
        let token_with_plaintext = create(&pool, node_id).await.unwrap();

        // 削除
        delete(&pool, node_id)
            .await
            .expect("Failed to delete node token");

        // 削除後は見つからない
        let token_hash = hash_with_sha256(&token_with_plaintext.token);
        let found = find_by_hash(&pool, &token_hash).await.unwrap();
        assert!(found.is_none());
    }
}
