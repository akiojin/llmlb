# データモデル: Web UI 画面一覧

## 画面定義

### Screen (画面)

```typescript
interface Screen {
  id: string;          // SCR-001, SCR-002, etc.
  path: string;        // /dashboard/login.html
  name: string;        // ログイン
  description: string; // 画面の説明
  relatedSpec: string; // SPEC-XXXXXXXX
  requiresAuth: boolean;
}
```

### Section (セクション)

```typescript
interface Section {
  id: string;          // SEC-001, SEC-002, etc.
  parentScreenId: string; // SCR-010
  name: string;        // Header
  description: string; // セクションの説明
  relatedSpec: string; // SPEC-XXXXXXXX
}
```

### Modal (モーダル)

```typescript
interface Modal {
  id: string;          // MOD-001, MOD-002, etc.
  parentScreenId: string; // SCR-010
  name: string;        // NodeSettingsModal
  description: string; // モーダルの説明
  relatedSpec: string; // SPEC-XXXXXXXX
}
```

## 画面遷移定義

### Navigation (遷移)

```typescript
interface Navigation {
  from: string;        // 遷移元画面ID
  to: string;          // 遷移先画面ID
  trigger: string;     // ボタンクリック、リンク等
  condition?: string;  // 遷移条件（認証済み等）
}
```

## 画面一覧データ

### 画面

| ID | パス | 名前 | 認証必須 |
|----|------|------|---------|
| SCR-001 | `/dashboard/login.html` | ログイン | No |
| SCR-002 | `/dashboard/register.html` | ユーザー登録 | No |
| SCR-010 | `/dashboard` | メインダッシュボード | Yes |
| SCR-011 | `/playground` | Playground | Yes |

### セクション

| ID | 親画面 | 名前 |
|----|--------|------|
| SEC-001 | SCR-010 | Header |
| SEC-002 | SCR-010 | StatsCards |
| SEC-003 | SCR-010 | NodeTable |
| SEC-004 | SCR-010 | RequestHistoryTable |
| SEC-005 | SCR-010 | LogViewer |
| SEC-006 | SCR-010 | ModelsSection |
| SEC-007 | SCR-010 | CloudProvidersSection |

### モーダル

| ID | 親画面 | 名前 |
|----|--------|------|
| MOD-001 | SCR-010 | NodeSettingsModal |
| MOD-002 | SCR-010 | NodeDeleteConfirm |
| MOD-003 | SCR-010 | RequestDetailModal |
| MOD-004 | SCR-010 | ModelRegisterModal |

## 遷移マトリクス

| From | To | トリガー |
|------|-----|---------|
| SCR-001 | SCR-002 | 招待コード入力 |
| SCR-001 | SCR-010 | ログイン成功 |
| SCR-002 | SCR-001 | 登録完了 |
| SCR-010 | SCR-011 | Chatボタン |
| SCR-011 | SCR-010 | 戻るボタン |
| any | SCR-001 | ログアウト |

## パフォーマンス要件

| 項目 | 値 |
|------|-----|
| 画面遷移時間 | 500ms以内 |
| 初回ロード時間 | 2秒以内 |
| セッション状態保持 | ブラウザリロード後も維持 |
