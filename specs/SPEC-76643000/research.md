# 技術リサーチ: エンドポイント単位リクエスト統計

**機能ID**: `SPEC-76643000` | **日付**: 2026-02-10

## 既存実装の分析

### リクエスト処理フロー

リクエストは以下の流れで処理される:

1. `api/openai.rs` でリクエスト受信
2. `balancer/mod.rs` の `begin_request()` でアクティブリクエスト数を増加
3. エンドポイントにリクエストを転送
4. `finish_request()` または `finish_request_with_tokens()` で結果を記録
5. `save_request_record()` で request_history テーブルに保存

**カウンタ更新の挿入ポイント**: `finish_request()`/`finish_request_with_tokens()`
の末尾が最適。ここでは既にendpoint_idとRequestOutcome（Success/Error）が
確定している。

### 既存のDBマイグレーション

最新: `013_remove_responses_api_support.sql`
→ 次のマイグレーション番号: **014**

### 既存の定期タスクパターン

`start_cleanup_task()` (request_history.rs:886-922) のパターン:

- `tokio::spawn` で非同期タスクを起動
- `tokio::time::interval` で周期実行
- 環境変数で間隔を設定可能
- Arc経由でストレージを共有

日次バッチもこのパターンに従う。

### SQLiteでのアトミックカウンタ更新

SQLiteではWALモード（本プロジェクトで使用）でも書き込みは単一ライターに
シリアライズされるため、`UPDATE endpoints SET total_requests = total_requests + 1`
はアトミックに動作する。明示的なロック不要。

### Rechartsの利用状況

`recharts@^3.7.0` がpackage.jsonに記載済みだが、現在ダッシュボードでは
未使用。TokenStatsSectionはテーブル表示のみ。
今回のチャート実装が初のRecharts活用となる。

## 技術的な判断

### カウンタ更新のタイミング

**選択**: tokio::spawnで非同期実行

**理由**: カウンタのDB書き込みがリクエストのレスポンスレイテンシに
影響を与えないようにするため。request_historyの保存も同様のパターンで
非同期実行されている。

### 日次バッチの実装方法

**選択**: tokio::spawnで0:00にトリガー

**方法**: サーバーのローカル時間で次の0:00までの Duration を計算し、
`tokio::time::sleep()` で待機後、`tokio::time::interval(Duration::from_secs(86400))`
で24時間周期実行。

### FOREIGN KEY制約の不使用

**選択**: endpoint_daily_statsにFOREIGN KEY制約を付けない

**理由**: エンドポイント削除時に日次集計データを保持する要件のため。
FK制約があるとCASCADE DELETEまたは削除ブロックが発生する。
