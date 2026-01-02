# クイックスタート: リクエスト履歴ページネーション

## 概要

リクエスト履歴一覧でページネーションを使用する方法を説明する。

## API使用例

### 基本的なページネーション

```bash
# 1ページ目を取得（デフォルト25件）
curl http://localhost:8080/api/dashboard/request-history

# 2ページ目を取得
curl "http://localhost:8080/api/dashboard/request-history?page=2"

# 1ページあたり50件で取得
curl "http://localhost:8080/api/dashboard/request-history?per_page=50"

# 3ページ目を100件/ページで取得
curl "http://localhost:8080/api/dashboard/request-history?page=3&per_page=100"
```

### フィルタとの組み合わせ

```bash
# モデル名でフィルタしつつページネーション
curl "http://localhost:8080/api/dashboard/request-history?model=llama&page=1&per_page=25"
```

## レスポンス形式

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

## パラメータ詳細

| パラメータ | 型 | デフォルト | 説明 |
|-----------|-----|----------|------|
| page | integer | 1 | ページ番号（1始まり） |
| per_page | integer | 25 | 1ページの件数（10/25/50/100） |
| model | string | - | モデル名フィルタ（部分一致） |

## ページサイズの正規化

許可されていない値を指定した場合、最も近い許可値に自動変換される。

| 指定値 | 変換後 |
|--------|--------|
| 5 | 10 |
| 15 | 10 |
| 30 | 25 |
| 75 | 50 |
| 200 | 100 |

## ダッシュボードUI

### 表示件数セレクタ

ページ上部のドロップダウンで表示件数を選択可能。

- 10件
- 25件（デフォルト）
- 50件
- 100件

### ページナビゲーション

```text
[<前へ] [1 / 6] [次へ>]
```

- 1ページ目では「前へ」ボタン無効
- 最終ページでは「次へ」ボタン無効
- 0件時は「- / -」表示

## エラーハンドリング

### 範囲外のページ指定

存在しないページを指定した場合、空の`records`が返る。

```json
{
  "records": [],
  "total": 150,
  "page": 100,
  "per_page": 25,
  "total_pages": 6
}
```

### 0件の場合

```json
{
  "records": [],
  "total": 0,
  "page": 1,
  "per_page": 25,
  "total_pages": 0
}
```

## パフォーマンス

- ページ切替: 0.1秒以内
- 初期ロード: 2秒以内（100件以上でも）
- SQLインデックス活用でOFFSETクエリを最適化
