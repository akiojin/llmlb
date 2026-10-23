//! ロードバランサー自身が観測した直近60秒のリクエスト開始数。

use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

pub(super) const ACTIVITY_WINDOW_SECONDS: usize = 60;

#[derive(Debug, Clone, Copy, Default)]
struct RequestBucket {
    second: Option<i64>,
    requests: u64,
}

/// Epoch秒でスロットを識別する固定長リング。空白秒の更新タスクは不要。
pub(super) struct RequestRateRing {
    buckets: [RequestBucket; ACTIVITY_WINDOW_SECONDS],
}

impl Default for RequestRateRing {
    fn default() -> Self {
        Self {
            buckets: [RequestBucket::default(); ACTIVITY_WINDOW_SECONDS],
        }
    }
}

impl RequestRateRing {
    pub(super) fn record(&mut self, second: i64) {
        let bucket = &mut self.buckets[second.rem_euclid(ACTIVITY_WINDOW_SECONDS as i64) as usize];
        if bucket.second != Some(second) {
            *bucket = RequestBucket {
                second: Some(second),
                requests: 0,
            };
        }
        bucket.requests = bucket.requests.saturating_add(1);
    }

    pub(super) fn series(&self, now: i64) -> Vec<u64> {
        (0..ACTIVITY_WINDOW_SECONDS)
            .map(|offset| {
                let second = now - (ACTIVITY_WINDOW_SECONDS - 1) as i64 + offset as i64;
                let bucket =
                    &self.buckets[second.rem_euclid(ACTIVITY_WINDOW_SECONDS as i64) as usize];
                if bucket.second == Some(second) {
                    bucket.requests
                } else {
                    0
                }
            })
            .collect()
    }
}

#[derive(Default)]
pub(super) struct RequestActivityHistory {
    total: RequestRateRing,
    endpoints: HashMap<Uuid, RequestRateRing>,
}

impl RequestActivityHistory {
    pub(super) fn record(&mut self, endpoint_id: Uuid, second: i64) {
        self.total.record(second);
        self.endpoints
            .entry(endpoint_id)
            .or_default()
            .record(second);
    }

    pub(super) fn total_series(&self, second: i64) -> Vec<u64> {
        self.total.series(second)
    }

    pub(super) fn endpoint_series(&self, endpoint_id: Uuid, second: i64) -> Vec<u64> {
        self.endpoints
            .get(&endpoint_id)
            .map(|ring| ring.series(second))
            .unwrap_or_else(|| vec![0; ACTIVITY_WINDOW_SECONDS])
    }

    pub(super) fn forget_endpoint(&mut self, endpoint_id: Uuid) {
        self.endpoints.remove(&endpoint_id);
    }
}

/// 全体のゲートウェイ観測値。upstream内部の負荷やキューは含まない。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequestActivityTotals {
    /// ロードバランサーが割り当てており、まだ完了していない件数。
    pub active_requests: u32,
    /// エンドポイント未割当の全体待機キュー。
    pub queued_requests: usize,
    /// timestamp-59秒からtimestampまでの開始数60点。現在秒は途中値。
    pub requests_per_second: Vec<u64>,
}

/// エンドポイント単位のゲートウェイ観測値。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EndpointRequestActivity {
    /// 対象のエンドポイントID。
    pub endpoint_id: Uuid,
    /// ロードバランサーが割り当てており、まだ完了していない件数。
    pub active_requests: u32,
    /// エンドポイント別のゲートウェイキューは存在しないため常に0。
    pub queued_requests: usize,
    /// timestamp-59秒からtimestampまでの開始数60点。現在秒は途中値。
    pub requests_per_second: Vec<u64>,
}

/// ダッシュボードの追加的なアクティビティAPIレスポンス。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RequestActivitySnapshot {
    /// 最新バケットのUTC秒境界。
    pub timestamp: DateTime<Utc>,
    /// バケット幅（常に1秒）。
    pub bucket_seconds: u32,
    /// 系列の長さ（常に60秒）。
    pub window_seconds: u32,
    /// 全体の処理中・待機・開始数。
    pub total: RequestActivityTotals,
    /// 登録済みエンドポイントの処理中・待機・開始数。
    pub endpoints: Vec<EndpointRequestActivity>,
}
