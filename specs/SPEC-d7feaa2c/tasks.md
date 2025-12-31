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

## マルチGPU・負荷分散（Session 2025-12-31 Part 2）
- [ ] T117 GPU列挙とVRAM空き容量取得の実装
- [ ] T118 リクエスト到着時のGPU自動選択ロジック
- [ ] T119 既存ロード済みGPUの優先選択

## 継続バッチング（Session 2025-12-31 Part 2）
- [ ] T120 Prefillキューの実装
- [ ] T121 Decodeバッチの実装
- [ ] T122 Prefill/Decode分離スケジューラ
- [ ] T123 decodeステップ間での動的リクエスト追加

## メトリクス収集（Session 2025-12-31 Part 2）
- [ ] T124 on_token_callbackの定義（C ABI）
- [ ] T125 ホスト側TTFT・トークン/秒計算

## プラグイン管理（Session 2025-12-31 Part 2）
- [ ] T126 プラグイン定期再起動ポリシーの実装
  - N時間経過 or Nリクエスト後に再起動
- [ ] T127 プラグインクラッシュ検出と即座再起動
- [ ] T128 manifest.jsonへのarchitecturesフィールド追加

## Tests（Session 2025-12-31 Part 2）
- [ ] T129 マルチGPU負荷分散テスト
- [ ] T130 継続バッチングテスト
- [ ] T131 プラグイン定期再起動テスト
- [ ] T132 クラッシュ後自動再起動テスト

## 推論キャッシュ（Session 2025-12-31 Part 3）
- [ ] T133 インメモリLRUキャッシュの実装
  - プロンプトハッシュをキーとしたキャッシュ
  - 温度0の場合のみキャッシュ有効
- [ ] T134 キャッシュサイズ上限管理（RAM割合ベース）
  - 利用可能RAMの一定割合を上限に設定
- [ ] T135 キャッシュヒット時の推論スキップ

## リトライ機構（Session 2025-12-31 Part 3）
- [ ] T136 指数バックオフリトライの実装
  - 初期100ms、最大4回、上限30秒
- [ ] T137 クラッシュ後の透過的リトライ
  - クライアントに見えない形でリトライ実行

## キャンセル処理（Session 2025-12-31 Part 3）
- [ ] T138 キャンセルフラグのチェック機構
  - トークン生成ループで毎回フラグ確認
- [ ] T139 即座キャンセル応答の実装
  - 次トークン生成前に必ず中断
- [ ] T140 バッチからのリクエスト除外処理
  - キャンセル時に他リクエストへ影響なし

## 並行ロード（Session 2025-12-31 Part 3）
- [ ] T141 複数モデル並行ロードの実装
  - VRAM空き確認後に並行ロード許可
- [ ] T142 プラグイン単一インスタンス管理
  - プラグインIDごとに1インスタンスのみ

## Tests（Session 2025-12-31 Part 3）
- [ ] T143 推論キャッシュヒット/ミステスト
- [ ] T144 指数バックオフリトライテスト
- [ ] T145 キャンセル即時応答テスト
- [ ] T146 並行ロードテスト

## パラメータ検証（Session 2025-12-31 Part 4）
- [ ] T147 サンプリングパラメータ範囲検証の実装
  - temperature/top_p/top_kの範囲チェック
  - 不正値は400エラー
- [ ] T148 空プロンプト検証の実装
  - 空/空白のみプロンプトを拒否

## stop sequences（Session 2025-12-31 Part 4）
- [ ] T149 stop sequences検出ロジックの実装
  - 生成ループ内でトークン列マッチング
  - 複数stop sequenceの同時監視

## logprobs（Session 2025-12-31 Part 4）
- [ ] T150 logprobs返却の実装
  - OpenAI互換フォーマット
  - top_logprobsパラメータ対応

## max_tokens（Session 2025-12-31 Part 4）
- [ ] T151 max_tokensデフォルト値の実装
  - config.jsonからmax_position_embeddings取得
  - プロンプト長を差し引いて計算

## アーキテクチャ検証（Session 2025-12-31 Part 4）
- [ ] T152 ロード前アーキテクチャ検証の実装
  - manifestとconfig.jsonの照合
  - 不一致時はロード開始前エラー

## フォーマット統合（Session 2025-12-31 Part 4）
- [ ] T153 manifest.jsonへのformatsフィールド追加
  - サポートフォーマット一覧を宣言
- [ ] T154 マルチフォーマットローダーの実装
  - ファイル形式に応じたローダー振り分け

## Tests（Session 2025-12-31 Part 4）
- [ ] T155 パラメータ検証テスト
- [ ] T156 stop sequences検出テスト
- [ ] T157 logprobs返却テスト
- [ ] T158 アーキテクチャ検証テスト

## Deferred（TBD）
- Nemotron向けの新エンジン（推論エンジン）の仕様策定（別SPEC）
- Nemotron向けの新エンジン（推論エンジン）の実装（Metal/DirectML）
- Router側: HF chat_template(Jinja) を完全互換でレンダリングし、Nodeへ最終プロンプトを渡す方針の具体化（別SPEC想定）
- Nemotron GPU PoC: safetensors直読→GPU演算→E2E生成までの段階的検証（別SPEC想定）
