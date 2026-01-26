//! 招待コード管理API
//!
//! Admin専用の招待コードCRUD操作

use crate::common::auth::{Claims, UserRole};
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 招待コード発行リクエスト
#[derive(Debug, Deserialize)]
pub struct CreateInvitationRequest {
    /// 有効期限（時間）、デフォルト72時間
    pub expires_in_hours: Option<i64>,
}

/// 招待コード発行レスポンス（平文コード含む）
#[derive(Debug, Serialize)]
pub struct CreateInvitationResponse {
    /// 招待コードID
    pub id: String,
    /// 平文の招待コード（発行時のみ表示）
    pub code: String,
    /// 作成日時
    pub created_at: String,
    /// 有効期限
    pub expires_at: String,
}

/// 招待コードレスポンス（一覧用）
#[derive(Debug, Serialize)]
pub struct InvitationResponse {
    /// 招待コードID
    pub id: String,
    /// 作成者のユーザーID
    pub created_by: String,
    /// 作成日時
    pub created_at: String,
    /// 有効期限
    pub expires_at: String,
    /// ステータス（active/used/revoked）
    pub status: String,
    /// 使用したユーザーID
    pub used_by: Option<String>,
    /// 使用日時
    pub used_at: Option<String>,
}

/// 招待コード一覧レスポンス
#[derive(Debug, Serialize)]
pub struct ListInvitationsResponse {
    /// 招待コード一覧
    pub invitations: Vec<InvitationResponse>,
}

/// Admin権限チェックヘルパー
#[allow(clippy::result_large_err)]
fn check_admin(claims: &Claims) -> Result<(), Response> {
    if claims.role != UserRole::Admin {
        return Err((StatusCode::FORBIDDEN, "Admin access required").into_response());
    }
    Ok(())
}

/// POST /v0/invitations - 招待コード発行
///
/// Admin専用。新しい招待コードを発行する。平文コードは発行時のみ返却
///
/// # Arguments
/// * `Extension(claims)` - JWTクレーム（ミドルウェアで注入）
/// * `State(app_state)` - アプリケーション状態
/// * `Json(request)` - 招待コード発行リクエスト
///
/// # Returns
/// * `201 Created` - 作成された招待コード（平文コード含む）
/// * `403 Forbidden` - Admin権限なし
/// * `500 Internal Server Error` - サーバーエラー
pub async fn create_invitation(
    Extension(claims): Extension<Claims>,
    State(app_state): State<AppState>,
    Json(request): Json<CreateInvitationRequest>,
) -> Result<(StatusCode, Json<CreateInvitationResponse>), Response> {
    check_admin(&claims)?;

    // ユーザーIDをパース
    let user_id = claims.sub.parse::<Uuid>().map_err(|e| {
        tracing::error!("Failed to parse user ID: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
    })?;

    // 招待コードを発行
    let invitation =
        crate::db::invitations::create(&app_state.db_pool, user_id, request.expires_in_hours)
            .await
            .map_err(|e| {
                tracing::error!("Failed to create invitation code: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            })?;

    Ok((
        StatusCode::CREATED,
        Json(CreateInvitationResponse {
            id: invitation.id.to_string(),
            code: invitation.code,
            created_at: invitation.created_at.to_rfc3339(),
            expires_at: invitation.expires_at.to_rfc3339(),
        }),
    ))
}

/// GET /v0/invitations - 招待コード一覧取得
///
/// Admin専用。全招待コードの一覧を返す
///
/// # Arguments
/// * `Extension(claims)` - JWTクレーム（ミドルウェアで注入）
/// * `State(app_state)` - アプリケーション状態
///
/// # Returns
/// * `200 OK` - 招待コード一覧
/// * `403 Forbidden` - Admin権限なし
/// * `500 Internal Server Error` - サーバーエラー
pub async fn list_invitations(
    Extension(claims): Extension<Claims>,
    State(app_state): State<AppState>,
) -> Result<Json<ListInvitationsResponse>, Response> {
    check_admin(&claims)?;

    let invitations = crate::db::invitations::list(&app_state.db_pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list invitation codes: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
        })?;

    Ok(Json(ListInvitationsResponse {
        invitations: invitations
            .into_iter()
            .map(|inv| InvitationResponse {
                id: inv.id.to_string(),
                created_by: inv.created_by.to_string(),
                created_at: inv.created_at.to_rfc3339(),
                expires_at: inv.expires_at.to_rfc3339(),
                status: inv.status.to_string(),
                used_by: inv.used_by.map(|id| id.to_string()),
                used_at: inv.used_at.map(|dt| dt.to_rfc3339()),
            })
            .collect(),
    }))
}

/// DELETE /v0/invitations/:id - 招待コード無効化
///
/// Admin専用。招待コードを無効化（revoke）する
///
/// # Arguments
/// * `Extension(claims)` - JWTクレーム（ミドルウェアで注入）
/// * `State(app_state)` - アプリケーション状態
/// * `Path(id)` - 招待コードID
///
/// # Returns
/// * `204 No Content` - 無効化成功
/// * `403 Forbidden` - Admin権限なし
/// * `404 Not Found` - 招待コードが見つからない（または既に使用済み/無効化済み）
/// * `500 Internal Server Error` - サーバーエラー
pub async fn revoke_invitation(
    Extension(claims): Extension<Claims>,
    State(app_state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, Response> {
    check_admin(&claims)?;

    let revoked = crate::db::invitations::revoke(&app_state.db_pool, id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to revoke invitation code: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
        })?;

    if revoked {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((
            StatusCode::NOT_FOUND,
            "Invitation not found or already used/revoked",
        )
            .into_response())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_invitation_request_deserialize() {
        let json = r#"{"expires_in_hours": 48}"#;
        let request: CreateInvitationRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.expires_in_hours, Some(48));
    }

    #[test]
    fn test_create_invitation_request_default() {
        let json = r#"{}"#;
        let request: CreateInvitationRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.expires_in_hours, None);
    }

    #[test]
    fn test_create_invitation_response_serialize() {
        let response = CreateInvitationResponse {
            id: "test-id".to_string(),
            code: "inv_abcd1234".to_string(),
            created_at: "2025-12-20T15:00:00Z".to_string(),
            expires_at: "2025-12-23T15:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("inv_abcd1234"));
        assert!(json.contains("test-id"));
    }

    #[test]
    fn test_invitation_response_serialize() {
        let response = InvitationResponse {
            id: "id-123".to_string(),
            created_by: "admin-id".to_string(),
            created_at: "2025-12-20T15:00:00Z".to_string(),
            expires_at: "2025-12-23T15:00:00Z".to_string(),
            status: "active".to_string(),
            used_by: None,
            used_at: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("active"));
        assert!(json.contains("admin-id"));
    }

    #[test]
    fn test_invitation_response_with_used() {
        let response = InvitationResponse {
            id: "id-456".to_string(),
            created_by: "admin-id".to_string(),
            created_at: "2025-12-20T15:00:00Z".to_string(),
            expires_at: "2025-12-23T15:00:00Z".to_string(),
            status: "used".to_string(),
            used_by: Some("user-id".to_string()),
            used_at: Some("2025-12-21T10:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("used"));
        assert!(json.contains("user-id"));
    }

    #[test]
    fn test_list_invitations_response_serialize() {
        let response = ListInvitationsResponse {
            invitations: vec![InvitationResponse {
                id: "id-1".to_string(),
                created_by: "admin".to_string(),
                created_at: "2025-12-20T15:00:00Z".to_string(),
                expires_at: "2025-12-23T15:00:00Z".to_string(),
                status: "active".to_string(),
                used_by: None,
                used_at: None,
            }],
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("invitations"));
        assert!(json.contains("id-1"));
    }
}
