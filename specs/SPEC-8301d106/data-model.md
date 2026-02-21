# データモデル: 監査ログ（Audit Log）

**機能ID**: `SPEC-8301d106` | **日付**: 2026-02-20

## テーブル定義

### audit\_log\_entries（監査ログエントリ）

| カラム | 型 | 制約 | 説明 |
|--------|-----|------|------|
| id | INTEGER | PRIMARY KEY AUTOINCREMENT | レコードID |
| timestamp | TEXT | NOT NULL | ISO 8601タイムスタンプ |
| http\_method | TEXT | NOT NULL | HTTPメソッド（GET/POST/PUT/DELETE/PATCH） |
| request\_path | TEXT | NOT NULL | リクエストパス |
| status\_code | INTEGER | NOT NULL | HTTPステータスコード |
| actor\_type | TEXT | NOT NULL | アクター種別（user/api\_key/anonymous） |
| actor\_id | TEXT | | アクターID（user\_id or api\_key\_id） |
| actor\_username | TEXT | | ユーザー名（表示用、認証失敗時は入力値） |
| api\_key\_owner\_id | TEXT | | APIキー発行者のuser\_id |
| client\_ip | TEXT | | クライアントIPアドレス |
| duration\_ms | INTEGER | | リクエスト処理時間（ミリ秒） |
| input\_tokens | INTEGER | | 入力トークン数（推論リクエストのみ） |
| output\_tokens | INTEGER | | 出力トークン数（推論リクエストのみ） |
| total\_tokens | INTEGER | | 合計トークン数（推論リクエストのみ） |
| model\_name | TEXT | | モデル名（推論リクエストのみ） |
| endpoint\_id | TEXT | | エンドポイントID（推論リクエストのみ） |
| detail | TEXT | | 操作の追加情報（JSON） |
| batch\_id | INTEGER | REFERENCES audit\_batch\_hashes(id) | 所属バッチID |
| is\_migrated | INTEGER | NOT NULL DEFAULT 0 | request\_historyからの移行データフラグ |
| created\_at | TEXT | NOT NULL DEFAULT (datetime('now')) | レコード作成日時 |

### インデックス

```sql
CREATE INDEX idx_audit_log_timestamp ON audit_log_entries(timestamp);
CREATE INDEX idx_audit_log_actor ON audit_log_entries(actor_type, actor_id);
CREATE INDEX idx_audit_log_path ON audit_log_entries(request_path);
CREATE INDEX idx_audit_log_status ON audit_log_entries(status_code);
CREATE INDEX idx_audit_log_batch ON audit_log_entries(batch_id);
CREATE INDEX idx_audit_log_model ON audit_log_entries(model_name)
  WHERE model_name IS NOT NULL;
CREATE INDEX idx_audit_log_tokens ON audit_log_entries(timestamp, model_name)
  WHERE total_tokens IS NOT NULL;
```

### audit\_batch\_hashes（バッチハッシュ）

| カラム | 型 | 制約 | 説明 |
|--------|-----|------|------|
| id | INTEGER | PRIMARY KEY AUTOINCREMENT | バッチID |
| sequence\_number | INTEGER | NOT NULL UNIQUE | バッチ連番 |
| batch\_start | TEXT | NOT NULL | バッチ開始時刻 |
| batch\_end | TEXT | NOT NULL | バッチ終了時刻 |
| record\_count | INTEGER | NOT NULL | バッチ内レコード数 |
| hash | TEXT | NOT NULL | SHA-256ハッシュ値 |
| previous\_hash | TEXT | NOT NULL | 前バッチのハッシュ値 |
| created\_at | TEXT | NOT NULL DEFAULT (datetime('now')) | 作成日時 |

### audit\_log\_fts（全文検索仮想テーブル）

```sql
CREATE VIRTUAL TABLE audit_log_fts USING fts5(
    request_path,
    actor_id,
    actor_username,
    detail,
    content=audit_log_entries,
    content_rowid=id
);
```

## FTS同期トリガー

```sql
CREATE TRIGGER audit_log_fts_insert AFTER INSERT ON audit_log_entries BEGIN
    INSERT INTO audit_log_fts(rowid, request_path, actor_id, actor_username, detail)
    VALUES (new.id, new.request_path, new.actor_id, new.actor_username, new.detail);
END;

CREATE TRIGGER audit_log_fts_delete AFTER DELETE ON audit_log_entries BEGIN
    INSERT INTO audit_log_fts(audit_log_fts, rowid, request_path, actor_id, actor_username, detail)
    VALUES ('delete', old.id, old.request_path, old.actor_id, old.actor_username, old.detail);
END;
```

## エンティティ関係

```text
audit_batch_hashes 1 --- * audit_log_entries
  (batch_id FK)

audit_log_entries 1 --- 1 audit_log_fts
  (content table)

users 1 --- * audit_log_entries
  (actor_id = user.id WHERE actor_type = 'user')

api_keys 1 --- * audit_log_entries
  (actor_id = api_key.id WHERE actor_type = 'api_key')
```

## ハッシュチェーン構造

```text
Genesis Batch (seq=1)
  previous_hash: "0000...0000" (64 zeros)
  hash: SHA-256(previous_hash || seq || start || end || count || records_hash)

Batch N (seq=N)
  previous_hash: Batch(N-1).hash
  hash: SHA-256(previous_hash || seq || start || end || count || records_hash)

records_hash = SHA-256(
  record_1.timestamp || record_1.http_method || record_1.request_path || ...
  || record_2.timestamp || ...
  || ...
)
```

## request\_history移行SQL

```sql
INSERT INTO audit_log_entries (
    timestamp, http_method, request_path, status_code,
    actor_type, actor_id, duration_ms,
    input_tokens, output_tokens, total_tokens,
    model_name, endpoint_id, is_migrated
)
SELECT
    rh.created_at,
    'POST',
    '/v1/chat/completions',
    CASE WHEN rh.error IS NULL THEN 200 ELSE 500 END,
    'api_key',
    COALESCE(rh.api_key_id, 'unknown'),
    rh.duration_ms,
    rh.input_tokens,
    rh.output_tokens,
    rh.total_tokens,
    rh.model,
    rh.endpoint_id,
    1
FROM request_history rh;
```

## Rustデータ構造

### AuditLogEntry

```text
struct AuditLogEntry {
    id: Option<i64>,
    timestamp: DateTime<Utc>,
    http_method: String,
    request_path: String,
    status_code: u16,
    actor_type: ActorType,      // enum { User, ApiKey, Anonymous }
    actor_id: Option<String>,
    actor_username: Option<String>,
    api_key_owner_id: Option<String>,
    client_ip: Option<String>,
    duration_ms: Option<i64>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    total_tokens: Option<i64>,
    model_name: Option<String>,
    endpoint_id: Option<String>,
    detail: Option<String>,
    batch_id: Option<i64>,
    is_migrated: bool,
}
```

### ActorType

```text
enum ActorType {
    User,
    ApiKey,
    Anonymous,
}
```

### AuditBatchHash

```text
struct AuditBatchHash {
    id: Option<i64>,
    sequence_number: i64,
    batch_start: DateTime<Utc>,
    batch_end: DateTime<Utc>,
    record_count: i64,
    hash: String,
    previous_hash: String,
}
```
