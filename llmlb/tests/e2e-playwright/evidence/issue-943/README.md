# Issue #943: Endpoint Detail 回帰検証

担当: ニジカ。Primary Execution Owner: Issue #943。Parent SPEC: Issue #582。

## develop 基準の調査

fetch 後の `HEAD`、`origin/work/issue-943`、`origin/develop` は
`46f564a2958494cb215001f73f57b3748b69917f` で一致した。
未配信コミットと既存PRはなかった。

| AC | develop の不足と対応する検証 |
| --- | --- |
| 1–2 | 長名で表外の入力・ボタンも右端を約369px超過。3幅の幾何と全列到達を検証 |
| 3 | Save/Close、設定値、Test/Sync/Playground、Escape/focusを回帰検証 |
| 4 | 明暗・1920x1032/1280x720/375x720の実Chromiumで幾何と画像を保存 |
| 5 | 累計成功/全要求と80/95未満を説明。境界80/95のunit REDを確認 |
| 6 | health/statusの連続エラーと累計推論失敗を区別 |
| 7 | APIへendpoint_idとstatus=errorを渡す。モデル部分一致を追加しない |
| 8 | 既存error/responseと時刻を表示。空/期限外の可能性/権限/取得失敗を区別 |
| 9 | 親のspec/plan/tasks追記、明暗/キーボード/権限と配送確認 |

実装の存在だけでは配送済みとみなさず、Issue のACはdevelop包含確認後に更新する。
親SPECの既存完了記録を維持して、コメント形式の各artifactへ追記した。
`issue.spec.read/section` がこの既存形式を読めないため、GitHub原文で保存結果を照合した。

## 実ブラウザーの証跡

`endpoint-detail-overflow.spec.ts` は両テーマで次を検証する。

- 1920x1032、1280x720、375x720のDialog/フォーム/ボタンのbounding boxとscroll width
- 表の局所横scroll、矢印キーと最終列への到達、本文縦scroll
- Save/Close、Escapeと呼出元focus、設定値の維持
- 対象endpointの失敗だけをAPIで取得し、既存詳細のerrorとdatetimeを表示
- 記録なしの説明と、保持期限処理でページが縮小した後のPrevious操作
- console errors=0、page errors=0

既存 `endpoint-detail-viewmodel.spec.ts` も両テーマで実行し、
Test/Sync、フォーム設定、モデル統合、chart期間、polling契約を維持する。
停止時計を使うため、最後のtoastをUIで閉じてからフッターを操作する。

各テーマ・画面幅の `*-geometry.json` は表外要素の測定値、
`*-models-visible.png` と `*-form-visible.png` は実行時の画像である。
`before-light-1920.png` は修正前の再現画像。
`*-recorded-error.png` は既存詳細で記録されたエラーを表示した画像。
画像は合成・加工していない。

## 検証コマンド

```sh
BASE_URL=http://127.0.0.1:39443 make quality-checks
HEADED=1 BASE_URL=http://127.0.0.1:39443 pnpm --dir llmlb/tests/e2e-playwright exec \
  playwright test specs/dashboard/endpoint-detail-overflow.spec.ts \
  specs/dashboard/endpoint-detail-viewmodel.spec.ts \
  --project=chromium --workers=1 --retries=0 --headed
bash scripts/checks/check-commits.sh --from origin/main --to HEAD
```

正式結果は同じ行列を実行する `verify.run` のtool-generated記録で判定する。
PR作成前にcommit/push後の新しい記録を取得し、develop包含を確認してから完了する。

User Verification Result: n/a (autonomous)

## 現在の結果: 未配送

- `make quality-checks`: exit 2。完走したRust 3,803件、dashboard 474件は成功。
  `cargo audit` が既存依存 `crypto-bigint 0.5.5` の
  `RUSTSEC-2026-0387` で失敗した。失敗後の未起動ターゲットは成功と扱わない。
- 正式 `verify.run`: `vrr-e084389462e94334a354efdd3accbdba`。
  dashboard・ブラウザー14件・commitlint・markdownlintの4コマンド成功、audit失敗。
- ブラウザー実行は成功したが、gwtの明暗headed測定件数は双方0のままである。
  `HEADED=1` と明示configでの直接実行でも再現したため、Readyゲートは未充足。
  これは手動確認済みという意味ではない。
- gwtの計測問題はfeedback receipt
  `3e662bab-00e1-4a2e-988c-d598b6d521d8` に保存済み。送信承認待ちで、送信済みとは扱わない。
- 依存の互換patch更新は不可。JWT provider変更は認証境界かつfrontend外のため、
  PMへ別scopeの判断を引き継ぐ。audit ignoreは追加していない。
- 変更は作業ツリーに保存。コミット・push・PR・merge・Issueクローズは未実施。
  ACチェックボックスも更新しない。親SPECの既存記録は維持する。

Agent Visual Check: fail(gwtの明暗headed計測が欠落。実行14件自体は成功)
Overall: FAIL

## 再開時の診断結果

正式記録 `vrr-f948535883814c80aff903cc57972942` で前回の停止要因を再確認した。
これは診断用の行列であり、未実行のfull matrixを成功と扱わない。

| コマンド | 結果 |
| --- | --- |
| `make dashboard-checks` | exit 0、45ファイル・474テスト成功、lint error 0 |
| 同じ2specのheaded Chromium（port 39445） | exit 0、明暗各7件・計14件成功 |
| `cargo audit` | exit 1、同じRUSTSEC-2026-0387で失敗 |

ブラウザーは3画面幅の幾何、全列到達、フォームと保存、Escape/フォーカス、
記録エラーと時刻、空履歴、空ページからの復帰を再検証した。
両specのconsole/page errorのassertionは成功した。
独立コードレビューでも重大・重要な不具合は見つからなかった。

gwtの正式計測は明暗とも0件のまま。未加工の14トレースは全て
Playwright 1.64.1 / trace version 10である。
実行バイナリ `gwtd 9.145.0` に含まれるreporterは
`header.version !== 7` を拒否する。さらに旧launch `apiName` を要求するが、
現行トレースの `Launch browser` はtitleのみでlaunch paramsがない。
version許可の拡大だけではheadedの証明にならないため、reporterやtraceを書き換えない。
詳細は `resume-verification-summary.json` に保存した。

既存のmemoryにもtrace v9での同じ非互換がある。
同じ計測問題への反復実行・Playwright downgradeを行わず、PMへ根因を補足した。
既存feedback receiptは引き続きpending approvalであり、送信済みではない。
認証依存修正のscope裁定とgwtの正式計測修復が必要である。
コミット・push・PR・mergeは未実施、ACは未チェックを維持する。

User Verification Result: n/a (autonomous)
Agent Visual Check: fail(gwtの明暗headed計測各0。実ブラウザー14件は成功)
Overall: FAIL

## 再々開時の配信前提確認

今回の正式診断記録は `vrr-6ddcab3dedec484b9d0533778a35d8bd`。
`delivery-diagnostic-summary.json` に結果と未加工traceの形式を記録した。
これは診断用3コマンドであり、full matrixの成功証明ではない。

- `make dashboard-checks`: exit 0。45ファイル・474テスト成功。
  lintは既存箇所に5警告、エラー0。警告を解消済みとは扱わない。
- 明暗headed Chromium: exit 0、明暗各7件・14件成功、console/page errors=0。
  3画面幅・フォーム・全列到達・操作・focus・失敗履歴を再検証した。
- `cargo audit`: exit 1。既存の `RUSTSEC-2026-0387` が引き続きblocking。
  修正版は `>=0.7.6`。現在の `crypto-bigint 0.5.5` から互換patch更新できない。
- gwtの正式headed計測は明暗とも0件。今回の14traceもversion 10。
  ツールは同じgwtd 9.145.0であり、前回の非互換は解消していない。
- 独立レビューで重要・致命的なsource不具合やAC欠落なし。
  親SPECのspec/plan/tasks追記と、配送タスクT023の未完了をGitHub原文で確認した。
- HEADと両remoteは46f564a2のままでPRなし。sourceは未コミットの作業ツリー。
  developでは全AC未充足であり、Issueをcloseしない。
- 既存feedbackは引き続きpending approval。報告送信済みとは扱わない。
  認証依存のscope/owner裁定と、共通gwtツールの正式修復をPMへ引き継ぐ。

User Verification Result: n/a (autonomous)
Agent Visual Check: fail(gwtの明暗headed計測各0。実ブラウザー14件は成功)
Overall: FAIL

## 2026-10-27: 最新develop取込後の再診断

ニジカが最新develop `6a5fb9fe82a631400731f918d997ce16c9db721f` を
fast-forwardで取り込んだ。UIのtracked差分と未追跡45ファイルのSHA256一致を確認した。
Cargoの旧version差分はbaseと同値になり、Issue #948の修復をそのまま採用した。
独立レビューでも重要・致命的なsource不具合やAC欠落は見つからなかった。
親SPECの既存spec/plan/tasks追記と未完了T023は維持する。

正式診断記録は `vrr-1b3ee76f9e1446e4a93e72398e2aa9f4`。
`base-sync-verification-summary.json` に実行結果を保存した。

| コマンド | 結果 |
| --- | --- |
| `cargo audit` | exit 0、最新DB1378、脆弱性0、既存許容警告7 |
| `make dashboard-checks` | exit 0、45ファイル474件、lint error 0/既存warning 5 |
| 関連2specの明暗headed Chromium | exit 0、明暗各7件、console/page errors=0 |

依存監査の停止要因は解消した。一方で正規ツールは `gwtd 9.145.0` のまま、
成功したブラウザー14件の正式測定件数は明暗とも0である。
3コマンドのraw exit 0とは別に、tool-generated記録の `all_passed` はfalse。
この診断はfull matrixやcommit/push後のfresh配送検証の代替ではない。
Board escalation `43aa121e-9933-44de-9a63-a91cd1dd5466` で
canonical reporterの正式修復をPMへ引き継いだ。
既存feedback receiptはpending approvalのため重複送信しない。

User Verification Result: n/a (autonomous)
Agent Visual Check: fail(gwtの明暗headed正式測定が各0。実テスト14件は成功)
Overall: FAIL

### 全品質と失敗の再確認

`BASE_URL=http://127.0.0.1:39451 make quality-checks` は完走しexit 2。
Rust20 suiteの実行計3,864件が成功、失敗0、既存ignored 5。
この計数にはMakefileの別ターゲットで重複実行したテストを含む。
E2Eは261成功、7失敗、5flaky、9skip、10未実行だった。
`full-quality-summary.json` に集計した。

失敗はlast-adminの明暗、request-activityの明暗、LB routing、モデル登録、
playground-settings。別port39453・workers1・retries0で同じ7scenarioを再実行し、
7件すべて成功した。最初のfull品質失敗は成功に読み替えない。

調査で確認した範囲は次のとおり。

- last-adminとplayground-settingsの失敗は外部Google Fonts CSSの通信エラー。
- traceのAPI originは39451と正しい動的isolated port。32768の混入はない。
- 複数workerの916〜1032秒の空白と、OSの1032秒Sleep記録が一致した。
- request-activity初回のWS遅延とモデル登録502の根本原因は未確定。
  単独再実行は成功したため、恒久的な本番修正を推測して追加しない。

生データのarchive完了前に再実行を始めたため、一部traceが置換された。
部分archiveからfull結果JSONと15traceを回収した。
再実行の結果・ログ・artifactsは完了後に順次保存した。
元source、静的生成物、画像/寸法、正式失敗記録とコマンドログは保持している。
更新したMarkdownは全564ファイルのlintでissues 0を確認した。

残作業は、canonical reporter修復後の全品質とfresh full verify.run、
テスト先行順のcommit/push、Ready PR/auto-merge/develop包含確認である。
commit/push/PR/mergeとACチェックは行っていない。
Issue #943はOPEN・未配送としてPMへ引き継ぐ。
