# xLLM 統合仕様 - 実装計画

## 概要

xLLMの全機能を統合的に実装するための計画。
既存実装の拡張と未実装機能の追加を段階的に行う。

## 現状分析

### 実装済み機能

| 機能 | ファイル | 状態 |
|------|----------|------|
| モデルロード/アンロード | core/llama_manager.cpp | 完了 |
| OpenAI API（chat/completions） | api/openai_endpoints.cpp | 完了 |
| OpenAI API（embeddings） | api/openai_endpoints.cpp | 完了 |
| Responses API | api/openai_endpoints.cpp:867 | 完了 |
| Vision処理 | core/vision_processor.cpp | 完了 |
| ASR（transcriptions） | api/audio_endpoints.cpp | 完了 |
| TTS（speech）バッチ | api/audio_endpoints.cpp | 完了 |
| 画像生成（generations） | api/image_endpoints.cpp | 部分完了 |
| CLI: pull | cli/commands/pull.cpp | 完了 |
| CLI: run | cli/commands/run.cpp | 完了 |
| CLI: list | cli/commands/list.cpp | 完了 |
| CLI: show | cli/commands/show.cpp | 完了 |
| CLI: rm | cli/commands/rm.cpp | 完了 |
| CLI: ps | cli/commands/ps.cpp | 完了 |
| CLI: stop | cli/commands/stop.cpp | 完了 |
| Continuous Batching | core/continuous_batch_scheduler.cpp | 完了 |
| Prefix Cache | core/prefix_cache.cpp | 完了 |
| KV量子化 | core/safetensors_engine.cpp | 完了 |

### 未実装/TODO

| 機能 | ファイル | 状態 |
|------|----------|------|
| CLI: serve | cli/commands/serve.cpp | TODO |
| CLI: router_* | cli/commands/router_*.cpp | TODO |
| 画像URL配信 | api/image_endpoints.cpp | TODO |
| 画像TTL自動削除 | - | 未実装 |
| TTSストリーミング | api/audio_endpoints.cpp | 未実装 |
| Tensor Parallelism自動化 | - | 未実装 |
| LRU自動アンロード | core/llama_manager.cpp | 部分実装 |
| Speculative Decoding | - | 未実装 |
| Function Calling | core/function_calling.cpp | 部分実装 |
| Prometheus Metrics | metrics/ | 未実装 |
| Modelfile完全互換 | - | 未実装 |
| config.yaml | - | 未実装 |

## 実装フェーズ

### Phase 1: サーバーモード完成（高優先）

1. **serve コマンド実装**
   - HTTPサーバーのフォアグラウンド起動
   - シグナルハンドリング（SIGINT/SIGTERM）
   - --port, --host オプション

2. **config.yaml対応**
   - 設定ファイルの読み込み
   - デフォルト値の定義
   - 環境変数オーバーライド

### Phase 2: メモリ管理強化

1. **LRU自動アンロード**
   - VRAM使用量監視
   - アクティブモデル保護
   - アンロード対象の選定

2. **Tensor Parallelism自動化**
   - GPU検出
   - VRAM計算
   - split比率自動決定

### Phase 3: マルチモーダル強化

1. **TTSストリーミング**
   - チャンク単位の音声生成
   - SSE/chunked transfer

2. **画像URL配信**
   - ローカルファイル保存
   - 静的ファイル配信エンドポイント
   - TTL自動削除

### Phase 4: 高度な推論機能

1. **Function Calling強化**
   - grammar/constrained decoding
   - ツール出力形式の強制

2. **Speculative Decoding**
   - ドラフトモデル管理
   - 検証ロジック

### Phase 5: 運用機能

1. **Prometheus Metrics**
   - /metrics エンドポイント
   - トークンスループット
   - VRAM使用率

2. **Modelfile完全互換**
   - パーサー実装
   - 全命令対応

### Phase 6: 追加CLI機能

1. **プロファイリング・ベンチマーク**
   - profile コマンド（VRAM/速度計測）
   - benchmark コマンド（トークン/秒）
   - compare コマンド（モデル比較）

2. **モデル変換・エクスポート**
   - convert コマンド（GGUF↔safetensors）
   - export/import コマンド

### Phase 7: 高度な機能

1. **Dynamic LoRA**
   - 実行時LoRAロード
   - 複数LoRAの切り替え

2. **KVキャッシュ永続化**
   - ディスクへの保存
   - 復元機能

3. **モデルレプリカ**
   - 同一モデルの複数インスタンス
   - スループット向上

### Phase 8: HTTP機能強化

1. **CORS対応**
   - config.yamlで設定可能
   - allowed_origins等

2. **圧縮・トレーシング**
   - Gzip圧縮
   - X-Request-IDヘッダー

3. **ログ管理**
   - ログローテーション
   - サイズ/日次ベース

## 技術スタック

- **言語**: C++17
- **HTTP**: cpp-httplib
- **JSON**: nlohmann/json
- **推論**: llama.cpp, whisper.cpp, stable-diffusion.cpp
- **オーディオ**: ONNX Runtime (TTS), miniaudio
- **ログ**: spdlog

## リスクと対策

| リスク | 対策 |
|--------|------|
| llama.cppのAPI変更 | フォーク運用で安定化 |
| VRAM不足 | LRU + graceful degradation |
| 長時間推論 | クライアント切断検知 |

## 依存関係

```text
Cleanup ──── Phase 1 ──┬── Phase 2 ──┬── Phase 4 ──┬── Phase 5
                       │             │             │
                       ├── Phase 3 ──┘             └── Phase 7
                       │
                       ├── Phase 6
                       │
                       └── Phase 8
```
