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
  - **完了**: gpu_detector_test.cppにSelectGpuPrefersLoadedDevice/SelectGpuChoosesMostFreeMemory/SelectGpuSkipsUnavailableDevicesとして実装済み
- [x] T130 継続バッチングテスト
- [x] T131 プラグイン定期再起動テスト
- [x] T132 クラッシュ後自動再起動テスト

## 推論キャッシュ（Session 2025-12-31 Part 3）
- [x] T133 インメモリLRUキャッシュの実装
  - プロンプトハッシュをキーとしたキャッシュ
  - 温度0の場合のみキャッシュ有効
  - **完了**: inference_cache.h/cppにLRUキャッシュを実装、FNV-1aハッシュ使用
- [x] T134 キャッシュサイズ上限管理（RAM割合ベース）
  - 利用可能RAMの一定割合を上限に設定
  - **完了**: withRamLimit()で利用可能RAM割合ベースの上限設定、LRU削除実装
- [x] T135 キャッシュヒット時の推論スキップ
  - **完了**: get()でキャッシュヒット時に結果を返却、hit/miss統計追跡

## リトライ機構（Session 2025-12-31 Part 3）
- [x] T136 指数バックオフリトライの実装
  - 初期100ms、最大4回、上限30秒
  - **完了**: inference_engine.cppにwith_retry()テンプレート関数を追加、100ms→200ms→400ms→800msの指数バックオフ
- [x] T137 クラッシュ後の透過的リトライ
  - クライアントに見えない形でリトライ実行
  - **完了**: generateChat/generateCompletion/generateEmbeddingsでwith_retryを使用、handlePluginCrash()をリトライ時に呼び出し

## キャンセル処理（Session 2025-12-31 Part 3）
- [x] T138 キャンセルフラグのチェック機構
  - トークン生成ループで毎回フラグ確認
  - **完了**: Request構造体にis_cancelledコールバック追加、step()開始時にremoveCancelledRequests()実行
- [x] T139 即座キャンセル応答の実装
  - 次トークン生成前に必ず中断
  - **完了**: prefill/decode_step実行前にis_cancelledチェックを追加
- [x] T140 バッチからのリクエスト除外処理
  - キャンセル時に他リクエストへ影響なし
  - **完了**: cancel(request_id)関数で指定IDのみ除外、他リクエストは継続

## 並行ロード（Session 2025-12-31 Part 3）
- [x] T141 複数モデル並行ロードの実装
  - VRAM空き確認後に並行ロード許可
  - **完了**: LlamaManagerにestimateVramRequired(), canLoadConcurrently(), markAsLoading(), markAsLoaded()を実装
- [x] T142 プラグイン単一インスタンス管理
  - プラグインIDごとに1インスタンスのみ
  - **完了**: engine_host.cppのstagePlugin/applyPendingPluginsでengine_idによる重複チェック・置換を実装済み

## Tests（Session 2025-12-31 Part 3）
- [x] T143 推論キャッシュヒット/ミステスト
  - **完了**: inference_cache_test.cppに11個のテストケース（LRU削除、hit/miss統計、温度チェック等）
- [x] T144 指数バックオフリトライテスト
  - **完了**: inference_engine_test.cppに6個のテストケース（成功・リトライ・最大リトライ・コールバック・指数バックオフ・総時間制限）
- [x] T145 キャンセル即時応答テスト
  - **完了**: continuous_batch_scheduler_test.cppにCancelFlagSkipsRequest/CancelDuringDecodeSkipsImmediately/CancelByIdDoesNotAffectOthers/CancelledCountTracksStateテストを追加
- [x] T146 並行ロードテスト
  - **完了**: llama_manager_test.cppに6個のテストケース（VRAM推定、並行ロード許可、ロード中追跡等）

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
- [x] T161 Prefix Cacheの実装
  - 同一プレフィックスのKVキャッシュ共有
  - プロンプトハッシュをキーとした管理
  - **完了**: prefix_cache.h/cppにPrefixCacheクラス実装、FNV-1aハッシュでプレフィックス管理
- [x] T162 Prefix Cache VRAM割当管理
  - 空きVRAMの割合ベースで上限設定
  - LRUによるエントリ削除
  - **完了**: withVramLimit()でVRAM割合指定、evictIfNeeded()でLRU削除

## Vision対応（Session 2025-12-31 Part 5）
- [x] T163 mmproj自動検出の実装
  - モデルディレクトリ内のmmproj検索
  - 自動ロード機構
  - **完了**: vision_processor.cppのresolveMmprojPath()がメタデータ検索→ディレクトリ走査で自動検出

## スケーラビリティ（Session 2025-12-31 Part 5）
- [x] T164 レプリカ配置の実装
  - 同一モデルの複数GPUロード
  - レプリカステータス管理
  - **完了**: replica_manager.h/cppにReplicaManagerクラス実装、GPU IDベースのレプリカ登録・解除
- [x] T165 ラウンドロビン負荷分散の実装
  - レプリカ間でのリクエスト振り分け
  - 障害レプリカのスキップ
  - **完了**: selectReplica()でラウンドロビン選択、markReplicaFailed/Healthyで障害管理

## chat_template（Session 2025-12-31 Part 5）
- [x] T166 injaライブラリ統合
  - C++ Jinjaライブラリの導入
  - ビルドシステム統合
  - **完了**: llama.cppのvendor/minja/minja.hppがJinja互換レンダラーとして統合済み
- [x] T167 chat_templateレンダリングの実装
  - config.jsonからのテンプレート読み込み
  - messagesの変換とレンダリング
  - **完了**: llama_chat_apply_template()をllama_engine.cppとinference_engine.cppで使用、ChatMLフォールバック付き

## Function Calling（Session 2025-12-31 Part 5）
- [x] T168 Function Calling検出の実装
  - ツール定義のプロンプト埋め込み
  - 出力からのJSON検出
  - **完了**: formatToolsForPrompt()とdetectToolCalls()を実装、ネストJSONと複数ツール対応
- [x] T169 finish_reason="tool_calls"対応
  - ツール呼び出し検出時のレスポンス整形
  - **完了**: ToolCall構造体を追加、ユニークID生成機能付き

## manifest.json拡張（Session 2025-12-31 Part 5）
- [x] T170 manifest.jsonへのmodalities追加
  - completion/embeddingモードの宣言
- [x] T171 manifest.jsonへのlicense追加
  - ライセンス情報フィールド
- [x] T172 manifest.jsonへのsupports_vision追加
  - Vision対応フラグ

## Tests（Session 2025-12-31 Part 5）
- [x] T173 量子化指定パーステスト
- [x] T174 Prefix Cacheヒット/ミステスト
  - **完了**: prefix_cache_test.cppに9個のテストケース（格納・取得・上書き・LRU削除・VRAM使用量・クリア・割合制限・ハッシュ・カウント）
- [x] T175 mmproj自動検出テスト
  - **完了**: vision_processor_test.cppに7つのテストケースを追加（ファイル名パターン検出、大小文字無視、複数ファイル時アルファベット順選択など）
- [x] T176 レプリカ負荷分散テスト
  - **完了**: replica_manager_test.cppに11個のテストケース（登録・解除・ラウンドロビン・障害スキップ・回復・ステータス確認等）
- [x] T177 chat_templateレンダリングテスト
  - **完了**: inference_engine_test.cppに5つのChatTemplateTestを追加（単一/複数メッセージ、空コンテンツ、マルチライン、順序保持）
- [x] T178 Function Calling検出テスト
  - **完了**: function_calling_test.cppに11個のテストケース（プロンプト埋め込み、JSON検出、ネストJSON、マルチツール等）

## VRAM部分ロード障害（Session 2025-12-31 Part 6）
- [x] T179 部分ロード時VRAM不足の即時全解放実装
  - ロード済みテンソルの追跡
  - OOM検出時の全解放処理
  - クリーン状態への復帰
  - **完了**: handleLoadFailure()でloading状態クリア、evictForVram()でLRU evictionによるVRAM回復

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
  - **完了**: ServiceUnavailableError例外とisInRecoveryMode()を実装、全generate関数でリカバリモードチェック

## ストリーミングハング検出（Session 2025-12-31 Part 6）
- [x] T182 トークン間タイムアウト（5秒）の実装
  - 最終トークンからの経過時間追跡
  - 5秒タイムアウトでハング判定
  - 強制終了とエラー返却
  - **完了**: InterTokenWatchdogクラスで5秒タイムアウト監視、TokenTimeoutError例外、AbortCallbackでループ中断

## プラグインログ統合（Session 2025-12-31 Part 6）
- [x] T183 プラグインログのホスト統合実装
  - stdout/stderrのキャプチャ
  - プラグインIDプレフィックス付与
  - タイムスタンプとログレベル付与
  - **完了**: PluginLogCallback型とPluginLogLevel列挙型を追加、defaultPluginLogHandler()でspdlog経由のログ出力実装

## ロード進捗API（Session 2025-12-31 Part 6）
- [x] T184 ロード進捗非公開ポリシーの実装
  - 開始/完了/失敗のみ通知
  - 進捗率API無し
  - 内部デバッグ用追跡（オプション）
  - **完了**: 現在の実装は進捗率APIを公開せず、ログで開始/完了/失敗のみ通知

## モダリティ処理（Session 2025-12-31 Part 6）
- [x] T185 モダリティFIFO処理の実装
  - Completion/Embedding区別なし
  - 到着順処理の確認
  - 優先度なしの動作確認
  - **完了**: ContinuousBatchSchedulerがstd::dequeでFIFO処理、優先度なし

## Tests（Session 2025-12-31 Part 6）
- [x] T186 VRAM部分ロード障害テスト
  - **完了**: llama_manager_test.cppに5個のテストケース（LoadFailureClearsLoadingState、HandleLoadFailureWithEvictLru、EvictForVramReturnsZero、LoadFailureDoesNotAffectLoadingState、VramRecoveryAllowsRetryAfterFailure）
- [x] T187 量子化厳密マッチングテスト
- [x] T188 クラッシュ後503即時返却テスト
  - **完了**: inference_engine_test.cppに3個のServiceUnavailableTestケース（例外メッセージ、デフォルトリカバリモード、クリア動作）
- [x] T189 トークン間タイムアウトテスト
  - **完了**: inference_engine_test.cppに3個のTokenTimeoutTestケース（例外メッセージ、RuntimeError継承、AbortCallbackデフォルト値）
- [x] T190 プラグインログ統合テスト
  - **完了**: engine_host_test.cppに8個のPluginLogTestケース（コールバックフィールド、レベル変換、無効レベル、パラメータ受け渡し、null処理、全レベル動作、enum整数値）

## Deferred（TBD）
- Nemotron向けの新エンジン（推論エンジン）の仕様策定（別SPEC）
- Nemotron向けの新エンジン（推論エンジン）の実装（Metal/DirectML）
- Router側: HF chat_template(Jinja) を完全互換でレンダリングし、Nodeへ最終プロンプトを渡す方針の具体化（別SPEC想定）
- Nemotron GPU PoC: safetensors直読→GPU演算→E2E生成までの段階的検証（別SPEC想定）
