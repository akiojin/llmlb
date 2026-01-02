# リサーチ: リクエスト履歴ページネーション

## 調査目的

リクエスト履歴一覧のページネーション実装における技術選択を調査する。

## ページネーション方式の比較

### クライアントサイドページネーション

| 項目 | 評価 |
|------|------|
| 実装難易度 | 低 |
| 初期ロード | 遅い（全件取得） |
| メモリ使用量 | 高（クライアント側） |
| ページ切替速度 | 高速（即座） |
| 適用ケース | 少量データ（~1000件） |

### サーバーサイドページネーション

| 項目 | 評価 |
|------|------|
| 実装難易度 | 中 |
| 初期ロード | 高速（必要件数のみ） |
| メモリ使用量 | 低（サーバー側でLIMIT/OFFSET） |
| ページ切替速度 | APIコール必要 |
| 適用ケース | 大量データ（1000件以上） |

## 決定事項

**サーバーサイドページネーションを採用**

### 理由

1. リクエスト履歴は時間とともに増加し、数万件に達する可能性がある
2. SQLiteの`LIMIT/OFFSET`で効率的にクエリ可能
3. クライアントメモリを節約できる
4. APIレスポンスサイズを一定に保てる

## ページサイズ設計

### 許可値の決定

```text
[10, 25, 50, 100]
```

### 根拠

- **10件**: モバイル向け、最小表示
- **25件**: デフォルト、バランス重視
- **50件**: デスクトップ向け
- **100件**: 一括確認用、最大値

### 不正値の正規化

```rust
fn normalized_per_page(requested: usize) -> usize {
    const ALLOWED: [usize; 4] = [10, 25, 50, 100];
    // 最も近い許可値に丸める
    ALLOWED.iter()
        .min_by_key(|&&x| (x as i64 - requested as i64).abs())
        .copied()
        .unwrap_or(25)
}
```

## SQL最適化

### インデックス設計

```sql
CREATE INDEX idx_request_history_timestamp
ON request_history(timestamp DESC);
```

### クエリパターン

```sql
SELECT * FROM request_history
ORDER BY timestamp DESC
LIMIT :per_page OFFSET :offset;

SELECT COUNT(*) FROM request_history;
```

## フロントエンド実装

### 状態管理

- ページ番号: URLパラメータとして管理（任意）
- ページサイズ: ローカルストレージに保存
- フィルタ条件: コンポーネント状態

### UXガイドライン

- ページ切替時にローディング表示
- 0件時は「履歴がありません」メッセージ
- 最初/最後のページでボタン無効化

## 参考資料

- [Pagination Best Practices](https://www.moesif.com/blog/technical/api-design/REST-API-Design-Filtering-Sorting-and-Pagination/)
- [SQLite LIMIT OFFSET](https://www.sqlite.org/lang_select.html#limitoffset)
