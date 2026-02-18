# データモデル: llmlb serveコマンドのシングル実行制約

**機能ID**: `SPEC-787a0b27` | **日付**: 2026-01-30

## エンティティ定義

### LockInfo

サーバーインスタンスの排他制御情報を保持する構造体。
ロックファイルにJSON形式で永続化される。

```rust
/// ロックファイルに保存されるサーバー情報
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    /// サーバープロセスのPID
    pub pid: u32,
    /// サーバー起動時刻（UTC）
    pub started_at: DateTime<Utc>,
    /// リッスンポート番号
    pub port: u16,
}
```

**フィールド説明**:

| フィールド | 型 | 説明 |
|-----------|-----|------|
| `pid` | `u32` | サーバープロセスのプロセスID |
| `started_at` | `DateTime<Utc>` | サーバー起動時刻（RFC3339形式で保存） |
| `port` | `u16` | HTTPサーバーのリッスンポート番号 |

### ServerLock

ファイルロックを保持し、ライフタイム管理を行う構造体。
RAIIパターンでロック解除を保証する。

```rust
/// サーバーのファイルロックを管理する構造体
pub struct ServerLock {
    /// ロックを保持しているファイルハンドル
    lock_file: File,
    /// ロックファイルのパス
    lock_path: PathBuf,
    /// ロック情報
    info: LockInfo,
}
```

**メソッド**:

| メソッド | 説明 |
|---------|------|
| `acquire(port: u16) -> Result<Self, LockError>` | ロックを取得し、ServerLockを返す |
| `release(self) -> Result<(), LockError>` | ロックを明示的に解除する |
| `info(&self) -> &LockInfo` | ロック情報への参照を返す |

**Dropトレイト**:

```rust
impl Drop for ServerLock {
    fn drop(&mut self) {
        // ロック解除とファイル削除を試行
        // エラーはログ出力のみ（panicしない）
    }
}
```

### LockError

ロック操作に関するエラー型。

```rust
#[derive(Debug, thiserror::Error)]
pub enum LockError {
    /// 同一ポートで既にサーバーが起動中
    #[error("Server already running on port {port} (PID: {pid}, started: {started_at})\n\nTo stop: llmlb stop --port {port}\nOr:      kill -TERM {pid}")]
    AlreadyRunning {
        port: u16,
        pid: u32,
        started_at: DateTime<Utc>,
    },

    /// ロック取得に失敗
    #[error("Failed to acquire lock: {0}")]
    AcquireFailed(#[source] std::io::Error),

    /// ロック解除に失敗
    #[error("Failed to release lock: {0}")]
    ReleaseFailed(#[source] std::io::Error),

    /// ロックファイルが破損
    #[error("Lock file corrupted: {0}")]
    Corrupted(String),

    /// ロックディレクトリの作成に失敗
    #[error("Failed to create lock directory: {0}")]
    DirectoryCreationFailed(#[source] std::io::Error),
}
```

## ファイルフォーマット

### ロックファイル

**パス**: `/tmp/llmlb/serve_{port}.lock`（Unix）、`%TEMP%\llmlb\serve_{port}.lock`（Windows）

**フォーマット**: JSON

```json
{
  "pid": 12345,
  "started_at": "2026-01-30T12:00:00.000000Z",
  "port": 8000
}
```

**パーミッション**: 600（オーナーのみ読み書き可能）

## CLI引数定義

### ServeArgs

```rust
#[derive(Args, Debug)]
pub struct ServeArgs {
    /// Listen port (default: 32768)
    #[arg(short, long, default_value = "32768", env = "LLMLB_PORT")]
    pub port: u16,

    /// Bind address (default: 0.0.0.0)
    #[arg(short = 'H', long, default_value = "0.0.0.0", env = "LLMLB_HOST")]
    pub host: String,
}
```

### StopArgs

```rust
#[derive(Args, Debug)]
pub struct StopArgs {
    /// Port of the server to stop
    #[arg(short, long)]
    pub port: u16,

    /// Force stop without confirmation
    #[arg(short, long)]
    pub force: bool,
}
```

### StatusArgs

```rust
#[derive(Args, Debug)]
pub struct StatusArgs {
    /// Show status of specific port only
    #[arg(short, long)]
    pub port: Option<u16>,

    /// Output format
    #[arg(short, long, default_value = "table")]
    pub format: OutputFormat,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum OutputFormat {
    Table,
    Json,
}
```

## 関係図

```text
┌─────────────────────────────────────────────────────────────┐
│                        llmlb CLI                             │
├─────────────────────────────────────────────────────────────┤
│  Commands                                                    │
│  ├── serve (ServeArgs)  ──────────┐                         │
│  ├── stop (StopArgs)    ──────────┼── uses ── ServerLock    │
│  └── status (StatusArgs) ─────────┘            │            │
└─────────────────────────────────────────────────────────────┘
                                                  │
                                                  ▼
┌─────────────────────────────────────────────────────────────┐
│                      ServerLock                              │
├─────────────────────────────────────────────────────────────┤
│  - lock_file: File                                          │
│  - lock_path: PathBuf                                       │
│  - info: LockInfo ─────────────────────┐                    │
├─────────────────────────────────────────────────────────────┤
│  + acquire(port) -> Result<Self>       │                    │
│  + release(self) -> Result<()>         │                    │
│  + info() -> &LockInfo                 │                    │
└─────────────────────────────────────────────────────────────┘
                                         │
                                         ▼
┌─────────────────────────────────────────────────────────────┐
│                       LockInfo                               │
├─────────────────────────────────────────────────────────────┤
│  - pid: u32                                                  │
│  - started_at: DateTime<Utc>                                │
│  - port: u16                                                 │
└─────────────────────────────────────────────────────────────┘
                                         │
                                         ▼
┌─────────────────────────────────────────────────────────────┐
│              Lock File (JSON)                                │
│              /tmp/llmlb/serve_{port}.lock                   │
├─────────────────────────────────────────────────────────────┤
│  {                                                           │
│    "pid": 12345,                                            │
│    "started_at": "2026-01-30T12:00:00Z",                    │
│    "port": 8000                                             │
│  }                                                           │
└─────────────────────────────────────────────────────────────┘
```

## ライフサイクル

### 正常起動フロー

```text
1. llmlb serve --port 8000
2. lock_dir() でロックディレクトリを取得
3. ディレクトリが存在しなければ作成
4. lock_path(8000) でロックファイルパスを取得
5. ロックファイルが存在する場合:
   a. JSON読み込み → LockInfo取得
   b. PID検証: プロセスが存在するか確認
   c. 存在する → AlreadyRunning エラー
   d. 存在しない → 残留ロックとして削除
6. ファイルを作成/オープン
7. try_lock_exclusive() でflockを取得
8. LockInfoをJSON形式で書き込み
9. パーミッションを600に設定
10. ServerLockを返す
```

### 正常終了フロー

```text
1. SIGTERM/SIGINT受信 または Ctrl+C
2. シグナルハンドラがフラグ設定
3. メインループがフラグを検出
4. グレースフルシャットダウン開始
5. ServerLockのDrop実行:
   a. ファイルロック解除 (unlock)
   b. ロックファイル削除
   c. ログ出力（debug）
```

### stopコマンドフロー

```text
1. llmlb stop --port 8000
2. lock_path(8000) でロックファイルパスを取得
3. ファイルが存在しない → "起動中のサーバーがありません"
4. JSON読み込み → LockInfo取得
5. PID検証:
   a. 存在しない → ロックファイル削除、警告表示
   b. 存在する → SIGTERM送信
6. プロセス終了を待機（タイムアウト5秒）
7. 終了確認 → "サーバーを停止しました"
```
