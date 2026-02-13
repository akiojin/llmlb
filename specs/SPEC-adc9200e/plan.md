# SPEC-adc9200e: 実装計画

## 技術概要

Buttonコンポーネント（`button.tsx`）のバリアント定義を中心に改善し、
ghostバリアントの廃止・用途別置換、全バリアントの状態表現強化、
生`<button>`タグのコンポーネント化を行う。

## 変更対象ファイル

### コア変更（ボタンコンポーネント）

| ファイル | 変更内容 |
|----------|----------|
| `components/ui/button.tsx` | バリアント定義の全面改修、ghost廃止 |
| `index.css` | focus ring用CSS変数追加（必要に応じて） |

### ghost → outline/link 置換

| ファイル | ghost使用数 | 置換先 |
|----------|-------------|--------|
| `components/dashboard/Header.tsx` | 3箇所 | outline（アイコンボタン） |
| `components/dashboard/EndpointTable.tsx` | 4箇所 | outline（アイコンボタン） |
| `components/dashboard/RequestHistoryTable.tsx` | 2箇所 | outline（アイコンボタン） |
| `components/api-keys/ApiKeyModal.tsx` | 3箇所 | outline（アイコンボタン） |
| `components/invitations/InvitationModal.tsx` | 1箇所 | outline（アイコンボタン） |
| `components/users/UserModal.tsx` | 2箇所 | outline（アイコンボタン） |
| `pages/EndpointPlayground.tsx` | 2箇所 | outline（アイコンボタン） |
| `pages/LoadBalancerPlayground.tsx` | 4箇所 | outline（アイコン）/link（テキスト） |

### 生`<button>`/`<a>` → Buttonコンポーネント化

| ファイル | 箇所 | 置換方法 |
|----------|------|----------|
| `pages/Dashboard.tsx:192` | "Restart to update" | `<Button>` default |
| `pages/Dashboard.tsx:231` | "Try again" | `<Button variant="link">` |
| `pages/EndpointPlayground.tsx:671` | 添付ファイル削除（画像） | `<Button variant="destructive" size="icon">` |
| `pages/EndpointPlayground.tsx:683` | 添付ファイル削除（音声） | `<Button variant="destructive" size="icon">` |
| `pages/LoadBalancerPlayground.tsx:1169` | 添付ファイル削除 | `<Button variant="destructive" size="icon">` |

## button.tsx バリアント設計

### 共通ベーススタイル変更

```text
変更前: transition-all duration-200
変更後: transition-colors transition-shadow transition-transform duration-150
        active:duration-75
```

- `transition-all` → 明示的なプロパティ指定に変更
- hover: `duration-150`（即応性優先）
- active: `duration-75`（即座の反応）

### バリアント別変更

#### default

```text
変更前:
  bg-primary text-primary-foreground shadow-md
  hover:bg-primary/90 hover:shadow-lg
  active:scale-[0.98]

変更後:
  bg-primary text-primary-foreground shadow-md
  hover:bg-primary/85 hover:shadow-lg hover:-translate-y-0.5
  active:scale-[0.95] active:shadow-none active:translate-y-0
```

#### destructive

```text
変更前:
  bg-destructive text-destructive-foreground shadow-md
  hover:bg-destructive/90 hover:shadow-lg
  active:scale-[0.98]

変更後:
  bg-destructive text-destructive-foreground shadow-md
  hover:bg-destructive/85 hover:shadow-lg hover:-translate-y-0.5
  active:scale-[0.95] active:shadow-none active:translate-y-0
```

#### outline

```text
変更前:
  border border-input bg-background shadow-sm
  hover:bg-accent hover:text-accent-foreground hover:border-accent-foreground/20

変更後:
  border border-input bg-background shadow-sm
  hover:bg-accent hover:text-accent-foreground hover:border-accent-foreground/20
  hover:shadow-md hover:-translate-y-0.5
  active:scale-[0.95] active:shadow-none active:translate-y-0 active:bg-accent/80
```

#### secondary

```text
変更前:
  bg-secondary text-secondary-foreground shadow-sm
  hover:bg-secondary/80

変更後:
  bg-secondary text-secondary-foreground shadow-sm
  hover:bg-secondary/70 hover:shadow-md hover:-translate-y-0.5
  active:scale-[0.95] active:shadow-none active:translate-y-0
```

#### ghost → 廃止

- エクスポートから削除
- 使用箇所をすべてoutlineまたはlinkに置換

#### link

```text
変更前:
  text-primary underline-offset-4 hover:underline

変更後:
  text-primary underline-offset-4
  hover:underline hover:text-primary/80
  active:text-primary/60
```

#### glow（維持・改善）

```text
変更前:
  bg-primary text-primary-foreground shadow-md
  hover:shadow-lg glow-sm hover:glow
  active:scale-[0.98]

変更後:
  bg-primary text-primary-foreground shadow-md glow-sm
  hover:shadow-lg hover:glow hover:-translate-y-0.5
  active:scale-[0.95] active:shadow-none active:translate-y-0
```

### disabled状態の強化

```text
変更前: disabled:pointer-events-none disabled:opacity-50
変更後: disabled:pointer-events-none disabled:opacity-40
        disabled:cursor-not-allowed disabled:saturate-0
```

- `opacity-50` → `opacity-40`（より明確な無効感）
- `saturate-0` 追加（色味を消してグレー化）
- `cursor-not-allowed` 追加（明示的な操作不可表示）

### フォーカスリング改善

```text
変更前: focus-visible:outline-none focus-visible:ring-2
        focus-visible:ring-ring focus-visible:ring-offset-2
変更後: focus-visible:outline-none focus-visible:ring-2
        focus-visible:ring-foreground focus-visible:ring-offset-4
        focus-visible:ring-offset-background
```

- `ring-ring`（=primary色）→ `ring-foreground`（テキスト色=高コントラスト）
- `ring-offset-2` → `ring-offset-4`（ボタン境界との間隔拡大）
- `ring-offset-background` 追加（オフセット色を背景色に明示）

## テスト戦略

### E2Eテスト（MCP Playwright）

既存のE2Eテストでボタンの操作確認を行う。
ボタンの視覚的変更はCSS変更のみなので、機能テストは既存テストで網羅される。

### ビルド検証

- `pnpm --filter @llm/dashboard build` が成功すること
- TypeScript型エラーがないこと
- ghostバリアントの参照が残っていないこと

## リスク

| リスク | 対策 |
|--------|------|
| ghost廃止で型エラー | TypeScript型定義からghost削除→コンパイルエラーで全箇所検出 |
| 添付ファイル削除ボタンの特殊レイアウト | rounded-full等のclassNameはそのまま渡す |
| alertDialogがbuttonVariantsを直接使用 | buttonVariantsの変更で自動的に反映される |
