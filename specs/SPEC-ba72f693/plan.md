# 実装計画: ダッシュボードメトリクスの永続化と復元

**機能ID**: `SPEC-ba72f693` | **日付**: 2026-02-24 | **仕様**: [spec.md](spec.md)
**入力**: `/specs/SPEC-ba72f693/spec.md` の機能仕様

## 概要

ダッシュボードが表示する5つのメトリクス値（リクエストカウンタ、レイテンシ、
TPS、リクエスト履歴タイムライン、平均レスポンス時間）について、
インメモリキャッシュとDB間の同期不足および起動時復元ロジック欠如を修正する。

修正対象は5つの独立したバグであり、それぞれ最小限の変更で対応する。
詳細な技術調査は [research.md](research.md) を参照。

## 技術コンテキスト

**言語/バージョン**: Rust (Edition 2021)
**主要依存関係**: axum, sqlx, tokio, serde, chrono, tracing, uuid
**ストレージ**: SQLite (sqlx経由)
**テスト**: cargo test (unit + integration)
**対象プラットフォーム**: macOS (Apple Silicon), Linux
**プロジェクトタイプ**: single (Rustバイナリ + 埋め込みSPA)
**パフォーマンス目標**: ダッシュボードAPIポーリング5秒以内にメトリクス反映
**制約**: 既存API契約の維持、グレースフルデグラデーション必須
**スケール/スコープ**: 5つの独立バグ修正、新規モジュール追加なし

## 憲章チェック

| 原則 | 準拠状況 | 備考 |
|------|---------|------|
| I. Router-Nodeアーキテクチャ | 準拠 | Router内部のキャッシュ/DB同期のみ |
| III. テストファースト | 準拠 | 各バグに対しRED→GREEN→REFACTORで進行 |
| V. シンプルさ | 準拠 | 既存パターンの拡張のみ、新抽象化なし |
| VI. LLM最適化 | 準拠 | APIレスポンス形式変更なし |
| VII. 可観測性 | 準拠 | seed処理にinfo/warnログ追加 |

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-ba72f693/
├── plan.md              # このファイル
├── research.md          # 技術調査
├── data-model.md        # データモデル
├── quickstart.md        # 検証手順
└── tasks.md             # タスク分解 (/speckit.tasks で生成)
```

### ソースコード (変更対象ファイル)

```text
llmlb/src/
├── api/
│   ├── dashboard.rs     # Bug 2: Avg Response Timeフォールバック
│   ├── openai.rs        # Bug 1: 呼び出し元の引数変更
│   ├── proxy.rs         # Bug 1: record_endpoint_request_stats改修
│   └── responses.rs     # Bug 1: 呼び出し元の引数変更
├── balancer/
│   └── mod.rs           # Bug 4,5: seed_tps/history_from_db
├── bootstrap.rs         # Bug 4,5: 起動時seed呼び出し
├── db/
│   ├── endpoint_daily_stats.rs  # Bug 4: get_today_stats_allクエリ
│   ├── endpoints.rs     # Bug 3: update_endpoint_status SQL修正
│   └── request_history.rs       # Bug 5: get_recent_history_by_minuteクエリ
└── registry/
    └── endpoints.rs     # Bug 1,3: キャッシュ同期メソッド
```

**構造決定**: 既存モジュール構成を維持。新規ファイル・モジュール追加なし。
各バグは既存ファイル内の修正・メソッド追加のみで対応する。

## 修正方針

### Bug 1: リクエストカウンタのリアルタイム同期 (FR-001)

- `EndpointRegistry`に`increment_request_counters`メソッド追加
- DB更新成功後にキャッシュの`Endpoint`カウンタもインクリメント
- `record_endpoint_request_stats`の引数を`SqlitePool`から`EndpointRegistry`に変更
- 呼び出し元(openai.rs/responses.rs)の引数を更新

### Bug 2: Avg Response Timeのフォールバック (FR-003)

- `collect_stats`内で`average_response_time_ms`が`None`の場合
- オンラインエンドポイントの`latency_ms`から加重平均を算出
- 全エンドポイントがオフラインの場合は`None`維持

### Bug 3: オフラインLatency保持 (FR-002)

- DB: `latency_ms = ?` → `latency_ms = COALESCE(?, latency_ms)` に変更
- キャッシュ: `Some(v)`の場合のみ上書き、`None`の場合は既存値保持

### Bug 4: TPS起動時復元 (FR-004)

- `endpoint_daily_stats`から当日データを取得するクエリ追加
- `LoadManager::seed_tps_from_db`で取得データからTPS EMAを初期計算
- `bootstrap.rs`で起動時にseed呼び出し

### Bug 5: リクエスト履歴起動時復元 (FR-005)

- `request_history`から直近60分のデータを分単位集計するクエリ追加
- `LoadManager::seed_history_from_db`で取得データをVecDequeに投入
- `bootstrap.rs`で起動時にseed呼び出し

### 共通: グレースフルデグラデーション (FR-006)

- 全seed処理は`match`で`Err`をcatchし`warn!`ログ出力のみ
- 復元失敗時はゼロ/未計測状態で正常起動

## 実装順序

1. **Bug 3** (Latency保持) — SQL 1行 + Rust条件分岐1箇所
2. **Bug 1** (Request カウンタ同期) — メソッド追加 + 呼び出し元更新
3. **Bug 2** (Avg Response Time) — dashboard.rsのフォールバック追加
4. **Bug 5** (History seeding) — クエリ + seedメソッド + bootstrap
5. **Bug 4** (TPS seeding) — クエリ + seedメソッド + bootstrap

## 複雑さトラッキング

> 憲章違反なし。全修正は既存パターンの拡張であり、新たな抽象化は不要。
