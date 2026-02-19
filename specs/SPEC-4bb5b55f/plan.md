# 実装計画: エンドポイント×モデル単位TPS可視化

**機能ID**: `SPEC-4bb5b55f` | **日付**: 2026-02-19 | **仕様**: [spec.md](spec.md)
**入力**: `specs/SPEC-4bb5b55f/spec.md` の機能仕様

## 概要

ロードバランサーがプロキシする推論リクエストの応答から、
エンドポイント×モデルの粒度でTPS（output tokens per second）を
EMA（α=0.2）でリアルタイム計測し、インメモリ状態管理・DB永続化・
REST API・WebSocket・ダッシュボードUIで確認できるようにする。

既存のトークン追跡（エンドポイント単位）と日次統計（リクエスト数のみ）を
モデル粒度に拡張し、TPS計算ロジックを追加する。

## 技術コンテキスト

**言語/バージョン**: Rust (Edition 2021) + TypeScript/React (ダッシュボード)
**主要依存関係**: axum, sqlx, tokio, serde, tiktoken-rs, React, Vite
**ストレージ**: SQLite (sqlx::SqlitePool)
**テスト**: cargo test (unit + integration)
**対象プラットフォーム**: Linux/macOS サーバー
**プロジェクトタイプ**: web (Rustバックエンド + Reactフロントエンド)
**パフォーマンス目標**: TPS計算がリクエスト完了のクリティカルパスに影響しない
**制約**: EMA更新はO(1)、メモリはエンドポイント×モデル数に比例
**スケール/スコープ**: エンドポイント数十台 × モデル数十種

## 憲章チェック

| ゲート | 合否 | 根拠 |
|--------|------|------|
| III. テストファースト | PASS | TDDサイクル厳守。テスト→実装の順序でコミット |
| V. シンプルさ | PASS | 既存パターン（EMA、fire-and-forget DB更新）を再利用 |
| VI. LLM最適化 | PASS | APIレスポンスは小さいJSON、ページング不要な規模 |
| VII. 可観測性 | PASS | tracing使用、構造化ログ出力 |

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-4bb5b55f/
├── spec.md              # 機能仕様書（完了）
├── plan.md              # このファイル
├── research.md          # Phase 0: 技術リサーチ（完了）
├── data-model.md        # Phase 1: データモデル設計（完了）
├── quickstart.md        # Phase 1: クイックスタート（完了）
└── tasks.md             # Phase 2: タスク分解（次ステップ）
```

### ソースコード (変更対象)

```text
llmlb/
├── migrations/
│   └── 016_add_tps_columns.sql          # 新規: DBマイグレーション
├── src/
│   ├── balancer/
│   │   └── mod.rs                        # 改修: TpsTracker, EMA計算
│   ├── db/
│   │   └── endpoint_daily_stats.rs       # 改修: upsert拡張
│   ├── api/
│   │   ├── proxy.rs                      # 改修: stats関数拡張
│   │   ├── openai.rs                     # 改修: TPS情報渡し
│   │   ├── dashboard.rs                  # 改修: model-tps API
│   │   └── mod.rs                        # 改修: ルート追加
│   ├── events/
│   │   └── mod.rs                        # 改修: TpsUpdatedイベント
│   ├── types/
│   │   └── endpoint.rs                   # 改修: タイプ判定ヘルパー
│   └── web/dashboard/src/
│       └── pages/Dashboard.tsx           # 改修: TPSテーブルUI
```

## 実装フェーズ

### Phase 1: データ基盤（DB + インメモリ）

1. **マイグレーション**: `endpoint_daily_stats` に
   `total_output_tokens`、`total_duration_ms` カラム追加
2. **DB関数拡張**: `upsert_daily_stats()` にトークン数・処理時間の
   引数を追加し、UPSERTで累積
3. **TpsTracker**: `LoadManager` に `HashMap<(Uuid, String), ModelTpsState>`
   を追加し、EMA（α=0.2）でTPS値を管理
4. **EndpointType判定**: TPS計測対象タイプのフィルタヘルパー追加

### Phase 2: データフロー（計測→蓄積）

1. **record_endpoint_request_stats拡張**: output_tokens、duration_ms
   の引数を追加
2. **openai.rs呼び出し更新**: 非ストリーミング・ストリーミング両方の
   完了パスでトークン情報と処理時間をstats関数に渡す
3. **TpsTracker更新**: リクエスト完了時にEMAを計算・更新

### Phase 3: API公開

1. **REST API**: `GET /api/endpoints/{id}/model-tps` ハンドラー実装
2. **overview拡張**: `GET /api/dashboard/overview` レスポンスに
   TPS概要情報を含める
3. **WebSocket**: `DashboardEvent::TpsUpdated` イベント追加、
   リクエスト完了時にevent_busへ発行

### Phase 4: ダッシュボードUI

1. **モデルTPSテーブル**: エンドポイント詳細パネルにテーブル追加
2. **表示フォーマット**: 小数点1位 + "tok/s"、未計測は "—"
3. **WebSocket連携**: TpsUpdatedイベントでリアルタイム更新

## 設計判断

### TPS計算の注入ポイント

`record_endpoint_request_stats()` (`proxy.rs`) を拡張する。
この関数は既にfire-and-forgetパターンで `tokio::spawn` 内から呼ばれており、
endpoint_id・model_id・success が利用可能。ここに output_tokens と
duration_ms を追加するのが最小変更かつクリティカルパス非阻害。

### インメモリTPS状態の管理場所

`LoadManager` 内に新しいフィールドとして `TpsTracker` を追加する。
既存の `EndpointLoadState` はエンドポイント単位だが、TPS状態は
エンドポイント×モデル単位のため、別のHashMapで管理する。
`RwLock<HashMap<(Uuid, String), ModelTpsState>>` で保護する。

### DB永続化の方式

既存の `upsert_daily_stats()` のシグネチャを拡張し、
`total_output_tokens` と `total_duration_ms` を累積加算する。
新テーブルではなくALTER TABLEを使い、既存のPK
`(endpoint_id, model_id, date)` をそのまま活用する。

### OpenAI互換タイプの除外

`EndpointType` に `is_tps_trackable()` メソッドを追加し、
TPS計測対象かどうかを判定する。`record_endpoint_request_stats()` 内で
チェックし、`OpenaiCompatible` の場合はTPS更新をスキップする。

## 複雑さトラッキング

> 憲章違反なし。全ての設計は既存パターンを踏襲。
