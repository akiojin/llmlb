# SPEC-f8e3a1b7: 実装計画

## 技術スタック

- **言語**: Rust 2021 Edition
- **フレームワーク**: Axum 0.7
- **データベース**: SQLite（sqlx 0.8）
- **フロントエンド**: 既存ダッシュボード（SPA）

## アーキテクチャ設計

### データモデル変更

#### endpoints テーブル（新規/更新）

```sql
CREATE TABLE IF NOT EXISTS endpoints (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    base_url TEXT NOT NULL,
    api_key TEXT,
    status TEXT NOT NULL DEFAULT 'offline',
    health_check_interval_secs INTEGER NOT NULL DEFAULT 30,
    inference_timeout_secs INTEGER NOT NULL DEFAULT 120,
    latency_ms REAL,  -- 新規: EMAレイテンシ（ミリ秒）
    device_info TEXT, -- 新規: JSON形式のデバイス情報
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

#### 削除対象

- `Node` 型（llmlb/src/common/types.rs）
- `NodeRegistry` 型
- `nodes.json` ファイル形式
- `endpoint_type` フィールド

### Endpoint型拡張

```rust
pub struct Endpoint {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub status: EndpointStatus,
    pub health_check_interval_secs: u32,
    pub inference_timeout_secs: u32,
    pub latency_ms: Option<f64>,      // 新規
    pub device_info: Option<DeviceInfo>, // 新規
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct DeviceInfo {
    pub device_type: DeviceType,  // CPU or GPU
    pub gpu_devices: Vec<GpuDeviceInfo>,
}

pub enum DeviceType {
    Cpu,
    Gpu,
}
```

### レイテンシ計算

```rust
impl Endpoint {
    /// EMA (α=0.2) でレイテンシを更新
    pub fn update_latency(&mut self, new_latency_ms: f64) {
        const ALPHA: f64 = 0.2;
        self.latency_ms = Some(match self.latency_ms {
            Some(current) => ALPHA * new_latency_ms + (1.0 - ALPHA) * current,
            None => new_latency_ms,
        });
    }

    /// オフライン時にレイテンシをリセット
    pub fn reset_latency(&mut self) {
        self.latency_ms = Some(f64::INFINITY);
    }
}
```

### エラーレスポンス形式

```rust
#[derive(Serialize)]
pub struct OpenAIError {
    pub error: OpenAIErrorDetail,
}

#[derive(Serialize)]
pub struct OpenAIErrorDetail {
    pub message: String,
    pub r#type: String,
    pub code: Option<String>,
}

impl From<LbError> for OpenAIError {
    fn from(e: LbError) -> Self {
        OpenAIError {
            error: OpenAIErrorDetail {
                message: e.to_string(),
                r#type: e.error_type(),
                code: e.error_code(),
            }
        }
    }
}
```

## 実装フェーズ

### Phase 1: Node→Endpoint移行 + SQLite移行

1. SQLiteマイグレーションスクリプト作成
2. Endpoint型にlatency_ms、device_infoフィールド追加
3. EndpointRepository実装（SQLite CRUD）
4. JSON→SQLite自動マイグレーション実装
5. Node型・NodeRegistry削除
6. 16個のテスト有効化・修正

### Phase 2: unwrap()除去

1. LbErrorにOpenAI互換メソッド追加
2. models.rs のunwrap()除去
3. auth.rs のunwrap()除去
4. その他ファイルのunwrap()除去
5. エラーレスポンス形式統一

### Phase 3: レイテンシベース負荷分散

1. Endpoint::update_latency()実装
2. Endpoint::reset_latency()実装
3. EndpointRegistry::find_by_model_sorted_by_latency()更新
4. リクエスト成功時のレイテンシ計測・更新
5. タイブレーク用ラウンドロビンインデックス追加

### Phase 4: /api/system API対応

1. /api/system エンドポイント試行ロジック追加
2. DeviceInfo取得・保存実装
3. 登録時のみ呼び出し制御

### Phase 5: ダッシュボードUI更新

1. 「GPU」→「デバイス」リネーム
2. エンドポイント詳細ページにレイテンシ表示
3. 登録フォームからデバイス選択削除

### Phase 6: Visionテスト環境

1. LLaVA-1.5-7B-Q4_K_M取得・設定
2. 100x100テスト画像作成
3. 17個のVisionテスト有効化

## ドキュメント更新

- CLAUDE.md: GPU必須ポリシー削除、レイテンシ戦略追記
- docs/architecture.md: 負荷分散戦略更新
- README.md/README.ja.md: 対応エンドポイント説明更新

## リスク・注意点

1. **JSON→SQLite移行**: 既存ユーザーのデータ損失リスク
   - 対策: 移行前にJSONバックアップ、ロールバック手順用意

2. **unwrap()除去の影響範囲**: 多数のファイルに影響
   - 対策: ファイル単位で段階的に修正、各段階でテスト実行

3. **レイテンシ計算の精度**: ネットワーク揺らぎの影響
   - 対策: EMAで平滑化、極端な外れ値は除外検討

## 検証方法

1. `cargo test` - 全テストパス
2. `cargo clippy -- -D warnings` - 警告なし
3. SQLiteマイグレーション確認（既存JSON→SQLite）
4. ダッシュボードUI確認
5. OpenAI互換クライアントでエラーハンドリング確認
