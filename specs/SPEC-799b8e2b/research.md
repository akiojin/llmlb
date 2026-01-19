# 技術リサーチ: 共通ログシステム

## リサーチ課題

1. Router（Rust）とNode（C++）で統一されたログ形式の実現方法
2. 構造化ログ（JSONL）の出力ライブラリ選定
3. ログローテーションの実装方式
4. 非同期書き込みによるパフォーマンス最適化

## 1. ログライブラリ選定

### Router（Rust）

**決定**: `tracing` + `tracing-subscriber` を採用。

| ライブラリ | 利点 | 欠点 |
|-----------|------|------|
| tracing | 構造化ログ対応、非同期対応 | 学習コストやや高 |
| log | シンプル | 構造化ログ非対応 |
| slog | 高機能 | 設定が複雑 |

**理由**:

- Rust エコシステムで事実上の標準
- `tracing-subscriber` で JSON 出力が容易
- 非同期ランタイム（tokio）と相性が良い

**実装例**:

```rust
use tracing::{info, warn, error, instrument};
use tracing_subscriber::fmt::format::FmtSpan;

// カスタムレイヤーでファイル出力とコンソール出力を分離
let file_layer = fmt::layer()
    .json()
    .with_writer(file_appender);

let console_layer = fmt::layer()
    .pretty()
    .with_writer(std::io::stdout);

tracing_subscriber::registry()
    .with(file_layer)
    .with(console_layer)
    .init();
```

### Node（C++）

**決定**: `spdlog` を採用。

| ライブラリ | 利点 | 欠点 |
|-----------|------|------|
| spdlog | 高速、ヘッダーオンリー可 | JSONフォーマット要カスタム |
| glog | Google製、安定 | 設定が限定的 |
| fmt + 自作 | 完全制御 | 車輪の再発明 |

**理由**:

- 非同期ログに対応（`async_logger`）
- カスタムシンクでファイルローテーション対応
- フォーマッターのカスタマイズが容易

**実装例**:

```cpp
#include <spdlog/spdlog.h>
#include <spdlog/sinks/daily_file_sink.h>
#include <spdlog/sinks/stdout_color_sinks.h>

// 日付ローテーション付きファイルシンク
auto file_sink = std::make_shared<spdlog::sinks::daily_file_sink_mt>(
    "~/.llm-router/logs/allm.jsonl", 0, 0);

// コンソールシンク
auto console_sink = std::make_shared<spdlog::sinks::stdout_color_sink_mt>();

// マルチシンクロガー
auto logger = std::make_shared<spdlog::logger>(
    "allm", spdlog::sinks_init_list{file_sink, console_sink});
```

## 2. JSONL フォーマット設計

### 決定

統一フォーマットを採用し、Router/Node で同一形式を使用。

### フォーマット仕様

```json
{
  "ts": "2025-12-01T12:00:00.000Z",
  "level": "info",
  "category": "api",
  "msg": "Request received",
  "request_id": "abc123",
  "model": "llama-3.2-1b"
}
```

| フィールド | 型 | 必須 | 説明 |
|-----------|---|-----|------|
| ts | string | ✓ | ISO 8601 タイムスタンプ |
| level | string | ✓ | trace/debug/info/warn/error |
| category | string | ✓ | ログカテゴリ |
| msg | string | ✓ | メッセージ |
| * | any | | 追加コンテキスト |

### 理由

- JSONL は行単位で解析可能（ストリーム処理に適合）
- 構造化データで検索・フィルタリングが容易
- 既存のログ解析ツール（jq, Elasticsearch 等）と連携可能

## 3. ログローテーション

### 決定

日付単位でファイル分割し、起動時に古いファイルを削除。

### 実装方式

```text
~/.llm-router/logs/
├── llm-router.jsonl.2025-12-01
├── llm-router.jsonl.2025-12-02
├── llm-router.jsonl.2025-12-03
└── ...
```

**Rust実装**:

```rust
use tracing_appender::rolling::{RollingFileAppender, Rotation};

let file_appender = RollingFileAppender::new(
    Rotation::DAILY,
    "~/.llm-router/logs",
    "llm-router.jsonl",
);
```

**C++実装**:

```cpp
// spdlog の daily_file_sink は自動で日付サフィックスを付与
auto sink = std::make_shared<spdlog::sinks::daily_file_sink_mt>(
    "~/.llm-router/logs/allm.jsonl", 0, 0);
```

### 古いファイル削除

起動時に `LLM_LOG_RETENTION_DAYS` 日より古いファイルを削除:

```rust
fn cleanup_old_logs(log_dir: &Path, retention_days: u32) {
    let cutoff = Utc::now() - Duration::days(retention_days as i64);
    for entry in fs::read_dir(log_dir)? {
        if let Ok(metadata) = entry.metadata() {
            if metadata.modified()? < cutoff.into() {
                fs::remove_file(entry.path())?;
            }
        }
    }
}
```

## 4. 非同期書き込み

### 決定

バッファリング + バックグラウンドスレッドで非同期出力。

### 理由

- 推論リクエストのレイテンシに影響を与えない
- I/O 待ちによるスレッドブロックを回避
- バッファ溢れ時はログ破棄（アプリケーション優先）

### 実装方式

**Rust**:

```rust
// tracing-appender は内部で非同期書き込みを実装
let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
```

**C++**:

```cpp
// spdlog の async_logger を使用
spdlog::init_thread_pool(8192, 1); // キューサイズ、スレッド数
auto async_logger = std::make_shared<spdlog::async_logger>(
    "allm",
    sinks,
    spdlog::thread_pool(),
    spdlog::async_overflow_policy::overrun_oldest
);
```

## 参考リソース

- [tracing - Rust](https://docs.rs/tracing)
- [tracing-subscriber](https://docs.rs/tracing-subscriber)
- [spdlog - C++](https://github.com/gabime/spdlog)
- [JSONL Specification](https://jsonlines.org/)
