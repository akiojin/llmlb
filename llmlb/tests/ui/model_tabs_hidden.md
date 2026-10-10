# タブ検査の移行記録

旧メモは 2025-11-17 の静的 DOM のスナップショットであり、現在の要件ではない。
`static/index.html` は React のマウント先のみを持つため、旧 `.tabs-nav` や
`#tab-dashboard` を調べても画面のタブ状態は検査できない。

React 移行 `54d1408f` の後、SPEC-8795f98f の FR-001 / US1 は Models タブを
要求し、PR #448（`6fc4f5c6`）で admin の Models タブ欠落を修正した。
この後続仕様により、旧メモの「タブなし・dashboard のみ・initTabs は no-op」
という前提は廃止されている。旧機能を復活させる検査は追加しない。

現在の所在:

- 描画: `src/web/dashboard/src/pages/Dashboard.tsx` の `Tabs` / `TabsTrigger`
- 選択状態と URL: `src/web/dashboard/src/viewmodels/useDashboardViewModel.ts`
- 振る舞いの検査: `src/web/dashboard/src/pages/Dashboard.test.tsx`

後継の自動検査（Vitest）:

- `lists the registered endpoints on the default tab`: 初期選択と登録済み一覧
- `shows the request history when the Requests tab is selected`: タブ切り替え
- `limits viewers to the model list`: viewer の表示制限

旧 `.tabs-nav button` 不存在・dashboard パネルだけ active・他パネル hidden の
3 DOM 判定は、この後続仕様と上記の振る舞いテストを参照する。
`app.js` の `initTabs` は現在の実装では使われない。

実行: `pnpm --filter @llm/dashboard test -- src/pages/Dashboard.test.tsx`
