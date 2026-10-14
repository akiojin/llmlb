use super::*;

#[tokio::test]
async fn test_get_token_statistics() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let now = Utc::now();
    let entries = vec![
        make_token_entry("model-a", now, 100, 50, 150),
        make_token_entry("model-b", now, 200, 100, 300),
        make_token_entry("model-a", now, 50, 25, 75),
    ];
    storage.insert_batch(&entries).await.unwrap();

    let stats = storage.get_token_statistics().await.unwrap();
    assert_eq!(stats.total_input_tokens, 350);
    assert_eq!(stats.total_output_tokens, 175);
    assert_eq!(stats.total_tokens, 525);
}

#[tokio::test]
async fn test_get_token_statistics_infers_total_when_total_tokens_null() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let now = Utc::now();
    let entries = vec![
        make_token_entry("model-a", now, 100, 50, 150),
        make_token_entry_without_total("model-a", now, 70, 30),
    ];
    storage.insert_batch(&entries).await.unwrap();

    let stats = storage.get_token_statistics().await.unwrap();
    assert_eq!(stats.total_input_tokens, 170);
    assert_eq!(stats.total_output_tokens, 80);
    assert_eq!(stats.total_tokens, 250);
}

#[tokio::test]
async fn test_get_token_statistics_by_model() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let now = Utc::now();
    let entries = vec![
        make_token_entry("model-a", now, 100, 50, 150),
        make_token_entry("model-b", now, 200, 100, 300),
        make_token_entry("model-a", now, 50, 25, 75),
    ];
    storage.insert_batch(&entries).await.unwrap();

    let stats = storage.get_token_statistics_by_model().await.unwrap();
    assert_eq!(stats.len(), 2);

    // ORDER BY total_tokens DESC なので model-b が先
    assert_eq!(stats[0].model_name, "model-b");
    assert_eq!(stats[0].total_input_tokens, 200);
    assert_eq!(stats[0].total_output_tokens, 100);
    assert_eq!(stats[0].total_tokens, 300);

    assert_eq!(stats[1].model_name, "model-a");
    assert_eq!(stats[1].total_input_tokens, 150);
    assert_eq!(stats[1].total_output_tokens, 75);
    assert_eq!(stats[1].total_tokens, 225);
}

#[tokio::test]
async fn test_get_daily_token_statistics() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let now = Utc::now();
    let entries = vec![
        make_token_entry("model-a", now, 100, 50, 150),
        make_token_entry("model-b", now, 200, 100, 300),
    ];
    storage.insert_batch(&entries).await.unwrap();

    let stats = storage.get_daily_token_statistics(7).await.unwrap();
    // 同日のエントリなので1日分
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].total_input_tokens, 300);
    assert_eq!(stats[0].total_output_tokens, 150);
    assert_eq!(stats[0].total_tokens, 450);
}

#[tokio::test]
async fn test_get_daily_token_statistics_infers_total_when_total_tokens_null() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let now = Utc::now();
    let entries = vec![
        make_token_entry("model-a", now, 100, 50, 150),
        make_token_entry_without_total("model-b", now, 20, 10),
    ];
    storage.insert_batch(&entries).await.unwrap();

    let stats = storage.get_daily_token_statistics(7).await.unwrap();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].total_input_tokens, 120);
    assert_eq!(stats[0].total_output_tokens, 60);
    assert_eq!(stats[0].total_tokens, 180);
    assert_eq!(stats[0].request_count, 2);
}

#[tokio::test]
async fn test_get_monthly_token_statistics() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let now = Utc::now();
    let entries = vec![
        make_token_entry("model-a", now, 100, 50, 150),
        make_token_entry("model-b", now, 200, 100, 300),
        make_token_entry("model-a", now, 50, 25, 75),
    ];
    storage.insert_batch(&entries).await.unwrap();

    let stats = storage.get_monthly_token_statistics(3).await.unwrap();
    // 同月のエントリなので1月分
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].total_input_tokens, 350);
    assert_eq!(stats[0].total_output_tokens, 175);
    assert_eq!(stats[0].total_tokens, 525);
}

#[tokio::test]
async fn test_get_monthly_token_statistics_infers_total_when_total_tokens_null() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let now = Utc::now();
    let entries = vec![
        make_token_entry("model-a", now, 40, 20, 60),
        make_token_entry_without_total("model-b", now, 10, 5),
    ];
    storage.insert_batch(&entries).await.unwrap();

    let stats = storage.get_monthly_token_statistics(3).await.unwrap();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].total_input_tokens, 50);
    assert_eq!(stats[0].total_output_tokens, 25);
    assert_eq!(stats[0].total_tokens, 75);
    assert_eq!(stats[0].request_count, 2);
}

// --- 追加テスト ---

#[tokio::test]
async fn test_get_token_statistics_empty_db() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let stats = storage.get_token_statistics().await.unwrap();
    assert_eq!(stats.total_input_tokens, 0);
    assert_eq!(stats.total_output_tokens, 0);
    assert_eq!(stats.total_tokens, 0);
}

#[tokio::test]
async fn test_get_token_statistics_by_model_empty() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let stats = storage.get_token_statistics_by_model().await.unwrap();
    assert!(stats.is_empty());
}

// =====================================================================
// 追加テスト: TokenStatistics
// =====================================================================

#[test]
fn test_token_statistics_serialize() {
    let stats = TokenStatistics {
        total_input_tokens: 100,
        total_output_tokens: 50,
        total_tokens: 150,
    };
    let json = serde_json::to_value(&stats).unwrap();
    assert_eq!(json["total_input_tokens"], 100);
    assert_eq!(json["total_output_tokens"], 50);
    assert_eq!(json["total_tokens"], 150);
}

// =====================================================================
// 追加テスト: ModelTokenStatistics
// =====================================================================

#[test]
fn test_model_token_statistics_serialize() {
    let stats = ModelTokenStatistics {
        model_name: "llama".to_string(),
        total_input_tokens: 200,
        total_output_tokens: 100,
        total_tokens: 300,
    };
    let json = serde_json::to_value(&stats).unwrap();
    assert_eq!(json["model_name"], "llama");
    assert_eq!(json["total_tokens"], 300);
}

// =====================================================================
// 追加テスト: DailyTokenStatistics
// =====================================================================

#[test]
fn test_daily_token_statistics_serialize() {
    let stats = DailyTokenStatistics {
        date: "2024-06-15".to_string(),
        total_input_tokens: 500,
        total_output_tokens: 250,
        total_tokens: 750,
        request_count: 10,
    };
    let json = serde_json::to_value(&stats).unwrap();
    assert_eq!(json["date"], "2024-06-15");
    assert_eq!(json["request_count"], 10);
}

// =====================================================================
// 追加テスト: MonthlyTokenStatistics
// =====================================================================

#[test]
fn test_monthly_token_statistics_serialize() {
    let stats = MonthlyTokenStatistics {
        month: "2024-06".to_string(),
        total_input_tokens: 5000,
        total_output_tokens: 2500,
        total_tokens: 7500,
        request_count: 100,
    };
    let json = serde_json::to_value(&stats).unwrap();
    assert_eq!(json["month"], "2024-06");
    assert_eq!(json["request_count"], 100);
}

// =====================================================================
// 追加テスト: DB操作 - daily token statistics empty
// =====================================================================

#[tokio::test]
async fn test_get_daily_token_statistics_empty() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let stats = storage.get_daily_token_statistics(30).await.unwrap();
    assert!(stats.is_empty());
}

// =====================================================================
// 追加テスト: DB操作 - monthly token statistics empty
// =====================================================================

#[tokio::test]
async fn test_get_monthly_token_statistics_empty() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let stats = storage.get_monthly_token_statistics(12).await.unwrap();
    assert!(stats.is_empty());
}

// =====================================================================
// 追加テスト: DB操作 - token statistics with mixed null
// =====================================================================

#[tokio::test]
async fn test_get_token_statistics_all_null_tokens() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let mut entry = make_entry("POST", "/v1/chat", 200, ActorType::ApiKey);
    entry.input_tokens = None;
    entry.output_tokens = None;
    entry.total_tokens = None;
    storage.insert_batch(&[entry]).await.unwrap();

    let stats = storage.get_token_statistics().await.unwrap();
    assert_eq!(stats.total_input_tokens, 0);
    assert_eq!(stats.total_output_tokens, 0);
    assert_eq!(stats.total_tokens, 0);
}

// =====================================================================
// 追加テスト: DB操作 - model stats excludes null model_name
// =====================================================================

#[tokio::test]
async fn test_get_token_statistics_by_model_excludes_null_model() {
    let pool = create_test_pool().await;
    let storage = AuditLogStorage::new(pool);

    let mut entry = make_entry("POST", "/v1/chat", 200, ActorType::ApiKey);
    entry.model_name = None;
    entry.input_tokens = Some(100);
    entry.output_tokens = Some(50);
    entry.total_tokens = Some(150);
    storage.insert_batch(&[entry]).await.unwrap();

    let stats = storage.get_token_statistics_by_model().await.unwrap();
    // model_name IS NULL entries are excluded by WHERE clause
    assert!(stats.is_empty());
}
