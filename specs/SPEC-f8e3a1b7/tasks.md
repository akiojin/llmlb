# SPEC-f8e3a1b7: タスク一覧

## Phase 1: Node→Endpoint移行 + SQLite移行（最優先）

### Setup

- [x] T001: SQLiteマイグレーションスクリプト作成
  - `endpoints`テーブル定義追加
  - `latency_ms`、`device_info`カラム追加

### Core

- [x] T002: Endpoint型拡張
  - `latency_ms: Option<f64>`フィールド追加
  - `device_info: Option<DeviceInfo>`フィールド追加
  - `DeviceInfo`、`DeviceType`型定義

- [x] T003: EndpointRepository実装
  - CRUD操作をSQLiteベースに変更
  - 既存のJSONベース実装を置換

- [x] T004: JSON→SQLite自動マイグレーション
  - 既にSQLiteベース（対応不要）

- [x] T005: Node型削除
  - `llmlb/src/common/types.rs`からNode型削除
  - 関連する`#[deprecated]`コード削除

- [x] T006: NodeRegistry削除
  - `llmlb/src/balancer/`からNodeRegistry関連コード削除
  - EndpointRegistryに完全移行

- [x] T007: endpoint_typeフィールド削除
  - `CreateEndpointRequest`から削除
  - 関連するマッチング・フィルタリング削除

### Test

- [x] T008: Node関連テスト→EndpointRegistry書き換え（16テスト）
  - 空テスト16個を削除・コメント化
  - EndpointRegistry APIに移行完了

## Phase 2: unwrap()除去

### Core

- [x] T009: LbError OpenAI互換拡張
  - `error_type()`メソッド追加
  - `error_code()`メソッド追加
  - `OpenAIError`レスポンス型定義

- [x] T010: models.rs unwrap()除去
  - Line 1094: `iter().max().unwrap()` → Result
  - Line 1271-1538: JSON操作のunwrap() → Result
  - HF_INFO_CACHE RwLock poisoning対応

- [x] T011: auth.rs unwrap()除去
  - テストコード内のみ（本番コードは対応済み）

- [x] T012: proxy.rs エラーハンドリング改善
  - Line 205: `unwrap_or_default()` → 適切なエラーログ

- [x] T013: audio.rs/images.rs IPアドレス解析改善
  - 内部unwrap()をIpAddr定数に置換

- [x] T014: その他ファイルのunwrap()除去
  - 全ファイルスキャンで残存確認完了

### Test

- [x] T015: エラーレスポンス形式テスト追加
  - OpenAI互換形式の検証テスト
  - 各エラータイプのレスポンス確認

## Phase 3: レイテンシベース負荷分散

### Core

- [x] T016: Endpoint::update_latency()実装
  - EMA (α=0.2) 計算
  - None→Some変換処理

- [x] T017: Endpoint::reset_latency()実装
  - オフライン時にf64::INFINITY設定

- [x] T018: 推論リクエスト時のレイテンシ計測
  - リクエスト開始時刻記録
  - 成功時にupdate_latency()呼び出し

- [x] T019: EndpointRegistry::find_by_model_sorted_by_latency()更新
  - latency_ms考慮のソート
  - 同一レイテンシ時のラウンドロビン

- [x] T020: レイテンシSQLite永続化
  - 更新時にDBに保存
  - 起動時にDBから復元

### Test

- [x] T021: レイテンシ計算テスト
  - EMA計算の正確性
  - オフライン時リセット
  - タイブレークラウンドロビン

## Phase 4: /v0/system API対応

### Core

- [x] T022: /v0/system呼び出しロジック
  - エンドポイント登録時に試行
  - タイムアウト設定（5秒）
  - エラー時は無視して続行

- [x] T023: DeviceInfo取得・保存
  - レスポンスパース
  - Endpoint.device_infoに保存

### Test

- [x] T024: /v0/system統合テスト
  - xLLMエンドポイントでの取得確認
  - 非対応エンドポイントでの無視確認
  - CPU専用エンドポイントのデバイス情報取得
  - コミット: 549f2b8a

## Phase 5: ダッシュボードUI更新

### UI

- [x] T025: 「GPU」→「デバイス」リネーム
  - エンドポイント一覧: GPUカラム廃止済み（Latencyのみ）
  - エンドポイント詳細: 対応済み

- [x] T026: エンドポイント詳細にレイテンシ表示
  - 平均レイテンシ（ms）表示実装済み
  - EndpointTable.tsx, EndpointDetailModal.tsx

- [x] T027: 登録フォーム更新
  - デバイスタイプ選択: 元々存在しない
  - endpoint_type選択: 元々存在しない

## Phase 6: Visionテスト環境

### Setup

- [x] T028: LLaVA-1.5-7B-Q4_K_M取得
  - HuggingFaceからダウンロード
  - GitHub Actions Cacheで永続化（初回のみダウンロード）
  - CI環境への配置
  - `.github/workflows/vision-tests.yml` で実装

- [x] T029: 100x100テスト画像作成
  - PNG形式のシンプルな画像
  - Base64エンコード済み定数
  - `llmlb/tests/support/images.rs` で実装

### Test

- [ ] T030: Visionテスト有効化（17テスト）
  - `test_chat_completions_with_image_url`
  - `test_chat_completions_with_base64_image`
  - `test_supported_image_formats`
  - `test_vision_streaming_response`
  - `test_image_size_limit_exceeded`
  - `test_image_count_limit_exceeded`
  - その他11テスト
  - Note: CIワークフロー実行で検証予定

## Phase 7: ドキュメント更新

### Docs

- [x] T031: CLAUDE.md更新
  - 「GPU非搭載エンドポイント登録禁止」削除
  - 「CPU推論許容」追記
  - 「レイテンシ優先負荷分散」追記

- [x] T032: docs/architecture.md更新
  - 負荷分散戦略セクション更新
  - エンドポイントタイプ廃止記載

- [x] T033: README.md/README.ja.md更新
  - 対応エンドポイント説明
  - CPU推論サポート記載

## 依存関係

```
T001 → T002 → T003 → T004 → T005 → T006 → T007 → T008
                                           ↓
T009 → T010 → T011 → T012 → T013 → T014 → T015
                                           ↓
T016 → T017 → T018 → T019 → T020 → T021
                                    ↓
T022 → T023 → T024
         ↓
T025 → T026 → T027
         ↓
T028 → T029 → T030
         ↓
T031 → T032 → T033
```

## 並列実行可能タスク [P]

- [P] T009-T014（Phase 2）はPhase 1完了後に並列実行可能
- [P] T016-T020（Phase 3）はPhase 2と並列実行可能
- [P] T025-T027（Phase 5）はPhase 3-4と並列実行可能
- [P] T031-T033（Phase 7）は各フェーズ完了に合わせて順次更新可能
