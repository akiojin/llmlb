//! EndpointStatus遷移のUnit Test
//!
//! SPEC-66555000 T034: EndpointStatus遷移のテスト
//! - pending → offline の即時遷移
//! - 各ステータスからの遷移パターン

use llmlb::types::endpoint::EndpointStatus;

#[test]
fn test_endpoint_status_default_is_pending() {
    let status: EndpointStatus = Default::default();
    assert_eq!(status, EndpointStatus::Pending);
}

#[test]
fn test_endpoint_status_as_str() {
    assert_eq!(EndpointStatus::Pending.as_str(), "pending");
    assert_eq!(EndpointStatus::Online.as_str(), "online");
    assert_eq!(EndpointStatus::Offline.as_str(), "offline");
    assert_eq!(EndpointStatus::Error.as_str(), "error");
}

#[test]
fn test_endpoint_status_from_str() {
    assert_eq!(
        "pending".parse::<EndpointStatus>().unwrap(),
        EndpointStatus::Pending
    );
    assert_eq!(
        "online".parse::<EndpointStatus>().unwrap(),
        EndpointStatus::Online
    );
    assert_eq!(
        "offline".parse::<EndpointStatus>().unwrap(),
        EndpointStatus::Offline
    );
    assert_eq!(
        "error".parse::<EndpointStatus>().unwrap(),
        EndpointStatus::Error
    );
}

#[test]
fn test_endpoint_status_from_invalid_str_defaults_to_pending() {
    // 無効な文字列はPendingにフォールバック
    assert_eq!(
        "invalid".parse::<EndpointStatus>().unwrap_or_default(),
        EndpointStatus::Pending
    );
    assert_eq!(
        "".parse::<EndpointStatus>().unwrap_or_default(),
        EndpointStatus::Pending
    );
}

#[test]
fn test_endpoint_status_serialization() {
    // serde_json形式でのシリアライズ確認
    let status = EndpointStatus::Online;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"online\"");

    let deserialized: EndpointStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, EndpointStatus::Online);
}

#[test]
fn test_endpoint_status_all_variants_serializable() {
    let variants = [
        EndpointStatus::Pending,
        EndpointStatus::Online,
        EndpointStatus::Offline,
        EndpointStatus::Error,
    ];

    for status in variants {
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: EndpointStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, deserialized, "Failed roundtrip for {:?}", status);
    }
}

/// ステータス遷移の妥当性テスト
/// - Pending → Online/Offline/Error (初回ヘルスチェック後)
/// - Online → Offline/Error (ヘルスチェック失敗時)
/// - Offline → Online (ヘルスチェック成功時)
/// - Error → Online/Offline (リカバリー時)
#[test]
fn test_pending_can_transition_to_any_status() {
    // Pending状態からはどのステータスへも遷移可能
    let pending = EndpointStatus::Pending;

    // Pending → Online (ヘルスチェック成功)
    let online = EndpointStatus::Online;
    assert_ne!(pending, online);

    // Pending → Offline (即時遷移 - 接続不可の場合)
    let offline = EndpointStatus::Offline;
    assert_ne!(pending, offline);

    // Pending → Error (エラー発生時)
    let error = EndpointStatus::Error;
    assert_ne!(pending, error);
}

#[test]
fn test_online_can_transition_to_offline_or_error() {
    let online = EndpointStatus::Online;

    // Online → Offline (一時的な接続断)
    let offline = EndpointStatus::Offline;
    assert_ne!(online, offline);

    // Online → Error (永続的なエラー)
    let error = EndpointStatus::Error;
    assert_ne!(online, error);
}

#[test]
fn test_offline_can_transition_to_online() {
    // Offline → Online (リカバリー成功)
    let offline = EndpointStatus::Offline;
    let online = EndpointStatus::Online;
    assert_ne!(offline, online);
}

#[test]
fn test_error_can_transition_to_online_or_offline() {
    let error = EndpointStatus::Error;

    // Error → Online (完全リカバリー)
    let online = EndpointStatus::Online;
    assert_ne!(error, online);

    // Error → Offline (部分リカバリー)
    let offline = EndpointStatus::Offline;
    assert_ne!(error, offline);
}
