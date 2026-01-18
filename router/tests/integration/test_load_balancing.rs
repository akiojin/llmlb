//! Integration Test: ロードバランシング
//!
//! 複数エンドポイントへのリクエスト分散と負荷ベース選択の検証
//!
//! NOTE: NodeRegistry廃止（SPEC-66555000）に伴い、これらのテストは
//! EndpointRegistryベースのロードバランシングに移行が必要です。
//! 現在のLoadManagerはEndpointRegistryを使用しています。

// NOTE: ロードバランシングテストはEndpointRegistry移行後に再実装が必要
// 以下のテストは廃止されたNodeRegistry APIを使用しているため、
// EndpointRegistryベースのテストを tests/integration/endpoint_load_balancing.rs
// に新規作成する必要があります。

#[tokio::test]
#[ignore = "NodeRegistry廃止に伴い、EndpointRegistryベースに書き換えが必要 (SPEC-66555000)"]
async fn test_round_robin_load_balancing() {
    // TODO: EndpointRegistryベースのラウンドロビンテストを実装
}

#[tokio::test]
#[ignore = "NodeRegistry廃止に伴い、EndpointRegistryベースに書き換えが必要 (SPEC-66555000)"]
async fn test_load_based_balancing_favors_low_cpu_nodes() {
    // TODO: EndpointRegistryベースの負荷ベーステストを実装
}

#[tokio::test]
#[ignore = "NodeRegistry廃止に伴い、EndpointRegistryベースに書き換えが必要 (SPEC-66555000)"]
async fn test_load_based_balancing_prefers_lower_latency() {
    // TODO: EndpointRegistryベースのレイテンシーベーステストを実装
}

#[tokio::test]
#[ignore = "NodeRegistry廃止に伴い、EndpointRegistryベースに書き換えが必要 (SPEC-66555000)"]
async fn test_load_balancer_excludes_non_online_nodes() {
    // TODO: EndpointRegistryベースのオフラインノード除外テストを実装
}
