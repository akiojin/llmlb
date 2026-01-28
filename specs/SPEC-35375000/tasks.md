# タスク: ロードバランサー負荷最適化

**ステータス**: 完了

**入力**: `/specs/SPEC-35375000/`の設計ドキュメント
**前提条件**: plan.md (必須)

## 概要

クライアント増加時のロードバランサー負荷を軽減するため、以下の最適化を実装:

1. HTTPクライアントプーリング（P1）
2. 待機機構の改善（P2）
3. ノード選択の最適化（P3）

## Phase 3.1: セットアップ

- [x] T001 既存のlib.rsとbalancer/mod.rsの構造を確認し、変更箇所を特定

## Phase 3.2: テストファースト (TDD) - HTTPクライアントプーリング

**重要: これらのテストは記述され、実装前に失敗する必要がある**

- [x] T002 llmlb/src/lib.rs に `test_app_state_has_shared_http_client` テストを追加（RED）
  - AppStateにhttp_clientフィールドが存在することを確認
  - テストが失敗することを確認

- [x] T003 llmlb/src/lib.rs に `http_client: reqwest::Client` フィールドを追加（GREEN）
  - AppState構造体を拡張
  - テストが成功することを確認

## Phase 3.3: テストファースト (TDD) - 待機機構

- [x] T004 llmlb/src/balancer/mod.rs に `WaitResult` enum と `AdmissionDecision` enum を追加
  - WaitResult: Ready, Timeout, CapacityExceeded
  - AdmissionDecision: Accept, AcceptWithDelay(Duration), Reject

- [x] T005 llmlb/src/balancer/mod.rs に `test_wait_for_ready_timeout` テストを追加（RED）
  - タイムアウト時にWaitResult::Timeoutを返すことを確認
  - テストが失敗することを確認

- [x] T006 llmlb/src/balancer/mod.rs に `test_wait_for_ready_ready_immediately` テストを追加（RED）
  - ready状態時にWaitResult::Readyを返すことを確認
  - テストが失敗することを確認

- [x] T007 llmlb/src/balancer/mod.rs に `test_wait_for_ready_capacity_exceeded` テストを追加（RED）
  - 上限超過時にWaitResult::CapacityExceededを返すことを確認
  - テストが失敗することを確認

- [x] T008 llmlb/src/balancer/mod.rs に `wait_for_ready_with_timeout` メソッドを実装（GREEN）
  - tokio::time::timeoutを使用
  - 既存のwait_for_readyを拡張
  - T005-T007のテストが成功することを確認

- [x] T009 llmlb/src/balancer/mod.rs に `test_admission_control_accept` テストを追加（RED）
  - 50%未満でAdmissionDecision::Acceptを返すことを確認

- [x] T010 llmlb/src/balancer/mod.rs に `test_admission_control_delay` テストを追加（RED）
  - 50-80%でAdmissionDecision::AcceptWithDelayを返すことを確認

- [x] T011 llmlb/src/balancer/mod.rs に `test_admission_control_reject` テストを追加（RED）
  - 80%以上でAdmissionDecision::Rejectを返すことを確認

- [x] T012 llmlb/src/balancer/mod.rs に `admission_control` メソッドを実装（GREEN）
  - 待機者数に応じた段階的制御
  - T009-T011のテストが成功することを確認

## Phase 3.4: テストファースト (TDD) - ノード選択最適化

- [~] T013 llmlb/src/balancer/mod.rs に `test_cached_node_selection` テストを追加（RED）
  - 短時間の連続呼び出しで同一ノードを返すことを確認
  - **スキップ**: 微細最適化のため、現時点では不要と判断

- [~] T014 llmlb/src/balancer/mod.rs に `CachedSelection` 構造体と `select_node_cached` メソッドを実装（GREEN）
  - 短TTL（10ms程度）のキャッシュ
  - **スキップ**: 現在のselect_node()は十分高速

## Phase 3.5: 統合

- [x] T015 llmlb/src/main.rs にHTTPクライアント初期化を追加
  - reqwest::Client::builder()で接続プーリング設定
  - pool_max_idle_per_host(32), pool_idle_timeout(60s), tcp_keepalive(30s)

- [x] T016 [P] llmlb/src/api/proxy.rs の Client::new() を state.http_client.clone() に置換
  - 118行、343行付近を修正

- [x] T017 [P] llmlb/src/api/openai.rs の Client::new() を state.http_client.clone() に置換
  - テストコード内のみ残存（本番コードは完了）

- [x] T018 [P] llmlb/src/api/models.rs の Client::new() を state.http_client.clone() に置換
  - テストコード内のみ残存（本番コードは完了）

- [x] T019 [P] llmlb/src/api/nodes.rs の Client::new() を state.http_client.clone() に置換
  - テストコード内のみ残存（本番コードは完了）

- [x] T020 [P] llmlb/src/api/logs.rs の Client::new() を state.http_client.clone() に置換
  - テストコード内のみ残存（本番コードは完了）

- [x] T021 llmlb/src/api/proxy.rs の待機処理を wait_for_ready_with_timeout に更新
  - 85-95行付近を修正
  - admission_controlを呼び出し

## Phase 3.6: 仕上げ

- [x] T022 cargo fmt --check && cargo clippy -- -D warnings を実行
  - フォーマットとlint警告を解消

- [x] T023 cargo test を実行
  - すべてのテストが成功することを確認

- [x] T024 make quality-checks を実行
  - 品質チェックがすべて成功することを確認

- [x] T025 plan.md の進捗トラッキングを更新
  - Phase 3: Tasks生成済み → チェック
  - Phase 4: 実装完了 → チェック
  - Phase 5: 検証合格 → チェック

## 依存関係

```text
T001 (setup)
  ↓
T002 → T003 (HTTPクライアントプーリング)
  ↓
T004 → T005-T007 → T008 (待機タイムアウト)
       T009-T011 → T012 (バックプレッシャー)
  ↓
T013 → T014 (ノード選択キャッシュ)
  ↓
T015 → T016-T020 (並列: Client::new()置換)
  ↓
T021 (待機処理統合)
  ↓
T022 → T023 → T024 → T025 (仕上げ)
```

## 並列実行例

```text
# T016-T020 を並列実行:

**ステータス**: 完了
Task: "llmlb/src/api/proxy.rs の Client::new() を置換"
Task: "llmlb/src/api/openai.rs の Client::new() を置換"
Task: "llmlb/src/api/models.rs の Client::new() を置換"
Task: "llmlb/src/api/nodes.rs の Client::new() を置換"
Task: "llmlb/src/api/logs.rs の Client::new() を置換"
```

## 注意事項

- [P] タスク = 異なるファイル、依存関係なし
- 実装前にテストが失敗することを確認（RED）
- テスト成功後に次のタスクへ（GREEN）
- 各タスク後にコミット
- コミットメッセージはConventional Commits形式

## 検証チェックリスト

- [x] すべてのテストが実装より先にある
- [x] 並列タスクは本当に独立している
- [x] 各タスクは正確なファイルパスを指定
- [x] 同じファイルを変更する[P]タスクがない
