# リサーチ: クラウドプロバイダーモデル一覧統合

**機能ID**: `SPEC-82491000` | **日付**: 2025-12-25

## 1. クラウドプロバイダーAPI調査

### OpenAI Models API

**エンドポイント**: `GET https://api.openai.com/v1/models`

**認証**: `Authorization: Bearer {API_KEY}`

**レスポンス形式**:

```json
{
  "object": "list",
  "data": [
    {
      "id": "gpt-4o",
      "object": "model",
      "created": 1686935002,
      "owned_by": "openai"
    }
  ]
}
```

**決定**: OpenAIレスポンスはそのまま使用可能。`id`に`openai:`プレフィックスを付与。

### Google AI Models API

**エンドポイント**: `GET https://generativelanguage.googleapis.com/v1beta/models?key={API_KEY}`

**認証**: クエリパラメータ `key`

**レスポンス形式**:

```json
{
  "models": [
    {
      "name": "models/gemini-2.0-flash",
      "displayName": "Gemini 2.0 Flash",
      "description": "...",
      "inputTokenLimit": 1048576,
      "outputTokenLimit": 8192
    }
  ]
}
```

**決定**: `name`フィールドから`models/`プレフィックスを除去し、`google:`プレフィックスを付与。
`created`フィールドがないため0を設定。

### Anthropic Models API

**エンドポイント**: `GET https://api.anthropic.com/v1/models`

**認証**:

- `x-api-key: {API_KEY}`
- `anthropic-version: 2023-06-01`

**レスポンス形式**:

```json
{
  "data": [
    {
      "id": "claude-sonnet-4-20250514",
      "type": "model",
      "display_name": "Claude Sonnet 4",
      "created_at": "2025-05-14T00:00:00Z"
    }
  ],
  "has_more": false,
  "first_id": "...",
  "last_id": "..."
}
```

**決定**: `id`に`anthropic:`プレフィックスを付与。`created_at`をUnixタイムスタンプに変換。

## 2. キャッシュ実装パターン

### 既存パターン調査

コードベースの `GGUF_DISCOVERY_CACHE` パターンを参考:

```rust
static CACHE: OnceCell<RwLock<CacheEntry>> = OnceCell::const_new();

struct CacheEntry {
    data: Vec<ModelInfo>,
    fetched_at: Instant,
}
```

**決定**: 同様のパターンを採用。TTLは24時間（86400秒）。

### キャッシュ構造

```rust
pub struct CloudModelsCache {
    models: Vec<CloudModelInfo>,
    fetched_at: chrono::DateTime<Utc>,
    ttl_secs: u64,  // 86400 (24時間)
}
```

**フォールバック戦略**:

- キャッシュ有効 → キャッシュ返却
- キャッシュ期限切れ → API呼び出し
- API失敗時 → 古いキャッシュをフォールバック返却（stale-while-revalidate）

## 3. 並列API呼び出し

### 実装アプローチ

```rust
use futures::future::join_all;

async fn fetch_all_cloud_models() -> Vec<CloudModelInfo> {
    let futures = vec![
        fetch_openai_models(),
        fetch_google_models(),
        fetch_anthropic_models(),
    ];

    let results = join_all(futures).await;
    results.into_iter().flatten().collect()
}
```

**決定**: `futures::join_all`で並列実行。各プロバイダーは独立してタイムアウト（10秒）。

### エラーハンドリング

| 状況 | 処理 |
|------|------|
| APIキー未設定 | 該当プロバイダーをスキップ（空Vec返却） |
| タイムアウト | 該当プロバイダーをスキップ、warn!ログ |
| 認証エラー | 該当プロバイダーをスキップ、error!ログ |
| パースエラー | 該当プロバイダーをスキップ、error!ログ |

## 4. 検討した代替案

### キャッシュ保存場所

| 選択肢 | 採用 | 理由 |
|--------|------|------|
| インメモリ（static変数） | ✅ | シンプル、再起動時のみ再取得で問題なし |
| ファイルシステム | ❌ | 複雑化、メリット少ない |
| Redis | ❌ | 外部依存増加、オーバースペック |

### API呼び出しタイミング

| 選択肢 | 採用 | 理由 |
|--------|------|------|
| オンデマンド（初回リクエスト時） | ✅ | シンプル、リソース効率的 |
| 起動時プリフェッチ | ❌ | 起動時間増加、APIキー未設定時に問題 |
| バックグラウンド定期更新 | ❌ | 複雑化、24時間TTLなら不要 |

## 5. 技術的決定事項

1. **モジュール構成**: `llmlb/src/api/cloud_models.rs`を新規作成
2. **キャッシュTTL**: 24時間（86400秒）
3. **タイムアウト**: 各プロバイダー10秒
4. **並列実行**: `futures::join_all`使用
5. **プレフィックス形式**: `{provider}:{model_id}`（既存と統一）
6. **テスト**: `wiremock`でAPIモック

---

*Phase 0 完了*
