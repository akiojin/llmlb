use super::*;

#[tokio::test]
async fn test_insert_batch_and_query() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![
        make_entry("GET", "/v1/models", 200, ActorType::User),
        make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey),
    ];

    storage.insert_batch(&entries).await.unwrap();

    let filter = AuditLogFilter::default();
    let results = storage.query(&filter).await.unwrap();
    assert_eq!(results.len(), 2);

    // ORDER BY timestamp DESC なので最新が先
    assert!(results[0].id.is_some());
    assert!(results[1].id.is_some());
}

#[tokio::test]
async fn test_query_with_actor_type_filter() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![
        make_entry("GET", "/v1/models", 200, ActorType::User),
        make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey),
        make_entry("GET", "/v1/models", 401, ActorType::Anonymous),
    ];

    storage.insert_batch(&entries).await.unwrap();

    let filter = AuditLogFilter {
        actor_type: Some("user".to_string()),
        ..Default::default()
    };
    let results = storage.query(&filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].actor_type, ActorType::User);
}

#[tokio::test]
async fn test_query_with_pagination() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let mut entries = Vec::new();
    for i in 0..5 {
        let mut entry = make_entry("GET", "/v1/models", 200, ActorType::User);
        entry.duration_ms = Some(i);
        entries.push(entry);
    }

    storage.insert_batch(&entries).await.unwrap();

    // ページ1: 2件
    let filter = AuditLogFilter {
        page: Some(1),
        per_page: Some(2),
        ..Default::default()
    };
    let page1 = storage.query(&filter).await.unwrap();
    assert_eq!(page1.len(), 2);

    // ページ2: 2件
    let filter = AuditLogFilter {
        page: Some(2),
        per_page: Some(2),
        ..Default::default()
    };
    let page2 = storage.query(&filter).await.unwrap();
    assert_eq!(page2.len(), 2);

    // ページ3: 1件
    let filter = AuditLogFilter {
        page: Some(3),
        per_page: Some(2),
        ..Default::default()
    };
    let page3 = storage.query(&filter).await.unwrap();
    assert_eq!(page3.len(), 1);
}

#[tokio::test]
async fn test_count() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![
        make_entry("GET", "/v1/models", 200, ActorType::User),
        make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey),
        make_entry("GET", "/v1/models", 401, ActorType::Anonymous),
    ];

    storage.insert_batch(&entries).await.unwrap();

    let total = storage.count(&AuditLogFilter::default()).await.unwrap();
    assert_eq!(total, 3);

    let user_count = storage
        .count(&AuditLogFilter {
            actor_type: Some("user".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(user_count, 1);
}

#[tokio::test]
async fn test_get_by_id() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![make_entry(
        "POST",
        "/v1/chat/completions",
        200,
        ActorType::ApiKey,
    )];
    storage.insert_batch(&entries).await.unwrap();

    let all = storage.query(&AuditLogFilter::default()).await.unwrap();
    let id = all[0].id.unwrap();

    let found = storage.get_by_id(id).await.unwrap();
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(found.http_method, "POST");
    assert_eq!(found.request_path, "/v1/chat/completions");
    assert_eq!(found.status_code, 200);
    assert_eq!(found.actor_type, ActorType::ApiKey);

    // 存在しないID
    let not_found = storage.get_by_id(99999).await.unwrap();
    assert!(not_found.is_none());
}

#[tokio::test]
async fn test_insert_batch_empty() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    // 空配列の場合はエラーにならない
    storage.insert_batch(&[]).await.unwrap();

    let total = storage.count(&AuditLogFilter::default()).await.unwrap();
    assert_eq!(total, 0);
}

#[tokio::test]
async fn test_count_by_method() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![
        make_entry("GET", "/v1/models", 200, ActorType::User),
        make_entry("GET", "/v1/models", 200, ActorType::User),
        make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey),
        make_entry("DELETE", "/api/endpoints/1", 200, ActorType::User),
    ];
    storage.insert_batch(&entries).await.unwrap();

    let counts = storage.count_by_method().await.unwrap();
    assert!(!counts.is_empty());
    // GET=2 が最多
    assert_eq!(counts[0].0, "GET");
    assert_eq!(counts[0].1, 2);
}

#[tokio::test]
async fn test_count_by_actor_type() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![
        make_entry("GET", "/v1/models", 200, ActorType::User),
        make_entry("GET", "/v1/models", 200, ActorType::User),
        make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey),
    ];
    storage.insert_batch(&entries).await.unwrap();

    let counts = storage.count_by_actor_type().await.unwrap();
    assert!(!counts.is_empty());
    assert_eq!(counts[0].0, "user");
    assert_eq!(counts[0].1, 2);
}

#[tokio::test]
async fn test_query_with_http_method_filter() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![
        make_entry("GET", "/v1/models", 200, ActorType::User),
        make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey),
        make_entry("DELETE", "/api/endpoints/1", 204, ActorType::User),
    ];
    storage.insert_batch(&entries).await.unwrap();

    let filter = AuditLogFilter {
        http_method: Some("POST".to_string()),
        ..Default::default()
    };
    let results = storage.query(&filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].http_method, "POST");
}

#[tokio::test]
async fn test_query_with_status_code_filter() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![
        make_entry("GET", "/v1/models", 200, ActorType::User),
        make_entry("GET", "/v1/models", 401, ActorType::Anonymous),
        make_entry("POST", "/v1/chat/completions", 500, ActorType::ApiKey),
    ];
    storage.insert_batch(&entries).await.unwrap();

    let filter = AuditLogFilter {
        status_code: Some(401),
        ..Default::default()
    };
    let results = storage.query(&filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status_code, 401);
}

#[tokio::test]
async fn test_query_with_request_path_filter() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![
        make_entry("GET", "/v1/models", 200, ActorType::User),
        make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey),
    ];
    storage.insert_batch(&entries).await.unwrap();

    let filter = AuditLogFilter {
        request_path: Some("/v1/models".to_string()),
        ..Default::default()
    };
    let results = storage.query(&filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].request_path, "/v1/models");
}

// =====================================================================
// 追加テスト: DB操作 - query with time range filter
// =====================================================================

#[tokio::test]
async fn test_query_with_time_range_filter() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let now = Utc::now();
    let old_entry = AuditLogEntry {
        timestamp: now - chrono::Duration::hours(5),
        ..make_entry("GET", "/api/old", 200, ActorType::User)
    };
    let new_entry = AuditLogEntry {
        timestamp: now,
        ..make_entry("GET", "/api/new", 200, ActorType::User)
    };
    storage.insert_batch(&[old_entry, new_entry]).await.unwrap();

    // Filter: last 2 hours
    let filter = AuditLogFilter {
        time_from: Some(now - chrono::Duration::hours(2)),
        ..Default::default()
    };
    let results = storage.query(&filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].request_path, "/api/new");
}

// =====================================================================
// 追加テスト: DB操作 - query with actor_id filter
// =====================================================================

#[tokio::test]
async fn test_query_with_actor_id_filter() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let mut entry1 = make_entry("GET", "/api/a", 200, ActorType::User);
    entry1.actor_id = Some("alice".to_string());
    let mut entry2 = make_entry("GET", "/api/b", 200, ActorType::User);
    entry2.actor_id = Some("bob".to_string());
    storage.insert_batch(&[entry1, entry2]).await.unwrap();

    let filter = AuditLogFilter {
        actor_id: Some("alice".to_string()),
        ..Default::default()
    };
    let results = storage.query(&filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].actor_id, Some("alice".to_string()));
}

// =====================================================================
// 追加テスト: DB操作 - count with pagination does not affect count
// =====================================================================

#[tokio::test]
async fn test_count_ignores_pagination() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![
        make_entry("GET", "/a", 200, ActorType::User),
        make_entry("GET", "/b", 200, ActorType::User),
        make_entry("GET", "/c", 200, ActorType::User),
    ];
    storage.insert_batch(&entries).await.unwrap();

    let filter = AuditLogFilter {
        page: Some(1),
        per_page: Some(1),
        ..Default::default()
    };
    let count = storage.count(&filter).await.unwrap();
    assert_eq!(count, 3); // count ignores pagination
}

// =====================================================================
// 追加テスト: DB操作 - get_by_id returns correct fields
// =====================================================================

#[tokio::test]
async fn test_get_by_id_with_all_fields() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let mut entry = make_entry("DELETE", "/api/users/1", 204, ActorType::User);
    entry.actor_id = Some("admin-user".to_string());
    entry.actor_username = Some("Admin".to_string());
    entry.detail = Some("Deleted user 1".to_string());
    entry.input_tokens = None;
    entry.output_tokens = None;
    entry.total_tokens = None;
    entry.model_name = None;
    storage.insert_batch(&[entry]).await.unwrap();

    let all = storage.query(&AuditLogFilter::default()).await.unwrap();
    let id = all[0].id.unwrap();

    let found = storage.get_by_id(id).await.unwrap().unwrap();
    assert_eq!(found.http_method, "DELETE");
    assert_eq!(found.request_path, "/api/users/1");
    assert_eq!(found.status_code, 204);
    assert_eq!(found.actor_id, Some("admin-user".to_string()));
    assert_eq!(found.actor_username, Some("Admin".to_string()));
    assert_eq!(found.detail, Some("Deleted user 1".to_string()));
}

// =====================================================================
// 追加テスト: DB操作 - count_by_method empty db
// =====================================================================

#[tokio::test]
async fn test_count_by_method_empty() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let counts = storage.count_by_method().await.unwrap();
    assert!(counts.is_empty());
}

// =====================================================================
// 追加テスト: DB操作 - count_by_actor_type empty db
// =====================================================================

#[tokio::test]
async fn test_count_by_actor_type_empty() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let counts = storage.count_by_actor_type().await.unwrap();
    assert!(counts.is_empty());
}

// =====================================================================
// 追加テスト: DB操作 - insert_batch multiple times
// =====================================================================

#[tokio::test]
async fn test_insert_batch_multiple_batches() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let batch1 = vec![make_entry("GET", "/a", 200, ActorType::User)];
    let batch2 = vec![
        make_entry("POST", "/b", 201, ActorType::ApiKey),
        make_entry("DELETE", "/c", 204, ActorType::User),
    ];

    storage.insert_batch(&batch1).await.unwrap();
    storage.insert_batch(&batch2).await.unwrap();

    let total = storage.count(&AuditLogFilter::default()).await.unwrap();
    assert_eq!(total, 3);
}

// =====================================================================
// 追加テスト: DB操作 - query with combined filters
// =====================================================================

#[tokio::test]
async fn test_query_combined_actor_type_and_method() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![
        make_entry("GET", "/a", 200, ActorType::User),
        make_entry("POST", "/b", 200, ActorType::User),
        make_entry("GET", "/c", 200, ActorType::ApiKey),
    ];
    storage.insert_batch(&entries).await.unwrap();

    let filter = AuditLogFilter {
        actor_type: Some("user".to_string()),
        http_method: Some("GET".to_string()),
        ..Default::default()
    };
    let results = storage.query(&filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].request_path, "/a");
}

// =====================================================================
// 追加テスト: DB操作 - query pagination boundary
// =====================================================================

#[tokio::test]
async fn test_query_pagination_beyond_last_page() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let entries = vec![
        make_entry("GET", "/a", 200, ActorType::User),
        make_entry("GET", "/b", 200, ActorType::User),
    ];
    storage.insert_batch(&entries).await.unwrap();

    // Page far beyond data
    let filter = AuditLogFilter {
        page: Some(100),
        per_page: Some(10),
        ..Default::default()
    };
    let results = storage.query(&filter).await.unwrap();
    assert!(results.is_empty());
}
