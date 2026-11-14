## Summary

長いモデル名がEndpoint Detail本文を押し広げ、フォームと操作が見切れる問題を修正します。
成功率の累計分母分子と連続health/statusエラーを説明し、当該endpointの失敗要求から
記録されたエラーと時刻へ進めるようにします。

## Changes

- EndpointDetailModal/EndpointModelsTable: native縦scroll、操作の折返し、表のみkeyboard横scroll。
- EndpointFailureHistory/ViewModel: 既存endpoint_id/status=error APIとserver pagingを再利用。
- RequestHistoryTable: 既存error/response詳細、時刻、理由未記録、入れ子dialogのfocus復帰。
- 回帰testsとstaticを更新。新API/server/認証境界の変更なし。

## Testing

- [x] 通常pre-push全品質exit0。
- [x] 正式full品質exit0: Rust延べ3868/20suite、dashboard474、全E2E284pass/8skip/0flaky。
- [x] 専用headed Chromium14件、明暗各7、console/page errors0、retry0、3幅とkeyboard/focus。
- [x] 最新audit DB1395/脆弱性0、markdown566files/issues0、commitlint18成功。
- [x] 独立review重要/重大0。
- 正式record: vrr-c89ef64b95e34c0b8431ca43b445c637。3raw exit0、plan_covered=true。
- 署名付きall_passed=falseを保持。toolの失敗をPASSへ書き換えていない。

## PR Readiness

- State: Blocked（提出用本文を準備済み、PR未作成）。
- Releaseable slice: yes、frontendだけで完結。
- Known blockers: gwtd9.147.0 reporterのtrace v10非対応。
- Remaining acceptance: develop未配送の全AC、親SPEC T023。
- User Verification Result: n/a (autonomous)
- Agent Visual Check: blocked (実headed明暗各7件PASS、native正式計数light0/dark0)。

## Closing Issues

Closes #943

## Related Issues / Links

- Parent SPEC: Issue #582。前提Issue #948はPR #951 / PR #952で配送済み。
- PM訂正裁定: 038c1b04-bf9c-443a-b660-39094f9b3699。

## Checklist

- [x] テスト先行commit0e0545a8、source/static commit c4a14642、通常push済み。
- [x] 自動全品質と関連headedのraw実行成功。
- [ ] canonical Agent Visual Check: 外部reporter互換修復が必要。
- [x] 親spec/plan/tasks追記済み、既存記録を維持。
- [x] Migration/backfillはn/a（schema/data変更なし）。
- [x] CHANGELOG影響はfrontendバグ修正、破壊的変更なし。

## Screenshots

実headed画像はtest-results/gwt-attachments/attachment-8.png（375px form/light）と
attachment-17.png（375px Models右端/dark）。全画像・14traceをartifact archiveへ保全。
汎用results.jsonは全体E2E用であり、headed件数には使用しない。

## Risk / Impact

累計counterと保持期限付き履歴は異なる母集団。空履歴から原因を推測しない。
Rollbackはfrontend sourceとstaticのcommitをrevert。data移行不要。
.codex/hooks.jsonとevidenceはPM裁定どおりコミット対象外。

## Notes

製品sourceと正式検証対象はc4a14642a547b16ac779545a73f6335b9605833d。
その後、Workイベント31件のみのchore(work): commit 0b0f9d9cを通常hooksでpushした。
現在HEAD/remoteは0b0f9d9c3ed6a9e79d0e3cb78c8e37ea93acc5bfで一致し、製品treeはc4a14642と同一。
修復canonical版でfresh full verifyが通るまでReady/merge/Issue closeは行わない。

execution.blockedによる終端は成功。build.abortはWork Doneの状態矛盾で拒否されたため、
active build flagを保持しPMへ引き継ぐ。Board escalationはd98e8e2f-9100-4b12-8cb4-24712d76f1e6。
