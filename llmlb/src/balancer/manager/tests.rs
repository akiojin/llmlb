use super::*;
use crate::balancer::types::ModelTpsState;
use crate::db::test_utils::TEST_LOCK;
use crate::types::endpoint::{Endpoint, EndpointModel, EndpointStatus, EndpointType, SupportedAPI};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::time::{sleep, Duration};

mod history;
mod load_state;
mod model_tps;
mod request_lifecycle;
mod selection;
mod snapshots;

async fn setup_test_load_manager() -> (LoadManager, Uuid) {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create test database");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let registry = EndpointRegistry::new(pool)
        .await
        .expect("Failed to create endpoint registry");
    let endpoint = Endpoint::new(
        "lease-test-endpoint".to_string(),
        "http://localhost:11434".to_string(),
        EndpointType::OpenaiCompatible,
    );
    let endpoint_id = endpoint.id;
    registry
        .add(endpoint)
        .await
        .expect("Failed to add test endpoint");

    let load_manager = LoadManager::new(Arc::new(registry));
    (load_manager, endpoint_id)
}
