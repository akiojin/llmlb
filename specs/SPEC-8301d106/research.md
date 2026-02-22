# 技術リサーチ: 監査ログ（Audit Log）

**機能ID**: `SPEC-8301d106` | **日付**: 2026-02-20

## 1. axumミドルウェアによるリクエストキャプチャ

### アプローチ

axumの`middleware::from_fn`を使用してグローバルミドルウェアを実装する。
既存の認証ミドルウェア（`auth/middleware.rs`）と同じパターンを踏襲する。

### 実装パターン

```text
リクエスト受信
  → 監査ミドルウェア（before: メソッド/パス/ヘッダー取得）
    → 認証ミドルウェア（Claims/ApiKeyAuthContext注入）
      → ハンドラー実行
    ← レスポンス取得
  ← 監査ミドルウェア（after: ステータスコード/処理時間取得、バッファに投入）
```

### 考慮事項

- ミドルウェアのレイヤー順序が重要。
  監査ミドルウェアは認証ミドルウェアの**外側**に配置し、
  認証失敗も含めて全リクエストをキャプチャする
- `request.extensions()`からClaims/ApiKeyAuthContextを取得して
  アクター情報を決定する（認証後に注入されるため、レスポンス処理時に取得）
- 除外パターン（WebSocket、ヘルスチェック等）はパスベースのフィルタリングで実装

### 除外パスの判定

```text
除外対象:
- /ws/*           (WebSocket)
- /health*        (ヘルスチェック - 存在する場合)
- /dashboard/*    (静的アセット配信)
- 自動ポーリング（UserAgent/カスタムヘッダーで判定）
```

## 2. 非同期バッファリング

### アプローチ

`tokio::sync::mpsc`チャネルでバッファリングし、
バックグラウンドタスク（`tokio::spawn`）でDBへフラッシュする。

### 設計

- **送信側**: 監査ミドルウェアが`mpsc::Sender`でエントリを送信（非同期、ノンブロッキング）
- **受信側**: バックグラウンドタスクが受信し、メモリバッファに蓄積
- **フラッシュ条件**: 30秒経過 または バッファ件数が閾値に到達
- **バッファ上限**: 10,000件（超過時は最古エントリを破棄）
- **シャットダウン**: グレースフルシャットダウン時に残存バッファをフラッシュ

### 既存パターンとの整合性

`RequestHistoryStorage`が`save_record`で同期的にDBに書き込んでいるのに対し、
監査ログはバッファ経由の非同期書き込みとなる。
AppStateに`AuditLogWriter`（mpsc::Sender のラッパー）を追加する。

## 3. SHA-256バッチハッシュチェーン

### アルゴリズム

```text
batch_hash = SHA-256(
  previous_batch_hash
  || batch_sequence_number
  || batch_start_time
  || batch_end_time
  || record_count
  || SHA-256(record_1 || record_2 || ... || record_n)
)
```

- **genesis batch**: `previous_batch_hash = 0x00...00`（64文字のゼロ）
- **バッチ間隔**: 5分（環境変数`LLMLB_AUDIT_BATCH_INTERVAL_SECS`で変更可能）
- **レコードハッシュ**: 各レコードの主要フィールドを連結してSHA-256

### 検証

- 起動時: 全バッチを先頭から順に検証
- 24時間ごと: バックグラウンドタスクで定期検証
- 改ざん検出時: アラートログ出力 + 新しいチェーンを開始して記録続行

### ライブラリ

- `sha2` crate（既にプロジェクトで使用中: `auth/middleware.rs`の`hash_with_sha256`）

## 4. SQLite FTS5（全文検索）

### アプローチ

SQLite FTS5仮想テーブルを使用してパスやメタデータの全文検索を実装する。

### テーブル設計

```sql
CREATE VIRTUAL TABLE audit_log_fts USING fts5(
    request_path,
    actor_id,
    detail,
    content=audit_log_entries,
    content_rowid=id
);
```

- `content=`オプションで外部コンテンツテーブルとして定義
- INSERT/UPDATE/DELETEトリガーで自動同期
- `MATCH`演算子で検索、`rank`で関連度ソート

### sqlxとの互換性

- sqlxはFTS5クエリをサポート（raw SQLで実行可能）
- `sqlx::query`でMATCH句を含むクエリを直接実行

## 5. ATTACH DATABASE（アーカイブ検索）

### アプローチ

SQLiteの`ATTACH DATABASE`を使用して、
アーカイブDBをメインDBに接続し、統合検索を実現する。

### 制約事項

- sqlxの通常の接続プールでは`ATTACH DATABASE`が直接サポートされない
- アーカイブ検索時のみ専用コネクションを取得して`ATTACH`/`DETACH`を実行
- WALモードではATTACH先もWALモードである必要がある

### 代替案

アーカイブDBへの別途SqlitePoolを作成し、
検索APIでは両方のプールにクエリを発行して結果をマージする方式も検討可能。
こちらの方がsqlxとの互換性が高く、実装がシンプル。

**推奨**: 別途SqlitePool方式（シンプル、sqlx互換、テスト容易）

## 6. request\_history移行戦略

### 現状分析

`RequestHistoryStorage`（`db/request_history.rs`）は以下の機能を提供:

- `save_record`: リクエスト記録の保存
- `load_records`/`filter_and_paginate`: 一覧取得・フィルタ
- `get_token_statistics`: トークン統計（累計）
- `get_token_statistics_by_model`/`by_node`: モデル別/ノード別統計
- `get_daily_token_statistics`/`get_monthly_token_statistics`: 日次/月次統計
- `cleanup_old_records`: 古いレコードの削除

### 移行方針

1. 新しい`audit_log_entries`テーブルに`request_history`と同等の
   カラム（特にトークン関連）を含める
2. SQLマイグレーションで既存データを`audit_log_entries`にINSERT
   （`is_migrated = true`フラグ付き、ハッシュチェーン対象外）
3. トークン統計クエリを`audit_log_entries`テーブルに対して再実装
4. `RequestHistoryStorage`のメソッドを段階的に`AuditLogStorage`に委譲
5. 移行完了後に`request_history`テーブルと`RequestHistoryStorage`を削除

### AppStateの変更

```text
現在:
  request_history: Arc<RequestHistoryStorage>

移行後:
  audit_log: Arc<AuditLogStorage>    // 新規追加
  request_history: Arc<RequestHistoryStorage>  // 段階的に削除
```

## 7. ダッシュボードUI

### 既存パターン

- React + TypeScript + shadcn/ui
- ページ: `pages/Dashboard.tsx`（メインダッシュボード）
- コンポーネント: `components/dashboard/`配下に機能単位で分割
- 既存テーブル: `RequestHistoryTable.tsx`、`EndpointTable.tsx`
- UI部品: `components/ui/`（button, table, dialog, input, select等）

### 新規作成コンポーネント

```text
pages/AuditLog.tsx              - 監査ログページ（admin専用）
components/audit/
  AuditLogTable.tsx             - 監査ログテーブル
  AuditLogFilters.tsx           - フィルタパネル
  AuditLogSearch.tsx            - フリーテキスト検索
  HashChainVerification.tsx     - ハッシュチェーン検証UI
```

### ルーティング

`App.tsx`にadminロール制限付きの`/dashboard#audit-log`ルートを追加。
