# リサーチ: llmlb serveコマンドのシングル実行制約

**機能ID**: `SPEC-52587000` | **日付**: 2026-01-30

## 技術調査結果

### 1. クロスプラットフォームファイルロック

#### 選択: `fs2` クレート

**理由**:

- Unix（flock/fcntl）とWindows（LockFileEx）を統一APIで抽象化
- NFS対応（fcntlベース）
- 広く使われている安定クレート（crates.io で 500万+ ダウンロード）
- メンテナンスが活発

**代替案と却下理由**:

| 代替案 | 却下理由 |
|--------|----------|
| `fd-lock` | メンテナンス頻度が低い、最終更新が古い |
| 手動実装 | 車輪の再発明、プラットフォーム固有の複雑さ |
| `advisory-lock` | fs2より機能が限定的 |

**使用パターン**:

```rust
use fs2::FileExt;
use std::fs::OpenOptions;

// 排他ロック取得（非ブロッキング）
let file = OpenOptions::new()
    .write(true)
    .create(true)
    .open(&lock_path)?;

match file.try_lock_exclusive() {
    Ok(()) => { /* ロック取得成功 */ }
    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
        // 他プロセスがロック保持中
    }
    Err(e) => { /* その他のエラー */ }
}
```

### 2. ロックファイルの配置場所

#### 選択: OS標準の一時ディレクトリ

**パス**: `/tmp/llmlb/serve_{port}.lock`（Unix）、`%TEMP%\llmlb\serve_{port}.lock`（Windows）

**理由**:

- 再起動時に自動クリーンアップされる
- 権限問題を回避（ユーザー書き込み可能）
- 標準的なパターン

**代替案と却下理由**:

| 代替案 | 却下理由 |
|--------|----------|
| `~/.llmlb/locks/` | 再起動後も残る、手動クリーンアップが必要 |
| `/var/run/` | root権限が必要な場合がある |
| カレントディレクトリ | 実行場所に依存、予測不能 |

**実装**:

```rust
pub fn lock_dir() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push("llmlb");
    path
}

pub fn lock_path(port: u16) -> PathBuf {
    let mut path = lock_dir();
    path.push(format!("serve_{}.lock", port));
    path
}
```

### 3. 残留ロック検出と自動解除

#### 選択: PID検証による自動解除

**理由**:

- クラッシュ後の手動介入が不要
- 確実にプロセス存在を確認できる
- `sysinfo`クレート（既に依存に含まれる）で実装可能

**代替案と却下理由**:

| 代替案 | 却下理由 |
|--------|----------|
| タイムアウトベース | 正常稼働中のロックも解除される危険性 |
| ハートビート | 過剰な複雑さ、定期的なファイル更新が必要 |
| flock自動解除のみ | プロセス終了でflockは解除されるがファイルは残る |

**実装**:

```rust
use sysinfo::{System, Pid, ProcessRefreshKind, RefreshKind};

pub fn is_process_running(pid: u32) -> bool {
    let mut system = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::new())
    );
    system.refresh_processes();
    system.process(Pid::from(pid as usize)).is_some()
}
```

### 4. グレースフルシャットダウン

#### 選択: シグナルハンドラ + Dropトレイト（二重保護）

**理由**:

- SIGTERM/SIGINTの両方を確実にハンドル
- 正常終了・異常終了の両方でロック解除を保証
- Rustのオーナーシップシステムと自然に統合

**代替案と却下理由**:

| 代替案 | 却下理由 |
|--------|----------|
| シグナルハンドラのみ | Drop時の解除が保証されない |
| Dropのみ | シグナル時の即時クリーンアップが保証されない |
| atexit | Rustでは非推奨、Dropが推奨パターン |

**実装パターン**:

```rust
// シグナルハンドラ
use tokio::signal;

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

// Dropトレイト
impl Drop for ServerLock {
    fn drop(&mut self) {
        if let Err(e) = self.release_internal() {
            tracing::error!("Failed to release lock on drop: {}", e);
        }
    }
}
```

### 5. stopコマンドの実装

#### 選択: ロックファイルからPIDを読み、シグナル送信

**Unix**: `kill(pid, SIGTERM)`
**Windows**: `TerminateProcess`または`taskkill`コマンド

**理由**:

- 既存のロック情報を活用（追加の通信機構不要）
- プラットフォームネイティブなプロセス終了

**代替案と却下理由**:

| 代替案 | 却下理由 |
|--------|----------|
| UNIXソケット | Windows非対応 |
| HTTP API（/shutdown） | サーバー側変更が必要、認証問題 |
| Named Pipe | 複雑、既存のロック機構で十分 |

**実装**:

```rust
#[cfg(unix)]
pub fn stop_process(pid: u32) -> Result<(), std::io::Error> {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid as i32), Signal::SIGTERM)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
}

#[cfg(windows)]
pub fn stop_process(pid: u32) -> Result<(), std::io::Error> {
    use std::process::Command;
    Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()?;
    Ok(())
}
```

## ロックファイルフォーマット

```json
{
  "pid": 12345,
  "started_at": "2026-01-30T12:00:00Z",
  "port": 8000
}
```

## 依存関係追加

```toml
# Cargo.toml
[dependencies]
fs2 = "0.4"  # クロスプラットフォームファイルロック

# Unix シグナル送信用（既存のtokioで対応可能）
[target.'cfg(unix)'.dependencies]
nix = { version = "0.29", features = ["signal"] }
```

## 結論

すべての技術的不明点が解決され、実装計画の準備が整いました。
主要な技術選択は以下の通り:

1. **ファイルロック**: `fs2`クレート（クロスプラットフォーム対応）
2. **ロック場所**: OS一時ディレクトリ（`/tmp/llmlb/`）
3. **残留ロック**: PID検証で自動解除（`sysinfo`使用）
4. **シャットダウン**: シグナルハンドラ + Drop（二重保護）
5. **stopコマンド**: SIGTERM送信（Unix）/ taskkill（Windows）
