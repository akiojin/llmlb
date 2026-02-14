# タスク: 包括的E2Eテストスイート強化

**入力**: `/specs/SPEC-62241000/` の設計ドキュメント
**前提条件**: plan.md, spec.md, research.md, data-model.md, quickstart.md

## フォーマット: `[ID] [P?] [Story] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)
- **[Story]**: このタスクが属するユーザーストーリー (例: US1, US2)

## Phase 1: 基盤 - モックサーバー拡張

**目的**: 全ユーザーストーリーが依存するモックサーバーとヘルパー関数の整備

- [ ] T001 [Foundation] `llmlb/tests/e2e-playwright/helpers/mock-openai-endpoint.ts` にAudio APIモックハンドラーを追加。`POST /v1/audio/transcriptions` (multipart受信→`{text: "MOCK_TRANSCRIPTION ..."}` JSON応答) と `POST /v1/audio/speech` (JSON受信→`audio/mpeg` バイナリ応答) を実装。既存の `startMockOpenAIEndpointServer` のオプションに `supportAudio?: boolean` を追加し、trueの場合のみハンドラーを有効化
- [ ] T002 [Foundation] `llmlb/tests/e2e-playwright/helpers/mock-openai-endpoint.ts` にImage APIモックハンドラーを追加。`POST /v1/images/generations` (JSON→ImageResponse)、`POST /v1/images/edits` (multipart→ImageResponse)、`POST /v1/images/variations` (multipart→ImageResponse) を実装。オプション `supportImages?: boolean` で制御
- [ ] T003 [Foundation] `llmlb/tests/e2e-playwright/helpers/mock-openai-endpoint.ts` にResponses APIモックハンドラーを追加。`POST /v1/responses` (JSON→ResponsesAPIResponse) を実装。オプション `supportResponses?: boolean` で制御
- [ ] T004 [Foundation] `llmlb/tests/e2e-playwright/helpers/mock-openai-endpoint.ts` にエンドポイントタイプ別応答を追加。オプション `endpointType?: 'xllm' | 'ollama' | 'vllm' | 'openai'` を追加。xLLMの場合 `GET /v0/system` でxLLM固有レスポンス、Ollamaの場合 `GET /api/tags` でOllama固有レスポンス、vLLMの場合 `/v1/models` にvLLMメタデータ付きレスポンスを返す。xLLMの場合は `POST /v0/models/download` モックも追加
- [ ] T005 [Foundation] `llmlb/tests/e2e-playwright/helpers/api-helpers.ts` にユーザー管理ヘルパー関数を追加: `createUser(request, username, password, role)`, `updateUserRole(request, userId, role)`, `deleteUser(request, userId)`, `listUsers(request)`。全て `API_BASE` + `AUTH_HEADER` を使用。既存の `ensureDashboardLogin` パターンに準拠
- [ ] T006 [Foundation] `llmlb/tests/e2e-playwright/helpers/api-helpers.ts` にAPIキー管理ヘルパー関数を追加: `createApiKeyWithPermissions(request, name, permissions[], expiresAt?)` → `{id, key}`, `deleteApiKey(request, keyId)`。permissions配列は `'openai.inference' | 'openai.models.read' | 'endpoints.read' | ...` の文字列配列
- [ ] T007 [P] [Foundation] `llmlb/tests/e2e-playwright/helpers/api-helpers.ts` にログ・メトリクス・システムヘルパー関数を追加: `getLbLogs(request)`, `getEndpointLogs(request, endpointId)`, `getPrometheusMetrics(request)` → string (Prometheus text format), `getSystemInfo(request)`

---

## Phase 2: 基盤 - Page Object・セレクタ拡張

**目的**: テストで使用するPage ObjectとセレクタのUI操作メソッド追加

- [ ] T008 [Foundation] `llmlb/tests/e2e-playwright/pages/dashboard.page.ts` の `DashboardPage` クラスにStatistics Tab操作メソッドを追加: `goToStatisticsTab()`, `switchToDaily()`, `switchToMonthly()`, `getTokenStatsCards()` (Total Requests/Input Tokens/Output Tokens/Total Tokens の値取得)
- [ ] T009 [Foundation] `llmlb/tests/e2e-playwright/pages/dashboard.page.ts` の `DashboardPage` クラスにHistory Tab操作メソッドを追加: `goToHistoryTab()`, `getHistoryRows()`, `clickHistoryRow(index)`, `getHistoryDetailModal()`, `switchDetailTab(tab: 'overview' | 'request' | 'response')`, `goToHistoryPage(page)`, `getHistoryPagination()`
- [ ] T010 [Foundation] `llmlb/tests/e2e-playwright/pages/dashboard.page.ts` の `DashboardPage` クラスにLogs Tab操作メソッドを追加: `goToLogsTab()`, `selectLogType(type: 'lb' | 'endpoint')`, `selectEndpointForLogs(endpointName)`, `getLogEntries()`, `clickRefreshLogs()`
- [ ] T011 [Foundation] `llmlb/tests/e2e-playwright/pages/dashboard.page.ts` の `DashboardPage` クラスにUser Management操作メソッドを追加: `openUserManagement()`, `getUserRows()`, `clickAddUser()`, `fillUserForm(username, password, role)`, `submitUserForm()`, `changeUserRole(username, newRole)`, `deleteUser(username)`, `confirmDeleteUser()`
- [ ] T012 [Foundation] `llmlb/tests/e2e-playwright/pages/dashboard.page.ts` の `DashboardPage` クラスにEndpoint編集操作メソッドを追加: `openEndpointDetail(endpointName)`, `editDisplayName(newName)`, `editHealthCheckInterval(seconds)`, `editInferenceTimeout(seconds)`, `editNotes(text)`, `saveEndpointChanges()`, `getEndpointDetailValues()`
- [ ] T013 [Foundation] `llmlb/tests/e2e-playwright/pages/playground.page.ts` にPlayground設定操作メソッドを追加: `setTemperature(value)`, `setMaxTokens(value)`, `setSystemPrompt(text)`, `toggleStreaming()`, `getStreamingState()`, `openCurlDialog()`, `getCurlCommand()`, `copyCurlCommand()`, `closeCurlDialog()`

**チェックポイント**: 基盤準備完了 - 全ユーザーストーリーのテスト実装が開始可能

---

## Phase 3: US1 - セッション管理・認証ライフサイクルE2E (P1) 🎯

**目標**: ログアウト・JWT期限切れ・セッション管理のE2Eテスト追加

**独立テスト**: ログアウト→API 401確認、戻るボタン→リダイレクト確認で独立検証可能

- [ ] T014 [US1] `llmlb/tests/e2e-playwright/specs/auth/session-management.spec.ts` を新規作成。以下のテストケースを実装:
  1. `SM-01`: ログイン済み→Sign outクリック→ログインページにリダイレクト＆ダッシュボードAPIが401を返す
  2. `SM-02`: ログアウト後→ブラウザ戻るボタン→ログインページにリダイレクト（キャッシュからのダッシュボード表示なし）
  3. `SM-03`: ログイン済み→JWT cookieを手動削除→ダッシュボード操作→ログインページにリダイレクト
  4. `SM-04`: ログアウト後→直接ダッシュボードURLにアクセス→ログインページにリダイレクト
  `ensureDashboardLogin` で前提状態を作成。`page.context().clearCookies()` でcookie操作

**チェックポイント**: ログアウト・セッション管理テスト完了、`npx playwright test specs/auth/session-management.spec.ts --headed` で全パス確認

---

## Phase 4: US2 - ユーザー管理E2E (P1)

**目標**: ユーザーCRUD・ロール変更のE2Eテスト追加

**独立テスト**: ユーザー作成→ロール変更→削除→ログイン失敗の一連フローで独立検証可能

- [ ] T015 [US2] `llmlb/tests/e2e-playwright/specs/dashboard/user-management.spec.ts` を新規作成。以下のテストケースを実装:
  1. `UM-01`: admin権限でログイン→ユーザー管理モーダル開く→ユーザー作成→リストに表示確認
  2. `UM-02`: テストユーザー存在→ロールをUser→Adminに変更→バッジ更新確認
  3. `UM-03`: テストユーザー存在→ユーザー削除→リストから消失→そのユーザーでログイン失敗
  4. `UM-04`: 同一ユーザー名で重複作成→適切なエラーメッセージ表示
  5. `UM-05`: 非admin権限ユーザーでログイン→ユーザー管理にアクセス→拒否される
  テストデータは`e2e-user-{timestamp}`形式で一意名を生成。afterEachで作成したユーザーをAPIで削除

**チェックポイント**: ユーザー管理テスト完了

---

## Phase 5: US3 - APIキー権限マトリクスE2E (P1)

**目標**: 11権限×主要エンドポイント全組み合わせの網羅テスト

**独立テスト**: 特定権限のAPIキーを作成し、各エンドポイントへのアクセス可否を検証

- [ ] T016 [US3] `llmlb/tests/e2e-playwright/specs/api/permission-matrix.spec.ts` を新規作成。パラメトリックテスト設計で以下を実装:
  - 権限マトリクス定義: 各権限 (`openai.inference`, `openai.models.read`, `endpoints.read`, `endpoints.manage`, `api_keys.manage`, `users.manage`, `invitations.manage`, `models.manage`, `registry.read`, `logs.read`, `metrics.read`) に対して
  - テストエンドポイント: `GET /v1/models`, `POST /v1/chat/completions`, `GET /api/endpoints`, `POST /api/endpoints`, `GET /api/users`, `GET /api/keys`, `GET /api/dashboard/logs/lb`, `GET /api/metrics/cloud` 等
  - 各権限単独のAPIキーを`createApiKeyWithPermissions`で作成
  - 許可されたエンドポイント→200/201、拒否されたエンドポイント→403を検証
  - `test.describe`で権限ごとにグループ化
  - 追加: 全権限のキー→全エンドポイント200、権限ゼロのキー→全エンドポイント403
  モックエンドポイントを`beforeAll`で起動し、`/v1/chat/completions`等が動作する環境を準備

**チェックポイント**: 権限マトリクスの全組み合わせテスト完了 (SC-006)

---

## Phase 6: US4 - APIキーライフサイクルE2E (P1)

**目標**: APIキーの作成→使用→期限切れ→削除の全ライフサイクルテスト

**独立テスト**: APIキーの各フェーズで動作検証

- [ ] T017 [US4] `llmlb/tests/e2e-playwright/specs/api/api-key-lifecycle.spec.ts` を新規作成。以下のテストケースを実装:
  1. `AKL-01`: ダッシュボードからAPIキー作成→コピー→コピー成功トースト表示（「Copied full API key」）→`Failed to copy`非表示→そのキーでAPIアクセス成功
  2. `AKL-02`: 有効期限付きAPIキーをAPI直接作成 (expires_at を過去日時に設定)→APIアクセス→401
  3. `AKL-03`: 有効なAPIキー存在→ダッシュボードから削除→そのキーでAPIアクセス→401
  4. `AKL-04`: 複数APIキー存在（有効+期限切れ）→API Keys一覧表示→期限切れキーに「Expired」バッジ確認
  `serial`モードで実行。afterAllで作成したAPIキーをクリーンアップ

**チェックポイント**: APIキーライフサイクルテスト完了

---

## Phase 7: US5 - リクエスト履歴（History Tab）E2E (P1)

**目標**: History Tabの一覧表示・詳細モーダル・ページネーションのテスト

**独立テスト**: モックエンドポイント経由でリクエスト実行後、History Tabでデータ確認

- [ ] T018 [US5] `llmlb/tests/e2e-playwright/specs/dashboard/history-tab.spec.ts` を新規作成。以下のテストケースを実装:
  1. `HT-01`: モックエンドポイント経由でchat completionリクエスト実行→History Tab開く→リクエストがタイムスタンプ・モデル・ステータスと共に表示
  2. `HT-02`: 履歴表示中→リクエスト行クリック→詳細モーダルにOverview/Request/Responseの3タブ表示
  3. `HT-03`: 詳細モーダルのRequestタブ→JSONリクエストボディ表示＆コピーボタン機能確認
  4. `HT-04`: 詳細モーダルのResponseタブ→JSONレスポンスボディ表示
  5. `HT-05`: 複数リクエスト実行→ページネーション表示→2ページ目で異なるリクエスト表示
  `beforeAll`でモックサーバー起動＋エンドポイント登録＋テストリクエスト実行

**チェックポイント**: History Tabテスト完了

---

## Phase 8: US6 - トークン統計（Statistics Tab）E2E (P1)

**目標**: Statistics Tabの日次・月次統計表示テスト

**独立テスト**: モックエンドポイント経由でリクエスト実行後、統計値を確認

- [ ] T019 [US6] `llmlb/tests/e2e-playwright/specs/dashboard/statistics-tab.spec.ts` を新規作成。以下のテストケースを実装:
  1. `ST-01`: モックエンドポイント経由でリクエスト実行後→Statistics Tab→Dailyタブ→本日のリクエスト数・入力/出力/合計トークンがゼロでない値で表示
  2. `ST-02`: Statistics Tab表示中→Monthlyタブ切替→月別統計データ表示確認
  3. `ST-03`: Statistics Tab→統計カードのレイアウト・ラベルが正しく表示
  `beforeAll`でモックサーバー起動＋エンドポイント登録＋テストリクエスト複数回実行

**チェックポイント**: Statistics Tabテスト完了

---

## Phase 9: US7 - ログビューア（Logs Tab）E2E (P1)

**目標**: LBログ表示・エンドポイント別ログ・リフレッシュ機能のテスト

**独立テスト**: リクエスト実行後にLogs Tabでログ存在確認

- [ ] T020 [US7] `llmlb/tests/e2e-playwright/specs/dashboard/logs-tab.spec.ts` を新規作成。以下のテストケースを実装:
  1. `LT-01`: モックエンドポイント経由でリクエスト実行後→Logs Tab→LB Logs表示→ログエントリが存在
  2. `LT-02`: エンドポイント登録済み→Endpoint Logsでエンドポイント選択→対応するログ表示
  3. `LT-03`: ログ表示中→リフレッシュボタンクリック→ログが更新（またはエラーなく再表示）
  4. `LT-04`: ログエントリの内容（タイムスタンプ、レベル、メッセージ）が検証可能な形式で表示
  `beforeAll`でモックサーバー起動＋エンドポイント登録＋テストリクエスト実行

**チェックポイント**: Logs Tabテスト完了

---

## Phase 10: US8 - Endpoint編集・設定変更E2E (P1)

**目標**: Endpoint詳細モーダルからの設定変更と反映テスト

**独立テスト**: 設定変更後にAPIで値を確認

- [ ] T021 [US8] `llmlb/tests/e2e-playwright/specs/dashboard/endpoint-edit.spec.ts` を新規作成。以下のテストケースを実装:
  1. `EE-01`: エンドポイント登録済み→詳細モーダル→Display Name変更→保存→一覧に新名前反映
  2. `EE-02`: エンドポイント登録済み→Health Check Interval変更→保存→API取得で値更新確認
  3. `EE-03`: エンドポイント登録済み→Inference Timeout変更→保存→API取得で値更新確認
  4. `EE-04`: エンドポイント登録済み→Notes変更→保存→詳細モーダル再開→新Notes表示
  5. `EE-05`: Inference Timeoutに最小値10を設定→保存成功
  6. `EE-06`: Inference Timeoutに最大値600を設定→保存成功
  `afterEach`で作成したエンドポイントをAPIで削除

**チェックポイント**: Endpoint編集テスト完了

---

## Phase 11: US9 - Playground設定・cURLダイアログE2E (P1)

**目標**: Playground各設定の反映とcURLコマンド生成テスト

**独立テスト**: 設定変更後にチャット送信で動作確認、cURLダイアログの表示確認

- [ ] T022 [US9] `llmlb/tests/e2e-playwright/specs/workflows/playground-settings.spec.ts` を新規作成。以下のテストケースを実装:
  1. `PS-01`: Endpoint Playground→System Prompt設定→メッセージ送信→レスポンス返却確認
  2. `PS-02`: Endpoint Playground→Streaming ON→OFF切替→バッジ表示切替確認
  3. `PS-03`: Endpoint Playground→cURLボタンクリック→cURLコマンド表示→コピーボタン機能確認
  4. `PS-04`: LB Playground→Temperature/Max Tokens変更→チャット送信→レスポンス返却確認
  5. `PS-05`: LB Playground→cURLボタンクリック→cURLコマンド表示→正しいURLとヘッダー含む
  `beforeAll`でモックサーバー起動＋エンドポイント登録＋接続テスト＋モデル同期

**チェックポイント**: Playground設定・cURLテスト完了

---

## Phase 12: US10 - OpenAI互換マルチモーダルAPI E2E (P1)

**目標**: Audio/Image/Responses APIのルーティング・プロキシテスト

**独立テスト**: 各APIにHTTPリクエスト送信し、モックの応答が正しくプロキシされることを確認

- [ ] T023 [US10] `llmlb/tests/e2e-playwright/specs/api/audio-api.spec.ts` を新規作成。以下のテストケースを実装:
  1. `AA-01`: モックエンドポイント（supportAudio: true）登録→`/v1/audio/transcriptions`にmultipartリクエスト→モック応答がプロキシされ`{text: "MOCK_TRANSCRIPTION..."}`が返る
  2. `AA-02`: `/v1/audio/speech`にJSONリクエスト→バイナリ応答が返る
  3. `AA-03`: 認証なしで`/v1/audio/transcriptions`→401
  `request`フィクスチャ（APIRequestContext）でHTTPレベルテスト
- [ ] T024 [P] [US10] `llmlb/tests/e2e-playwright/specs/api/image-api.spec.ts` を新規作成。以下のテストケースを実装:
  1. `IA-01`: モックエンドポイント（supportImages: true）登録→`/v1/images/generations`にJSONリクエスト→ImageResponse返却
  2. `IA-02`: `/v1/images/edits`にmultipartリクエスト→ImageResponse返却
  3. `IA-03`: `/v1/images/variations`にmultipartリクエスト→ImageResponse返却
  4. `IA-04`: 認証なしで`/v1/images/generations`→401
- [ ] T025 [P] [US10] `llmlb/tests/e2e-playwright/specs/api/responses-api.spec.ts` を新規作成。以下のテストケースを実装:
  1. `RA-01`: モックエンドポイント（supportResponses: true）登録→`/v1/responses`にJSONリクエスト→ResponsesAPIResponse返却
  2. `RA-02`: 認証なしで`/v1/responses`→401

**チェックポイント**: マルチモーダルAPIテスト完了

---

## Phase 13: US11 - SSEストリーミング詳細検証E2E (P1)

**目標**: SSEチャンク順序・[DONE]シグナル・途中切断のクリーンアップテスト

**独立テスト**: HTTPレベルでSSEストリームを受信し、各イベントの内容と順序を検証

- [ ] T026 [US11] `llmlb/tests/e2e-playwright/specs/api/sse-streaming.spec.ts` を新規作成。以下のテストケースを実装:
  1. `SSE-01`: stream=trueでchat completionリクエスト→SSEイベント受信→チャンクが順序通りに到着→最後に`data: [DONE]`が送信される
  2. `SSE-02`: SSEストリームの各チャンクが`data:`プレフィックス付きJSON形式で、`choices[0].delta.content`にテキスト含む
  3. `SSE-03`: stream=trueでcompletionsリクエスト→正常なSSEストリーム受信
  4. `SSE-04`: 大きなレスポンスのストリーミング→全チャンク受信後にコンテンツを結合→完全なレスポンス
  `request`フィクスチャで直接HTTP接続。レスポンスボディをテキストとして読み取り、`data:`行をパース

**チェックポイント**: SSEストリーミング詳細テスト完了

---

## Phase 14: US12 - LBロードバランシング動作E2E (P1)

**目標**: レイテンシ優先ルーティング・オフライン除外・フェイルオーバーのテスト

**独立テスト**: 異なるレイテンシのモックエンドポイントでリクエスト分散を検証

- [ ] T027 [US12] `llmlb/tests/e2e-playwright/specs/workflows/lb-load-balancing.spec.ts` を新規作成。以下のテストケースを実装:
  1. `LB-01`: 高速モック（responseDelayMs: 50）と低速モック（responseDelayMs: 500）の2エンドポイント登録→接続テスト→20回リクエスト送信→高速エンドポイントにより多くルーティングされる（レイテンシ優先確認）
  2. `LB-02`: 2つのオンラインエンドポイント→1つをオフラインに（mock.close()）→リクエスト送信→オフラインエンドポイントにルーティングされない
  3. `LB-03`: プライマリエンドポイントを応答不能にする→リクエスト送信→セカンダリにフォールバック→成功
  `serial`モード。各テストでモックサーバーを都度起動・停止してエンドポイント状態を制御。`request`フィクスチャでHTTPレベルテスト

**チェックポイント**: LBロードバランシングテスト完了

---

## Phase 15: US13 - エンドポイントタイプ検出E2E (P2)

**目標**: 全エンドポイントタイプ（xLLM/Ollama/vLLM/OpenAI/Unknown）の検出テスト

**独立テスト**: 各タイプの応答パターンモックで検出結果を確認

- [ ] T028 [US13] `llmlb/tests/e2e-playwright/specs/workflows/endpoint-type-detection.spec.ts` を新規作成。以下のテストケースを実装:
  1. `ETD-01`: OpenAI互換モック（endpointType: 'openai'）登録→接続テスト→API取得でendpoint_typeが`openai`
  2. `ETD-02`: xLLMモック（endpointType: 'xllm'）登録→接続テスト→API取得でendpoint_typeが`xllm`
  3. `ETD-03`: Ollamaモック（endpointType: 'ollama'）登録→接続テスト→API取得でendpoint_typeが`ollama`
  4. `ETD-04`: vLLMモック（endpointType: 'vllm'）登録→接続テスト→API取得でendpoint_typeが`vllm`
  5. `ETD-05`: 全タイプ登録済み→ダッシュボードのタイプフィルターで「OpenAI」選択→OpenAIタイプのみ表示
  T004のモックサーバー拡張に依存

**チェックポイント**: エンドポイントタイプ検出テスト完了

---

## Phase 16: US14 - PrometheusメトリクスエクスポートE2E (P2)

**目標**: Prometheusメトリクスのフォーマット検証とカウンター増加テスト

**独立テスト**: メトリクスエンドポイントの応答形式とリクエスト前後のカウンター差分を確認

- [ ] T029 [US14] `llmlb/tests/e2e-playwright/specs/api/prometheus-metrics.spec.ts` を新規作成。以下のテストケースを実装:
  1. `PM-01`: `GET /api/metrics/cloud` (Authorization: Bearer sk_debug)→200→Prometheus text format (`# HELP`, `# TYPE`, メトリクス行) が含まれる
  2. `PM-02`: 初期メトリクス取得→chat completionリクエスト実行→メトリクス再取得→リクエストカウンター増加確認
  3. `PM-03`: 認証なしで`/api/metrics/cloud`→401 or 403
  `request`フィクスチャでHTTPレベルテスト。Prometheus形式のパースは正規表現で行う

**チェックポイント**: Prometheusメトリクステスト完了

---

## Phase 17: US15 - モバイルレスポンシブE2E (P2)

**目標**: モバイルビューポートでの主要フロー動作テスト

**独立テスト**: モバイルビューポートに切り替えてログイン・ダッシュボード表示を確認

- [ ] T030 [US15] `llmlb/tests/e2e-playwright/specs/dashboard/mobile-responsive.spec.ts` を新規作成。以下のテストケースを実装:
  1. `MR-01`: モバイルビューポート(375x667)→ログイン→ダッシュボード表示→レイアウト崩れなし（主要要素が全て表示）
  2. `MR-02`: モバイルビューポートでダッシュボード表示中→ユーザードロップダウン→メニュー項目（API Keys, LB Playground）がモバイル用として表示
  3. `MR-03`: モバイルビューポートでPlayground→チャット送信→レスポンス表示→正常動作
  `test.use({ viewport: { width: 375, height: 667 } })` でビューポート設定

**チェックポイント**: モバイルレスポンシブテスト完了

---

## Phase 18: US16 - 大規模負荷テストE2E (P2)

**目標**: 100+同時リクエストでの安定性テスト

**独立テスト**: 直接APIで大量リクエスト送信し、成功率を確認

- [ ] T031 [US16] `llmlb/tests/e2e-playwright/specs/workflows/large-scale-load-test.spec.ts` を新規作成。以下のテストケースを実装:
  1. `LSL-01`: 2つのモックエンドポイント（responseDelayMs: 50）稼働→Promise.allで120同時リクエスト送信→全リクエスト成功（200）→レスポンスタイム記録
  2. `LSL-02`: 負荷テスト完了後→ダッシュボードの統計確認→リクエスト数が正しく記録
  `test.setTimeout(120_000)`で十分なタイムアウト。`request`フィクスチャで直接API呼び出し

**チェックポイント**: 大規模負荷テスト完了 (SC-007)

---

## Phase 19: US17 - モデルダウンロード機能E2E (P2)

**目標**: xLLMエンドポイントのモデルダウンロードUIフローテスト

**独立テスト**: xLLMモックでダウンロードダイアログ表示を確認

- [ ] T032 [US17] `llmlb/tests/e2e-playwright/specs/workflows/model-download.spec.ts` を新規作成。以下のテストケースを実装:
  1. `MD-01`: xLLMタイプモック（endpointType: 'xllm'）登録→接続テスト→ダッシュボードで詳細モーダル→Download Model関連UIが表示される
  T004のモックサーバー拡張に依存

**チェックポイント**: モデルダウンロードテスト完了

---

## Phase 20: US18 - システム更新バナーE2E (P2)

**目標**: システム情報取得とバナー表示テスト

**独立テスト**: `/api/system` APIの応答確認、ダッシュボードでのバナー表示確認

- [ ] T033 [US18] `llmlb/tests/e2e-playwright/specs/dashboard/system-update-banner.spec.ts` を新規作成。以下のテストケースを実装:
  1. `SUB-01`: `GET /api/system`→200→バージョン情報と更新ステータスが含まれるJSONレスポンス
  2. `SUB-02`: ダッシュボードにログイン→システムバージョン情報がヘッダーまたはフッターに表示される
  `request`フィクスチャでAPI検証 + `page`でUI確認

**チェックポイント**: システム更新バナーテスト完了

---

## Phase 21: US19 - Endpoint詳細データビジュアライゼーションE2E (P2)

**目標**: エンドポイント詳細モーダルの統計カード・チャート・テーブルのデータ存在確認

**独立テスト**: リクエスト実行後にエンドポイント詳細を開き、データ存在を確認

- [ ] T034 [US19] `llmlb/tests/e2e-playwright/specs/dashboard/endpoint-detail-viz.spec.ts` を新規作成。以下のテストケースを実装:
  1. `EDV-01`: モックエンドポイントで複数リクエスト実行後→詳細モーダル→統計カード（Total Requests/Today's Requests/Success Rate/Avg Response Time）にゼロでない値
  2. `EDV-02`: 詳細モーダルの日次トレンドセクション→チャートコンテナ表示＆データ存在
  3. `EDV-03`: 詳細モーダルのモデル別統計テーブル→モデル名・リクエスト数・成功率・レイテンシ表示
  `beforeAll`でモックサーバー起動＋エンドポイント登録＋複数回リクエスト実行

**チェックポイント**: Endpoint詳細データビジュアライゼーションテスト完了

---

## Phase 22: 仕上げ＆横断的関心事

**目的**: テスト全体の統合確認・品質保証

- [ ] T035 全テスト統合実行。`npx playwright test --headed` で全テスト（既存68 + 新規追加分）を実行し、全パスを確認。既存61パステストが引き続き全パスすることを確認 (SC-005)
- [ ] T036 テスト数カウント確認。総テスト数が200以上であることを確認 (SC-001)。`npx playwright test --list` でテスト一覧を出力し、カテゴリ別にカウント
- [ ] T037 [P] テストカバレッジマッピング確認。SC-002 (全UI機能テスト追加)、SC-003 (全APIテスト追加)、SC-004 (外部依存なし実行) の各成功基準を検証
- [ ] T038 [P] specのtasks.mdチェックリスト全タスク完了を確認し、`.specify/scripts/checks/check-tasks.sh` を実行

---

## 依存関係＆実行順序

### フェーズ依存関係

- **Phase 1 (基盤-モック)**: 依存なし→すぐに開始可能
- **Phase 2 (基盤-PageObject)**: Phase 1完了に依存（ヘルパー関数が必要）
- **Phase 3-14 (US1-US12, P1)**: Phase 2完了に依存。ただし以下の独立性あり:
  - US1 (セッション管理), US2 (ユーザー管理), US5 (History), US6 (Statistics), US7 (Logs), US8 (Endpoint編集) は相互独立→並列可能
  - US3 (権限マトリクス), US4 (APIキーライフサイクル) はAPIキーヘルパー(T006)に依存→Phase 1完了後に並列可能
  - US10 (マルチモーダルAPI) はモックサーバー拡張(T001-T003)に依存
  - US11 (SSE), US12 (LB) はPhase 2完了後に独立実行可能
- **Phase 15-21 (US13-US19, P2)**: Phase 2完了に依存。P1と並列可能だが優先度は下:
  - US13 (タイプ検出), US17 (モデルダウンロード) はT004に依存
  - US14 (Prometheus), US15 (モバイル), US16 (負荷テスト), US18 (更新バナー), US19 (詳細Viz) は相互独立→並列可能
- **Phase 22 (仕上げ)**: 全ユーザーストーリー完了に依存

### 並列実行マップ

```text
Phase 1: T001 → T002 → T003 → T004 (順次: 同一ファイル)
         T005 → T006 (順次: 同一ファイル)
         T007 (並列: 別パート)

Phase 2: T008, T009, T010, T011, T012 (順次: 同一ファイル dashboard.page.ts)
         T013 (並列: 別ファイル playground.page.ts)

Phase 3+: [P1ストーリー群]        [P2ストーリー群]
          US1(T014)  ─┐            US13(T028) ─┐
          US2(T015)  ─┤            US14(T029) ─┤
          US3(T016)  ─┤ 並列可能   US15(T030) ─┤ 並列可能
          US5(T018)  ─┤            US16(T031) ─┤
          US6(T019)  ─┤            US17(T032) ─┤
          US7(T020)  ─┤            US18(T033) ─┤
          US8(T021)  ─┤            US19(T034) ─┘
          US9(T022)  ─┤
          US10(T023-T025) ─┤
          US11(T026) ─┤
          US12(T027) ─┘
          US4(T017)  (US3後が望ましい)

Phase 22: T035 → T036, T037, T038 (T035完了後に並列)
```

## 注意事項

- [P]タスク = 異なるファイル、依存関係なし
- [Story]ラベルはタスクを特定のユーザーストーリーにマッピング
- 各ユーザーストーリーは独立して完了・テスト可能
- テストコード自体がTDDの成果物（RED: テスト作成→GREEN: テスト実行→REFACTOR）
- 各タスクまたは論理グループ後にコミット
- 既存の68テストは一切変更しない（SC-005準拠）
