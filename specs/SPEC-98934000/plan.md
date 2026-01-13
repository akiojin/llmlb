# 実装計画: ノード登録承認フロー

**機能ID**: `SPEC-98934000` | **日付**: 2026-01-09 | **仕様**: [spec.md](./spec.md)
**入力**: `/specs/SPEC-98934000/spec.md`の機能仕様

## 概要

ノードがルーターに登録リクエストを送信（Push型）し、管理者が手動で承認/拒否する機能。
承認前のノードには推論リクエストがルーティングされない。

**実装状況**: バックエンドはほぼ完全実装済み。ダッシュボードUIのみ追加が必要。

## 技術コンテキスト

**言語/バージョン**: Rust 1.75+ (Router), TypeScript 5.x (Dashboard)
**主要依存関係**: Axum, React, shadcn/ui
**ストレージ**: SQLite (nodes テーブル)
**テスト**: cargo test, vitest
**対象プラットフォーム**: Linux/macOS サーバー
**プロジェクトタイプ**: web (backend + frontend)

## 憲章チェック

**シンプルさ**: ✅ 合格
- 既存アーキテクチャに沿った最小限の変更
- 新規プロジェクト追加なし

**テスト (妥協不可)**: ✅ 計画済み
- RED-GREEN-Refactorサイクル厳守
- Contract → Integration → Unit の順序

**可観測性**: ✅ 既存実装で対応済み
- 構造化ロギング（spdlog/tracing）実装済み

## 既存実装状況

| コンポーネント | 実装状況 | ファイル |
|---------------|---------|---------|
| NodeStatus enum | ✅ 完了 | `common/src/types.rs` |
| approve_node API | ✅ 完了 | `router/src/api/nodes.rs:319` |
| delete_node API | ✅ 完了 | `router/src/api/nodes.rs:356` |
| 状態遷移ロジック | ✅ 完了 | `router/src/registry/mod.rs:504` |
| ルーティング除外 | ✅ 完了 | `router/src/balancer/mod.rs` |
| UI - 承認ボタン | ❌ 未実装 | `NodeTable.tsx` |
| UI - 拒否ボタン | ❌ 未実装 | `NodeTable.tsx` |
| テスト - 承認フロー | ⚠️ 要強化 | `router/src/api/nodes.rs` |

## Phase 0: リサーチ（完了）

既存実装の調査により、以下が判明:

1. **バックエンドAPI**: 完全実装済み
   - `POST /v0/nodes/:id/approve` - 承認API（Admin権限必須）
   - `DELETE /v0/nodes/:id` - 削除API（拒否として利用可）

2. **状態遷移**: 完全実装済み
   - Pending → Registering → Online フロー実装済み
   - Offline復帰時の状態維持実装済み

3. **UIコンポーネント**: 部分実装
   - `nodesApi.approve()`, `nodesApi.delete()` - API呼び出し実装済み
   - ボタンUI未実装

**出力**: research.md は不要（既存実装調査で完了）

## Phase 1: 設計＆契約

### データモデル（既存）

```
Node {
  id: UUID
  status: NodeStatus (Pending | Registering | Online | Offline)
  gpu_devices: GpuDevice[]
  ip_address: String
  port: u16
  registered_at: DateTime
  online_since: Option<DateTime>
  ...
}
```

### API契約（既存）

| メソッド | エンドポイント | 権限 | 説明 |
|---------|---------------|------|------|
| POST | /v0/nodes | - | ノード登録（Pending状態で開始） |
| POST | /v0/nodes/:id/approve | Admin | ノード承認 |
| DELETE | /v0/nodes/:id | Admin | ノード削除（拒否） |
| GET | /v0/nodes | - | ノード一覧取得 |

### UIコンポーネント設計（新規）

**NodeTable.tsx 変更点**:

1. **承認ボタン**: Pending状態のノードに表示
   - クリック → `nodesApi.approve(nodeId)` 呼び出し
   - 成功 → ノード一覧を再取得

2. **拒否ボタン**: Pending状態のノードに表示
   - クリック → 確認ダイアログ表示
   - 確認 → `nodesApi.delete(nodeId)` 呼び出し
   - 成功 → ノード一覧を再取得

3. **削除ボタン**: Online/Offline状態のノードに表示
   - 既存の削除機能を活用

**出力**: data-model.md は不要（既存データモデル利用）

## Phase 2: タスク計画アプローチ

### TDD順序（テストが実装より先）

1. **Contract Tests** (RED)
   - 承認APIのレスポンス形式テスト
   - 拒否（削除）APIのレスポンス形式テスト

2. **Integration Tests** (RED)
   - Pending → Online 状態遷移テスト
   - Pending → 削除 テスト
   - 非Admin権限での承認拒否テスト
   - Pendingノードへのルーティング除外テスト

3. **Unit Tests** (RED)
   - approve() メソッドの単体テスト
   - delete() メソッドの単体テスト

4. **実装** (GREEN)
   - テストを通すための最小実装
   - UIコンポーネント追加

5. **E2E Tests**
   - ダッシュボードからの承認/拒否操作テスト

### 並列実行可能タスク [P]

- [P] Contract Tests（独立）
- [P] Integration Tests（独立）
- [P] UI実装（バックエンド完了済み）

### 推定タスク数

- テスト関連: 10タスク
- UI実装: 5タスク
- ドキュメント: 2タスク
- **合計**: 約17タスク

## 複雑さトラッキング

なし - 既存アーキテクチャに沿った最小限の変更

## 進捗トラッキング

**フェーズステータス**:

- [x] Phase 0: Research完了
- [x] Phase 1: Design完了
- [x] Phase 2: Task planning完了（アプローチ記述）
- [ ] Phase 3: Tasks生成済み (/speckit.tasks)
- [ ] Phase 4: 実装完了
- [ ] Phase 5: 検証合格

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み（なし）

---

*憲章 v2.0.0 に基づく - `/memory/constitution.md` 参照*
