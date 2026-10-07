# フレーク台帳

SPEC #838。retry で成功しても初回失敗を消さず、観測と根因を区別する。
回数は観測できた失敗数で、未測定をゼロにしない。

| spec | 観測日 | 発生回数 | 根因パターン | 根因（未確定可） | 関連 Issue・PR | 検査の捕捉 |
| --- | --- | --- | --- | --- | --- | --- |
| `llmlb/tests/e2e-playwright/specs/dashboard/endpoint-status-colors.spec.ts:320` Offline alpha | 2026-10-08 | 1 | (b) 仮説 | 初回 alpha `0.973421`、期待 `<0.95`、retry 成功。根因未確定。仮説は `transition-colors` の途中値。 | [PR #837](https://github.com/akiojin/llmlb/pull/837) pre-push | `computed-style` が背景色取得 helper を捕捉。解消は検査着地後の T007。 |
| xLLM `/v1/models` fallback | 2026-09-29 | 1以上（総数未測定） | (e)、残存待機は(a) | エフェメラルポート再利用で別プロセスの要求が混入。非同期モデル同期も厳密な回数assertに混入。UUID base pathと自動同期のthrottleで隔離。修正後の負荷実行30/30成功。 | [#748](https://github.com/akiojin/llmlb/issues/748) / [#753](https://github.com/akiojin/llmlb/pull/753) | `mock-count` が固定path＋回数観測の候補、`fixed-wait` が300msの残存待機を捕捉。UUID隔離・同期の正しさは静的検査だけでは証明できない。 |
| `download_background_transitions_to_downloading` | 2026-09-29 | 初期1/18 | (c)、撤去した危険形は(a) | 根因は固定名 `.llmlb_write_probe` のcreate_new衝突。待機不足ではない。PID＋UUID化後、固定100/200msを状態poll・仮想時間に置換。80/80、負荷下20/20成功。 | [#754](https://github.com/akiojin/llmlb/issues/754) / [#756](https://github.com/akiojin/llmlb/issues/756) / [#759](https://github.com/akiojin/llmlb/pull/759) / [#763](https://github.com/akiojin/llmlb/pull/763) | `fixed-wait` が旧sleepを捕捉。固定プローブはproduction内部のため本検査の対象外、根因回帰テストで検証済み。 |
| `check_only_does_not_download_payload` | 2026-09-29 | 再現25/80プロセス | (c) | 実 `~/.llmlb` の固定 `update-check.tmp` を共有しrenameが競合。テスト専用data dir constructorで隔離。修正後0/80。 | [#761](https://github.com/akiojin/llmlb/issues/761) / [#766](https://github.com/akiojin/llmlb/pull/766) | `shared-resource` がhome/data/temp dirと実data dirを暗黙利用するUpdateManager constructorを捕捉。 |
| navigationの4 spec（NAV-06 / NAV-08 / PS-01 / PS-02） | 2026-09-30 | CI hard fail 1、flaky 3 | (d)、NAV-08の危険形は(a) | 二重redirectがwaitForURLをabort。広いdashboard URL条件がlogin.htmlにも一致し、後続gotoと遅延redirectが競合。URL＋描画状態poll、hash変更へ置換。修正後80/80成功。 | [#770](https://github.com/akiojin/llmlb/issues/770) / [#774](https://github.com/akiojin/llmlb/pull/774) | `navigation-race` が広いURL条件・navigation event待ち・dashboard hashへのgotoを捕捉。 |

観測日の過去4件は Issue・初回観測コメントの報告日。発生回数は異なる実行のため合算しない。
既知の4件に(b)の確定根因はない。今回の仮説で過去の根因を書き換えない。

## 分類と禁止候補

- **(a) `fixed-wait`**: sleep、setTimeout、waitForTimeout。状態poll内の間隔やモックの遅延注入も
  候補として拾う。テストdeadlineを設定する `test.setTimeout` は待機に含めない。
- **(b) `computed-style`**: getComputedStyle、toHaveCSS。遷移途中の数値を一度だけassertする形を
  防ぐため観測点を検出する。状態に同期する安全な利用は個別allowlistで説明する。
- **(c) `shared-resource`**: 固定temp名、home/data dir、固定ポートのbind/listen・API_BASE、
  実data dirを暗黙利用するUpdateManager constructor。単なる例示endpoint URLは含めない。
- **(d) `navigation-race`**: #770を根拠に追加。広すぎるdashboard/login URL待ち、
  waitForNavigation、dashboard hashへのgoto。redirect後の最終状態を待つこと。
- **(e) `mock-count`**: #748を根拠に追加。固定mock pathとfetch_add・expect(n)・回数assertの
  同一ファイル内の組合せ。外部要求やspawnの副作用に対する隔離・同期を確認すること。

(d)/(e)を追加した理由は、#770のredirect競合と#748のエフェメラルポート再利用・非同期副作用が
初期3分類のどれにも当てはまらないため。タイトルではなく修正Issue/PRの根因に従う。

## 検査と回復方法

`make flaky-patterns` が未知の候補と削除済みallowlistを失敗にする。
診断にはpattern、ファイル、行番号と内容が出る。
`bash scripts/checks/check-flaky-patterns.sh --inventory` で現在の候補を一覧できる。
修正して不要なallowlist行を削除するか、根拠を確認してコメント付きの個別キーを追加する。
CIが自動でallowlistを拡大することはない。

allowlistは `pattern|file|行内容のcksumとbyte数|同じ内容の出現番号`。
行移動は許可を壊さず、行内容変更や同一違反追加は新しい違反として失敗する。
各既存候補は(a)〜(e)の分類とFR-005（既存assert維持）の根拠コメントを持つ。
広いファイル単位の許可は使わない。

検査はbash/awkの保守的な候補検出で、意味解析ではない。
Rustはtests配下・tests.rs・cfg(test)のitem、TypeScriptはテストとE2E helperを対象にする。
複数行に分割した呼出し、別関数に隠れた共有資源、モックの同期完了は完全には判断できない。
(a)〜(e)の違反サンプル、安全な状態poll・UUID/tempdir・production timeout、allowlistの
追加/削除境界は `tests/checks/test-check-flaky-patterns.bats` で実行して検証する。

## CI の観測を台帳へ集める

既存 Playwright JSON reporterの `flaky` と、Rust初回FAILED→同じharnessの再実行okを
`scripts/checks/collect-flaky-tests.mjs` で同じMarkdown表・JSONへ変換する。
ログのstdoutには `playwright_flaky` / `rust_flaky` の機械可読件数が出る。
欠損・壊れた・途中のreportは失敗になり、ゼロ件として扱わない。

- Playwright: `flaky-tests-playwright-<attempt>` artifactに共通表、JSON、元results.jsonを保存する。
  成功した実行も保存するため、retry成功が見える。
- Rust: 初回失敗stepは失敗のまま、FAILEDテストがあるときだけ同じsuiteを診断再実行する。
  `--test-threads=1` を維持し、`flaky-tests-rust-<OS>-<attempt>` artifactに両ログ・共通表・JSONを保存する。
  初回から成功したテストや、再実行しても失敗したテストはフレーク成功に含めない。

共通表には観測日・spec・発生回数・未確定根因・CI runリンクが入る。
artifactの観測を調査して、このファイルへ根因・Issue/PR・捕捉可否を追記する。
CIはsource台帳を直接commitせず、同じ台帳形式のartifactを残す。未確定の根因を推測で確定しない。

artifact名とrunリンクにはattempt番号を含め、CIの再実行で初回観測を上書きしない。
