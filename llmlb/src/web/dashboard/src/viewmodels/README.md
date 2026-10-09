# Dashboard ViewModel 規約

SPEC #821 T002 の規約。試験移行は Audit Log 画面で行い、後続タスクで他の画面へ適用する。

## 配置と公開型

- 画面単位の hook を `src/viewmodels/use<Name>ViewModel.ts` に置く。
- production ViewModel は `.ts` とし、JSX を含めない。テストの provider wrapper は `.test.tsx` でよい。
- 戻り値に明示的な `<Name>ViewModel` 型を付け、描画に必要な状態・派生値・操作を公開する。
- 子 View には必要な部分だけ渡す。`Pick` や用途別の小さな型を用い、汎用の基底クラスや registry は導入しない。
- API 応答全体や React Query の戻り値をそのまま View に渡さず、表示に必要な値を公開する。

## 責務

ViewModel は取得、認可に基づく取得条件、画面状態、派生値、操作を所有する。
`useQuery`、`useMutation`、`useQueryClient` が必要なら ViewModel 内で呼ぶ。
HTTP 通信は `fetchWithAuth` を使う `src/lib/api/*` のクライアントを通し、クエリキーは
`src/lib/queryKeys.ts` から取得する。直接の `fetch`、認証の省略、キーの改名は行わない。

View は公開された値を JSX で描画し、入力やクリックをコマンドに渡す。
日付・件数・リンクなどの表示用データは ViewModel で導出する。
色、badge variant、CSS、アイコン、loading/empty の描画分岐は View に置く。
画面遷移など親が所有する操作は props で受け取り、そのまま接続してよい。

タイマーや購読などの寿命は hook の寿命に結び付け、unmount 時に解除する。
移行時は既存の query key、取得条件、更新間隔、エラー表示、操作結果の保持を確認する。
resource に依存する query は、取得を所有する ViewModel 内で
`useInvalidateOn(resources, queryKey, { id? })` を宣言する。集計 query は id を宣言せず、
詳細 query は同じ id の通知だけに反応するよう明示する。id 無し通知は全購読に一致する。
WebSocket transport は購読レジストリへの通知だけを行い、画面と query の対応を持たない。

T005 では通知対象の query だけを `useDashboardDataViewModel`、`useEndpointViewModel`、
`useEndpointModelTpsViewModel` に抽出した。残りの画面状態や操作は T007 以降で移行する。
Audit Log は現在の通知 resource に依存しないため、購読を追加しない。
`dashboardResourceCoverage.test.tsx` は実 ViewModel の購読を全 resource で検査し、
resource union を増やすとコンパイラも新しい期待行を要求する。

T007 では `useDashboardViewModel` が Dashboard ページの認可、WS 接続、取得、履歴整形、
タブを所有し、既存の `useDashboardDataViewModel` を組み合わせる。
`useSystemUpdateViewModel` は更新バナーの表示値、操作、ダイアログ、カウントダウンを所有する。
更新コマンドは system クエリを手動で無効化せず、data ViewModel の `system` 購読で取得する。
更新チェック応答のキャッシュ反映と、WS 切断時 5 秒・接続時 10 秒のポーリングは維持する。

## 参照実装: usePlayground

[`hooks/usePlayground.ts`](../hooks/usePlayground.ts) は、JSX を持たず、state・ref と
`resetChat`、`removeAttachment`、`stopGeneration` などのコマンドを返す既存の参照実装である。
入力や streaming 状態を hook が所有し、View がそれを描画する形を踏襲する。
既存の配置と呼び出し側は維持し、移行のための re-export は追加しない。

この hook は HTTP/API 取得を行わないが、FileReader、clipboard、DOM 操作と toast を使う。
副作用が存在することと、JSX を持たないことは別の契約である。
Playground の API 取得や推論要求との責務分担は T010 で扱う。

## 試験移行: Audit Log

[`useAuditLogViewModel.ts`](./useAuditLogViewModel.ts) は次の型を公開する。

- `AuditLogViewModel`: admin 判定、一覧、フィルター、300ms の検索 debounce、ページ数と操作。
- `AuditLogRow`: timestamp、actor、duration、token の表示値と client drilldown のリンク。
- `AuditLogVerificationViewModel`: 検証中の状態、結果、エラー、`verify` コマンド。

`pages/AuditLog.tsx` は hook を呼び、`components/audit/` の View に値と操作を渡す。
検証の再実行開始時はエラーだけを消し、前の結果は成功・失敗にかかわらず次の結果まで保持する。
非 admin は一覧を取得せず、検証コマンドも API を呼ばない。
Select の status 値は API 型に合わせて数値化する。HTTP 上のフィルター値は同じである。

## 検証

hook テストは公開契約を `renderHook` で検証し、通信境界だけを置き換える。
認可、フィルター変更とページリセット、debounce と解除、nullable/zero 値、検証の再試行を確認する。
既存の `pages/AuditLog.test.tsx` は変更せず、View の表示と操作が維持されたことを確認する。
Playwright は実認証した Chromium で明暗両テーマの操作と console/page error 0 件を確認する。

実行コマンド:

```sh
pnpm --filter @llm/dashboard test
pnpm --filter @llm/dashboard typecheck
pnpm --filter @llm/dashboard lint
pnpm --filter @llm/dashboard build
```

全体の品質検証には repository の `make quality-checks` と正式な `verify.plan` / `verify.run` を使う。
