# SPEC-d7feaa2c: Tasks

## Note
- Nemotron向けの新エンジン（推論エンジン）は後日仕様化（TBD）。本タスクはエンジンローダー抽象化が主対象。
- Windows CUDA（gpt-oss API互換）による Nemotron 実行パスは実装済み。詳細は SPEC 本文を参照。

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
- [x] Nemotron CUDA（Windows）実行パスの追加（DLLロード/アーティファクト選択）

## EngineHost拡張（Session 2025-12-31）

### ホットリロード
- [x] T101 プラグインのシャドウロード機能を実装
  - 新プラグインを旧と並行してロード
  - 新規リクエストを新プラグインに振り分け
- [x] T102 旧プラグインのグレースフルシャットダウン
  - 処理中リクエスト完了を待機
  - 完了後にアンロード

### リソース監視
- [x] T103 VRAM使用率ポーリング（1秒間隔）
  - GPU API経由で使用率を取得
  - 90%閾値でアラート
- [x] T104 RAM使用率ポーリング（1秒間隔）
  - システムAPI経由で使用率を取得
  - 90%閾値でアラート
- [x] T105 閾値超過時のLRUアンロード
  - 最も古いモデルを自動アンロード
  - 閾値以下になるまで繰り返し

### VRAM管理
- [x] T106 プラグインからのVRAM必要量申告API
  - ロード前に必要VRAMを問い合わせ
  - 不足時はロード拒否エラー
- [x] T107 複数プラグイン間のVRAM配分管理
  - ホストが各プラグインにVRAM割当を指示
  - 割当て超過を監視

### KVキャッシュ
- [x] T108 リクエスト独立KVキャッシュの実装
  - 各リクエストが独自のKVキャッシュを持つ
  - リクエスト完了時に即座に解放

### エラーコード
- [x] T109 エラーコード体系の定義（C ABIヘッダー）
  - 10種類の粗粒度コード
  - メッセージ文字列付与機構
- [x] T110 エラーコードのドキュメント化

### ストリーミング
- [x] T111 トークン単位即時送信の実装
  - 1トークン生成ごとにコールバック
  - バッファリングなし

### タイムアウト
- [x] T112 30秒ウォッチドッグの強制終了処理
  - タイムアウト検出後即座に強制終了
  - VRAM解放処理

## Tests（Session 2025-12-31）
- [x] T113 ホットリロードテスト
- [x] T114 リソース監視閾値テスト
- [x] T115 VRAM不足時ロード拒否テスト
- [x] T116 タイムアウト強制終了テスト

## マルチGPU・負荷分散（Session 2025-12-31 Part 2）
- [x] T117 GPU列挙とVRAM空き容量取得の実装
- [x] T118 リクエスト到着時のGPU自動選択ロジック
- [x] T119 既存ロード済みGPUの優先選択

## 継続バッチング（Session 2025-12-31 Part 2）
- [x] T120 Prefillキューの実装
- [x] T121 Decodeバッチの実装
- [x] T122 Prefill/Decode分離スケジューラ
- [x] T123 decodeステップ間での動的リクエスト追加

## メトリクス収集（Session 2025-12-31 Part 2）
- [x] T124 on_token_callbackの定義（C ABI）
- [x] T125 ホスト側TTFT・トークン/秒計算

## プラグイン管理（Session 2025-12-31 Part 2）
- [x] T126 プラグイン定期再起動ポリシーの実装
  - N時間経過 or Nリクエスト後に再起動
- [x] T127 プラグインクラッシュ検出と即座再起動
- [x] T128 manifest.jsonへのarchitecturesフィールド追加

## Tests（Session 2025-12-31 Part 2）
- [x] T129 マルチGPU負荷分散テスト
- [x] T130 継続バッチングテスト
- [x] T131 プラグイン定期再起動テスト
- [x] T132 クラッシュ後自動再起動テスト

## 推論キャッシュ（Session 2025-12-31 Part 3）
- [x] T133 インメモリLRUキャッシュの実装
  - プロンプトハッシュをキーとしたキャッシュ
  - 温度0の場合のみキャッシュ有効
- [x] T134 キャッシュサイズ上限管理（RAM割合ベース）
  - 利用可能RAMの一定割合を上限に設定
- [x] T135 キャッシュヒット時の推論スキップ

## リトライ機構（Session 2025-12-31 Part 3）
- [x] T136 指数バックオフリトライの実装
  - 初期100ms、最大4回、上限30秒
- [x] T137 クラッシュ後の透過的リトライ
  - クライアントに見えない形でリトライ実行

## キャンセル処理（Session 2025-12-31 Part 3）
- [x] T138 キャンセルフラグのチェック機構
  - トークン生成ループで毎回フラグ確認
  - InferenceParamsにcancellation_tokenを追加
- [x] T139 即座キャンセル応答の実装
  - 次トークン生成前に必ず中断
  - GenerationCancelledExceptionでリトライをスキップ
- [x] T140 バッチからのリクエスト除外処理
  - キャンセル時に他リクエストへ影響なし
  - 各リクエストが独自のcancellation_tokenを持つ

## 並行ロード（Session 2025-12-31 Part 3）
- [ ] T141 複数モデル並行ロードの実装
  - VRAM空き確認後に並行ロード許可
- [ ] T142 プラグイン単一インスタンス管理
  - プラグインIDごとに1インスタンスのみ

## Tests（Session 2025-12-31 Part 3）
- [x] T143 推論キャッシュヒット/ミステスト
- [x] T144 指数バックオフリトライテスト
- [x] T145 キャンセル即時応答テスト
- [ ] T146 並行ロードテスト

## パラメータ検証（Session 2025-12-31 Part 4）
- [x] T147 サンプリングパラメータ範囲検証の実装
  - temperature/top_p/top_kの範囲チェック
  - 不正値は400エラー
- [x] T148 空プロンプト検証の実装
  - 空/空白のみプロンプトを拒否

## stop sequences（Session 2025-12-31 Part 4）
- [x] T149 stop sequences検出ロジックの実装
  - 生成ループ内でトークン列マッチング
  - 複数stop sequenceの同時監視

## logprobs（Session 2025-12-31 Part 4）
- [x] T150 logprobs返却の実装
  - OpenAI互換フォーマット
  - top_logprobsパラメータ対応

## max_tokens（Session 2025-12-31 Part 4）
- [x] T151 max_tokensデフォルト値の実装
  - config.jsonからmax_position_embeddings取得
  - プロンプト長を差し引いて計算

## アーキテクチャ検証（Session 2025-12-31 Part 4）
- [x] T152 ロード前アーキテクチャ検証の実装
  - manifestとconfig.jsonの照合
  - 不一致時はロード開始前エラー

## フォーマット統合（Session 2025-12-31 Part 4）
- [x] T153 manifest.jsonへのformatsフィールド追加
  - サポートフォーマット一覧を宣言
- [x] T154 マルチフォーマットローダーの実装
  - ファイル形式に応じたローダー振り分け

## Tests（Session 2025-12-31 Part 4）
- [x] T155 パラメータ検証テスト
- [x] T156 stop sequences検出テスト
- [x] T157 logprobs返却テスト
- [x] T158 アーキテクチャ検証テスト

## 量子化・モデル指定（Session 2025-12-31 Part 5）
- [x] T159 量子化指定パーサーの実装
  - `modelname:quantization` フォーマットのパース
  - 完全一致マッチング（大文字小文字・記号区別）
- [x] T160 デフォルト量子化の設定機構
  - モデル登録時にデフォルト量子化を指定
  - 未指定時のフォールバック

## Prefix Caching（Session 2025-12-31 Part 5）
- [ ] T161 Prefix Cacheの実装
  - 同一プレフィックスのKVキャッシュ共有
  - プロンプトハッシュをキーとした管理
- [ ] T162 Prefix Cache VRAM割当管理
  - 空きVRAMの割合ベースで上限設定
  - LRUによるエントリ削除

## Vision対応（Session 2025-12-31 Part 5）
- [ ] T163 mmproj自動検出の実装
  - モデルディレクトリ内のmmproj検索
  - 自動ロード機構

## スケーラビリティ（Session 2025-12-31 Part 5）
- [ ] T164 レプリカ配置の実装
  - 同一モデルの複数GPUロード
  - レプリカステータス管理
- [ ] T165 ラウンドロビン負荷分散の実装
  - レプリカ間でのリクエスト振り分け
  - 障害レプリカのスキップ

## chat_template（Session 2025-12-31 Part 5）
- [x] T166 injaライブラリ統合
  - llama.cpp内蔵のminja（Jinja2パーサー）を使用
  - CMakeLists.txtにvendorインクルードパスを追加
- [x] T167 chat_templateレンダリングの実装
  - ChatTemplateRendererクラスを新規作成
  - config.jsonからのテンプレート読み込み対応
  - messagesの変換とレンダリング実装

## Function Calling（Session 2025-12-31 Part 5）
- [ ] T168 Function Calling検出の実装
  - ツール定義のプロンプト埋め込み
  - 出力からのJSON検出
- [ ] T169 finish_reason="tool_calls"対応
  - ツール呼び出し検出時のレスポンス整形

## manifest.json拡張（Session 2025-12-31 Part 5）
- [x] T170 manifest.jsonへのmodalities追加
  - completion/embeddingモードの宣言
- [x] T171 manifest.jsonへのlicense追加
  - ライセンス情報フィールド
- [x] T172 manifest.jsonへのsupports_vision追加
  - Vision対応フラグ

## Tests（Session 2025-12-31 Part 5）
- [x] T173 量子化指定パーステスト
- [ ] T174 Prefix Cacheヒット/ミステスト
- [ ] T175 mmproj自動検出テスト
- [ ] T176 レプリカ負荷分散テスト
- [x] T177 chat_templateレンダリングテスト
- [ ] T178 Function Calling検出テスト

## VRAM部分ロード障害（Session 2025-12-31 Part 6）
- [ ] T179 部分ロード時VRAM不足の即時全解放実装
  - ロード済みテンソルの追跡
  - OOM検出時の全解放処理
  - クリーン状態への復帰

## 量子化選択ポリシー（Session 2025-12-31 Part 6）
- [x] T180 量子化厳密マッチング実装
  - 自動アップグレード無効化
  - 指定量子化の厳密使用
  - 未登録量子化のエラー返却

## クラッシュ後処理（Session 2025-12-31 Part 6）
- [x] T181 クラッシュ後即時503返却の実装
  - プラグインクラッシュ検出
  - 再起動待ちなしの即時エラー
  - 再起動完了までの新規リクエスト拒否

## ストリーミングハング検出（Session 2025-12-31 Part 6）
- [x] T182 トークン間タイムアウト（5秒）の実装
  - 最終トークンからの経過時間追跡
  - 5秒タイムアウトでハング判定
  - 強制終了とエラー返却

## プラグインログ統合（Session 2025-12-31 Part 6）
- [ ] T183 プラグインログのホスト統合実装
  - stdout/stderrのキャプチャ
  - プラグインIDプレフィックス付与
  - タイムスタンプとログレベル付与

## ロード進捗API（Session 2025-12-31 Part 6）
- [ ] T184 ロード進捗非公開ポリシーの実装
  - 開始/完了/失敗のみ通知
  - 進捗率API無し
  - 内部デバッグ用追跡（オプション）

## モダリティ処理（Session 2025-12-31 Part 6）
- [ ] T185 モダリティFIFO処理の実装
  - Completion/Embedding区別なし
  - 到着順処理の確認
  - 優先度なしの動作確認

## Tests（Session 2025-12-31 Part 6）
- [ ] T186 VRAM部分ロード障害テスト
- [x] T187 量子化厳密マッチングテスト
- [ ] T188 クラッシュ後503即時返却テスト
- [ ] T189 トークン間タイムアウトテスト
- [ ] T190 プラグインログ統合テスト

## Deferred（TBD）
- Nemotron向けの新エンジン（推論エンジン）の仕様策定（別SPEC）
- Nemotron向けの新エンジン（推論エンジン）の実装（Metal/CUDA）
- Router側: HF chat_template(Jinja) を完全互換でレンダリングし、Nodeへ最終プロンプトを渡す方針の具体化（別SPEC想定）
- Nemotron GPU PoC: safetensors直読→GPU演算→E2E生成までの段階的検証（別SPEC想定）
