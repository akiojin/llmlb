use super::*;

// =====================================================================
// 追加テスト: client_ip LIKE prefix filtering
// =====================================================================

#[tokio::test]
async fn test_query_with_client_ip_prefix_filter_ipv4() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let mut entry1 = make_entry("GET", "/v1/models", 200, ActorType::User);
    entry1.client_ip = Some("192.168.1.1".to_string());

    let mut entry2 = make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey);
    entry2.client_ip = Some("192.168.2.1".to_string());

    let mut entry3 = make_entry("GET", "/v1/models", 200, ActorType::User);
    entry3.client_ip = Some("10.0.0.1".to_string());

    storage
        .insert_batch(&[entry1, entry2, entry3])
        .await
        .unwrap();

    // "192.168.1" で前方一致フィルタ
    let filter = AuditLogFilter {
        client_ip: Some("192.168.1".to_string()),
        ..Default::default()
    };
    let results = storage.query(&filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].client_ip, Some("192.168.1.1".to_string()));

    // "192.168" で前方一致フィルタ（2件マッチ）
    let filter = AuditLogFilter {
        client_ip: Some("192.168".to_string()),
        ..Default::default()
    };
    let results = storage.query(&filter).await.unwrap();
    assert_eq!(results.len(), 2);

    // "10.0" で前方一致フィルタ
    let filter = AuditLogFilter {
        client_ip: Some("10.0".to_string()),
        ..Default::default()
    };
    let results = storage.query(&filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].client_ip, Some("10.0.0.1".to_string()));
}

#[tokio::test]
async fn test_query_with_client_ip_prefix_filter_ipv6() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let mut entry1 = make_entry("GET", "/v1/models", 200, ActorType::User);
    entry1.client_ip = Some("2001:db8::1".to_string());

    let mut entry2 = make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey);
    entry2.client_ip = Some("2001:db9::1".to_string());

    let mut entry3 = make_entry("GET", "/v1/models", 200, ActorType::User);
    entry3.client_ip = Some("::1".to_string());

    storage
        .insert_batch(&[entry1, entry2, entry3])
        .await
        .unwrap();

    // "2001:db8" で前方一致フィルタ
    let filter = AuditLogFilter {
        client_ip: Some("2001:db8".to_string()),
        ..Default::default()
    };
    let results = storage.query(&filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].client_ip, Some("2001:db8::1".to_string()));

    // "::" で前方一致フィルタ（"::1" のみマッチ）
    let filter = AuditLogFilter {
        client_ip: Some("::".to_string()),
        ..Default::default()
    };
    let results = storage.query(&filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].client_ip, Some("::1".to_string()));
}

#[tokio::test]
async fn test_query_with_client_ip_and_other_filters() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let mut entry1 = make_entry("GET", "/v1/models", 200, ActorType::User);
    entry1.client_ip = Some("192.168.1.1".to_string());

    let mut entry2 = make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey);
    entry2.client_ip = Some("192.168.1.2".to_string());

    let mut entry3 = make_entry("GET", "/v1/models", 200, ActorType::User);
    entry3.client_ip = Some("10.0.0.1".to_string());

    storage
        .insert_batch(&[entry1, entry2, entry3])
        .await
        .unwrap();

    // client_ip + actor_type フィルタ
    let filter = AuditLogFilter {
        client_ip: Some("192.168.1".to_string()),
        actor_type: Some("user".to_string()),
        ..Default::default()
    };
    let results = storage.query(&filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].actor_type, ActorType::User);
    assert_eq!(results[0].client_ip, Some("192.168.1.1".to_string()));

    // client_ip + http_method フィルタ
    let filter = AuditLogFilter {
        client_ip: Some("192.168.1".to_string()),
        http_method: Some("POST".to_string()),
        ..Default::default()
    };
    let results = storage.query(&filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].http_method, "POST");
    assert_eq!(results[0].client_ip, Some("192.168.1.2".to_string()));
}

#[tokio::test]
async fn test_count_with_client_ip_filter() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let mut entry1 = make_entry("GET", "/v1/models", 200, ActorType::User);
    entry1.client_ip = Some("192.168.1.1".to_string());

    let mut entry2 = make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey);
    entry2.client_ip = Some("192.168.2.1".to_string());

    let mut entry3 = make_entry("GET", "/v1/models", 200, ActorType::User);
    entry3.client_ip = Some("10.0.0.1".to_string());

    storage
        .insert_batch(&[entry1, entry2, entry3])
        .await
        .unwrap();

    let total = storage.count(&AuditLogFilter::default()).await.unwrap();
    assert_eq!(total, 3);

    let count = storage
        .count(&AuditLogFilter {
            client_ip: Some("192.168".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(count, 2);

    let count = storage
        .count(&AuditLogFilter {
            client_ip: Some("10.0".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(count, 1);

    let count = storage
        .count(&AuditLogFilter {
            client_ip: Some("1.1.1".to_string()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn test_search_fts_with_client_ip() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let mut entry1 = make_entry("GET", "/v1/models", 200, ActorType::User);
    entry1.client_ip = Some("192.168.1.10".to_string());
    entry1.actor_username = Some("alice".to_string());

    let mut entry2 = make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey);
    entry2.client_ip = Some("10.0.0.5".to_string());

    let mut entry3 = make_entry("DELETE", "/api/endpoints", 200, ActorType::User);
    entry3.client_ip = Some("192.168.2.20".to_string());
    entry3.actor_username = Some("bob".to_string());

    storage
        .insert_batch(&[entry1, entry2, entry3])
        .await
        .unwrap();

    let filter = AuditLogFilter::default();

    // "192.168" で2件マッチ（192.168.1.10と192.168.2.20）
    let results = storage.search_fts("192.168", &filter).await.unwrap();
    assert_eq!(results.len(), 2);

    // "10.0.0" で1件マッチ（10.0.0.5）
    let results = storage.search_fts("10.0.0", &filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].client_ip.as_deref(), Some("10.0.0.5"));

    // "alice" で1件マッチ（actor_usernameで検索）
    let results = storage.search_fts("alice", &filter).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].actor_username.as_deref(), Some("alice"));
}

#[tokio::test]
async fn test_count_fts_with_client_ip() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let mut entry1 = make_entry("GET", "/v1/models", 200, ActorType::User);
    entry1.client_ip = Some("203.0.113.1".to_string());

    let mut entry2 = make_entry("POST", "/v1/chat/completions", 200, ActorType::ApiKey);
    entry2.client_ip = Some("203.0.113.2".to_string());

    let mut entry3 = make_entry("GET", "/api/endpoints", 200, ActorType::User);
    entry3.client_ip = Some("198.51.100.1".to_string());

    storage
        .insert_batch(&[entry1, entry2, entry3])
        .await
        .unwrap();

    // "203.0.113" で2件
    let count = storage
        .count_fts("203.0.113", &AuditLogFilter::default())
        .await
        .unwrap();
    assert_eq!(count, 2);

    // "198.51.100" で1件
    let count = storage
        .count_fts("198.51.100", &AuditLogFilter::default())
        .await
        .unwrap();
    assert_eq!(count, 1);

    // "999" で0件
    let count = storage
        .count_fts("999", &AuditLogFilter::default())
        .await
        .unwrap();
    assert_eq!(count, 0);
}
