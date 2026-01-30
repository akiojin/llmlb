# タスク: llmlb serveコマンドのシングル実行制約

**入力**: `/specs/SPEC-52587000/` の設計ドキュメント
**前提条件**: plan.md (✓), spec.md (✓), research.md (✓), data-model.md (✓), quickstart.md (✓)

## フォーマット: `[ID] [P?] [Story] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)
- **[Story]**: このタスクが属するユーザーストーリー (US1-US5)
- 説明には正確なファイルパスを含める

## パス規約

このプロジェクトは**単一プロジェクト構造**（llmlb クレート）を使用:

- `llmlb/src/` - ソースコード
- `llmlb/tests/` - テストコード

---

## Phase 1: セットアップ (共有インフラ)

**目的**: 依存関係追加とモジュール構造の作成

- [x] T001 [P] `llmlb/Cargo.toml` に `fs2 = "0.4"` 依存関係を追加
  - クロスプラットフォームファイルロック用
  - 依存: なし

- [x] T002 [P] `llmlb/Cargo.toml` に `nix = { version = "0.29", features = ["signal"] }` を
  Unix専用依存関係として追加
  - `[target.'cfg(unix)'.dependencies]` セクションに追加
  - SIGTERM送信用
  - 依存: なし

- [x] T003 [P] `llmlb/src/lock/mod.rs` にモジュール構造を作成
  - 空のモジュールファイルを作成
  - 依存: なし

- [x] T004 `llmlb/src/lib.rs` に `pub mod lock;` を追加してlockモジュールを公開
  - 依存: T003

---

## Phase 2: 基盤 (ブロッキング前提条件)

**目的**: ロック機構のコア実装（全ユーザーストーリーの前提）

**⚠️ 重要**: このフェーズが完了するまで、ユーザーストーリーの作業は開始できません

### 基盤テスト (TDD RED) ⚠️

> **注意: これらのテストを最初に書き、実装前に失敗することを確認**

- [x] T005 [P] `llmlb/src/lock/mod.rs` に `LockInfo` 構造体のシリアライズ/デシリアライズテストを作成
  - `#[cfg(test)]` モジュール内にテスト作成
  - JSON形式での往復テスト
  - 依存: T003

- [x] T006 [P] `llmlb/src/lock/mod.rs` に `lock_dir()` と `lock_path()` 関数のテストを作成
  - OS一時ディレクトリの取得テスト
  - ポート番号に基づくパス生成テスト
  - 依存: T003

- [x] T007 [P] `llmlb/src/lock/mod.rs` に `is_process_running()` 関数のテストを作成
  - 現在のプロセス（自分自身）が存在することを確認
  - 存在しないPIDで false を返すことを確認
  - 依存: T003

### 基盤実装 (TDD GREEN)

- [x] T008 `llmlb/src/lock/mod.rs` に `LockInfo` 構造体を実装
  - `pid: u32`, `started_at: DateTime<Utc>`, `port: u16`
  - `Serialize`, `Deserialize`, `Debug`, `Clone` derive
  - 依存: T005

- [x] T009 `llmlb/src/lock/mod.rs` に `LockError` enum を実装
  - `AlreadyRunning { port, pid, started_at }`: 重複起動エラー
  - `AcquireFailed(io::Error)`: ロック取得失敗
  - `ReleaseFailed(io::Error)`: ロック解除失敗
  - `Corrupted(String)`: ロックファイル破損
  - `DirectoryCreationFailed(io::Error)`: ディレクトリ作成失敗
  - thiserror を使用
  - 依存: T008

- [x] T010 `llmlb/src/lock/mod.rs` に `lock_dir()` と `lock_path()` 関数を実装
  - `lock_dir()`: `std::env::temp_dir().join("llmlb")` を返す
  - `lock_path(port)`: `lock_dir().join(format!("serve_{}.lock", port))` を返す
  - 依存: T006

- [x] T011 `llmlb/src/lock/mod.rs` に `is_process_running()` 関数を実装
  - `sysinfo` クレートを使用してPID存在確認
  - 依存: T007

- [x] T012 `llmlb/src/lock/mod.rs` に `read_lock_info()` 関数を実装
  - ロックファイルが存在しない場合は `Ok(None)` を返す
  - 存在する場合はJSONをパースして `Ok(Some(LockInfo))` を返す
  - パース失敗時は `Err(LockError::Corrupted)` を返す
  - 依存: T008, T010

**チェックポイント**: 基盤準備完了 - ユーザーストーリー実装が開始可能になりました

---

## Phase 3: ユーザーストーリー1 - 重複起動の防止 (優先度: P1) 🎯 MVP

**目標**: 同一ポートでの重複起動を検知してエラー終了する

**独立テスト**: 同一ポートで2つ目のserveコマンドを実行し、即座にエラー終了することで検証

### US1テスト (TDD RED) ⚠️

- [x] T013 [P] [US1] `llmlb/src/lock/mod.rs` に `ServerLock::acquire()` のテストを作成
  - 新規ロック取得が成功することを確認
  - ロックファイルが作成されることを確認
  - JSON内容が正しいことを確認
  - 依存: T012

- [x] T014 [P] [US1] `llmlb/src/lock/mod.rs` に重複ロック取得のテストを作成
  - 同一ポートで2回目のacquireが `AlreadyRunning` エラーを返すことを確認
  - エラーにPID、起動時刻、ポートが含まれることを確認
  - 依存: T012

### US1実装 (TDD GREEN)

- [x] T015 [US1] `llmlb/src/lock/mod.rs` に `ServerLock` 構造体を実装
  - `lock_file: File`, `lock_path: PathBuf`, `info: LockInfo`
  - `info(&self) -> &LockInfo` メソッド
  - 依存: T013

- [x] T016 [US1] `llmlb/src/lock/mod.rs` に `ServerLock::acquire()` を実装
  - ロックディレクトリを作成（存在しない場合）
  - 既存ロックファイルをチェック
  - 既存ロックのPIDが生存中なら `AlreadyRunning` エラー
  - 残留ロック（PID不存在）は削除して続行
  - `fs2::FileExt::try_lock_exclusive()` でflockを取得
  - `LockInfo` をJSON形式で書き込み
  - パーミッションを600に設定
  - ログ出力（debug）: "Lock acquired for port {port}"
  - 依存: T013, T014

**チェックポイント**: ロック取得が動作し、重複起動を検知できる

---

## Phase 4: ユーザーストーリー2 - 既存プロセスの停止 (優先度: P1)

**目標**: `llmlb stop --port <port>` で起動中のサーバーを停止する

**独立テスト**: 起動中のサーバーに対してstopコマンドを実行し、終了することで検証

### US2テスト (TDD RED) ⚠️

- [x] T017 [P] [US2] `llmlb/src/lock/mod.rs` に `ServerLock::release()` のテストを作成
  - ロック解除が成功することを確認
  - ロックファイルが削除されることを確認
  - 依存: T015

- [x] T018 [P] [US2] `llmlb/src/lock/mod.rs` に `stop_process()` 関数のテストを作成
  - Unix: SIGTERM送信のテスト（モック不可のため統合テストで確認）
  - Windows: taskkill呼び出しのテスト
  - 依存: T011

### US2実装 (TDD GREEN)

- [x] T019 [US2] `llmlb/src/lock/mod.rs` に `ServerLock::release()` を実装
  - `fs2::FileExt::unlock()` でflockを解除
  - ロックファイルを削除
  - ログ出力（debug）: "Lock released for port {port}"
  - 依存: T017

- [x] T020 [US2] `llmlb/src/lock/mod.rs` に `Drop` トレイトを実装
  - `release_internal()` を呼び出し（エラーはログのみ、panicしない）
  - 依存: T019

- [x] T021 [US2] `llmlb/src/lock/mod.rs` に `stop_process()` 関数を実装
  - Unix: `nix::sys::signal::kill(pid, SIGTERM)`
  - Windows: `std::process::Command::new("taskkill").args(["/PID", pid, "/F"])`
  - 依存: T018

- [x] T022 [US2] `llmlb/src/cli/stop.rs` を新規作成
  - `StopArgs` 構造体: `port: u16`
  - `execute()` 関数:
    - `read_lock_info()` でロック情報を取得
    - ロックが存在しない場合: "No server running on port {port}"
    - PIDが存在しない場合: ロックファイル削除、警告表示
    - PIDが存在する場合: `stop_process()` を呼び出し
    - 終了を待機（タイムアウト5秒）
    - 成功時: "Server stopped successfully"
  - 依存: T021

**チェックポイント**: stopコマンドでサーバーを停止できる

---

## Phase 5: ユーザーストーリー3 - サーバー状態の確認 (優先度: P2)

**目標**: `llmlb status` で起動中のサーバー情報を表示する

**独立テスト**: 起動中のサーバーがある状態でstatusコマンドを実行し、情報が表示されることで検証

### US3テスト (TDD RED) ⚠️

- [x] T023 [P] [US3] `llmlb/src/lock/mod.rs` に `list_all_locks()` 関数のテストを作成
  - 複数のロックファイルが存在する場合に全て列挙できることを確認
  - ロックファイルが存在しない場合は空のベクタを返すことを確認
  - 依存: T012

### US3実装 (TDD GREEN)

- [x] T024 [US3] `llmlb/src/lock/mod.rs` に `list_all_locks()` 関数を実装
  - `lock_dir()` 内の `serve_*.lock` ファイルを列挙
  - 各ファイルを `read_lock_info()` で読み込み
  - PID検証して生存中のもののみ返す
  - 依存: T023

- [x] T025 [US3] `llmlb/src/cli/status.rs` を新規作成
  - `StatusArgs` 構造体: `port: Option<u16>`
  - `execute()` 関数:
    - `port` 指定あり: 特定ポートのロック情報を表示
    - `port` 指定なし: `list_all_locks()` で全サーバー情報を表示
    - テーブル形式で出力: PORT, PID, STARTED, STATUS
    - サーバーがない場合: "No servers running"
  - 依存: T024

**チェックポイント**: statusコマンドでサーバー状態を確認できる

---

## Phase 6: ユーザーストーリー4 - クラッシュ後の自動復旧 (優先度: P2)

**目標**: クラッシュ後の残留ロックを自動検出して削除する

**独立テスト**: プロセスを強制終了（kill -9）した後、新しいサーバーを起動できることで検証

### US4テスト (TDD RED) ⚠️

- [x] T026 [P] [US4] `llmlb/src/lock/mod.rs` に残留ロック自動解除のテストを作成
  - 存在しないPIDのロックファイルを作成
  - `ServerLock::acquire()` が成功することを確認
  - 警告ログが出力されることを確認
  - 依存: T016

### US4実装 (TDD GREEN)

- [x] T027 [US4] `llmlb/src/lock/mod.rs` の `ServerLock::acquire()` に残留ロック検出ロジックを追加
  - PID検証で存在しない場合、警告ログを出力してロックファイルを削除
  - ログ出力（warn）: "Stale lock file detected (PID {pid} not running), cleaning up"
  - 依存: T026
  - 注: T016で基本実装済みの場合はテスト追加のみ

**チェックポイント**: クラッシュ後も新しいサーバーを起動できる

---

## Phase 7: ユーザーストーリー5 - グレースフルシャットダウン (優先度: P2)

**目標**: SIGTERM/SIGINT受信時にサーバーが安全に終了し、ロックが解除される

**独立テスト**: Ctrl+CまたはSIGTERMを送信し、ロックファイルが削除されることで検証

### US5テスト (TDD RED) ⚠️

- [x] T028 [P] [US5] `llmlb/src/lock/mod.rs` に `Drop` トレイトのテストを作成
  - `ServerLock` がスコープを抜けた時にロックファイルが削除されることを確認
  - 依存: T020

### US5実装 (TDD GREEN)

- [x] T029 [US5] `llmlb/src/main.rs` にシグナルハンドリングを追加
  - `tokio::signal::ctrl_c()` でCtrl+Cをハンドル
  - Unix: `tokio::signal::unix::signal(SignalKind::terminate())` でSIGTERMをハンドル
  - シグナル受信時にグレースフルシャットダウンを開始
  - `ServerLock` のDropが呼ばれることを保証
  - ログ出力（info）: "Received shutdown signal, cleaning up..."
  - 依存: T028

**チェックポイント**: SIGTERM/SIGINTで安全に終了し、ロックが解除される

---

## Phase 8: CLI統合

**目的**: サブコマンド対応のCLI実装

- [x] T030 `llmlb/src/cli/mod.rs` を拡張してサブコマンド対応
  - `Commands` enum: `Serve(ServeArgs)`, `Stop(StopArgs)`, `Status(StatusArgs)`
  - `Cli` 構造体に `#[command(subcommand)] command: Option<Commands>` を追加
  - 依存: T022, T025

- [x] T031 `llmlb/src/cli/serve.rs` を新規作成
  - `ServeArgs` 構造体: `port: u16`, `host: String`
  - 環境変数: `LLMLB_PORT`, `LLMLB_HOST`
  - 依存: T016

- [x] T032 `llmlb/src/main.rs` を修正してサブコマンドをルーティング
  - `serve` (デフォルト): サーバー起動 + ロック取得
  - `stop`: サーバー停止
  - `status`: 状態表示
  - サブコマンドなし: 従来のサーバー起動（後方互換性）
  - 依存: T030, T031

- [x] T033 `llmlb/src/main.rs` の `run_server()` にロック統合
  - サーバー起動前に `ServerLock::acquire()` を呼び出し
  - ロック取得失敗時はエラーメッセージを表示して終了（終了コード1）
  - `ServerLock` をサーバーライフタイム全体で保持
  - 依存: T032, T029

---

## Phase 9: 仕上げ＆横断的関心事

**目的**: エラーメッセージ改善、ログ追加、品質チェック

- [x] T034 [P] `llmlb/src/lock/mod.rs` のエラーメッセージを改善
  - `AlreadyRunning` エラーにPID、起動時刻、停止方法のヒントを含める
  - フォーマット例:
    ```text
    Error: Server already running on port 8000 (PID: 12345, started: 2026-01-30T12:00:00Z)

    To stop: llmlb stop --port 8000
    Or:      kill -TERM 12345
    ```
  - 依存: T009

- [x] T035 [P] 全モジュールにdebugレベルのログを追加
  - ロック取得: "Lock acquired for port {port} (PID: {pid})"
  - ロック解除: "Lock released for port {port}"
  - 残留ロック検出: "Stale lock file detected..."
  - 依存: T016, T019, T027

- [x] T036 すべてのテストを実行して合格確認
  - `cargo test` 実行
  - 全テスト合格を確認
  - 依存: すべての実装タスク

- [x] T037 ローカル検証を実行
  - `cargo fmt --check` 合格
  - `cargo clippy -- -D warnings` 合格
  - `make quality-checks` 合格（または個別チェック）
  - markdownlint 合格
  - 依存: T036

- [x] T038 `quickstart.md` の手順を実際に実行して検証
  - `llmlb serve --port 8000` でサーバー起動
  - 別ターミナルで同じコマンドを実行してエラーを確認
  - `llmlb status` で状態確認
  - `llmlb stop --port 8000` でサーバー停止
  - 依存: T037

- [x] T039 コミット＆プッシュ
  - commitlint 準拠のコミットメッセージ
  - 依存: T038

---

## 依存関係グラフ

```text
Phase 1: セットアップ (T001-T004) → すべて並列実行可能、T004はT003に依存
  ↓
Phase 2: 基盤 (T005-T012)
  ├─ テスト (T005-T007) → 並列実行可能
  └─ 実装 (T008-T012) → テスト後に順次実装
  ↓
Phase 3: US1 重複起動防止 (T013-T016)
  ├─ テスト (T013-T014) → 並列実行可能
  └─ 実装 (T015-T016) → テスト後に順次実装
  ↓
Phase 4: US2 プロセス停止 (T017-T022)
  ├─ テスト (T017-T018) → 並列実行可能
  └─ 実装 (T019-T022) → テスト後に順次実装
  ↓
Phase 5: US3 状態確認 (T023-T025)
  ├─ テスト (T023) → 単独
  └─ 実装 (T024-T025) → テスト後に順次実装
  ↓
Phase 6: US4 クラッシュ復旧 (T026-T027)
  ├─ テスト (T026) → 単独
  └─ 実装 (T027) → テスト後に実装
  ↓
Phase 7: US5 グレースフルシャットダウン (T028-T029)
  ├─ テスト (T028) → 単独
  └─ 実装 (T029) → テスト後に実装
  ↓
Phase 8: CLI統合 (T030-T033) → 順次実装
  ↓
Phase 9: 仕上げ (T034-T039)
  ├─ T034, T035 → 並列実行可能
  └─ T036-T039 → 順次実行
```

---

## 並列実行例

### Phase 1: セットアップ

```bash
# T001-T003 を並列実行
Task T001: "llmlb/Cargo.toml に fs2 依存関係を追加"
Task T002: "llmlb/Cargo.toml に nix 依存関係を追加"
Task T003: "llmlb/src/lock/mod.rs にモジュール構造を作成"
```

### Phase 2: 基盤テスト

```bash
# T005-T007 を並列実行
Task T005: "LockInfo シリアライズ/デシリアライズテスト"
Task T006: "lock_dir() と lock_path() テスト"
Task T007: "is_process_running() テスト"
```

### Phase 3: US1テスト

```bash
# T013-T014 を並列実行
Task T013: "ServerLock::acquire() テスト"
Task T014: "重複ロック取得テスト"
```

---

## 実装戦略

### MVPファースト (US1のみ)

1. Phase 1: セットアップを完了
2. Phase 2: 基盤を完了 (重要 - すべてのストーリーをブロック)
3. Phase 3: US1を完了
4. **停止して検証**: 重複起動防止が動作することを確認
5. Phase 8の一部: serveコマンドにロック統合

### インクリメンタルデリバリー

1. セットアップ + 基盤を完了 → 基盤準備完了
2. US1を追加 → 重複起動防止が動作
3. US2を追加 → stopコマンドが動作
4. US3を追加 → statusコマンドが動作
5. US4-US5を追加 → クラッシュ復旧とグレースフルシャットダウンが動作
6. CLI統合を完了 → 全コマンドが統合された状態

---

## 注意事項

- [P]タスク = 異なるファイル、依存関係なし
- [Story]ラベルはタスクを特定のユーザーストーリーにマッピング
- **TDD厳守**: 実装前にテストを書き、テストが失敗することを確認
- 各タスクまたは論理グループ後にコミット
- 回避: 曖昧なタスク、同じファイルの競合

---

## タスク完全性検証チェックリスト

- [x] すべてのユーザーストーリーに対応するタスクがある (US1-US5)
- [x] すべてのエンティティに model タスクがある (LockInfo, ServerLock, LockError)
- [x] すべてのテストが実装より先にある (TDD RED → GREEN)
- [x] 並列タスクは本当に独立している（ファイル単位で確認済み）
- [x] 各タスクは正確なファイルパスを指定している
- [x] 同じファイルを変更する [P] タスクがない

---

## 実装準備完了

✅ すべてのタスクが定義され、実装の準備が整いました。

**次のステップ**: T001 から順番にタスクを実行し、TDD サイクル（RED → GREEN → REFACTOR）
を厳守してください。
