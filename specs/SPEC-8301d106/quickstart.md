# クイックスタート: 監査ログ（Audit Log）

**機能ID**: `SPEC-8301d106` | **日付**: 2026-02-20

## 前提条件

- Rust toolchain（既存のプロジェクトと同一バージョン）
- Node.js / pnpm（ダッシュボードビルド用）
- SQLite（sqlx経由、追加インストール不要）

## 新規依存クレート

追加不要。以下の既存クレートを使用:

- `sha2`: SHA-256ハッシュ（既に`auth/middleware.rs`で使用中）
- `sqlx`: SQLiteアクセス（既存）
- `tokio`: 非同期ランタイム・mpscチャネル（既存）
- `chrono`: タイムスタンプ（既存）
- `serde`/`serde_json`: シリアライゼーション（既存）
- `tracing`: ロギング（既存）

## 環境変数

| 変数名 | デフォルト | 説明 |
|--------|-----------|------|
| LLMLB\_AUDIT\_BATCH\_INTERVAL\_SECS | 300 | ハッシュチェーンバッチ間隔（秒） |
| LLMLB\_AUDIT\_FLUSH\_INTERVAL\_SECS | 30 | バッファフラッシュ間隔（秒） |
| LLMLB\_AUDIT\_BUFFER\_CAPACITY | 10000 | バッファ上限エントリ数 |
| LLMLB\_AUDIT\_RETENTION\_DAYS | 90 | オンラインデータ保持日数 |
| LLMLB\_AUDIT\_ARCHIVE\_PATH | (data\_dir)/audit\_archive.db | アーカイブDBパス |

## セットアップ手順

1. マイグレーション実行（自動: サーバー起動時に`sqlx::migrate!`で適用）
2. サーバー起動 → 監査ログ自動記録開始
3. ダッシュボード → Audit Logページ（admin限定）

## テスト実行

```bash
# 監査ログ関連テストのみ
cargo test audit_log

# 全テスト
cargo test
```

## 開発時の確認方法

1. 開発モードでサーバー起動（`cargo run`）
2. `admin`/`test`でダッシュボードにログイン
3. 各種操作を実行（エンドポイント追加、APIキー作成等）
4. ダッシュボードのAudit Logページで記録を確認
5. REST API確認:
   - `GET /api/dashboard/audit-logs` - 監査ログ一覧
   - `POST /api/dashboard/audit-logs/verify` - ハッシュチェーン検証
   - `GET /api/dashboard/audit-logs/stats` - 統計情報
