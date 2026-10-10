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

T008 では `useModelsTableViewModel` が ModelsTable の集計統計取得、モデル正規化、
検索・フィルター・ソート・展開・列の表示状態、ダイアログの開閉と対象、Playground 遷移を所有する。
`useModelEndpointStatsViewModel` は展開行の統計取得と model ごとの production TPS 表示値を所有し、
既存 `useEndpointModelTpsViewModel` の `tps` / endpoint id 購読を組み合わせる。
集計統計・行統計は移行前と同じ provider の既定ポーリングを継承し、新たな通知購読を追加しない。
列の JSX、色、badge、アイコンは ModelsTable に残し、子ダイアログ自身の取得移行は T012 で扱う。

T009 では `useEndpointTableViewModel` が EndpointTable の検索・フィルター・ソート・ページ、
ダイアログ・フォーム、表示文字列、作成・削除・接続テスト・同期の操作を所有する。
一覧取得とポーリングは親の data ViewModel に残し、一覧キーは id 無しで購読する。
操作成功時も `endpoints` / 対象 id を購読レジストリへ渡し、再取得完了まで実行状態を維持する。
接続テストの状態不変時とモデル同期は WS 通知がないため、この即時更新を維持する。
詳細 id の一致は FR-004 に従い、別 id の詳細は更新しない。子モーダルの取得移行は T012 で扱う。

T012a の `useEndpointDetailViewModel` は詳細モーダルのフォーム、today 統計、表示値、
保存・接続テスト・同期・Playground 遷移を所有する。endpoint 本体は親の snapshot を維持し、
`useEndpointViewModel` の追加 GET や `useEndpointTableViewModel` の一覧・選択状態を複製しない。
フォームの初期化は View の endpoint id key に従い、同 id の再描画では入力を保持する。
操作後の一覧更新は `endpoints` の集計購読と既存 `invalidateDashboardSubscriptions` を利用し、
元の mutation と同じく再取得の完了では pending を延長しない。別 id の詳細登録は更新しない。
`useEndpointModelsTableViewModel` はモデル一覧と統計の取得・結合・表示値を所有し、
既存 `useEndpointModelTpsViewModel` の `tps` / 同 id 購読と 10 秒 polling を合成する。
モデル一覧・統計・today は既存の provider polling と open/enabled 条件を維持する。
`useEndpointRequestChartViewModel` は 7/30/90 日の選択、日別取得、日付整形を所有する。
グラフの取得条件は id のみで、モーダル open 条件を追加しない。
現在 WS 無効化を持たない統計 query に新購読を加えず、更新タイミングを維持する。

T012b の管理モーダルは `useApiKeyModalViewModel`、`useUserModalViewModel`、
`useInvitationModalViewModel` が一覧取得、フォーム、派生表示、CRUD、コピー状態を所有する。
queryKeys と既存の clipboard / manual-copy ユーティリティを共用し、汎用 CRUD hook は追加しない。
API キーの作成は公開情報だけをキャッシュへ挿入し、平文は VM 内だけに保持する。
明示 Refresh と主モーダルの close で平文を消し、polling / focus 再取得なしを維持する。
User の全 CRUD は users と通知先設定を再取得し、Invitation は生成コードの close で
コードと作成ダイアログを閉じる。これらの異なる成功処理・フォーム寿命を共通化しない。
User / Invitation の open 条件と provider の 5 秒 polling は維持する。
現在の WS resource は管理データの変更を通知しないため、新たな購読は加えない。
`useInvalidateOn` は既存 resource に依存する query の契約であり、管理 CRUD のローカル
無効化は従来どおり VM が行う。無関係な resource への結合や wire の拡張は行わない。
badge / アイコン / フィールドの JSX は View に残す。

T012c の `useModelAddWizardViewModel` は catalog の3クエリ、300ms debounce、
段階遷移・選択・逐次ダウンロードと結果表示を所有する。View の open/closed key による
セッション破棄を保ち、catalog の取得条件と provider polling に新たなWS購読を加えない。
`useModelDeleteDialogViewModel` は削除対応判定、mutation、toast、close を所有する。
両VMは `useInvalidateOn` の宣言から返るキー専用更新コマンドを使い、操作成功で
無関係な resource 購読へ配信しない。一覧は既存 `endpoints` の集計購読を再利用し、
通知更新の無かった models / endpointModels は空の resource 宣言で polling を維持する。
削除の `endpointModels(id)` には対象 id を明示する。更新は再取得完了を待たず、
操作中に unmount しても完了時には元のキーを刷新する。
追加はバッチ終了後、削除は成功直後に従来のキャッシュを更新する。
モデル表の取得・統計は既存 `useEndpointModelsTableViewModel` の責務として複製しない。

T012d の `useClientsTabViewModel` はランキング、timeline、model 分布、heatmap、
URL の IP フィルターとページを所有する。`useClientDrilldownViewModel` は展開中の IP の
詳細と API キー使用量だけを取得し、集計取得を複製しない。詳細の寿命は既存の行展開に従う。
`useTokenStatsViewModel` は既存の 7 日・6 か月の同時取得、グラフと表の表示値を所有する。
overview や endpoint 統計は別 API なので、それらの VM を合成して余分な取得を作らない。
これらは API クライアント、queryKeys、表示の共通 utility を再利用し、provider の polling を保つ。
`useLogViewerViewModel` は 200 件・5 秒 polling、フィルター、auto-scroll、手動更新、
clear の通知、フィルター済みログのダウンロードと URL 解除を所有する。
`useAlertThresholdSettingsViewModel` は編集・正整数保存・pending と成功時 close を所有し、
`useInvalidateOn` のキー専用更新コマンドで threshold と ranking prefix だけを刷新する。
両キーは空 resource 宣言とし、新しい WS 依存を加えない。再取得を await しない既存の成功時点と、
操作中に unmount しても元のキャッシュを更新する性質を維持する。

## 参照実装: usePlayground

[`hooks/usePlayground.ts`](../hooks/usePlayground.ts) は、JSX を持たず、state・ref と
`resetChat`、`removeAttachment`、`stopGeneration` などのコマンドを返す既存の参照実装である。
入力や streaming 状態を hook が所有し、View がそれを描画する形を踏襲する。
既存の配置と呼び出し側は維持し、移行のための re-export は追加しない。

この hook は HTTP/API 取得を行わないが、FileReader、clipboard、DOM 操作と toast を使う。
副作用が存在することと、JSX を持たないことは別の契約である。
T010 の `useLoadBalancerPlaygroundViewModel` はこの hook を合成し、モデル取得・選択、
Chat の推論要求、admin 負荷試験、停止と寿命管理、分布取得・集計・表示文字列を所有する。
共通状態は複製しない。モデル一覧は既存の query key と provider の 5 秒 polling を維持し、
通知による無効化が元からないため、新しい購読や WebSocket 接続は追加しない（US-004）。
分布は Chat 成功後・負荷試験終了後・手動 Refresh で取得し、polling へ変更しない。

T011 の `useEndpointPlaygroundViewModel` も `usePlayground` を合成する。
共通の会話・入力・設定・添付・DOM・clipboard・停止コマンドを複製せず、専用 VM は
エンドポイントのモデル取得・初期選択・直接推論・エラー復旧・寿命管理・cURL と表示値を所有する。
T010 の VM はゲートウェイ推論と負荷試験・分布を所有する別の利用者であり、その挙動は変更しない。
詳細取得は既存 `useEndpointViewModel` に委譲し、`endpoints` / 同 id の `useInvalidateOn` 宣言を維持する。
モデル一覧は既存キー・retry:false・provider の 5 秒 polling を継承し、通知購読や WS 接続を追加しない。
モデル未選択時だけ先頭を選ぶ既存条件も維持する。色・badge・JSX と親の戻る操作は View が所有する。
Audit Log は T002 で移行済みのため、T011 では Query hook がページに残っていないことだけ確認する。

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
