# 実装計画: llmlb serveコマンドのシングル実行制約

**機能ID**: `SPEC-52587000` | **日付**: 2026-01-30 | **仕様**: [spec.md](./spec.md)
**入力**: `/specs/SPEC-52587000/spec.md` の機能仕様

## 概要

llmlb serveコマンドで同一ポート番号での重複起動を防止する機能。
flock（Unix）/ LockFile（Windows）を使用したクロスプラットフォーム対応の
ファイルロック機構を実装し、ロックファイルにはJSON形式でPID・起動時刻・ポートを記録。
既存プロセスの停止（stop）、状態確認（status）コマンドも追加する。

## 技術コンテキスト

**言語/バージョン**: Rust 1.75+
**主要依存関係**: fs2（クロスプラットフォームファイルロック）、serde_json、chrono、clap
**ストレージ**: ロックファイル（JSON形式、/tmp/llmlb/serve_{port}.lock）
**テスト**: cargo test（unit/integration）
**対象プラットフォーム**: macOS, Linux, Windows
**プロジェクトタイプ**: single（llmlbクレート内に実装）
**パフォーマンス目標**: ロック取得/解除は1ms以内
**制約**: NFS対応（fcntl使用）、グレースフルシャットダウン必須
**スケール/スコープ**: 単一マシン上での複数ポート管理

## 憲章チェック

*ゲート: Phase 0 research前に合格必須。Phase 1 design後に再チェック。*

**シンプルさ**:

- プロジェクト数: 1（llmlbクレートのみ）✓
- フレームワークを直接使用? Yes（clap直接使用、ラッパーなし）✓
- 単一データモデル? Yes（LockInfo構造体のみ）✓
- パターン回避? Yes（Repositoryパターン不使用、直接ファイルI/O）✓

**アーキテクチャ**:

- すべての機能をライブラリとして? Yes（llmlb/src/以下にモジュール実装）✓
- ライブラリリスト:
  - `llmlb::lock` - ファイルロック機構
  - `llmlb::cli::stop` - stopコマンド
  - `llmlb::cli::status` - statusコマンド
- ライブラリごとのCLI: `llmlb --help/--version`（既存CLIを拡張）✓

**テスト (妥協不可)**:

- RED-GREEN-Refactorサイクルを強制? Yes ✓
- Gitコミットはテストが実装より先に表示? Yes（TDD厳守）✓
- 順序: Contract→Integration→E2E→Unitを厳密に遵守? Yes ✓
- 実依存関係を使用? Yes（実ファイルシステム）✓
- 禁止: テスト前の実装、REDフェーズのスキップ ✓

**可観測性**:

- 構造化ロギング含む? Yes（tracingクレート使用、既存パターン準拠）✓
- ロック取得/解除をdebugレベルでログ出力 ✓
- エラーコンテキスト十分? Yes（PID、起動時刻、停止方法のヒント）✓

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-52587000/
├── spec.md              # 機能仕様 (/speckit.specify コマンド出力) ✓
├── plan.md              # このファイル (/speckit.plan コマンド出力)
├── research.md          # Phase 0 出力 (/speckit.plan コマンド)
├── data-model.md        # Phase 1 出力 (/speckit.plan コマンド)
├── quickstart.md        # Phase 1 出力 (/speckit.plan コマンド)
└── tasks.md             # Phase 2 出力 (/speckit.tasks コマンド)
```

### ソースコード (リポジトリルート)

```text
llmlb/
├── src/
│   ├── lock/
│   │   └── mod.rs             # NEW: ファイルロック機構
│   ├── cli/
│   │   ├── mod.rs             # MODIFY: サブコマンド追加（serve/stop/status）
│   │   ├── serve.rs           # NEW: serveサブコマンド
│   │   ├── stop.rs            # NEW: stopサブコマンド
│   │   └── status.rs          # NEW: statusサブコマンド
│   ├── main.rs                # MODIFY: サブコマンドルーティング
│   └── lib.rs                 # MODIFY: lockモジュール公開
│
├── tests/
│   ├── integration/
│   │   └── lock_test.rs       # NEW: ロック機構統合テスト
│   └── unit/
│       └── lock_test.rs       # NEW: ロック機構ユニットテスト
│
└── Cargo.toml                 # MODIFY: fs2依存関係追加
```

**構造決定**: 既存の単一プロジェクト構造を維持し、llmlbクレート内に
ロック機構とCLIサブコマンドを追加

## Phase 0: アウトライン＆リサーチ

### 不明点の抽出と解決

1. **クロスプラットフォームファイルロックの実装方法**
   - 決定: `fs2`クレートを使用
   - 理由: Unix（flock/fcntl）とWindows（LockFileEx）を抽象化
   - 検討した代替案:
     - `fd-lock` → メンテナンス頻度が低い
     - 手動実装 → 車輪の再発明
   - トレードオフ: 外部依存追加（但し、広く使われている安定クレート）

2. **ロックファイルの配置場所**
   - 決定: OS標準の一時ディレクトリ（`/tmp/llmlb/`または`%TEMP%\llmlb\`）
   - 理由: 再起動時に自動クリーンアップ、権限問題を回避
   - 検討した代替案:
     - `~/.llmlb/locks/` → ユーザー固有だが再起動後も残る
     - `/var/run/` → root権限が必要な場合がある
   - パターン: `std::env::temp_dir()`で取得

3. **残留ロックの検出と解除**
   - 決定: PID検証による自動解除
   - 理由: クラッシュ後の手動介入を不要にする
   - 検討した代替案:
     - タイムアウトベース → 正常稼働中のロックも解除される危険
     - ハートビート → 過剰な複雑さ
   - 実装: `sysinfo`クレートでプロセス存在確認

4. **グレースフルシャットダウンの実装**
   - 決定: `ctrlc`クレート + Dropトレイト
   - 理由: SIGTERM/SIGINTの両方をハンドル、二重保護
   - 検討した代替案:
     - シグナルハンドラのみ → Dropで対応できない場合がある
     - Dropのみ → シグナル時のクリーンアップが保証されない
   - パターン: シグナルハンドラでフラグ設定、メインループで検出して終了

5. **stopコマンドの実装**
   - 決定: ロックファイルからPIDを読み、SIGTERMを送信
   - 理由: 既存のロック情報を活用
   - 検討した代替案:
     - UNIXソケット → Windows非対応
     - HTTP API → サーバー側変更が必要
   - Windowsでの代替: `TerminateProcess`または`taskkill`

### 技術選択のベストプラクティス

**fs2でのファイルロック**:

```rust
use fs2::FileExt;
use std::fs::File;

let lock_file = File::create(&lock_path)?;
lock_file.try_lock_exclusive()?; // 非ブロッキング
// ロック保持中...
lock_file.unlock()?;
```

**ctrlcでのシグナルハンドリング**:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

let running = Arc::new(AtomicBool::new(true));
let r = running.clone();
ctrlc::set_handler(move || {
    r.store(false, Ordering::SeqCst);
})?;
```

**プロセス存在確認（sysinfo）**:

```rust
use sysinfo::{System, Pid, ProcessRefreshKind, RefreshKind};

let mut system = System::new_with_specifics(
    RefreshKind::nothing().with_processes(ProcessRefreshKind::new())
);
system.refresh_processes();
let exists = system.process(Pid::from(pid)).is_some();
```

**出力**: すべての要明確化が解決された `research.md` を作成予定

## Phase 1: 設計＆契約

### 1. データモデル (`data-model.md`)

**LockInfo構造体** (`llmlb/src/lock/mod.rs`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    pub pid: u32,
    pub started_at: DateTime<Utc>,
    pub port: u16,
}
```

**ServerLock構造体** (`llmlb/src/lock/mod.rs`):

```rust
pub struct ServerLock {
    lock_file: File,
    lock_path: PathBuf,
    info: LockInfo,
}

impl ServerLock {
    pub fn acquire(port: u16) -> Result<Self, LockError>;
    pub fn release(self) -> Result<(), LockError>;
    pub fn info(&self) -> &LockInfo;
}

impl Drop for ServerLock {
    fn drop(&mut self);
}
```

**LockError enum** (`llmlb/src/lock/mod.rs`):

```rust
#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("Server already running on port {port} (PID: {pid}, started: {started_at})")]
    AlreadyRunning { port: u16, pid: u32, started_at: DateTime<Utc> },
    #[error("Failed to acquire lock: {0}")]
    AcquireFailed(#[source] std::io::Error),
    #[error("Failed to release lock: {0}")]
    ReleaseFailed(#[source] std::io::Error),
    #[error("Lock file corrupted: {0}")]
    Corrupted(String),
}
```

### 2. CLIサブコマンド設計

**Cli enum拡張** (`llmlb/src/cli/mod.rs`):

```rust
#[derive(Parser, Debug)]
#[command(name = "llmlb")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the load balancer server
    Serve(ServeArgs),
    /// Stop a running server
    Stop(StopArgs),
    /// Show status of running servers
    Status(StatusArgs),
}

#[derive(Args, Debug)]
pub struct ServeArgs {
    #[arg(short, long, default_value = "32768")]
    pub port: u16,
}

#[derive(Args, Debug)]
pub struct StopArgs {
    #[arg(short, long)]
    pub port: u16,
}

#[derive(Args, Debug)]
pub struct StatusArgs {
    #[arg(short, long)]
    pub port: Option<u16>,
}
```

### 3. 関数シグネチャ

**ロック取得/解除** (`llmlb/src/lock/mod.rs`):

```rust
pub fn lock_dir() -> PathBuf;
pub fn lock_path(port: u16) -> PathBuf;
pub fn read_lock_info(port: u16) -> Result<Option<LockInfo>, LockError>;
pub fn is_process_running(pid: u32) -> bool;
```

**stopコマンド** (`llmlb/src/cli/stop.rs`):

```rust
pub async fn execute(args: &StopArgs) -> Result<(), anyhow::Error>;
```

**statusコマンド** (`llmlb/src/cli/status.rs`):

```rust
pub async fn execute(args: &StatusArgs) -> Result<(), anyhow::Error>;
```

**出力**: data-model.md, quickstart.md を作成予定

## Phase 2: タスク計画アプローチ

*このセクションは /speckit.tasks コマンドが実行することを記述*

**タスク生成戦略**:

1. **Setup タスク**:
   - [P] `Cargo.toml`に`fs2`依存関係を追加
   - [P] `llmlb/src/lock/mod.rs`にモジュール構造を作成
   - [P] `llmlb/src/lib.rs`にlockモジュールを公開

2. **Contract Test タスク**:
   - [P] LockInfo構造体のシリアライズ/デシリアライズテスト
   - [P] ServerLock::acquire()の契約テスト
   - [P] ServerLock::release()の契約テスト

3. **Integration Test タスク** (依存順):
   - ロック取得/解除の統合テスト
   - 重複起動検知のテスト
   - 残留ロック自動解除のテスト
   - グレースフルシャットダウンのテスト

4. **Core 実装タスク** (TDD: Test → Impl):
   - LockInfo構造体実装
   - ServerLock構造体実装
   - ロック取得ロジック（flock + JSON書き込み）
   - ロック解除ロジック（unlock + ファイル削除）
   - 残留ロック検出（PID検証）

5. **CLI 実装タスク**:
   - Cliをサブコマンド対応に拡張
   - serveサブコマンド実装（ロック統合）
   - stopサブコマンド実装（SIGTERM送信）
   - statusサブコマンド実装（ロック情報表示）

6. **E2E Test タスク**:
   - 完全なワークフロー: serve → status → stop

7. **Polish タスク**:
   - エラーメッセージの改善（PID、起動時刻、停止ヒント）
   - ログ出力の追加（debug レベル）
   - ローカル検証 (`make quality-checks`)

**順序戦略**:

- TDD順序厳守: Contract Test → Integration Test → Impl → E2E
- 並列実行可能: Setup, Contract tests は並列実行可 [P]
- Lock層 → CLI層の依存関係順

**推定出力**: tasks.mdに約25個の番号付き、順序付きタスク

## 複雑さトラッキング

*憲章チェックに正当化が必要な違反がある場合のみ記入*

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|--------------------------------|
| なし | - | - |

すべての憲章要件を満たしています。

## 進捗トラッキング

*このチェックリストは実行フロー中に更新される*

**フェーズステータス**:

- [x] Phase 0: Research完了 (/speckit.plan コマンド)
- [x] Phase 1: Design完了 (/speckit.plan コマンド)
- [x] Phase 2: Task planning完了 (/speckit.plan コマンド - アプローチのみ記述)
- [x] Phase 3: Tasks生成済み (/speckit.tasks コマンド)
- [x] Phase 4: 実装完了
- [ ] Phase 5: 検証合格

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み (なし)

---

*憲章 v2.0.0 に基づく - `/memory/constitution.md` 参照*
