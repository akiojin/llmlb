# データモデル: モデルファイル破損時の自動修復機能

**機能ID**: `SPEC-3df1b977`
**日付**: 2025-11-27

## エンティティ

### 1. ModelLoadError (新規)

モデルロード失敗時のエラー情報を表す列挙型。

```cpp
enum class ModelLoadError {
    None,              // エラーなし
    FileNotFound,      // ファイルが存在しない
    InvalidFormat,     // 拡張子が.ggufでない
    Corrupted,         // ファイル破損（ロード失敗）
    ContextFailed,     // コンテキスト作成失敗
    Unknown            // その他のエラー
};
```

### 2. RepairStatus (新規)

修復処理の状態を表す列挙型。

```cpp
enum class RepairStatus {
    Idle,           // 修復処理なし
    InProgress,     // 修復中
    Success,        // 修復成功
    Failed          // 修復失敗
};
```

### 3. RepairResult (新規)

修復処理の結果を表す構造体。

```cpp
struct RepairResult {
    RepairStatus status{RepairStatus::Idle};
    std::string error_message;        // 失敗時のエラーメッセージ
    std::string model_path;           // 修復対象のモデルパス
    std::chrono::milliseconds elapsed; // 処理時間
};
```

### 4. RepairTask (新規・内部)

進行中の修復タスクを追跡する内部構造体。

```cpp
struct RepairTask {
    std::string model_name;                              // モデル名
    std::chrono::system_clock::time_point started_at;   // 開始時刻
    std::atomic<bool> completed{false};                  // 完了フラグ
    RepairResult result;                                 // 結果
};
```

## 既存エンティティの拡張

### LlamaManager

追加メンバ:

| フィールド | 型 | 説明 |
|-----------|-----|------|
| `repair_mutex_` | `std::mutex` | 修復タスクの排他制御 |
| `repair_cv_` | `std::condition_variable` | 修復完了待機用 |
| `repairing_models_` | `std::unordered_map<std::string, std::shared_ptr<RepairTask>>` | 進行中の修復タスク |
| `auto_repair_enabled_` | `bool` | 自動修復有効フラグ |
| `repair_timeout_` | `std::chrono::milliseconds` | 修復タイムアウト |

追加メソッド:

| メソッド | 戻り値 | 説明 |
|---------|-------|------|
| `loadModelWithRepair(path, model_name)` | `std::pair<bool, ModelLoadError>` | 修復付きロード |
| `setAutoRepair(enabled)` | `void` | 自動修復の有効/無効 |
| `setRepairTimeout(timeout)` | `void` | タイムアウト設定 |
| `isRepairing(model_path)` | `bool` | 修復中か確認 |
| `waitForRepair(model_path, timeout)` | `bool` | 修復完了待機 |

### InferenceEngine

追加メンバ:

| フィールド | 型 | 説明 |
|-----------|-----|------|
| `model_sync_` | `ModelSync*` | モデル同期（ダウンロード用） |
| `model_downloader_` | `ModelDownloader*` | ダウンローダー |

## 状態遷移図

### モデルロードフロー

```
[リクエスト受信]
       ↓
[モデルパス解決] ─ 失敗 → [404エラー]
       ↓ 成功
[ロード済み確認] ─ YES → [推論実行]
       ↓ NO
[モデルロード試行]
       ↓
   ┌─ 成功 → [推論実行]
   │
   └─ 失敗
       ↓
[自動修復有効?] ─ NO → [エラー返却]
       ↓ YES
[修復中確認] ─ YES → [完了待機]
       ↓ NO                ↓
[修復開始]            [タイムアウト?]
   │                    │
   ↓                    ├─ YES → [504エラー]
[ダウンロード]          └─ NO → [再ロード試行]
   │
   ├─ 成功 → [再ロード] → [推論実行]
   │
   └─ 失敗 → [エラー返却]
```

## API レスポンス形式

### エラーレスポンス

```json
{
  "error": {
    "message": "Model repair failed: network error",
    "type": "repair_failed",
    "code": "model_repair_error",
    "details": {
      "model": "gpt-oss:7b",
      "reason": "Connection timeout"
    }
  }
}
```

### 修復中レスポンス (503)

```json
{
  "error": {
    "message": "Model is being repaired, please retry",
    "type": "service_unavailable",
    "code": "model_repairing",
    "retry_after": 30
  }
}
```
