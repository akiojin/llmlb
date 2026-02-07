# クイックスタート: SPEC-5045f436: トークン累積統計機能

## 前提条件

| 項目 | 要件 |
|------|------|
| 認証 | ダッシュボードJWT（`Authorization: Bearer <jwt>`） |
| API | `/api/dashboard/stats/tokens` 系エンドポイント |

## 基本的な使用例

### 累積トークン統計

```bash
curl -X GET http://localhost:8080/api/dashboard/stats/tokens \
  -H "Authorization: Bearer your-dashboard-jwt"
```

### 日次統計

```bash
curl -X GET "http://localhost:8080/api/dashboard/stats/tokens/daily?days=7" \
  -H "Authorization: Bearer your-dashboard-jwt"
```

### 月次統計

```bash
curl -X GET "http://localhost:8080/api/dashboard/stats/tokens/monthly?months=6" \
  -H "Authorization: Bearer your-dashboard-jwt"
```

## 参照

- spec.md
- plan.md
