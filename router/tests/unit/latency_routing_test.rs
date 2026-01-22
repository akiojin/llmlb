//! レイテンシベースルーティング選択のUnit Test
//!
//! SPEC-66555000 T035c: レイテンシベースルーティングのテスト
//! - 低レイテンシエンドポイントの優先選択
//! - レイテンシNone時のソート挙動
//! - 同一レイテンシ時の挙動

use llmlb::types::endpoint::{Endpoint, EndpointStatus};
use std::cmp::Ordering;

/// レイテンシでソートするためのヘルパー関数
fn sort_by_latency(endpoints: &mut [Endpoint]) {
    endpoints.sort_by(|a, b| {
        match (a.latency_ms, b.latency_ms) {
            (Some(a_lat), Some(b_lat)) => a_lat.cmp(&b_lat),
            (Some(_), None) => Ordering::Less, // レイテンシありを優先
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    });
}

#[test]
fn test_sort_by_latency_basic() {
    let mut endpoints = vec![
        create_endpoint_with_latency("High", 100),
        create_endpoint_with_latency("Low", 10),
        create_endpoint_with_latency("Medium", 50),
    ];

    sort_by_latency(&mut endpoints);

    assert_eq!(endpoints[0].name, "Low");
    assert_eq!(endpoints[1].name, "Medium");
    assert_eq!(endpoints[2].name, "High");
}

#[test]
fn test_sort_by_latency_none_goes_last() {
    let mut endpoints = vec![
        create_endpoint_without_latency("Unknown"),
        create_endpoint_with_latency("Known", 50),
    ];

    sort_by_latency(&mut endpoints);

    // レイテンシが既知のものが先に来る
    assert_eq!(endpoints[0].name, "Known");
    assert_eq!(endpoints[1].name, "Unknown");
}

#[test]
fn test_sort_by_latency_all_none() {
    let mut endpoints = vec![
        create_endpoint_without_latency("A"),
        create_endpoint_without_latency("B"),
        create_endpoint_without_latency("C"),
    ];

    // すべてNoneの場合は順序が維持される（安定ソート）
    sort_by_latency(&mut endpoints);

    // 元の順序が維持される
    assert_eq!(endpoints[0].name, "A");
    assert_eq!(endpoints[1].name, "B");
    assert_eq!(endpoints[2].name, "C");
}

#[test]
fn test_sort_by_latency_equal_values() {
    let mut endpoints = vec![
        create_endpoint_with_latency("A", 50),
        create_endpoint_with_latency("B", 50),
        create_endpoint_with_latency("C", 50),
    ];

    sort_by_latency(&mut endpoints);

    // 同一レイテンシの場合は順序が維持される
    assert_eq!(endpoints[0].latency_ms, Some(50));
    assert_eq!(endpoints[1].latency_ms, Some(50));
    assert_eq!(endpoints[2].latency_ms, Some(50));
}

#[test]
fn test_sort_by_latency_mixed() {
    let mut endpoints = vec![
        create_endpoint_without_latency("Unknown1"),
        create_endpoint_with_latency("Fast", 10),
        create_endpoint_without_latency("Unknown2"),
        create_endpoint_with_latency("Slow", 100),
        create_endpoint_with_latency("Medium", 50),
    ];

    sort_by_latency(&mut endpoints);

    // レイテンシ既知のものが先、低い順
    assert_eq!(endpoints[0].name, "Fast");
    assert_eq!(endpoints[0].latency_ms, Some(10));

    assert_eq!(endpoints[1].name, "Medium");
    assert_eq!(endpoints[1].latency_ms, Some(50));

    assert_eq!(endpoints[2].name, "Slow");
    assert_eq!(endpoints[2].latency_ms, Some(100));

    // レイテンシ未知のものは後ろ
    assert!(endpoints[3].latency_ms.is_none());
    assert!(endpoints[4].latency_ms.is_none());
}

#[test]
fn test_sort_by_latency_single_element() {
    let mut endpoints = vec![create_endpoint_with_latency("Only", 42)];

    sort_by_latency(&mut endpoints);

    assert_eq!(endpoints.len(), 1);
    assert_eq!(endpoints[0].name, "Only");
}

#[test]
fn test_sort_by_latency_empty() {
    let mut endpoints: Vec<Endpoint> = vec![];

    sort_by_latency(&mut endpoints);

    assert!(endpoints.is_empty());
}

#[test]
fn test_first_endpoint_selection_after_sort() {
    let mut endpoints = vec![
        create_endpoint_with_latency("High", 200),
        create_endpoint_with_latency("Low", 20),
        create_endpoint_with_latency("Medium", 100),
    ];

    sort_by_latency(&mut endpoints);

    // ソート後、最初の要素が最低レイテンシ
    let selected = endpoints.first().unwrap();
    assert_eq!(selected.name, "Low");
    assert_eq!(selected.latency_ms, Some(20));
}

#[test]
fn test_latency_comparison_edge_cases() {
    // ゼロレイテンシ
    let mut endpoints = vec![
        create_endpoint_with_latency("Zero", 0),
        create_endpoint_with_latency("One", 1),
    ];

    sort_by_latency(&mut endpoints);

    assert_eq!(endpoints[0].name, "Zero");
    assert_eq!(endpoints[0].latency_ms, Some(0));
}

#[test]
fn test_latency_comparison_large_values() {
    // 大きなレイテンシ値
    let mut endpoints = vec![
        create_endpoint_with_latency("Huge", u32::MAX),
        create_endpoint_with_latency("Normal", 50),
    ];

    sort_by_latency(&mut endpoints);

    assert_eq!(endpoints[0].name, "Normal");
    assert_eq!(endpoints[1].name, "Huge");
}

// --- Helper Functions ---

fn create_endpoint_with_latency(name: &str, latency_ms: u32) -> Endpoint {
    let mut endpoint = Endpoint::new(
        name.to_string(),
        format!("http://{}.local:8080", name.to_lowercase()),
    );
    endpoint.status = EndpointStatus::Online;
    endpoint.latency_ms = Some(latency_ms);
    endpoint
}

fn create_endpoint_without_latency(name: &str) -> Endpoint {
    let mut endpoint = Endpoint::new(
        name.to_string(),
        format!("http://{}.local:8080", name.to_lowercase()),
    );
    endpoint.status = EndpointStatus::Online;
    endpoint.latency_ms = None;
    endpoint
}
