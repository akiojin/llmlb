# xLLM 統合仕様

status: in-progress

## 概要

xLLMは、llama.cppベースのC++製ローカル推論エンジンである。
Ollamaと同等のCLI体験を提供しつつ、llmlbのエンドポイントとしても機能する。

## ビジネス価値

- ローカルGPUを活用した高速LLM推論の提供
- Ollamaからのシームレスな移行パス
- llmlbとの統合による分散推論環境の実現

## ユーザーストーリー

### US-001: モデルのダウンロードと実行

ユーザーとして、`xllm pull` でモデルをダウンロードし、
`xllm run` で即座に対話を開始したい。

### US-002: サーバーモードでの運用

ユーザーとして、`xllm serve` でHTTPサーバーを起動し、
OpenAI互換APIでアプリケーションから利用したい。

### US-003: マルチモーダル対応

ユーザーとして、画像認識（Vision）、音声認識（ASR）、
音声合成（TTS）、画像生成をローカルで実行したい。

## 機能要件

### FR-001: メモリ管理

- **VRAM管理**: LRU方式による自動アンロード
- **アクティブ保護**: 推論中のモデルはアンロード対象外
- **アイドルタイムアウト**: 設定時間未使用で自動アンロード
  - デフォルト: 300秒（5分）
  - config.yamlの `inference.idle_timeout` で設定可能
- **Tensor Parallelism**: 複数GPUへの自動分散
  - GPU数・VRAMを自動検出
  - **split比率**: VRAM比例配分で自動決定
  - 手動オーバーライドは不要（自動最適化のみ）
- **GPU Offload**: VRAMに入りきらない場合はRAMを併用
  - **方式**: 最大限度GPUに載せ、残りはRAM（n_gpu_layers自動調整）
- **モデル優先度**: config.yamlでVRAM保持優先度を設定可能（high/normal/low）

### FR-002: サーバーモード

- **デフォルトポート**: 32769
- **API認証**: なし（Ollama踏襲、localhost専用想定）
- **ヘルスチェック**: モデルロード中は `{"status":"loading"}` を返却（HTTP 200）
  - ロード済みモデル一覧、GPU/VRAM情報も含む
  - ロード進捗も `/v0/health` で取得可能
- **クライアント切断検知**: 推論中にクライアントが切断したら**即座に推論中止**
- **ログ出力**: stderr + ファイル（`--log-file` で有効化）
- **ログローテーション**: **日次ローテーション**（デフォルト）
- **デフォルトログレベル**: info（`XLLM_LOG=debug` で詳細化）
- **最大ペイロード**: 50MB（Vision画像もこの上限に含む）
- **CORS**: config.yamlで設定可能
  - **デフォルト**: `*`（全許可）
- **Gzip圧縮**: レスポンス圧縮をサポート
  - **閾値**: 10KB以上のレスポンスを圧縮
- **TraceID**: `X-Request-ID` ヘッダーをサポート
- **ストリーミング**: SSE（Server-Sent Events）形式、OpenAI互換
- **シグナルハンドリング**: 2段階方式
  - 1回目SIGINT/SIGTERM: Graceful shutdown（30秒タイムアウト）
  - 2回目: 強制終了
- **リトライ**: なし（即座にエラー返却）

### FR-003: モデル管理

- **ダウンロード元優先順位**: HuggingFace Hub > Ollama Registry
- **HuggingFace認証**: `HF_TOKEN` 環境変数で指定
- **ダウンロード失敗時**: レジュームなし、最初からやり直し
- **部分ダウンロードファイル**: 再起動時に自動削除
- **破損ファイル検出時**: 確認プロンプトを表示（自動再ダウンロードはしない）
- **保存先**: `~/.xllm/models/`（`XLLM_MODELS_DIR` でカスタマイズ可能）
- **整合性検証**: ダウンロード時のみSHA256チェック
- **PGP署名検証**: HuggingFaceの署名検証をサポート
- **メタデータキャッシュ**: モデルメタデータをキャッシュして高速化
- **対応形式**: llama.cppがサポートする全形式（GGUF、AWQ、GPTQ、EXL2等）+ safetensors
- **エンジン選択**: ファイル形式で自動判定（GGUF→llama.cpp、それ以外→safetensors.cpp）
  - safetensors.cppは `vendor/safetensors.cpp` を使用（ggmlバックエンド）
- **ローカルファイル**: `xllm run /path/to/model.gguf` で直接指定可能
- **複数バリアント**: 同一モデルの異なる量子化を同時ロード可能
- **モデルレプリカ**: 同一モデルを複数インスタンスでロード可能
  - **負荷分散**: ラウンドロビン
- **タグ形式**: Ollama互換（`model:tag` 形式）
- **モデル未指定時**: エラーを返却（自動選択なし）
- **自動アップデート確認**: `xllm list --check-updates` でHuggingFace APIにより新版確認
- **Preload設定**: CLIオプションとconfig.yamlの両方で指定可能
- **VRAM優先度**: モデルごとにVRAM保持優先度を設定可能
- **オンデマンドロード**: タイムアウトなし（ロード完了まで待機）

### FR-004: 推論機能

- **Continuous Batching**: 必須（複数リクエストの同時処理）
  - **デフォルトバッチサイズ**: 16
- **リクエストキュー**: バッチサイズ超過時はキューイング
  - **タイムアウト**: 無制限
  - **キュー上限**: 1000リクエスト
- **Prompt Cache**: 自動有効（llama.cppのprefix cache活用）
- **Speculative Decoding**: サポート
  - **指定方法**: CLI（`--draft-model`）、Modelfile、config.yaml、APIリクエスト
- **Context Length**: リクエスト毎に動的指定可能
- **Embeddings Batch**: 複数テキストの一括埋め込みをサポート
  - **バッチサイズ上限**: 無制限（メモリが許す限り）
- **Usage返却**: prompt_tokens, completion_tokens, total_tokensを常に返却
- **ストリーミング**: 1トークン毎に送信
- **ChatTemplate**: Modelfileで指定があれば優先、なければGGUFメタデータから自動取得
  - **テンプレート形式**: Jinja2（HuggingFace標準）
- **StopSequence**: Modelfileで指定があれば優先、なければGGUF/HFから自動取得
- **Capabilities検出**: GGUFメタデータ + HuggingFaceカタログの両方から取得
- **推論パラメータ**: Modelfileでデフォルト値を設定可能
- **Dynamic LoRA**: 実行時にLoRAアダプターをロード可能
  - **保存先**: `~/.xllm/loras/`
  - **指定方法**: リクエストパラメータで指定（自動ロード）
  - **複数LoRA**: サポート（マージして適用）
  - **スケール**: 指定可能（例: `{"lora": "name", "scale": 0.5}`）
- **KVキャッシュ永続化**: ディスクへのKVキャッシュ保存をサポート
  - **保存先**: `~/.xllm/cache/`
  - **保存タイミング**: リクエスト完了毎に差分保存

### FR-005: Function Calling

- **実装方式**: llama.cppのgrammar/constrained decoding活用
- **出力形式**: OpenAI API互換のみ（tools/function_callパラメータ）
- **優先度**: 高

### FR-006: 画像生成

- **バックエンド**: stable-diffusion.cpp（vendor/stable-diffusion.cpp）
- **モデル形式**: stable-diffusion.cppのデフォルトに従う
- **Storage**: ローカルファイル + URL配信
  - 保存先: `~/.xllm/images/`
  - 配信: `http://localhost:32769/images/xxx.png`
- **レスポンス形式**: OpenAI互換（response_formatでurl/b64_json選択可能）
- **TTL**: 1時間後に自動削除
- **サイズ**: 自由指定（OpenAI APIの固定サイズ制限なし）

### FR-007: 音声合成（TTS）

- **ストリーミング**: 必須（チャンク単位でオーディオ送信）
  - **チャンク送信タイミング**: 内部バッファ満杯時
- **出力形式**: OpenAI API互換

### FR-008: 音声認識（ASR）

- **言語自動検出**: languageパラメータ未指定時はwhisperの自動検出を使用
- **対応フォーマット**: whisper.cppネイティブサポート形式

### FR-009: safetensors対応

- **開発方針**: GGUFと並行して開発
- **現状**: KVキャッシュ量子化（kv_int8/kv_fp8）のみ対応済み

### FR-010: Prometheus Metrics

- **エンドポイント**: `/v0/metrics`
- **プレフィックス**: `xllm_`（例: `xllm_tokens_total`, `xllm_vram_bytes`）
- **メトリクス**: トークンスループット、VRAM使用率、リクエスト数等

### FR-011: エラー処理

- **エラーコード**: OpenAI API準拠（invalid_request_error, model_not_found等）
- **詳細度**: デバッグ情報を含む詳細なエラーメッセージ
  - llama.cppの内部エラー、ファイルパス、VRAM状況を返却
- **ログレベル**: `XLLM_LOG=debug` で詳細化

### FR-012: プログレス表示

- **形式**: Ollama互換（プログレスバー + パーセント + 速度）

## CLI仕様（Ollama踏襲）

### コマンド一覧

| コマンド | 説明 |
|----------|------|
| `xllm pull <model>` | モデルをダウンロード |
| `xllm run <model>` | モデルを実行（REPLモード） |
| `xllm serve` | HTTPサーバーを起動 |
| `xllm list` | ダウンロード済みモデル一覧 |
| `xllm show <model>` | モデル詳細を表示 |
| `xllm rm <model>` | モデルを削除 |
| `xllm ps` | 実行中モデル一覧 |
| `xllm stop <model>` | モデルを停止 |
| `xllm profile <model>` | モデルのプロファイリング（VRAM/速度計測） |
| `xllm benchmark <model>` | ベンチマーク実行（トークン/秒等） |
| `xllm compare <model1> <model2>` | モデル間の比較 |
| `xllm convert <input> <output>` | モデル形式変換（GGUF↔safetensors等） |
| `xllm export <model> <path>` | モデルをエクスポート |
| `xllm import <path>` | モデルをインポート |

### ps出力フォーマット（Ollama互換）

```text
NAME          ID            SIZE      PROCESSOR    UNTIL
llama3:8b     abc123def     4.7 GB    100% GPU     4 minutes from now
```

### ps出力（Ollama互換）

- **UNTIL計算**: Ollama互換方式

### REPLモード

- **マルチモーダル**: `/image /path/to/img.png` でファイルパス指定
- **マルチライン入力**: ヒアドキュメント形式（`<<<EOF` ... `EOF`）
- **ヒストリー**: `~/.xllm/history` に保存

### show コマンド

- **表示情報**: 全メタデータ（サイズ、量子化、パラメータ数、レイヤー数、コンテキスト長、Capabilities等）

### stop コマンド

- **動作**: リクエスト中止 + モデルアンロード（両方）

### profile コマンド

- **計測項目**:
  - VRAM使用量
  - モデルロード時間
  - First Token Latency
  - Tokens/sec

### benchmark コマンド

- **出力形式**: テキスト（デフォルト）、JSON（`--json`）、Markdown（`--report`）、Prometheus（`--prometheus`）

### compare コマンド

- **比較項目**:
  - 推論速度（Tokens/sec）
  - 品質（Perplexity）
  - VRAM消費
  - 同一プロンプトでの出力差分

### convert コマンド

- **変換方向**: 双方向（GGUF ↔ safetensors）
- **量子化オプション**: `--quantize Q4_K_M` 等で指定可能

### export コマンド

- **出力内容**: モデル + 全メタデータ（KVキャッシュ等を含む）

## 設定ファイル

### config.yaml

場所: `~/.xllm/config.yaml`

```yaml
server:
  port: 32769
  host: 0.0.0.0
  log_file: ~/.xllm/logs/xllm.log
  log_rotation:
    max_size: 100MB
    max_files: 10
  cors:
    allowed_origins:
      - "*"
    allowed_methods:
      - GET
      - POST
  gzip: true

models:
  directory: ~/.xllm/models
  preload:
    - llama3:8b
    - mistral:7b
  priority:
    llama3:8b: high

images:
  directory: ~/.xllm/images
  ttl_hours: 1

gpu:
  # 自動検出（手動設定は不要）

inference:
  idle_timeout: 300  # 秒
  request_timeout: 60  # 秒
```

### Modelfile（Ollama完全互換）

場所: `~/.xllm/Modelfiles/<model>/Modelfile`

サポートする命令:

- `FROM`: ベースモデル指定
- `PARAMETER`: パラメータ設定（Ollama完全互換）
- `SYSTEM`: システムプロンプト
- `TEMPLATE`: プロンプトテンプレート（**Jinja2形式**）
- `MESSAGE`: 初期メッセージ

### 設定優先順位

1. コマンドライン引数（最優先）
2. 環境変数
3. config.yaml（最低優先）

## API仕様

### OpenAI互換エンドポイント（/v1/）

| エンドポイント | 説明 |
|----------------|------|
| POST `/v1/chat/completions` | チャット補完 |
| POST `/v1/completions` | テキスト補完 |
| POST `/v1/embeddings` | 埋め込み |
| GET `/v1/models` | モデル一覧 |
| POST `/v1/audio/transcriptions` | ASR（音声認識） |
| POST `/v1/audio/speech` | TTS（音声合成） |
| POST `/v1/images/generations` | 画像生成 |
| POST `/v1/responses` | Responses API（完全互換） |

### xLLM独自エンドポイント（/v0/）

| エンドポイント | 説明 |
|----------------|------|
| GET `/v0/health` | ヘルスチェック（status + ロード済みモデル + GPU/VRAM） |
| GET `/v0/system` | システム情報（GPU/CPU/RAM/バージョン全て） |
| GET `/v0/metrics` | Prometheus形式メトリクス |

**注**: `/v0/models/load` と `/v0/models/unload` は**不要**（オンデマンドロード + LRUアンロードで管理）

## llmlb連携

**重要**: llmlbは単なるロードバランサーであり、xLLM→llmlbへのルートは存在しない。
llmlbが必要なデータはllmlbからのポーリング/API呼び出しで取得する。

- **連携方式**: xLLMが自律的にオンデマンドロード + LRUアンロード
- **セッション**: ステートレス（会話履歴はクライアント側で管理）
- **router_*コマンド**: 削除（llmlb CLIで同等の機能を提供）

## 将来対応予定

- モデルのホットリロード（ダウンロードなしでの切替）
- モデルエイリアス設定
- レートリミット

## 非機能要件

### NFR-001: 互換性

- Ollamaクライアント（CLI、ライブラリ）との互換性を維持
- OpenAI APIエラーコード準拠

### NFR-002: パフォーマンス

- GPU利用時、llama.cpp本体と同等のスループット

### NFR-003: 国際化

- ヘルプメッセージ・エラーメッセージは英語のみ

### NFR-004: バイナリ

- バイナリ名: `xllm`
- バージョン管理: llmlbと同一バージョン（モノレポ統一管理）

### NFR-005: テスト

- テストカバレッジ目標: 90%以上
- CI環境: GPU付きself-hosted runner + ローカルでのE2E
- ドキュメント: README.md + コードコメント

## 成功基準

- [ ] 全CLIコマンドがOllamaと同等に動作
- [ ] OpenAI互換APIが正常に動作
- [ ] マルチGPU環境でTensor Parallelismが動作
- [ ] LRUによるVRAM管理が正常に動作
- [ ] TTSストリーミングが正常に動作
- [ ] 画像生成のURL配信が正常に動作
- [ ] Prometheus metricsが正常に動作

## 関連SPEC

- SPEC-1a74455c: xLLM Responses API
- SPEC-e03a404c: xLLM Vision対応
- SPEC-afa11b7d: safetensors量子化対応
- SPEC-d7feaa2c: xLLMエンジン管理
