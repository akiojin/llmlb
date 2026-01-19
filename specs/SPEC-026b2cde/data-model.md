# データモデル: リクエスト履歴ページネーション

## エンティティ定義

### RequestHistoryQuery

APIリクエストのクエリパラメータを表現する。

```rust
pub struct RequestHistoryQuery {
    /// ページ番号（1始まり）
    pub page: usize,
    /// 1ページあたりの表示件数
    pub per_page: usize,
    /// モデル名フィルタ（部分一致）
    pub model: Option<String>,
}

impl Default for RequestHistoryQuery {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 25,
            model: None,
        }
    }
}

impl RequestHistoryQuery {
    /// 許可されたページサイズに正規化
    pub fn normalized_per_page(&self) -> usize {
        const ALLOWED: [usize; 4] = [10, 25, 50, 100];
        ALLOWED.iter()
            .min_by_key(|&&x| (x as i64 - self.per_page as i64).abs())
            .copied()
            .unwrap_or(25)
    }

    /// SQLのOFFSET値を計算
    pub fn offset(&self) -> usize {
        (self.page.saturating_sub(1)) * self.normalized_per_page()
    }
}
```

### PaginatedResponse

ページネーション付きレスポンスの共通構造。

```rust
pub struct PaginatedResponse<T> {
    /// データレコード
    pub records: Vec<T>,
    /// 総レコード数
    pub total: usize,
    /// 現在のページ番号
    pub page: usize,
    /// 1ページあたりの件数
    pub per_page: usize,
    /// 総ページ数
    pub total_pages: usize,
}

impl<T> PaginatedResponse<T> {
    pub fn new(records: Vec<T>, total: usize, query: &RequestHistoryQuery) -> Self {
        let per_page = query.normalized_per_page();
        let total_pages = (total + per_page - 1) / per_page;
        Self {
            records,
            total,
            page: query.page,
            per_page,
            total_pages,
        }
    }
}
```

## フィールド制約

| フィールド | 型 | 制約 | デフォルト |
|-----------|-----|------|----------|
| page | usize | >= 1 | 1 |
| per_page | usize | [10, 25, 50, 100] | 25 |
| model | Option&lt;String&gt; | 任意 | None |
| total | usize | >= 0 | - |
| total_pages | usize | >= 0 | - |

## 状態遷移

### ページ遷移

```text
[Page 1] --next--> [Page 2] --next--> [Page N]
   ^                  |                  |
   |                  v                  v
   +-----prev--------+-------prev-------+
```

### フィルタ適用時

```text
[Any Page] --filter_change--> [Page 1]
[Any Page] --per_page_change--> [Page 1]
```

## JSONシリアライゼーション

### レスポンス例

```json
{
  "records": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "timestamp": "2025-01-02T10:30:00Z",
      "model": "llama-3.1-8b",
      "status": "success",
      "duration_ms": 1250
    }
  ],
  "total": 150,
  "page": 1,
  "per_page": 25,
  "total_pages": 6
}
```

## 関連テーブル

### request_history

```sql
CREATE TABLE request_history (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    request_type TEXT NOT NULL,
    model TEXT NOT NULL,
    runtime_id TEXT,
    client_ip TEXT,
    duration_ms INTEGER NOT NULL,
    status TEXT NOT NULL
);

CREATE INDEX idx_request_history_timestamp
ON request_history(timestamp DESC);

CREATE INDEX idx_request_history_model
ON request_history(model);
```
