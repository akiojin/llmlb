# データモデル: 共通ログシステム

## エンティティ定義

### ログエントリ（共通形式）

```rust
// router/src/logging/mod.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// ログレベル
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// ログカテゴリ
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogCategory {
    System,     // 起動/終了/設定
    Api,        // HTTPリクエスト/レスポンス
    Model,      // モデルロード/アンロード
    Inference,  // 推論処理
    Sync,       // モデル同期
    Repair,     // 自動修復
    Health,     // ヘルスチェック/ハートビート
}

/// ログエントリ（JSONL形式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// タイムスタンプ（ISO 8601）
    pub ts: DateTime<Utc>,

    /// ログレベル
    pub level: LogLevel,

    /// カテゴリ
    pub category: LogCategory,

    /// メッセージ
    pub msg: String,

    /// リクエストID（オプション）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,

    /// モデル名（オプション）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// ノードID（オプション）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_id: Option<String>,

    /// 追加フィールド（動的）
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}
```

### C++ 版ログエントリ

```cpp
// node/include/logging/log_entry.h

#include <chrono>
#include <string>
#include <optional>
#include <nlohmann/json.hpp>

namespace llm_router {

/// ログレベル
enum class LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error
};

/// ログカテゴリ
enum class LogCategory {
    System,
    Api,
    Model,
    Inference,
    Sync,
    Repair,
    Health
};

/// ログエントリ
struct LogEntry {
    std::chrono::system_clock::time_point ts;
    LogLevel level;
    LogCategory category;
    std::string msg;
    std::optional<std::string> request_id;
    std::optional<std::string> model;
    std::optional<std::string> runtime_id;
    nlohmann::json extra;

    /// JSONL形式にシリアライズ
    std::string to_jsonl() const;
};

} // namespace llm_router
```

### ログ設定

```rust
// router/src/logging/config.rs

/// ログ設定
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// ログディレクトリ
    pub log_dir: std::path::PathBuf,

    /// ログレベル
    pub level: LogLevel,

    /// 保持日数
    pub retention_days: u32,

    /// 非同期書き込みのバッファサイズ
    pub buffer_size: usize,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            log_dir: dirs::home_dir()
                .unwrap_or_default()
                .join(".llm-router")
                .join("logs"),
            level: LogLevel::Info,
            retention_days: 7,
            buffer_size: 8192,
        }
    }
}

impl LogConfig {
    /// 環境変数から設定を読み込む
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(dir) = std::env::var("LLM_LOG_DIR") {
            config.log_dir = dir.into();
        }

        if let Ok(level) = std::env::var("LLM_LOG_LEVEL") {
            config.level = level.parse().unwrap_or(LogLevel::Info);
        }

        if let Ok(days) = std::env::var("LLM_LOG_RETENTION_DAYS") {
            config.retention_days = days.parse().unwrap_or(7);
        }

        config
    }
}
```

## 検証ルール

| フィールド | ルール | エラーメッセージ |
|-----------|--------|-----------------|
| `ts` | 有効なISO 8601形式 | "Invalid timestamp format" |
| `level` | 許可された値のみ | "Invalid log level" |
| `category` | 許可された値のみ | "Invalid log category" |
| `msg` | 空でない | "Empty log message" |
| `retention_days` | 1以上365以下 | "Retention days out of range" |

## 関係図

```text
┌─────────────────────────────────────────────────────────────┐
│                     ログシステム                             │
│                                                             │
│  ┌─────────────────┐                                        │
│  │    LogConfig    │ ← 環境変数 (LLM_LOG_*)                 │
│  │  - log_dir      │                                        │
│  │  - level        │                                        │
│  │  - retention    │                                        │
│  └────────┬────────┘                                        │
│           │                                                 │
│           ▼                                                 │
│  ┌─────────────────┐    ┌──────────────────┐               │
│  │   LogWriter     │───→│   LogEntry       │               │
│  │  - file_sink    │    │  - ts            │               │
│  │  - console_sink │    │  - level         │               │
│  └────────┬────────┘    │  - category      │               │
│           │             │  - msg           │               │
│           │             │  - extra         │               │
│           ▼             └──────────────────┘               │
│  ┌─────────────────────────────────────────┐               │
│  │              Output                      │               │
│  │  ┌──────────────┐  ┌─────────────────┐  │               │
│  │  │ JSONL File   │  │ Console (Pretty)│  │               │
│  │  │ (非同期)      │  │ (同期)          │  │               │
│  │  └──────────────┘  └─────────────────┘  │               │
│  └─────────────────────────────────────────┘               │
└─────────────────────────────────────────────────────────────┘
```

## ファイル構造

```text
~/.llm-router/
└── logs/
    ├── llm-router.jsonl.2025-12-01
    ├── llm-router.jsonl.2025-12-02
    ├── llm-router.jsonl.2025-12-03
    ├── allm.jsonl.2025-12-01
    ├── allm.jsonl.2025-12-02
    └── allm.jsonl.2025-12-03
```

## APIレスポンス形式

### GET /v0/logs

```json
{
  "logs": [
    {
      "ts": "2025-12-01T12:00:00.000Z",
      "level": "info",
      "category": "api",
      "msg": "Request received",
      "request_id": "abc123"
    },
    {
      "ts": "2025-12-01T12:00:01.000Z",
      "level": "info",
      "category": "inference",
      "msg": "Generation complete",
      "model": "llama-3.2-1b",
      "tokens": 50
    }
  ],
  "total": 2,
  "has_more": false
}
```
