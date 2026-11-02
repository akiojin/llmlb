use super::*;

async fn state_and_user() -> (AppState, crate::common::auth::User) {
    let state = crate::db::test_utils::TestAppStateBuilder::new()
        .await
        .build()
        .await;
    let user = crate::db::users::create(
        &state.db_pool,
        "current-identity",
        "hash",
        UserRole::Viewer,
        false,
    )
    .await
    .unwrap();
    (state, user)
}

fn claims_for(id: Uuid) -> Claims {
    Claims {
        sub: id.to_string(),
        role: UserRole::Admin,
        exp: (Utc::now() + chrono::Duration::hours(1)).timestamp() as usize,
        must_change_password: false,
        password_changed_at: 0,
    }
}

#[tokio::test]
async fn current_identity_rejects_deleted_unknown_and_malformed_subjects() {
    let (state, user) = state_and_user().await;
    crate::db::users::delete(&state.db_pool, user.id)
        .await
        .unwrap();
    for subject in [
        user.id.to_string(),
        Uuid::new_v4().to_string(),
        Uuid::nil().to_string(),
        "not-a-uuid".into(),
    ] {
        let mut claims = claims_for(user.id);
        claims.sub = subject;
        assert!(
            enforce_session_not_revoked(&state.db_pool, &claims)
                .await
                .is_err(),
            "unknown principal accepted: {}",
            claims.sub
        );
    }
}

#[tokio::test]
async fn current_identity_database_failure_is_closed() {
    let (state, user) = state_and_user().await;
    state.db_pool.close().await;
    assert!(
        enforce_session_not_revoked(&state.db_pool, &claims_for(user.id))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn current_identity_uses_database_role_instead_of_stale_jwt_role() {
    let (state, user) = state_and_user().await;
    let jwt = crate::auth::jwt::create_jwt(
        &user.id.to_string(),
        UserRole::Admin,
        &state.auth.jwt_secret,
        false,
        0,
    )
    .unwrap();
    let app = Router::new()
        .route("/private", get(|| async { "private" }))
        .layer(axum_middleware::from_fn(require_admin_role_middleware))
        .layer(axum_middleware::from_fn_with_state(
            state,
            require_jwt_auth_middleware,
        ));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/private")
                .header("authorization", format!("Bearer {jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn current_identity_still_accepts_existing_current_session() {
    let (state, user) = state_and_user().await;
    assert!(
        enforce_session_not_revoked(&state.db_pool, &claims_for(user.id))
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn current_identity_early_role_denial_reports_current_scope_for_cache_invalidation() {
    let (state, user) = state_and_user().await;
    let jwt = crate::auth::jwt::create_jwt(
        &user.id.to_string(),
        UserRole::Admin,
        &state.auth.jwt_secret,
        false,
        0,
    )
    .unwrap();
    let config = JwtOrApiKeyPermissionConfig {
        app_state: state,
        required_permission: ApiKeyPermission::UsersManage,
        jwt_required_role: Some(UserRole::Admin),
        api_key_role: UserRole::Admin,
    };
    let app = Router::new()
        .route("/private", get(|| async { "private" }))
        .layer(axum_middleware::from_fn_with_state(
            config,
            jwt_or_api_key_permission_middleware,
        ));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/private")
                .header("authorization", format!("Bearer {jwt}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response
            .headers()
            .get("x-llmlb-scope")
            .and_then(|value| value.to_str().ok()),
        Some(format!("unassigned:{}", user.id).as_str())
    );
    assert_eq!(
        response
            .headers()
            .get("cache-control")
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
    assert_eq!(
        response
            .headers()
            .get("x-llmlb-account")
            .and_then(|value| value.to_str().ok()),
        Some(user.id.to_string().as_str())
    );
}

#[tokio::test]
async fn current_identity_actor_header_distinguishes_accounts_with_the_same_global_scope() {
    let (state, _) = state_and_user().await;
    for name in ["first-global", "second-global"] {
        let user = crate::db::users::create(&state.db_pool, name, "hash", UserRole::Admin, false)
            .await
            .unwrap();
        let jwt = crate::auth::jwt::create_jwt(
            &user.id.to_string(),
            UserRole::Admin,
            &state.auth.jwt_secret,
            false,
            0,
        )
        .unwrap();
        let app = Router::new()
            .route("/private", get(|| async { "ok" }))
            .layer(axum_middleware::from_fn_with_state(
                state.clone(),
                require_jwt_auth_middleware,
            ));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/private")
                    .header("authorization", format!("Bearer {jwt}"))
                    .header("x-llmlb-account", Uuid::new_v4().to_string())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("x-llmlb-scope")
                .and_then(|value| value.to_str().ok()),
            Some("global")
        );
        assert_eq!(
            response
                .headers()
                .get("x-llmlb-account")
                .and_then(|value| value.to_str().ok()),
            Some(user.id.to_string().as_str())
        );
    }
}
