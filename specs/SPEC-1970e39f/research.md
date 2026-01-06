# リサーチ: 構造化ロギング強化

## 調査目的

ルーターとノードのHTTPリクエスト/レスポンスを構造化ログとして出力するための技術調査。

## ロギングライブラリ比較

### Rust側

| ライブラリ | 特徴 | 評価 |
|-----------|------|------|
| tracing | 構造化ログ、非同期対応、スパン機能 | 採用 |
| log + env_logger | シンプル、広く使用 | 構造化ログに弱い |
| slog | 構造化ログ対応 | tracingの方がエコシステム充実 |

**決定**: `tracing` + `tracing-subscriber`（既存導入済み）

### C++側

| ライブラリ | 特徴 | 評価 |
|-----------|------|------|
| spdlog | 高速、ヘッダーオンリー、JSON対応 | 採用 |
| glog | Google製、安定 | JSON対応が弱い |
| plog | 軽量 | spdlogの方が高速 |

**決定**: `spdlog`（既存導入済み）

## ログフォーマット設計

### JSON構造化ログ

```json
{
  "timestamp": "2025-01-02T10:30:00.123Z",
  "level": "INFO",
  "target": "llm_router::api::openai",
  "message": "Request received",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "endpoint": "/v1/chat/completions",
  "model": "llama-3.1-8b",
  "client_ip": "192.168.1.100"
}
```

### ログレベル定義

| レベル | 用途 | 例 |
|--------|------|-----|
| ERROR | 障害、復旧不能エラー | ノード選択失敗 |
| WARN | 異常、復旧可能エラー | タイムアウト、リトライ |
| INFO | 通常イベント | リクエスト受信、完了 |
| DEBUG | 詳細情報 | リクエストボディ |
| TRACE | 最詳細 | 内部状態 |

## 非同期ログ出力

### Rust (tracing-appender)

```rust
use tracing_appender::non_blocking;

let (non_blocking, _guard) = non_blocking(file_appender);
tracing_subscriber::fmt()
    .json()
    .with_writer(non_blocking)
    .init();
```

### C++ (spdlog async)

```cpp
auto async_logger = spdlog::create_async<spdlog::sinks::rotating_file_sink_mt>(
    "node_logger",
    "logs/node.log",
    1024 * 1024 * 10,  // 10MB
    3  // 3 files
);
```

## ログローテーション

### 設計

- 日次ローテーション
- 7日間保持
- 最大ファイルサイズ: 10MB

### 実装

```rust
use tracing_appender::rolling::{RollingFileAppender, Rotation};

let file_appender = RollingFileAppender::new(
    Rotation::DAILY,
    "~/.llm-router/logs",
    "router.log",
);
```

## 機密情報の取り扱い

### マスキング対象

| 情報 | 処理 |
|------|------|
| APIキー | 最初4文字のみ表示（`sk_d***`） |
| プロンプト | DEBUGレベルのみ出力 |
| レスポンス内容 | DEBUGレベルのみ出力 |
| クライアントIP | INFOレベルで出力 |

### 実装例

```rust
fn mask_api_key(key: &str) -> String {
    if key.len() > 4 {
        format!("{}***", &key[..4])
    } else {
        "***".to_string()
    }
}
```

## パフォーマンス目標

| 指標 | 目標値 |
|------|--------|
| ログ出力レイテンシ | < 1ms |
| スループット影響 | < 1% |
| メモリ使用量増加 | < 10MB |

## 参考資料

- [tracing (Rust)](https://github.com/tokio-rs/tracing)
- [spdlog (C++)](https://github.com/gabime/spdlog)
- [Structured Logging Best Practices](https://www.datadoghq.com/blog/structured-logging/)
