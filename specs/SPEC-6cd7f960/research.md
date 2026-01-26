# 技術リサーチ: 対応モデルリスト型管理

## リサーチ課題

1. 対応モデルリストの管理方式
2. HuggingFace APIとの連携設計
3. リアルタイムUI更新アーキテクチャ

## 1. 対応モデルリストの管理方式

### 決定

**静的JSONファイル（supported_models.json）** をロードバランサーに組み込む方式を採用

### 理由

- ビルド時に検証済みモデルリストを確定できる
- 外部APIに依存せずにモデル一覧を返せる
- バージョン管理が容易
- リリースプロセスと連携しやすい

### 代替案比較表

| 方式 | 長所 | 短所 | 判定 |
|------|------|------|------|
| 静的JSONファイル | シンプル、高速、オフライン動作 | 更新にはビルドが必要 | ○ |
| データベース管理 | 動的更新可能 | 複雑、マイグレーション必要 | × |
| 外部API（HF直接） | 常に最新 | 依存性高、レート制限 | × |
| 設定ファイル（TOML） | 編集しやすい | スキーマ検証が困難 | △ |

### 実装方法

```rust
// ビルド時に読み込み
const SUPPORTED_MODELS_JSON: &str = include_str!("../supported_models.json");

pub fn load_supported_models() -> Vec<SupportedModel> {
    serde_json::from_str(SUPPORTED_MODELS_JSON)
        .expect("Invalid supported_models.json")
}
```

## 2. HuggingFace APIとの連携設計

### 決定

**キャッシュ付きAPIクライアント（TTL: 10分）** を採用

### 理由

- レート制限の回避
- レスポンス速度の向上
- HF API障害時のフォールバック

### HuggingFace API情報

| エンドポイント | 情報 | レート制限 |
|--------------|------|-----------|
| `/api/models/{repo_id}` | ダウンロード数、スター数 | 1000/日（匿名） |
| `/api/models/{repo_id}/files` | ファイル一覧 | 同上 |

### キャッシュ戦略

```rust
pub struct HfApiClient {
    http_client: reqwest::Client,
    cache: Arc<RwLock<LruCache<String, CachedResponse>>>,
    cache_ttl: Duration,
}

struct CachedResponse {
    data: ModelMetadata,
    fetched_at: Instant,
}

impl HfApiClient {
    async fn get_model_stats(&self, repo_id: &str) -> Result<ModelStats> {
        // キャッシュチェック
        if let Some(cached) = self.cache.read().get(repo_id) {
            if cached.fetched_at.elapsed() < self.cache_ttl {
                return Ok(cached.data.stats.clone());
            }
        }

        // API呼び出し（失敗時はキャッシュを使用）
        match self.fetch_from_hf(repo_id).await {
            Ok(data) => {
                self.cache.write().put(repo_id.to_string(), CachedResponse {
                    data: data.clone(),
                    fetched_at: Instant::now(),
                });
                Ok(data.stats)
            }
            Err(_) if let Some(cached) = self.cache.read().get(repo_id) => {
                // 古いキャッシュを返す
                Ok(cached.data.stats.clone())
            }
            Err(e) => Err(e),
        }
    }
}
```

## 3. リアルタイムUI更新アーキテクチャ

### 決定

**WebSocket全面移行**（Session 2026-01-02のClarificationで確定）

### 理由

- ポーリングよりも効率的
- リアルタイム性の向上
- サーバー負荷の軽減

### 代替案比較表

| 方式 | リアルタイム性 | サーバー負荷 | 実装難易度 | 判定 |
|------|--------------|------------|-----------|------|
| ポーリング（10秒） | 低 | 高 | 低 | × |
| SSE (Server-Sent Events) | 中 | 中 | 中 | △ |
| WebSocket | 高 | 低 | 中 | ○ |
| Long Polling | 中 | 中 | 中 | × |

### WebSocket設計

```rust
// イベントタイプ
enum WsEvent {
    // モデル状態更新
    ModelStatusChanged {
        model_id: String,
        status: ModelStatus,
        progress: Option<f32>,
    },
    // ノード状態更新
    NodeStatusChanged {
        runtime_id: String,
        status: NodeStatus,
    },
    // メトリクス更新
    MetricsUpdated {
        runtime_id: String,
        metrics: NodeMetrics,
    },
}

// クライアント購読
struct WsSubscription {
    topics: HashSet<String>,
    sender: mpsc::Sender<WsEvent>,
}
```

### フロントエンド実装

```typescript
// React Query + WebSocket統合
const useModelStatus = (modelId: string) => {
  const queryClient = useQueryClient();

  useEffect(() => {
    const ws = new WebSocket('/ws/models');
    ws.onmessage = (event) => {
      const data = JSON.parse(event.data);
      if (data.model_id === modelId) {
        queryClient.setQueryData(['model', modelId], data);
      }
    };
    return () => ws.close();
  }, [modelId]);

  return useQuery(['model', modelId], fetchModel);
};
```

## 4. モデル状態管理

### 状態遷移図

```text
┌─────────────────────────────────────────────────────────────────┐
│                       Model Lifecycle                            │
│                                                                  │
│  ┌──────────┐    register    ┌──────────┐                       │
│  │          │ ─────────────→ │          │                       │
│  │ available│                │registered│                       │
│  │          │ ←───────────── │          │                       │
│  └──────────┘    unregister  └────┬─────┘                       │
│       ▲                           │                             │
│       │                           │ node sync                   │
│       │                           ▼                             │
│       │                     ┌──────────┐                        │
│       │                     │          │                        │
│       │                     │downloading│                       │
│       │                     │          │                        │
│       │                     └────┬─────┘                        │
│       │                          │                              │
│       │                          │ complete                     │
│       │                          ▼                              │
│       │                     ┌──────────┐                        │
│       │    delete           │          │                        │
│       └──────────────────── │  ready   │                        │
│                             │          │                        │
│                             └──────────┘                        │
└─────────────────────────────────────────────────────────────────┘
```

## 参考リソース

- [HuggingFace Hub API Documentation](https://huggingface.co/docs/hub/api)
- [Axum WebSockets](https://docs.rs/axum/latest/axum/extract/ws/index.html)
- [React Query WebSocket Integration](https://tanstack.com/query/latest/docs/react/guides/websockets)
- [shadcn/ui Components](https://ui.shadcn.com/)
