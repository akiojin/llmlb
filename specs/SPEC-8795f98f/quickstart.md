# クイックスタート: SPEC-8795f98f

## 前提条件

- ダッシュボードの開発環境が起動可能であること
- `pnpm --filter @llm/dashboard dev` でローカル開発サーバーが起動すること
- llmlbサーバーが起動済みで、1つ以上のエンドポイントが登録されていること

## 開発手順

### 1. ダッシュボード開発サーバー起動

```bash
pnpm --filter @llm/dashboard dev
```

### 2. 変更対象ファイル一覧

| ファイル | 変更内容 |
|----------|----------|
| `pages/Dashboard.tsx` | Modelsタブ追加（TabsTrigger + TabsContent） |
| `components/dashboard/ModelsTable.tsx` | 新規作成: Modelsタブ本体 |
| `components/dashboard/EndpointDetailModal.tsx` | ScrollArea追加 |
| `pages/LoadBalancerPlayground.tsx` | initialModel対応 |
| `App.tsx` | parseHash拡張（?model=xxx） |

### 3. ビルドと検証

```bash
# ダッシュボードビルド
pnpm --filter @llm/dashboard build

# Rustサーバー再ビルド（静的アセット埋め込み）
cargo build

# 品質チェック
make quality-checks
```

### 4. 動作確認

1. ブラウザで `http://localhost:3100/dashboard/` にアクセス
2. admin/test でログイン
3. Modelsタブが2番目に表示されることを確認
4. エンドポイント詳細モーダルがスクロール可能なことを確認
5. ModelsタブからPlaygroundへの遷移を確認
