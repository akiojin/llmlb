# フレーク台帳

SPEC #838。retry で成功しても初回失敗を消さず、観測と根因を区別する。
回数は観測できた失敗数で、未測定をゼロにしない。

| spec | 観測日 | 発生回数 | 根因パターン | 根因（未確定可） | 関連 Issue・PR | 検査の捕捉 |
| --- | --- | --- | --- | --- | --- | --- |
| S-EP-01 Offline alpha（`endpoint-status-colors.spec.ts`、旧:320） | 2026-10-08、2026-10-11調査 | 初回1、回帰fixture 1/1 RED | (b) 確定 | 初回 alpha `0.973421`、期待 `<0.95`、retry 成功。text/class確定後もCSS色は遷移中。新回帰fixtureでは旧helperが14msでalpha=1・animation runningのまま成功を返した。animation.finishedへの同期で解消。 | [PR #837](https://github.com/akiojin/llmlb/pull/837) / [#880](https://github.com/akiojin/llmlb/issues/880)（SPEC #838 T007） | `computed-style` が最終色取得と回帰fixtureを捕捉。遷移完了待ち・旧helperでのREDを根拠に個別allowlistを更新。 |
| S-EP-01 Playground Offline表示 | 2026-10-10、2026-10-11調査 | #876のbaseline・serial・pre-pushで各1（計3） | (d) hash遷移とcache寿命の競合 | 一覧・詳細Offline後もPlaygroundはError。hashだけの遷移で5秒freshなendpoint queryを再利用し、既定5秒pollと5秒class assertが競合。新documentへreloadして色mappingの観測をcacheから分離。alpha取得前の失敗で、上の色遷移とは別根因。 | [#876](https://github.com/akiojin/llmlb/issues/876) / [PR #877](https://github.com/akiojin/llmlb/pull/877) / [#880](https://github.com/akiojin/llmlb/issues/880) | `navigation-race` が旧hash goto 4箇所を捕捉済み。hash設定→reloadへ置換して不要な4キーを削除。cache寿命自体は静的検査では証明せず、失敗snapshotと実行証跡で確認。 |
| xLLM `/v1/models` fallback | 2026-09-29 | 1以上（総数未測定） | (e)、残存待機は(a) | エフェメラルポート再利用で別プロセスの要求が混入。非同期モデル同期も厳密な回数assertに混入。UUID base pathと自動同期のthrottleで隔離。修正後の負荷実行30/30成功。 | [#748](https://github.com/akiojin/llmlb/issues/748) / [#753](https://github.com/akiojin/llmlb/pull/753) | `mock-count` が固定path＋回数観測の候補、`fixed-wait` が300msの残存待機を捕捉。UUID隔離・同期の正しさは静的検査だけでは証明できない。 |
| `download_background_transitions_to_downloading` | 2026-09-29 | 初期1/18 | (c)、撤去した危険形は(a) | 根因は固定名 `.llmlb_write_probe` のcreate_new衝突。待機不足ではない。PID＋UUID化後、固定100/200msを状態poll・仮想時間に置換。80/80、負荷下20/20成功。 | [#754](https://github.com/akiojin/llmlb/issues/754) / [#756](https://github.com/akiojin/llmlb/issues/756) / [#759](https://github.com/akiojin/llmlb/pull/759) / [#763](https://github.com/akiojin/llmlb/pull/763) | `fixed-wait` が旧sleepを捕捉。固定プローブはproduction内部のため本検査の対象外、根因回帰テストで検証済み。 |
| `check_only_does_not_download_payload` | 2026-09-29 | 再現25/80プロセス | (c) | 実 `~/.llmlb` の固定 `update-check.tmp` を共有しrenameが競合。テスト専用data dir constructorで隔離。修正後0/80。 | [#761](https://github.com/akiojin/llmlb/issues/761) / [#766](https://github.com/akiojin/llmlb/pull/766) | `shared-resource` がhome/data/temp dirと実data dirを暗黙利用するUpdateManager constructorを捕捉。 |
| navigationの4 spec（NAV-06 / NAV-08 / PS-01 / PS-02） | 2026-09-30 | CI hard fail 1、flaky 3 | (d)、NAV-08の危険形は(a) | 二重redirectがwaitForURLをabort。広いdashboard URL条件がlogin.htmlにも一致し、後続gotoと遅延redirectが競合。URL＋描画状態poll、hash変更へ置換。修正後80/80成功。 | [#770](https://github.com/akiojin/llmlb/issues/770) / [#774](https://github.com/akiojin/llmlb/pull/774) | `navigation-race` が広いURL条件・navigation event待ち・dashboard hashへのgotoを捕捉。 |
| `endpoint_health_check_test::test_endpoint_offline_status_detection` | 2026-10-09、2026-10-11調査 | #858の品質target並走で1（Rust harnessは逐次、総試行数未測定）、今回0/3 | 既存分類未確定（非同期状態競合の疑い） | 接続失敗後のstatusがoffline/errorでない。登録APIがspawnする接続確認と手動接続確認の状態書込が競合する仮説はあるが、書込traceがなく根因は未確定。色遷移とは別。 | [#858](https://github.com/akiojin/llmlb/issues/858) / [PR #859](https://github.com/akiojin/llmlb/pull/859) / [#882](https://github.com/akiojin/llmlb/issues/882) | 静的検査では捕捉不能。対象関数だけの空allowlist検査は0候補・exit 0。spawn/reset後の状態書込順は(a)〜(e)の字句検査では判定できない。同ファイルの`fixed-wait`は別TPS helper。 |
| EDV-01（`endpoint-detail-viz.spec.ts`） | 2026-10-09、2026-10-11調査 | #862のworkers=4で初回1（総試行数未測定）、今回0/3 | (a)/(c)候補、根因未確定 | endpoint行不在で20秒timeout。describe共有endpoint、作成応答未検証、固定3秒待機、共通model名が残る。どれが当該失敗を起こしたかは未確定。#877の専用fixture移行対象外。 | [#862](https://github.com/akiojin/llmlb/issues/862) / [PR #863](https://github.com/akiojin/llmlb/pull/863) / [#882](https://github.com/akiojin/llmlb/issues/882) | 空allowlistで`fixed-wait`（3秒）と`shared-resource`（API_BASE）を捕捉しexit 1。行不在の原因は静的検査では捕捉不能。作成成功・他specの削除・共有modelの意味を解析しない。 |
| EE-01（`endpoint-edit.spec.ts`） | 2026-10-09、2026-10-11調査 | #862のworkers=4で初回1（総試行数未測定）、今回0/3 | (c)候補、旧失敗の根因未確定 | 旧fixtureのendpoint行不在で10秒timeout。PR #877でテスト単位UUID名・専用model・作成成功確認・ID cleanupへ移行済み。元の行消失原因のtraceはなく、fixture改善と根治の確定は区別する。 | [#862](https://github.com/akiojin/llmlb/issues/862) / [#876](https://github.com/akiojin/llmlb/issues/876) / [PR #877](https://github.com/akiojin/llmlb/pull/877) / [#882](https://github.com/akiojin/llmlb/issues/882) | `shared-resource`がAPI_BASEを捕捉。EE-01のデータ分離・行消失自体は静的検査では捕捉不能。同ファイルの`fixed-wait`はEE-08の500msで、EE-01の根因捕捉ではない。専用fixtureに候補なし。 |
| 通知設定（`notification-settings.spec.ts`、admin sets a notification email and configures the daily digest） | 2026-10-09、2026-10-11調査 | #862のworkers=4で初回1（総試行数未測定）、今回明暗各0/3 | 未確定（HTTP作成失敗） | `createUser`がHTTP非成功を空idへ変換し、idのassertで失敗。元のstatus/bodyを保存しないためHTTP失敗理由は未確定。ランダムusernameと同spec内serialは他specとの隔離を証明しない。 | [#862](https://github.com/akiojin/llmlb/issues/862) / [PR #863](https://github.com/akiojin/llmlb/pull/863) / [#882](https://github.com/akiojin/llmlb/issues/882) | 静的検査では捕捉不能。通知specの空allowlist検査は0候補・exit 0。HTTP応答や空idへの変換は(a)〜(e)の対象構文でなく、helperのAPI_BASE候補もこの失敗の捕捉ではない。 |

観測日の過去4件は Issue・初回観測コメントの報告日。発生回数は異なる実行のため合算しない。
既知の4件に(b)の確定根因はない。S-EP-01の調査で過去4件の根因は書き換えない。

## S-EP-01 の根因調査（2026-10-11、Issue #880）

色と状態取得は独立した競合だった。実画面の計測ではOfflineのclass/textが変わった直後に
`oklab(0.635585 0.188125 0.0892456 / 0.973536)` を観測した。
その時点のtransition currentTimeは16.729ms。確定色は同じoklabのalpha `0.2`。
既存の閾値pollは途中色を許容し、opaque rgbではexplicit alphaなしとして通過する。
1秒のCSS遷移を発生させる回帰fixtureで、旧helperは14ms・`rgb(255, 0, 0)`・alpha `1`・
animation runningのまま返ってRED。修正後は1022ms・`rgba(255, 0, 0, 0.2)`・alpha `0.2`・
running animationゼロでGREEN。固定sleepやassert期限延長は導入しない。

Issue #876の保存JSONでは3回ともPlaygroundのOffline locatorを取得できず、色assertより前に失敗。
pre-pushの失敗は開始から32739msで、snapshotは`Status: Error`。
一覧・詳細のOffline確認後、同じendpointのPlaygroundへ5秒以内に戻っていた。
Playgroundを離れるとendpoint queryのイベント購読も解除されるため、その間のOffline通知は
一覧queryを更新してもinactiveなendpoint queryを更新しない。
色mapping専用テストはPlaygroundを新documentで読み込み、初回取得した状態を観測する。
製品のcache/pollingやヘルスチェックは変更しない。

新しい待機はブラウザのanimation.finishedという完了条件で、固定時間待ちではない。
取消・失敗は成功扱いせず既存toPassが再評価する。computed styleの検査は維持し、
安全な取得点と回帰fixtureだけを個別allowlistで説明するため、新パターンは追加しない。
Rustの`test_endpoint_offline_status_detection`に同じ根因があるという証拠はなく、変更対象外。

修正後の実行（Chromium、retryなし）:

- `playwright test endpoint-status-colors --repeat-each=10 --workers=1 --retries=0`:
  S-EP-01本体10/10、遷移回帰10/10、計20 PASS（5.6分）、失敗・flakyゼロ。
- `playwright test endpoint-status-colors --repeat-each=4 --workers=4 --retries=0`:
  実際の4 workerでS-EP-01本体4/4、遷移回帰4/4、計8 PASS（44.7秒）、失敗・flakyゼロ。

## 並列時の観測と捕捉判定（2026-10-11、Issue #882）

過去の一次記録はSPEC #838の[2026-10-09 Rust観測](https://github.com/akiojin/llmlb/issues/838#issuecomment-6080739402)と
[Playwright観測](https://github.com/akiojin/llmlb/issues/838#issuecomment-6083738034)。
後者の保存JSONではEDV-01・EE-01・通知設定lightが各1失敗、234成功・flaky 0。
24 skippedのうち14は既存skip、10はserial失敗後の未実行で、成功に数えない。
同じソースの直列全件は247成功・14既存skip・失敗0だった。
Rustの保存pre-pushログは接続失敗後のstatus assertで1失敗を示すが、実際のstatus値は未保存。
当時のRust harnessは`--test-threads=1`で、並走したのはmakeの品質target間だった。
今回AC指定の通常並列Rust harnessとは並列の軸が異なり、同条件での再現とは扱わない。
過去の失敗数と以下の現在の測定を合算せず、総試行数不明をゼロに置き換えない。

現在のソースはdevelop `e726b4ff8944f79dbdddbc643037fdf09cf5d232`。
Playwrightはこのcheckoutの専用port/data dirで`@real-runtimes`を除く全specを実行し、
他specとの競合も観測する。
各回は独立したCLI実行で、専用data dirは同じものを再利用する。
Rustはintegration harness全180件を通常の並列で実行し、`--test-threads=1`は指定しない。
製品・テスト・禁止パターン・allowlistは変更しない。

実行コマンド（それぞれ独立に3回、retryなし）:

- Rust: `cargo test -p llmlb --test integration_tests`。`RUST_TEST_THREADS`も未設定。
- Playwright（`llmlb/tests/e2e-playwright`）:
  `BASE_URL=http://127.0.0.1:32982 CI=1 pnpm exec playwright test --project=chromium --grep-invert @real-runtimes --workers=4 --retries=0`

| 回 | Rust全体（成功/失敗、秒） | offline検知 | Playwright全体（成功/既存skip/失敗/flaky、秒） | EDV-01 / EE-01 / 通知light / 通知dark |
| --- | --- | --- | --- | --- |
| 1 | 180/0、29.40 | PASS | 264/8/0/0、396.63 | 全てPASS |
| 2 | 180/0、33.51 | PASS | 264/8/0/0、184.19 | 全てPASS |
| 3 | 180/0、33.81 | PASS | 264/8/0/0、182.52 | 全てPASS |

全コマンドexit 0。Playwright JSONでも各回workers=4・retries=0、対象4ケースはretry=0を確認。
初回Playwrightの所要時間には新checkoutのコンパイル待ちを含む。
今回の通常並列Rustは540成功、Playwrightは792成功・24既存skip、対象ケースの新しい失敗は0。
EE-01はfixture移行後3/3成功で、旧共有fixtureの改善と整合する。
EDV-01はfixture未移行のまま3/3成功、通知設定も明暗各3/3成功、Rustも3/3成功だが、
いずれも3回で未再現という結果であり、根因の解消・競合の不存在は証明しない。

捕捉判定には実ソースの`--inventory`と、対象4ファイルを隔離コピーした空allowlist検査を使った。
後者はexit 1でEDVの3秒待機・API_BASE、EEのAPI_BASE・EE-08の500ms、別Rust TPS helperの
50ms待機の計5候補を検出した。offline対象関数だけ、通知specだけ、専用endpoint fixtureだけの
検査はそれぞれ0候補・exit 0。別関数の候補を当該失敗の捕捉とは数えない。
既存allowlistで検査が通ることも、根因の不在やfixtureの安全性を証明しない。

Rustの登録APIは接続確認をspawnして201を返す。対象テストはmockをresetし、別の接続確認で
失敗を起こして直後のstatusを読む。先の成功処理が後からOnlineへ戻す仮説はコードから成立するが、
状態書込traceを測っていないため根因は未確定。spawn/resetの意味と実際の書込順は静的検査で
捕捉不能であり、S-EP-01のCSS同期修正をこのRustテストの解消根拠には使わない。

EE-01のPR #877（`d0f44e30`）はテストごとのUUID endpoint名・専用model、作成/接続/syncの成功確認、
renameに依存しないID cleanupを導入した。EDVは同fixtureを使わず、describe共有データと
共通model名が残るため、EE-01の改善をEDV-01へ広げて解消済みとは記録しない。
通知の`createUser`はHTTP非成功時に空idを返すが、status/bodyを失うので元のHTTP失敗理由は
不明。引数の空passwordはhelper内で未使用であり、それを根因とはしない。

実行ログ・各回のJSON・失敗artifact・scanner出力は`.gwt/tmp/issue-882-evidence/`に保全する。
新しい禁止パターンやフレーク修正は本変更に含めず、必要な追跡事項は#882のPR本文で提案する。

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
