# API契約: エンドポイント管理API

**機能ID**: `SPEC-66555000`
**日付**: 2026-01-14
**ベースパス**: `/api/endpoints`

## 認証

すべてのエンドポイントで認証必須:

- **ダッシュボード**: JWT認証（Cookie: `auth_token`）
- **API**: APIキー認証（`Authorization: Bearer {api_key}`）

## エンドポイント一覧

### POST /api/endpoints - エンドポイント登録

新しいエンドポイントを登録する。

**リクエスト**:

```json
{
  "name": "string (required, 1-100 chars)",
  "base_url": "string (required, valid URL)",
  "api_key": "string (optional)",
  "health_check_interval_secs": "number (optional, default: 30, range: 10-300)",
  "notes": "string (optional)"
}
```

**レスポンス (201 Created)**:

```json
{
  "id": "uuid",
  "name": "string",
  "base_url": "string",
  "status": "pending",
  "health_check_interval_secs": 30,
  "last_seen": null,
  "last_error": null,
  "error_count": 0,
  "registered_at": "ISO8601 datetime",
  "notes": "string | null"
}
```

**エラーレスポンス**:

- `400 Bad Request`: バリデーションエラー
- `401 Unauthorized`: 認証エラー
- `409 Conflict`: URLが重複

```json
{
  "error": {
    "code": "string",
    "message": "string"
  }
}
```

---

### GET /api/endpoints - エンドポイント一覧

登録済みエンドポイントの一覧を取得する。

**クエリパラメータ**:

- `status`: フィルタ（`pending`, `online`, `offline`, `error`）

**レスポンス (200 OK)**:

```json
{
  "endpoints": [
    {
      "id": "uuid",
      "name": "string",
      "base_url": "string",
      "status": "string",
      "health_check_interval_secs": 30,
      "last_seen": "ISO8601 datetime | null",
      "last_error": "string | null",
      "error_count": 0,
      "registered_at": "ISO8601 datetime",
      "notes": "string | null",
      "model_count": 0
    }
  ],
  "total": 0
}
```

---

### GET /api/endpoints/:id - エンドポイント詳細

特定のエンドポイントの詳細を取得する。

**パスパラメータ**:

- `id`: エンドポイントID（UUID）

**レスポンス (200 OK)**:

```json
{
  "id": "uuid",
  "name": "string",
  "base_url": "string",
  "status": "string",
  "health_check_interval_secs": 30,
  "last_seen": "ISO8601 datetime | null",
  "last_error": "string | null",
  "error_count": 0,
  "registered_at": "ISO8601 datetime",
  "notes": "string | null",
  "models": [
    {
      "model_id": "string",
      "capabilities": ["string"],
      "last_checked": "ISO8601 datetime | null"
    }
  ]
}
```

**エラーレスポンス**:

- `404 Not Found`: エンドポイントが存在しない

---

### PUT /api/endpoints/:id - エンドポイント更新

エンドポイントの情報を更新する。

**パスパラメータ**:

- `id`: エンドポイントID（UUID）

**リクエスト**:

```json
{
  "name": "string (optional)",
  "api_key": "string | null (optional, null to remove)",
  "health_check_interval_secs": "number (optional)",
  "notes": "string | null (optional)"
}
```

**レスポンス (200 OK)**:

```json
{
  "id": "uuid",
  "name": "string",
  "base_url": "string",
  "status": "string",
  "health_check_interval_secs": 30,
  "last_seen": "ISO8601 datetime | null",
  "last_error": "string | null",
  "error_count": 0,
  "registered_at": "ISO8601 datetime",
  "notes": "string | null"
}
```

**エラーレスポンス**:

- `400 Bad Request`: バリデーションエラー
- `404 Not Found`: エンドポイントが存在しない

---

### DELETE /api/endpoints/:id - エンドポイント削除

エンドポイントを削除する。

**パスパラメータ**:

- `id`: エンドポイントID（UUID）

**レスポンス (204 No Content)**:

（本文なし）

**エラーレスポンス**:

- `404 Not Found`: エンドポイントが存在しない

---

### POST /api/endpoints/:id/test - 接続テスト

エンドポイントへの接続テストを実行する。

**パスパラメータ**:

- `id`: エンドポイントID（UUID）

**レスポンス (200 OK)**:

```json
{
  "success": true,
  "latency_ms": 45,
  "endpoint_info": {
    "version": "string | null",
    "model_count": 5
  }
}
```

**エラーレスポンス (200 OK with success=false)**:

```json
{
  "success": false,
  "error": "Connection refused",
  "latency_ms": null
}
```

---

### POST /api/endpoints/:id/sync - モデル同期

エンドポイントからモデル一覧を同期する。

**パスパラメータ**:

- `id`: エンドポイントID（UUID）

**レスポンス (200 OK)**:

```json
{
  "synced_models": [
    {
      "model_id": "string",
      "capabilities": ["chat", "embeddings"]
    }
  ],
  "added": 3,
  "removed": 1,
  "updated": 2
}
```

**エラーレスポンス**:

- `404 Not Found`: エンドポイントが存在しない
- `503 Service Unavailable`: エンドポイントがオフライン

---

## エラーコード一覧

| コード | 説明 |
|--------|------|
| `invalid_request` | リクエスト形式が不正 |
| `validation_error` | バリデーションエラー |
| `unauthorized` | 認証エラー |
| `not_found` | リソースが存在しない |
| `conflict` | リソースが重複 |
| `endpoint_offline` | エンドポイントがオフライン |
| `internal_error` | 内部エラー |

---

*API契約定義完了*
