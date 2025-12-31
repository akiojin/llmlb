# SPEC-d7feaa2c: Tasks

## Note
- Nemotron向けの新エンジン（推論エンジン）は後日仕様化（TBD）。本タスクはエンジンローダー抽象化が主対象。

## Setup
- [x] Nodeエンジン抽象化の設計メモ整理
- [x] safetensors-cpp ヘッダをNode配下へ配置

## Core
- [x] ModelStorageの`metadata.json`依存を削除し、safetensors/GGUFを検出できるようにする
- [x] EngineインターフェースとEngineRegistryを追加
- [x] LlamaEngineを既存実装から切り出し
- [x] InferenceEngineからエンジン選択に委譲

## Plugin Migration（追加）
- [x] EngineHost（プラグインローダー）を導入する
- [x] Engine ABI/manifest の必須項目・ABI一致を検証する
- [x] Engine ABI/manifest をJSONとして定義し、互換性検証を実装する
- [x] llama.cpp を plugin 化してロードできるようにする

## Integration
- [x] /v1/models で未対応モデルを除外するロジック追加
- [x] Router/Nodeともに`metadata.json`を生成・参照しない
- [x] 登録時の選択（safetensors/GGUF）に基づき、Nodeが適切なEngineを選択できるようにする（`config.json`優先）

## Tests
- [x] ModelStorage: safetensors/GGUF検出テスト
- [x] Engine選択テスト
- [x] EngineHost: プラグイン manifest 検証テスト

## Docs
- [x] Nodeのモデル登録/選択仕様をREADMEに追記（metadata.jsonなし）

## Polish
- [x] ログとエラーメッセージを整理

## Spike（任意）
- [x] NemotronEngineを追加（mmap + validation）
- [x] NemotronEngineロードテスト

## EngineHost拡張（Session 2025-12-31）

### ホットリロード
- [ ] T101 プラグインのシャドウロード機能を実装
  - 新プラグインを旧と並行してロード
  - 新規リクエストを新プラグインに振り分け
- [ ] T102 旧プラグインのグレースフルシャットダウン
  - 処理中リクエスト完了を待機
  - 完了後にアンロード

### リソース監視
- [ ] T103 VRAM使用率ポーリング（1秒間隔）
  - GPU API経由で使用率を取得
  - 90%閾値でアラート
- [ ] T104 RAM使用率ポーリング（1秒間隔）
  - システムAPI経由で使用率を取得
  - 90%閾値でアラート
- [ ] T105 閾値超過時のLRUアンロード
  - 最も古いモデルを自動アンロード
  - 閾値以下になるまで繰り返し

### VRAM管理
- [ ] T106 プラグインからのVRAM必要量申告API
  - ロード前に必要VRAMを問い合わせ
  - 不足時はロード拒否エラー
- [ ] T107 複数プラグイン間のVRAM配分管理
  - ホストが各プラグインにVRAM割当を指示
  - 割当て超過を監視

### KVキャッシュ
- [ ] T108 リクエスト独立KVキャッシュの実装
  - 各リクエストが独自のKVキャッシュを持つ
  - リクエスト完了時に即座に解放

### エラーコード
- [ ] T109 エラーコード体系の定義（C ABIヘッダー）
  - 10種類の粗粒度コード
  - メッセージ文字列付与機構
- [ ] T110 エラーコードのドキュメント化

### ストリーミング
- [ ] T111 トークン単位即時送信の実装
  - 1トークン生成ごとにコールバック
  - バッファリングなし

### タイムアウト
- [ ] T112 30秒ウォッチドッグの強制終了処理
  - タイムアウト検出後即座に強制終了
  - VRAM解放処理

## Tests（Session 2025-12-31）
- [ ] T113 ホットリロードテスト
- [ ] T114 リソース監視閾値テスト
- [ ] T115 VRAM不足時ロード拒否テスト
- [ ] T116 タイムアウト強制終了テスト

## Deferred（TBD）
- Nemotron向けの新エンジン（推論エンジン）の仕様策定（別SPEC）
- Nemotron向けの新エンジン（推論エンジン）の実装（Metal/DirectML）
- Router側: HF chat_template(Jinja) を完全互換でレンダリングし、Nodeへ最終プロンプトを渡す方針の具体化（別SPEC想定）
- Nemotron GPU PoC: safetensors直読→GPU演算→E2E生成までの段階的検証（別SPEC想定）
