# SPEC-5045f436: タスク一覧

## 凡例

- `[ ]` 未着手
- `[x]` 完了
- `[P]` 並列実行可能

## Setup

- [x] `[P]` S-1: tiktoken-rs依存をrouter/Cargo.tomlに追加
- [x] `[P]` S-2: マイグレーションファイル 004_add_token_statistics.sql 作成

## Test（RED）

### データモデル

- [x] T-1: RequestResponseRecordのトークンフィールドシリアライズテスト
- [x] T-2: NodeLoadStateトークン累積テスト

### トークン抽出

- [x] `[P]` T-3: usageフィールドからのトークン抽出テスト
- [x] `[P]` T-4: tiktoken推定テスト
- [x] `[P]` T-5: usageフィールド欠如時のフォールバックテスト

### 永続化

- [x] T-6: request_historyテーブルへのトークン保存テスト
- [x] T-7: トークン集計クエリテスト（累計/日次/月次）

### API

- [x] `[P]` T-8: DashboardNodeトークンフィールド応答テスト
- [x] `[P]` T-9: DashboardStatsトークンフィールド応答テスト
- [x] `[P]` T-10: /api/dashboard/stats/tokens エンドポイントテスト

### ストリーミング

- [x] T-11: SSEチャンクごとのトークン累積テスト
- [x] T-12: ストリーミング完了時の最終集計テスト

### エラーケース

- [ ] `[P]` T-13: エラー応答時のトークンカウントテスト
- [ ] `[P]` T-14: オフラインノードの統計保持テスト

## Core（GREEN）

### データモデル実装

- [x] C-1: common/src/protocol.rs - RequestResponseRecordにトークンフィールド追加
  - 依存: T-1
- [x] C-2: router/src/balancer/mod.rs - NodeLoadStateにトークンフィールド追加
  - 依存: T-2

### トークン抽出実装

- [x] C-3: トークン抽出モジュール作成（router/src/token/mod.rs）
  - 依存: T-3, T-4, T-5
- [x] C-4: usageフィールド抽出ロジック実装
  - 依存: C-3
- [x] C-5: tiktoken推定ロジック実装
  - 依存: C-3, S-1

### finish_request統合

- [x] C-6: finish_request()にトークン集計呼び出し追加
  - 依存: C-2, C-3
- [x] C-7: NodeLoadStateトークン累積ロジック実装
  - 依存: C-6

### 永続化実装

- [x] C-8: マイグレーション適用（SQLiteスキーマ変更）
  - 依存: S-2
- [x] C-9: insert_record()でトークン値バインド追加
  - 依存: C-1, C-8, T-6
- [x] C-10: トークン集計クエリ実装
  - 依存: C-8, T-7

### ストリーミング対応

- [x] C-11: SSEチャンク処理でのトークン累積実装
  - 依存: C-3, T-11, T-12

## Integration

### API実装

- [x] I-1: DashboardNodeにトークン統計フィールド追加
  - 依存: C-7, C-10, T-8
- [x] I-2: DashboardStatsにトークン統計フィールド追加
  - 依存: C-10, T-9
- [x] I-3: /api/dashboard/stats/tokens エンドポイント実装
  - 依存: C-10, T-10
- [x] `[P]` I-4: /api/dashboard/stats/tokens/daily エンドポイント実装
  - 依存: I-3
- [x] `[P]` I-5: /api/dashboard/stats/tokens/monthly エンドポイント実装
  - 依存: I-3

### エラーケース対応

- [ ] I-6: エラー応答時のトークンカウント実装
  - 依存: C-3, T-13
- [ ] I-7: オフラインノードの統計保持確認
  - 依存: C-7, T-14

## Polish

### ダッシュボードUI

- [ ] P-1: ノード一覧にトークン統計サマリ表示
  - 依存: I-1
- [ ] P-2: 専用統計ページ作成
  - 依存: I-3, I-4, I-5

### ドキュメント

- [ ] `[P]` P-3: API仕様書更新
- [ ] `[P]` P-4: DEVELOPMENT.md更新（トークン統計機能説明追加）

### 品質保証

- [ ] P-5: 統合テスト実行・全パス確認
- [ ] P-6: E2Eテスト実行・全パス確認
- [ ] P-7: markdownlint / cargo fmt / cargo clippy 実行

## 依存関係図

```text
S-1, S-2 (Setup)
    ↓
T-1〜T-14 (Test RED)
    ↓
C-1〜C-11 (Core GREEN)
    ↓
I-1〜I-7 (Integration)
    ↓
P-1〜P-7 (Polish)
```

## 進捗サマリ

| カテゴリ | 完了 | 合計 | 進捗率 |
|----------|------|------|--------|
| Setup | 2 | 2 | 100% |
| Test | 12 | 14 | 86% |
| Core | 11 | 11 | 100% |
| Integration | 5 | 7 | 71% |
| Polish | 0 | 7 | 0% |
| **合計** | **30** | **41** | **73%** |
