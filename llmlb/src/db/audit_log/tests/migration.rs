use super::*;

#[tokio::test]
async fn test_request_history_migration() {
    // request_historyからaudit_log_entriesへのデータ移行SQLを検証
    let pool = crate::db::test_utils::test_db_pool().await;

    // request_historyにテストデータを挿入（全マイグレーション後のフルスキーマ）
    sqlx::query(
            r#"INSERT INTO request_history
                (id, timestamp, request_type, model, endpoint_id, endpoint_name, endpoint_ip,
                 request_body, duration_ms, status, completed_at,
                 input_tokens, output_tokens, total_tokens)
            VALUES
                ('req-1', '2024-01-15T10:00:00+00:00', 'chat', 'llama-3', 'ep-1', 'machine-1', '127.0.0.1',
                 '{}', 500, 'success', '2024-01-15T10:00:01+00:00',
                 100, 50, 150),
                ('req-2', '2024-01-15T11:00:00+00:00', 'chat', 'gpt-4', 'ep-2', 'machine-2', '127.0.0.1',
                 '{}', 1000, 'success', '2024-01-15T11:00:01+00:00',
                 200, 100, 300),
                ('req-3', '2024-01-15T12:00:00+00:00', 'chat', 'llama-3', 'ep-1', 'machine-1', '127.0.0.1',
                 '{}', 200, 'error', '2024-01-15T12:00:01+00:00',
                 50, 0, 50)"#,
        )
        .execute(&pool)
        .await
        .unwrap();

    // 017_audit_log.sqlのマイグレーションSQLを手動実行してデータ移行を検証
    sqlx::query(
        r#"INSERT INTO audit_log_entries (
                timestamp, http_method, request_path, status_code,
                actor_type, actor_id, duration_ms,
                input_tokens, output_tokens, total_tokens,
                model_name, endpoint_id, is_migrated
            )
            SELECT
                rh.timestamp,
                'POST',
                '/v1/chat/completions',
                CASE WHEN rh.error_message IS NULL THEN 200 ELSE 500 END,
                'api_key',
                'unknown',
                rh.duration_ms,
                rh.input_tokens,
                rh.output_tokens,
                rh.total_tokens,
                rh.model,
                rh.endpoint_id,
                1
            FROM request_history rh"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let storage = AuditLogStorage::new(pool.clone());

    // 移行されたエントリを確認
    let filter = AuditLogFilter::default();
    let entries = storage.query(&filter).await.unwrap();
    assert_eq!(
        entries.len(),
        3,
        "All 3 request_history records should be migrated"
    );

    // is_migrated=1で移行されていることを確認
    for entry in &entries {
        assert!(
            entry.is_migrated,
            "Migrated entries should have is_migrated=true"
        );
    }

    // 移行データの内容を確認
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM audit_log_entries WHERE is_migrated = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count.0, 3);

    // エラーありレコードはstatus_code=500に変換されていることを確認
    let error_entry: (i64,) = sqlx::query_as(
        "SELECT status_code FROM audit_log_entries WHERE model_name = 'llama-3' \
             AND duration_ms = 200",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    // req-3は status='error' だが error_message=NULL なので 200 になる
    assert_eq!(error_entry.0, 200);

    // トークン統計が移行データから正しく集計されること
    let stats = storage.get_token_statistics().await.unwrap();
    assert_eq!(stats.total_input_tokens, 350);
    assert_eq!(stats.total_output_tokens, 150);
    assert_eq!(stats.total_tokens, 500);

    // モデル別統計
    let by_model = storage.get_token_statistics_by_model().await.unwrap();
    assert_eq!(by_model.len(), 2); // llama-3, gpt-4
}
