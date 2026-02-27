# データモデル: llmlb 自動アップデート

## UpdateState（サーバー内状態）

```rust
enum UpdateState {
  UpToDate { checked_at: Option<DateTime<Utc>> },
  Available {
    current: String,
    latest: String,
    release_url: String,
    portable_asset_url: Option<String>,
    installer_asset_url: Option<String>,
    payload: PayloadState,
    checked_at: DateTime<Utc>,
  },
  Draining {
    latest: String,
    in_flight: usize,
    requested_at: DateTime<Utc>,
  },
  Applying {
    latest: String,
    method: ApplyMethod,
  },
  Failed {
    latest: Option<String>,
    release_url: Option<String>,
    message: String,
    failed_at: DateTime<Utc>,
  },
}

enum PayloadState {
  NotReady,
  Downloading { started_at: DateTime<Utc> },
  Ready { path: PathBuf },
  Error { message: String },
}

enum ApplyMethod {
  PortableReplace,
  MacPkg,
  WindowsSetup,
}
```

## `/api/system` レスポンス

```json
{
  "version": "3.1.0",
  "update": {
    "state": "ready",
    "latest_version": "3.2.0",
    "release_url": "https://github.com/akiojin/llmlb/releases/tag/v3.2.0",
    "detail": "Restart to update",
    "in_flight": 0,
    "checked_at": "2026-02-10T01:23:45Z"
  }
}
```
