# 共通ログシステム 実装計画

## 現状分析

### Load Balancer (Rust)

- **ファイル**: `llmlb/src/logging.rs`
- **出力先**: `~/.llmlb/logs/router.log.jsonl`（ファイルのみ）
- **形式**: JSON Lines（tracing-subscriber fmt::layer().json()）
- **ローテーション**: なし（append）
- **環境変数**: `LLMLB_LOG_LEVEL`, `RUST_LOG`, `LLMLB_DATA_DIR`

### Node (C++)

- **ファイル**: `node/src/utils/logger.cpp`, `node/include/utils/logger.h`
- **出力先**: stdout + オプションでファイル
- **形式**: テキスト or JSON（`LOG_FORMAT=json`）
- **ローテーション**: サイズベース（`LOG_MAX_SIZE_MB`, `LOG_MAX_FILES`）
- **環境変数**: `LOG_LEVEL`, `LOG_FILE`, `LOG_FORMAT`, `LOG_MAX_SIZE_MB`, `LOG_MAX_FILES`

## 技術設計

### ログディレクトリ構造

```text
~/.llmlb/
└── logs/
    ├── llmlb.jsonl.2025-11-28
    ├── llmlb.jsonl.2025-11-27
    ├── xllm.jsonl.2025-11-28
    └── xllm.jsonl.2025-11-27
```

### 環境変数（統一）

| 変数名 | 説明 | デフォルト値 |
|--------|------|-------------|
| `LLM_LOG_DIR` | ログディレクトリ | `~/.llmlb/logs` |
| `LLM_LOG_LEVEL` | ログレベル | `info` |
| `LLM_LOG_RETENTION_DAYS` | 保持日数 | `7` |

既存の環境変数（`LLMLB_LOG_LEVEL`, `LOG_LEVEL`等）も後方互換として維持。

### ログエントリ形式（共通）

```json
{"ts":"2025-11-28T12:00:00.000Z","level":"info","category":"api","msg":"Request received"}
```

## Load Balancer側修正

### 修正ファイル

- `llmlb/src/logging.rs`

### 変更内容

1. ログディレクトリを `~/.llmlb/logs/` に変更
2. ファイル名を `llmlb.jsonl.YYYY-MM-DD` に変更
3. 日付ベースローテーション実装（tracing-appender::rolling::daily）
4. 起動時に7日超の古いファイル削除
5. `target`フィールドを`category`として出力するカスタムフォーマッタ
6. 新環境変数のサポート

### 依存関係追加

```toml
[dependencies]
chrono = "0.4"
```

## Node側修正

### 修正ファイル

- `node/src/utils/logger.cpp`
- `node/include/utils/logger.h`

### 変更内容

1. デフォルト出力先を `~/.llmlb/logs/xllm.jsonl.YYYY-MM-DD` に変更
2. stdout出力を削除
3. 日付ベースローテーション実装（spdlog::sinks::daily_file_sink）
4. 起動時に7日超の古いファイル削除
5. カテゴリフィールド追加
6. 新環境変数のサポート

### 依存関係

spdlogの既存機能で対応可能（daily_file_sink_mtを使用）。

## 共通ログ読み取り

### 修正ファイル

- `common/src/log.rs`

### 変更内容

1. ログディレクトリパスを新形式に対応
2. 複数日付ファイルの読み取りサポート

## 実装順序（TDD）

### Phase 1: Load Balancer側

1. テスト作成（ローテーション、フォーマット、古いファイル削除）
2. `logging.rs`修正
3. テスト実行確認

### Phase 2: Node側

1. テスト作成（ローテーション、フォーマット、古いファイル削除）
2. `logger.cpp`/`logger.h`修正
3. テスト実行確認

### Phase 3: 統合確認

1. Load Balancer/Node両方を起動してログ出力確認
2. `GET /v0/logs` エンドポイント動作確認
