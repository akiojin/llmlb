use super::*;

#[tokio::test]
async fn test_search_fts_by_path() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![
        make_entry("GET", "/v1/models", 200, ActorType::User),
        make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey),
        make_entry("GET", "/v1/embeddings", 200, ActorType::User),
    ];
    storage.insert_batch(&entries).await.unwrap();

    // "chat"を含むパスを検索
    let results = storage
        .search_fts("chat", &AuditLogFilter::default())
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].request_path, "/v1/chat/completions");
}

#[tokio::test]
async fn test_search_fts_by_actor() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let mut entry1 = make_entry("GET", "/v1/models", 200, ActorType::User);
    entry1.actor_id = Some("alice".to_string());
    entry1.actor_username = Some("Alice Smith".to_string());

    let mut entry2 = make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey);
    entry2.actor_id = Some("bob-key".to_string());
    entry2.actor_username = None;

    storage.insert_batch(&[entry1, entry2]).await.unwrap();

    // actor_usernameで検索
    let results = storage
        .search_fts("Alice", &AuditLogFilter::default())
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].actor_username, Some("Alice Smith".to_string()));

    // actor_idで検索
    let results = storage
        .search_fts("bob", &AuditLogFilter::default())
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].actor_id, Some("bob-key".to_string()));
}

#[tokio::test]
async fn test_search_fts_with_filter() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![
        make_entry("GET", "/v1/models", 200, ActorType::User),
        make_entry("POST", "/v1/models/list", 200, ActorType::ApiKey),
        make_entry("GET", "/v1/models/detail", 401, ActorType::Anonymous),
    ];
    storage.insert_batch(&entries).await.unwrap();

    // "models"で検索（全3件ヒット）
    let results = storage
        .search_fts("models", &AuditLogFilter::default())
        .await
        .unwrap();
    assert_eq!(results.len(), 3);

    // "models"で検索 + actor_typeフィルタ
    let filter = AuditLogFilter {
        actor_type: Some("user".to_string()),
        ..Default::default()
    };
    let results = storage.search_fts("models", &filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].actor_type, ActorType::User);
}

#[tokio::test]
async fn test_search_fts_no_results() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![make_entry("GET", "/v1/models", 200, ActorType::User)];
    storage.insert_batch(&entries).await.unwrap();

    let results = storage
        .search_fts("nonexistent", &AuditLogFilter::default())
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_search_fts_basic() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let mut entry1 = make_entry("GET", "/api/users", 200, ActorType::User);
    entry1.actor_username = Some("admin".to_string());
    let entry2 = make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey);
    let mut entry3 = make_entry("DELETE", "/api/endpoints", 200, ActorType::User);
    entry3.actor_username = Some("admin".to_string());

    storage
        .insert_batch(&[entry1, entry2, entry3])
        .await
        .unwrap();

    let filter = AuditLogFilter::default();

    // "users" でマッチするのは1件
    let results = storage.search_fts("users", &filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].request_path, "/api/users");

    // "completions" でマッチするのは1件
    let results = storage.search_fts("completions", &filter).await.unwrap();
    assert_eq!(results.len(), 1);

    // "api" でマッチするのは2件（/api/users, /api/endpoints）
    let results = storage.search_fts("api", &filter).await.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_count_fts() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![
        make_entry("GET", "/v1/models", 200, ActorType::User),
        make_entry("POST", "/v1/models/list", 200, ActorType::ApiKey),
        make_entry("GET", "/v1/chat/completions", 200, ActorType::User),
    ];
    storage.insert_batch(&entries).await.unwrap();

    // "models" → 2件
    let count = storage
        .count_fts("models", &AuditLogFilter::default())
        .await
        .unwrap();
    assert_eq!(count, 2);

    // "models" + actor_typeフィルタ → 1件
    let filter = AuditLogFilter {
        actor_type: Some("user".to_string()),
        ..Default::default()
    };
    let count = storage.count_fts("models", &filter).await.unwrap();
    assert_eq!(count, 1);

    // 空クエリ → 0
    let count = storage
        .count_fts("", &AuditLogFilter::default())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_sanitize_fts_query() {
    // 通常の単語
    assert_eq!(sanitize_fts_query("hello"), "\"hello\"");

    // 複数単語
    assert_eq!(sanitize_fts_query("hello world"), "\"hello\" \"world\"");

    // 特殊文字のエスケープ
    assert_eq!(sanitize_fts_query("he\"llo"), "\"hello\"");

    // 空文字列
    assert_eq!(sanitize_fts_query(""), "");

    // 空白のみ
    assert_eq!(sanitize_fts_query("   "), "");
}

#[test]
fn test_sanitize_fts_query_special_chars() {
    // Double quotes are stripped
    assert_eq!(sanitize_fts_query("te\"st"), "\"test\"");
    // Multiple words with quotes
    assert_eq!(sanitize_fts_query("he\"llo wor\"ld"), "\"hello\" \"world\"");
}

#[tokio::test]
async fn test_count_fts_empty_query() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let count = storage
        .count_fts("", &AuditLogFilter::default())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

// =====================================================================
// 追加テスト: sanitize_fts_query edge cases
// =====================================================================

#[test]
fn test_sanitize_fts_query_only_quotes() {
    // A word that is only quotes should be filtered out
    assert_eq!(sanitize_fts_query("\"\"\""), "");
}

#[test]
fn test_sanitize_fts_query_tabs_and_newlines() {
    // Whitespace characters are split by split_whitespace
    assert_eq!(
        sanitize_fts_query("hello\tworld\nnew"),
        "\"hello\" \"world\" \"new\""
    );
}

// =====================================================================
// 追加テスト: DB操作 - search_fts empty query
// =====================================================================

#[tokio::test]
async fn test_search_fts_empty_query() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![make_entry("GET", "/v1/models", 200, ActorType::User)];
    storage.insert_batch(&entries).await.unwrap();

    let results = storage
        .search_fts("", &AuditLogFilter::default())
        .await
        .unwrap();
    assert!(results.is_empty());
}

// =====================================================================
// 追加テスト: DB操作 - search_fts whitespace-only query
// =====================================================================

#[tokio::test]
async fn test_search_fts_whitespace_query() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![make_entry("GET", "/v1/models", 200, ActorType::User)];
    storage.insert_batch(&entries).await.unwrap();

    let results = storage
        .search_fts("   ", &AuditLogFilter::default())
        .await
        .unwrap();
    assert!(results.is_empty());
}
