# データモデル: llmlb CLIコマンド

**機能ID**: `SPEC-669176b2` | **日付**: 2026-01-08

## エンティティ定義

### 1. Model

ローカルに保存されたLLMモデル。

```
Model
├── name: string              # モデル名 (例: "meta-llama/Llama-3.2-3B-Instruct")
├── alias: string?            # 短縮名 (例: "llama3.2-3b")
├── path: string              # ファイルパス
├── format: enum              # GGUF | Safetensors
├── size_bytes: uint64        # ファイルサイズ
├── architecture: string      # アーキテクチャ (例: "llama")
├── quantization: string?     # 量子化 (例: "Q4_K_M")
├── parameters: uint64?       # パラメータ数
├── context_length: uint32?   # コンテキスト長
├── source: enum              # Local | Ollama | HuggingFace
├── created_at: datetime      # 作成日時
└── last_used_at: datetime?   # 最終使用日時
```

**検証ルール**:

- name: 空でない、英数字・`/`・`-`・`_`・`.`のみ
- path: 存在するファイルパス
- format: GGUF または Safetensors
- size_bytes: 0より大きい

### 2. Node

推論を実行するサーバー。

```
Node
├── id: uuid                  # ノードID
├── host: string              # ホスト名/IP
├── port: uint16              # ポート番号
├── status: enum              # Running | Stopped | Error
├── loaded_models: [string]   # ロード中モデル名リスト
├── vram_total_bytes: uint64  # 総VRAM
├── vram_used_bytes: uint64   # 使用中VRAM
├── gpu_temperature: float?   # GPU温度 (℃)
└── uptime_secs: uint64       # 稼働時間
```

**状態遷移**:

```
Stopped → Running (serve起動)
Running → Stopped (Ctrl+C / stop)
Running → Error (異常終了)
Error → Running (再起動)
```

### 3. Session

REPLの対話セッション。

```
Session
├── id: uuid                  # セッションID
├── model_name: string        # 使用モデル名
├── history: [Message]        # 会話履歴
├── settings: SessionSettings # セッション設定
├── created_at: datetime      # 開始日時
└── token_count: uint64       # 累計トークン数
```

### 4. Message

会話履歴の1メッセージ。

```
Message
├── role: enum                # User | Assistant | System
├── content: string           # テキスト内容
├── images: [ImageData]?      # 画像データ (Vision用)
├── thinking: string?         # 思考過程 (Reasoning用)
└── timestamp: datetime       # タイムスタンプ
```

### 5. SessionSettings

セッション設定。

```
SessionSettings
├── temperature: float        # 温度 (0.0-2.0, default: 0.7)
├── top_p: float              # Top-p (0.0-1.0, default: 0.9)
├── max_tokens: uint32?       # 最大トークン数
├── show_thinking: bool       # 思考過程表示 (default: false)
└── stream: bool              # ストリーミング (default: true)
```

### 6. DownloadProgress

ダウンロード進捗情報。

```
DownloadProgress
├── model_name: string        # ダウンロード中モデル名
├── total_bytes: uint64       # 総バイト数
├── downloaded_bytes: uint64  # ダウンロード済みバイト数
├── speed_bps: uint64         # 現在の速度 (bytes/sec)
├── eta_secs: uint32?         # 残り時間 (秒)
└── status: enum              # Pending | Downloading | Completed | Failed
```

### 7. OllamaModel

ollama参照用モデル情報。

```
OllamaModel
├── name: string              # ollamaモデル名 (例: "llama3.2")
├── manifest_path: string     # manifest.jsonパス
├── blob_digest: string       # blobのsha256ダイジェスト
├── blob_path: string         # blobファイルパス
├── size_bytes: uint64        # ファイルサイズ
└── readonly: bool            # 常にtrue (読み取り専用)
```

## API契約

### CLIコマンド一覧

| コマンド | 説明 | 引数 |
|---------|------|------|
| `llmlb node serve` | サーバー起動 | なし |
| `llmlb node run <model>` | REPL起動 | model名, --think |
| `llmlb node pull <model>` | ダウンロード | model名/URL |
| `llmlb node list` | モデル一覧 | なし |
| `llmlb node show <model>` | 詳細表示 | model名, --license |
| `llmlb node rm <model>` | 削除 | model名 |
| `llmlb node stop <model>` | アンロード | model名 |
| `llmlb node ps` | 実行中一覧 | なし |
| `llmlb router nodes` | ノード一覧 | なし |
| `llmlb router models` | モデル配信状況 | なし |
| `llmlb router status` | クラスタ状態 | なし |

### 終了コード

| コード | 意味 |
|--------|------|
| 0 | 成功 |
| 1 | 一般エラー |
| 2 | 接続エラー |

### 環境変数

| 変数名 | デフォルト | 説明 |
|--------|----------|------|
| `LLMLB_HOST` | `127.0.0.1:32769` | サーバー接続先 |
| `LLMLB_DEBUG` | `false` | デバッグログ有効化 |
| `HF_TOKEN` | なし | HuggingFace認証トークン |

## 出力フォーマット

### `list` コマンド出力

```
NAME                                    SIZE     MODIFIED
meta-llama/Llama-3.2-3B-Instruct       6.4 GB   2 hours ago
deepseek/deepseek-r1-0528              14.2 GB  1 day ago
ollama:llama3.2 (readonly)             4.1 GB   3 days ago
```

### `ps` コマンド出力

```
NAME                              ID       SIZE     PROCESSOR  UNTIL        VRAM    TEMP
meta-llama/Llama-3.2-3B-Instruct  abc123   6.4 GB   100% GPU   4 minutes    85%     62°C
```

### `show` コマンド出力

```
Model: meta-llama/Llama-3.2-3B-Instruct
  Architecture: llama
  Parameters: 3.21B
  Quantization: Q4_K_M
  Context Length: 131072
  Format: GGUF
  Size: 6.4 GB
  License: Llama 3.2 Community License
```

### `pull` プログレス表示

```
pulling manifest ✓
pulling abc123def456... 100% ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓ 6.4 GB
verifying sha256 digest ✓
writing manifest ✓
success
```
