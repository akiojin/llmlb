# タスク: Playground Chat マルチモーダル対応

**入力**: `/specs/SPEC-5fc9fe92/` の設計ドキュメント  
**前提条件**: plan.md, research.md, data-model.md, contracts/, quickstart.md

## 実行フロー

```
✓ 1. plan.mdから技術スタック抽出完了
✓ 2. 設計ドキュメント読み込み完了
✓ 3. カテゴリ別タスク生成完了
✓ 4. TDD順序適用完了
✓ 5. 並列実行マーク完了
✓ 6. タスク検証完了
→ 7. 実装開始準備完了
```

## フォーマット: `[ID] [P?] 説明`

- **[P]**: 並列実行可能（異なるファイル、依存関係なし）
- 説明には正確なファイルパスを含める

## Phase 3.1: セットアップ

- [x] **T001** [P] `router/src/web/dashboard/src/lib/api.ts` にマルチモーダル用の型（content parts、添付、capabilities）を追加
- [x] **T002** [P] `router/src/web/dashboard/src/pages/Playground.tsx` にE2E用の安定セレクタ（idまたはdata-testid）を付与
- [x] **T003** [P] `router/src/web/dashboard/src/pages/Playground.tsx` に添付の制約（許可MIME、サイズ上限）を定数化して追加

## Phase 3.2: テストファースト (TDD) ⚠️ 3.3の前に完了必須

**重要: これらのテストは記述され、実装前に失敗する必要がある（RED）**

- [x] **T004** [P] `router/tests/e2e-playwright/specs/playground/playground-multimodal-input.spec.ts` に画像添付（ファイル選択）UIのE2Eテスト（REDを確認）
- [x] **T005** [P] `router/tests/e2e-playwright/specs/playground/playground-multimodal-input.spec.ts` に音声添付（ファイル選択）UIのE2Eテスト（REDを確認）
- [x] **T006** [P] `router/tests/e2e-playwright/specs/playground/playground-multimodal-input.spec.ts` に画像/音声の貼り付け（ペースト）入力のE2Eテスト（REDを確認）
- [x] **T007** [P] `router/tests/e2e-playwright/specs/playground/playground-multimodal-output.spec.ts` にアシスタント本文中の画像URL/データURLをプレビュー表示できるE2Eテスト（REDを確認）
- [x] **T008** [P] `router/tests/e2e-playwright/specs/playground/playground-multimodal-output.spec.ts` にアシスタント本文中の音声URL/データURLを再生UI表示できるE2Eテスト（REDを確認）
- [x] **T009** [P] `router/tests/contract/openai_request_sanitization_spec.rs` にリクエスト履歴保存時の添付データ伏せ字化の契約テスト（REDを確認）

## Phase 3.3: コア実装 (テストが失敗した後のみ)

### 入力（画像/音声の添付・貼り付け）

- [x] **T010** `router/src/web/dashboard/src/pages/Playground.tsx` に画像の添付（ファイル選択）UI、プレビュー、削除を実装
- [x] **T011** `router/src/web/dashboard/src/pages/Playground.tsx` に画像の貼り付け（ペースト）入力を実装
- [x] **T012** `router/src/web/dashboard/src/pages/Playground.tsx` に音声の添付（ファイル選択）UI、プレビュー（プレイヤー）、削除を実装
- [x] **T013** `router/src/web/dashboard/src/pages/Playground.tsx` に音声の貼り付け（ペースト）入力を実装

### 送信payload（OpenAI互換 / content parts）

- [x] **T014** `router/src/web/dashboard/src/pages/Playground.tsx` に送信payload生成（テキストのみ/マルチモーダル）の共通関数を実装
- [x] **T015** `router/src/web/dashboard/src/pages/Playground.tsx` で「テキストが空でも添付があれば送信可能」にする（送信ボタン活性条件の更新）
- [x] **T016** `router/src/web/dashboard/src/pages/Playground.tsx` で送信後に添付を確実にクリアし、セッション永続化（localStorage）に生データを残さない

### 表示（生成AIからの画像/音声）

- [x] **T017** `router/src/web/dashboard/src/pages/Playground.tsx` にアシスタント本文から画像URL/データURLを抽出してプレビュー表示するレンダリングを実装
- [x] **T018** `router/src/web/dashboard/src/pages/Playground.tsx` にアシスタント本文から音声URL/データURLを抽出してプレイヤー表示するレンダリングを実装
- [x] **T019** `router/src/web/dashboard/src/pages/Playground.tsx` に表示失敗時のフォールバック（リンク表示、エラー表示）を実装

### 監査/履歴（添付データの肥大化回避）

- [x] **T020** `router/src/api/openai.rs` にリクエスト履歴保存前のサニタイズ処理（添付のbase64/データURLを要約に置換）を実装
- [x] **T021** `router/src/api/openai.rs` でストリーミング/非ストリーミング双方でサニタイズが適用されるよう統一

## Phase 3.4: 統合

- [x] **T022** `router/src/web/dashboard/src/pages/Playground.tsx` のcURL生成がマルチモーダル送信（content parts）を反映するよう更新
- [x] **T023** `router/src/web/dashboard` で `pnpm build` を実行し、成果物が `router/src/web/static/` に出力されることを確認（差分をコミット対象に含める）

## Phase 3.5: 仕上げ

- [x] **T024** `specs/SPEC-5fc9fe92/spec.md` と実装の差分がないことを確認（要件逸脱があれば仕様更新）
- [x] **T025** 品質チェックをローカルで全て成功させる（`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `.specify/scripts/checks/check-tests.sh`, `pnpm dlx markdownlint-cli2 "**/*.md" ...`）

## 依存関係

- T004-T009（RED確認）→ T010-T023（実装）
- T020/T021（サニタイズ）→ T025（品質チェック）
