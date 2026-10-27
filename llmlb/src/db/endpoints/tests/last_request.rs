use super::*;
use chrono::{TimeZone, Utc};

#[tokio::test]
async fn last_request_migration_preserves_legacy_endpoint_as_null() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    let mut old = sqlx::migrate!("./migrations");
    old.migrations
        .to_mut()
        .retain(|migration| migration.version < 20261026000000);
    old.run(&pool).await.unwrap();
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO endpoints (id, name, base_url, registered_at) VALUES (?, 'legacy', 'http://localhost', '2026-01-01T00:00:00Z')")
        .bind(id.to_string()).execute(&pool).await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let endpoint = get_endpoint(&pool, id).await.unwrap().unwrap();
    assert!(endpoint.last_request_at.is_none());
    assert!(endpoint.last_seen.is_none());
}

#[tokio::test]
async fn last_request_max_survives_concurrent_starts_stale_edits_and_all_reads() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let registry = crate::registry::endpoints::EndpointRegistry::new(pool.clone())
        .await
        .unwrap();
    let endpoint = Endpoint::new(
        "max-start".into(),
        "http://localhost:9999".into(),
        crate::types::endpoint::EndpointType::Xllm,
    );
    let id = endpoint.id;
    registry.add(endpoint.clone()).await.unwrap();
    let second = Utc.with_ymd_and_hms(2026, 10, 26, 0, 0, 0).unwrap();
    // Cross a fractional precision boundary: .1 must stay newer than .01.
    let newest = second + chrono::Duration::milliseconds(100);
    let mut tasks = Vec::new();
    for millis in [100, 10, 0, 50, 99] {
        let registry = registry.clone();
        tasks.push(tokio::spawn(async move {
            registry
                .record_request_started(id, second + chrono::Duration::milliseconds(millis))
                .await
                .unwrap();
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    registry.record_request_started(id, second).await.unwrap();
    registry.update(endpoint).await.unwrap();
    assert_eq!(
        registry.get(id).await.unwrap().last_request_at,
        Some(newest)
    );
    assert_eq!(
        get_endpoint(&pool, id)
            .await
            .unwrap()
            .unwrap()
            .last_request_at,
        Some(newest)
    );
    assert_eq!(
        find_by_name(&pool, "max-start")
            .await
            .unwrap()
            .unwrap()
            .last_request_at,
        Some(newest)
    );
    let lists = [
        list_endpoints(&pool).await.unwrap(),
        list_endpoints_by_status(&pool, EndpointStatus::Pending)
            .await
            .unwrap(),
        list_endpoints_by_type(&pool, crate::types::endpoint::EndpointType::Xllm)
            .await
            .unwrap(),
        list_endpoints_by_type_and_status(
            &pool,
            crate::types::endpoint::EndpointType::Xllm,
            EndpointStatus::Pending,
        )
        .await
        .unwrap(),
    ];
    for endpoints in lists {
        assert_eq!(endpoints[0].last_request_at, Some(newest));
    }
    let reopened = crate::registry::endpoints::EndpointRegistry::new(pool)
        .await
        .unwrap();
    assert_eq!(
        reopened.get(id).await.unwrap().last_request_at,
        Some(newest)
    );
}
