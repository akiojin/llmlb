# Plans

## 2025-12-28

### 今後の対応（作業開始前に必ず更新）

- 作業開始前に本日の対応方針を更新し、完了した項目は随時反映する
- CLAUDE.md に PLANS.md 更新の徹底手順を追記する
- ✅ SS.png の有無を確認し、見つからないため削除不要であることを報告する
- ✅ bun.lock が .gitignore 済みであることを確認する
- 未完了SPECの残タスクを棚卸しし、P1の優先度と進行順を整理する
- ✅ SPEC-e03a404c: Phase 3.3 の型定義（T006/T007）を実装し tasks.md を更新する
- 手動検証が必要なP1タスク（SPEC-82491000 T023 / SPEC-d4eb8796 T102）について必要なAPIキー/環境を整理する
- 変更が入った場合はローカル検証を全て実行し、コミット＆プッシュする
- SPEC-05098000 は tasks.md が全完了のため、`specs/specs.md` 側の完了反映と整合チェックのみ行う
- P1の SPEC-26006000 に集中（残タスク: T053 quickstart 手動実行）
  - ✅ T055: `node/` clang-tidy 実行と修正完了（fmt consteval回避フラグ付き）
  - T053: quickstart.md を手動実行（router/node起動、TOKEN、音声ファイル準備）
  - T053: GPU/実行環境が不足する場合は阻害要因を明記し、代替案を整理する
  - T053: ASR入力のMP3/FLAC/OGGデコード対応を確認/必要なら実装
  - T053: VibeVoiceの出力フォーマット整合（response_formatと実出力の一致）を確認/調整
  - T053: quickstart.mdの手順と実装の齟齬があれば更新し、再検証
- 現在の変更内容を棚卸しし、`specs/SPEC-26006000/tasks.md` と `specs/specs.md` に完了反映する
- `SS.png` の削除と `bun.lock` の `.gitignore` 追加を確認する
- 既存変更をコミット前提でローカル検証を全て実行し、コミット＆プッシュする
- SPEC-799b8e2b の手動検証（Router/Node同時起動ログ）を実施する
- SPEC-e03a404c の Phase 3.3 着手として image_understanding capability を追加する
- `specs/specs.md` を未完了SPECの実装状況に合わせて更新する

### 未実装完了の進め方（目的共有）

- `specs/**/tasks.md` を起点に未完了タスクを棚卸しする
- 優先順位（影響度・依存関係・テスト可否）を決め、最優先SPECから着手する
- SPEC単位でTDD順守（RED→GREEN→REFACTOR）し、完了ごとにtasks.mdを更新する
- 変更が広範囲に及ぶ場合はREADME/ドキュメント/Specの整合性を都度確認する
- すべてのローカル検証を通してからコミット/プッシュする
