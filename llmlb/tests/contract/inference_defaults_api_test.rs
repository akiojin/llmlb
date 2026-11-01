//! Issue #950: dedicated inference-default routes retain their strict admin contract.
//!
//! The standalone inference_defaults_api_test binary covers storage failures and
//! the full validation matrix; these tests bind the wire contract to this suite.

use super::api_route_coverage_test::{response_json, send_request};
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use llmlb::{common::auth::UserRole, db::settings::SettingsStorage};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tower::ServiceExt;

const URI: &str = "/api/dashboard/inference-defaults";
const MODEL: &str = "inference-default-contract-text";

async fn jwt(pool: &SqlitePool, name: &str, role: UserRole, must_change: bool) -> String {
    let user = llmlb::db::users::create(pool, name, "unused", role, must_change)
        .await
        .unwrap();
    llmlb::auth::jwt::create_jwt(
        &user.id.to_string(),
        role,
        &crate::support::lb::test_jwt_secret(),
        must_change,
        user.password_changed_at,
    )
    .unwrap()
}

async fn fixture() -> (Router, SqlitePool, String) {
    let pool = crate::support::lb::create_test_db_pool().await;
    llmlb::db::models::ModelStorage::new(pool.clone())
        .save_model(&llmlb::registry::models::ModelInfo::new(
            MODEL.into(),
            1,
            "contract model".into(),
            1,
            vec![],
        ))
        .await
        .unwrap();
    let admin = jwt(
        &pool,
        "inference-default-contract-admin",
        UserRole::Admin,
        false,
    )
    .await;
    let app = crate::support::lb::build_test_router(pool.clone()).await;
    (app, pool, admin)
}

async fn assert_get(app: &Router, admin: &str, expected: Value) {
    let response = send_request(app, Method::GET, URI, Some(admin), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response_json(response).await,
        expected,
        "wire response has only the nullable public model"
    );
}

#[tokio::test]
async fn get_inference_defaults_requires_admin_and_returns_the_public_nullable_shape() {
    let (app, pool, admin) = fixture().await;
    let viewer = jwt(
        &pool,
        "inference-default-contract-viewer",
        UserRole::Viewer,
        false,
    )
    .await;
    let password_change = jwt(
        &pool,
        "inference-default-contract-password",
        UserRole::Admin,
        true,
    )
    .await;
    for (token, status) in [
        (None, StatusCode::UNAUTHORIZED),
        (Some("sk_debug"), StatusCode::UNAUTHORIZED),
        (Some(viewer.as_str()), StatusCode::FORBIDDEN),
        (Some(password_change.as_str()), StatusCode::FORBIDDEN),
    ] {
        let response = send_request(&app, Method::GET, URI, token, None).await;
        assert_eq!(response.status(), status);
    }
    assert_get(&app, &admin, json!({"default_text_model":null})).await;
    SettingsStorage::new(pool)
        .set_setting(
            "inference.defaults",
            &json!({"version":1,"default_text_model":MODEL}).to_string(),
        )
        .await
        .unwrap();
    assert_get(&app, &admin, json!({"default_text_model":MODEL})).await;
}

#[tokio::test]
async fn put_inference_defaults_requires_the_field_and_keeps_old_value_on_invalid_input() {
    let (app, pool, admin) = fixture().await;
    let desired = json!({"default_text_model":MODEL});
    let response = send_request(&app, Method::PUT, URI, Some(&admin), Some(desired.clone())).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await, desired);
    assert_get(&app, &admin, desired.clone()).await;
    let stored = SettingsStorage::new(pool)
        .get_setting("inference.defaults")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&stored).unwrap(),
        json!({"version":1,"default_text_model":MODEL})
    );
    for invalid in [
        json!({}),
        json!(null),
        json!([]),
        json!({"default_text_model":null,"unknown":true}),
        json!({"default_text_model":7}),
        json!({"default_text_model":""}),
        json!({"default_text_model":format!(" {MODEL}")}),
        json!({"default_text_model":"unknown-contract-model"}),
    ] {
        let response =
            send_request(&app, Method::PUT, URI, Some(&admin), Some(invalid.clone())).await;
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "invalid body: {invalid}"
        );
        let error = response_json(response).await;
        assert!(
            error["error"].is_string(),
            "existing management error shape: {error}"
        );
        assert_get(&app, &admin, desired.clone()).await;
    }
    let cleared = json!({"default_text_model":null});
    let response = send_request(&app, Method::PUT, URI, Some(&admin), Some(cleared.clone())).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response_json(response).await, cleared);
    assert_get(&app, &admin, cleared).await;
}

#[tokio::test]
async fn put_inference_defaults_preserves_jwt_role_password_and_cookie_csrf_boundaries() {
    let (app, pool, admin) = fixture().await;
    let viewer = jwt(
        &pool,
        "inference-default-put-viewer",
        UserRole::Viewer,
        false,
    )
    .await;
    let password_change = jwt(
        &pool,
        "inference-default-put-password",
        UserRole::Admin,
        true,
    )
    .await;
    for (token, status) in [
        (None, StatusCode::UNAUTHORIZED),
        (Some("sk_debug"), StatusCode::UNAUTHORIZED),
        (Some(viewer.as_str()), StatusCode::FORBIDDEN),
        (Some(password_change.as_str()), StatusCode::FORBIDDEN),
    ] {
        let response = send_request(
            &app,
            Method::PUT,
            URI,
            token,
            Some(json!({"default_text_model":MODEL})),
        )
        .await;
        assert_eq!(response.status(), status);
        assert_get(&app, &admin, json!({"default_text_model":null})).await;
    }
    for with_csrf in [false, true] {
        let mut request = Request::builder()
            .method(Method::PUT)
            .uri(URI)
            .header("content-type", "application/json")
            .header("host", "localhost")
            .header("origin", "http://localhost")
            .header(
                "cookie",
                format!("llmlb_jwt={admin}; llmlb_csrf=contract-token"),
            );
        if with_csrf {
            request = request.header("x-csrf-token", "contract-token");
        }
        let response = app
            .clone()
            .oneshot(
                request
                    .body(Body::from(json!({"default_text_model":MODEL}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            if with_csrf {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            }
        );
        assert_get(
            &app,
            &admin,
            json!({"default_text_model": if with_csrf { Some(MODEL) } else { None }}),
        )
        .await;
    }
}
