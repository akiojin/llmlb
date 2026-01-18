//! Integration Test: ダッシュボードAPIでのGPU情報表示
//!
//! ダッシュボードエンドポイントがエンドポイントのGPU情報を返すことを検証する。
//!
//! NOTE: NodeRegistry廃止（SPEC-66555000）に伴い、EndpointRegistryベースに移行が必要です。

#[tokio::test]
#[ignore = "NodeRegistry廃止に伴い、EndpointRegistryベースに書き換えが必要 (SPEC-66555000)"]
async fn dashboard_nodes_include_gpu_devices() {
    // TODO: EndpointRegistryベースのGPU情報表示テストを実装
    // エンドポイント登録時のGPU情報がダッシュボードAPIで返されることを検証
}
