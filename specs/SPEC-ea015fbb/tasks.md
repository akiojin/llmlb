# タスク: Web UI 画面一覧

**入力**: `/specs/SPEC-ea015fbb/`の設計ドキュメント
**前提条件**: plan.md (必須)
**タイプ**: ドキュメント専用SPEC（コード実装なし）

## フォーマット: `[ID] [P?] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)

## Phase 3.1: 検証

- [x] T001 spec.mdの画面一覧が実装ファイルと一致することを確認
  - ✅ login.html, register.html, index.html, playground.html 存在確認済み
- [x] T002 [P] 各画面IDと関連SPECのリンクが正しいことを確認
  - ✅ 全関連SPEC（d4eb8796, 712c20cf, fbc50d97, 5fc9fe92, 11106000, 1f2a9c3d, 026b2cde, 4b6e9f2a）存在確認済み
- [x] T003 [P] 画面遷移図が実装のルーティングと一致することを確認
  - ✅ /dashboard, /playground ルーティング (api/mod.rs:209-215) 確認済み

## Phase 3.2: 整合性チェック

- [x] T004 specs.mdにSPEC-ea015fbbが登録されていることを確認
  - ✅ specs.md:106 に登録済み
- [x] T005 依存関係マトリクスにSPEC-ea015fbbが含まれていることを確認
  - ✅ specs.md:26 に追加済み

## 依存関係

- T001が完了後にT002, T003を並列実行可能
- T004, T005は独立して実行可能

## 並列実行例

```text
# T002-T003 を一緒に起動:
Task: "各画面IDと関連SPECのリンクが正しいことを確認"
Task: "画面遷移図が実装のルーティングと一致することを確認"

# T004-T005 を一緒に起動:
Task: "specs.mdにSPEC-ea015fbbが登録されていることを確認"
Task: "依存関係マトリクスにSPEC-ea015fbbが含まれていることを確認"
```

## 検証コマンド

```bash
# T001: 画面ファイルの存在確認
ls router/src/web/static/*.html

# T004: specs.mdの登録確認
grep "SPEC-ea015fbb" specs/specs.md

# T005: 依存関係マトリクスの確認
grep "SPEC-ea015fbb" specs/specs.md | grep -A1 "依存関係"
```

## 検証チェックリスト

- [x] spec.mdに全画面が定義されている
- [x] plan.mdが作成されている
- [x] specs.mdに登録されている
- [x] 依存関係マトリクスに含まれている
- [x] 実装との整合性が確認されている
