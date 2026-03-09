---
name: gwt-issue-spec-ops
description: GitHub Issue-first の仕様管理（SPEC）。要件定義・仕様作成・仕様策定・仕様設計・TDD設計・計画作成・タスク生成・品質チェックリスト生成を GitHub Issue (gwt-spec ラベル) 上で実行する。「仕様を書いて」「specを作成」「TDD」「要件定義」「plan/tasks を生成」「仕様の曖昧さ解消」「整合性分析」と言われたときに使用。
---

# gwt Issue SPEC Ops

GitHub Issue が仕様の Single Source of Truth。すべての仕様はラベル `gwt-spec` 付き
の Issue として管理する。

## Conventions

### SPEC ID

SPEC ID = **Issue 番号**。旧形式 `SPEC-[a-f0-9]{8}` は使わない。

### Label

`gwt-spec` ラベルが付いた Issue = Spec Issue。

### Issue body sections

Issue body は以下のセクション構造に従う:

```markdown
<!-- GWT_SPEC_ID:#{number} -->

## Spec

(背景、ユーザーシナリオ、要件、成功基準)

## Plan

(実装計画)

## Tasks

(タスクリスト)

## TDD

(テスト設計)

## Research

(技術調査)

## Data Model

(データモデル)

## Quickstart

(最小実装手順)

## Contracts

Artifact files are managed in issue comments with `contract:<name>` entries.

## Checklists

Artifact files are managed in issue comments with `checklist:<name>` entries.

## Acceptance Checklist

- [ ] (受け入れチェック項目)
```

### Artifact comments

Contract/Checklist はコメントとして管理。先頭行にマーカーを付ける:

```markdown
<!-- GWT_SPEC_ARTIFACT:contract:openapi.md -->
contract:openapi.md

(content)
```

## Operations (gh CLI)

### Create new spec issue

```bash
gh issue create --label gwt-spec --title "feat: ..." --body "$(cat <<'EOF'
<!-- GWT_SPEC_ID:#NEW -->

## Spec

_TODO_

## Plan

_TODO_

## Tasks

_TODO_

## TDD

_TODO_

## Research

_TODO_

## Data Model

_TODO_

## Quickstart

_TODO_

## Contracts

Artifact files are managed in issue comments with `contract:<name>` entries.

## Checklists

Artifact files are managed in issue comments with `checklist:<name>` entries.

## Acceptance Checklist

- [ ] Add acceptance checklist
EOF
)"
```

作成後、Issue 番号で `GWT_SPEC_ID` マーカーを更新:

```bash
gh issue edit {number} --body "$(updated body with <!-- GWT_SPEC_ID:#{number} -->)"
```

### Read spec issue

```bash
gh issue view {number} --json body,title,labels
```

### Update section

```bash
gh issue edit {number} --body "$(updated body)"
```

### Add artifact comment

```bash
gh issue comment {number} --body "$(cat <<'EOF'
<!-- GWT_SPEC_ARTIFACT:contract:openapi.md -->
contract:openapi.md

(content)
EOF
)"
```

### Sync to project

```bash
gh project item-add {project-number} --owner {owner} --url {issue-url}
gh project item-edit --project-id {project-id} --id {item-id} --field-id {field-id} --single-select-option-id {option-id}
```

### List spec issues

```bash
gh issue list --label gwt-spec --state open --json number,title
gh issue list --label gwt-spec --state all --json number,title
```

## Workflow guide

### 1. Specify (仕様作成)

仕様作成の手順:

1. Issue body のセクション構造に従い、`## Spec` セクションを作成
2. **必須要素**:
   - **背景**: なぜこの機能/修正が必要か
   - **ユーザーシナリオ**: 具体的な操作フローと期待結果。優先度 P0/P1/P2
   - **機能要件**: `FR-001` 形式で番号付け
   - **非機能要件**: `NFR-001` 形式（パフォーマンス、セキュリティ等）
   - **成功基準**: `SC-001` 形式。測定可能な完了条件
3. 不明確な箇所は `【要確認】` マーカーを付けて仮置き
4. エッジケースとエラーハンドリングを明記

### 2. Clarify (曖昧さ解消)

`## Spec` に不明確な点がある場合:

1. 影響度の高い順に最大 **5問** の質問を作成
2. 質問対象:
   - スコープの境界が不明確な箇所
   - 受け入れ基準がテスト不能な箇所
   - 非機能要件の具体的な閾値
   - 他機能との依存関係
3. `【要確認】` マーカーを質問結果で置換
4. 質問と回答を Spec セクションに反映

### 3. Plan (計画作成)

`## Plan` セクションに実装計画を記載:

1. **技術コンテキスト**: 影響するファイル・モジュール一覧
2. **実装アプローチ**: 選択したアーキテクチャとその理由
3. **フェーズ分割**: 段階的な実装計画

追加セクション生成:

- `## Research`: 技術調査結果（ライブラリ選定、API 調査等）
- `## Data Model`: スキーマ、型定義の設計
- `## Quickstart`: 最小動作に必要な手順
- `## Contracts`: API コントラクト（アーティファクトコメントで管理）

### 4. Tasks (タスク生成)

`## Tasks` セクションにタスクリストを記載:

1. **フェーズ構成**: Setup → Foundation → User Stories → Finalization
2. **タスク書式**: `- [ ] T001 [Phase] [US1] description`
   - `T001`: 連番
   - `[Phase]`: `[S]`etup, `[F]`oundation, `[U]`ser story, `[FIN]`alization
   - `[USn]`: 関連ユーザーシナリオ番号
3. **依存関係**: タスク間の依存を明記（blocked-by）
4. **完了時**: チェックボックスを `[x]` に更新

### 5. Analyze (整合性分析)

Spec → Plan → Tasks のカバレッジを検証:

1. すべての FR/NFR が Tasks にカバーされているか
2. すべてのユーザーシナリオがタスクにマッピングされているか
3. 循環依存がないか
4. テスト可能性が担保されているか
5. 問題があれば修正を提案

### 6. Implement (実装実行)

タスクの実行手順:

1. `## Tasks` からチェックされていない最優先タスクを選択
2. **TDD**: テストを先に書く → RED 確認 → 実装 → GREEN 確認
3. 独立したタスクは並列実行可能
4. 完了したタスクのチェックボックスを `[x]` に更新
5. Issue body を更新して進捗を反映

### 7. Tasks to child issues

大きなタスクを子 Issue に分割する場合:

1. 依存順に子 Issue を作成
2. 親 Issue から子 Issue へのリンクを `## Tasks` に追加
3. 子 Issue にも `gwt-spec` ラベルを付ける（仕様を含む場合）

### 8. Quality checklists

品質チェックリストの生成:

- **requirements**: 要件の完全性・一貫性
- **security**: セキュリティ考慮事項（OWASP Top 10 等）
- **ux**: ユーザビリティ・アクセシビリティ
- **api**: API 設計の整合性
- **testing**: テスト戦略の網羅性

チェックリストはアーティファクトコメントとして Issue に追加:

```bash
gh issue comment {number} --body "$(cat <<'EOF'
<!-- GWT_SPEC_ARTIFACT:checklist:requirements.md -->
checklist:requirements.md

- [ ] CHK001 All FR covered by tests
- [ ] CHK002 All NFR have measurable thresholds
...
EOF
)"
```

## Integration with normal issues

### Branch creation

```bash
gh issue develop {number}
```

### PR link

コミットメッセージまたは PR body に `Fixes #{number}` を含めて自動リンク。

### Project phase transition

Phase フィールドでライフサイクルを管理:

| Phase | 意味 |
|---|---|
| Draft | 仕様作成中 |
| Ready | 仕様完了、レビュー待ち |
| Planned | 計画策定完了 |
| Ready for Dev | 実装開始可能 |
| In Progress | 実装中 |
| Done | 完了 |
| Blocked | ブロック中 |

## Requirements

- `gh` must be installed and authenticated.
- Repository must have `gwt-spec` label created.
- Agent CWD must be inside the target repository (enforced by gwt worktree hooks).
- `$GWT_PROJECT_ROOT` environment variable is available for explicit repo resolution.
