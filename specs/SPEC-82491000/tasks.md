# タスク: クラウドプロバイダーモデル一覧統合

**機能ID**: `SPEC-82491000` | **日付**: 2025-12-25
**入力**: `/specs/SPEC-82491000/`の設計ドキュメント
**前提条件**: plan.md, research.md, data-model.md, quickstart.md

## フォーマット: `[ID] [P?] 説明`

- **[P]**: 並列実行可能（異なるファイル、依存関係なし）
- 説明には正確なファイルパスを含める

## Phase 3.1: セットアップ

- [ ] T001 `router/src/api/cloud_models.rs` に空のモジュールスケルトンを作成
  - CloudModelInfo, CloudModelsCache 構造体の定義
  - CLOUD_MODELS_CACHE_TTL_SECS, CLOUD_MODELS_FETCH_TIMEOUT_SECS 定数
- [ ] T002 `router/src/api/mod.rs` に `pub mod cloud_models;` を追加

## Phase 3.2: テストファースト (TDD) - 3.3の前に完了必須

**重要: これらのテストは記述され、実装前に失敗する必要がある**

### ユニットテスト（パース処理）

- [ ] T003 [P] `router/src/api/cloud_models.rs` に OpenAI レスポンスパーステストを追加
  - `#[cfg(test)] mod tests` 内に `test_parse_openai_models()`
  - JSON文字列からCloudModelInfoへの変換を検証
- [ ] T004 [P] `router/src/api/cloud_models.rs` に Google レスポンスパーステストを追加
  - `test_parse_google_models()`
  - `models/` プレフィックス除去を検証
- [ ] T005 [P] `router/src/api/cloud_models.rs` に Anthropic レスポンスパーステストを追加
  - `test_parse_anthropic_models()`
  - ISO 8601日時変換を検証

### 統合テスト（API呼び出し）

- [ ] T006 [P] `router/src/api/cloud_models.rs` に fetch_openai_models テストを追加
  - wiremock でモックサーバー設定
  - 正常系: モデル一覧取得成功
  - 異常系: APIキー未設定時はスキップ
- [ ] T007 [P] `router/src/api/cloud_models.rs` に fetch_google_models テストを追加
  - wiremock でモックサーバー設定
  - 正常系/異常系
- [ ] T008 [P] `router/src/api/cloud_models.rs` に fetch_anthropic_models テストを追加
  - wiremock でモックサーバー設定
  - 正常系/異常系

### キャッシュテスト

- [ ] T009 `router/src/api/cloud_models.rs` にキャッシュ動作テストを追加
  - `test_cache_hit()`: TTL内はキャッシュから返却
  - `test_cache_miss()`: TTL超過時は再取得
  - `test_cache_fallback()`: API失敗時は古いキャッシュを返却

### list_models 統合テスト

- [ ] T010 `router/src/api/openai.rs` のテストモジュールに list_models 拡張テストを追加
  - クラウドモデルがレスポンスに含まれることを検証
  - ローカルモデルとクラウドモデルのマージを検証

## Phase 3.3: コア実装（テストが失敗した後のみ）

### データ型定義

- [ ] T011 `router/src/api/cloud_models.rs` に CloudModelInfo 構造体を実装
  - Serialize, Deserialize, Clone, Debug derive
  - id, object, created, owned_by フィールド

- [ ] T012 `router/src/api/cloud_models.rs` に CloudModelsCache 構造体を実装
  - models: Vec&lt;CloudModelInfo&gt;
  - fetched_at: DateTime&lt;Utc&gt;
  - is_valid() メソッド（TTLチェック）

- [ ] T013 `router/src/api/cloud_models.rs` にプロバイダー固有レスポンス型を追加
  - OpenAIModelsResponse, OpenAIModel
  - GoogleModelsResponse, GoogleModel
  - AnthropicModelsResponse, AnthropicModel

### フェッチ関数

- [ ] T014 [P] `router/src/api/cloud_models.rs` に fetch_openai_models() を実装
  - OPENAI_API_KEY 環境変数チェック
  - `GET https://api.openai.com/v1/models`
  - Authorization: Bearer ヘッダー
  - 10秒タイムアウト
  - エラー時は空Vec返却 + warn!ログ

- [ ] T015 [P] `router/src/api/cloud_models.rs` に fetch_google_models() を実装
  - GOOGLE_API_KEY 環境変数チェック
  - `GET https://generativelanguage.googleapis.com/v1beta/models?key=`
  - models/ プレフィックス除去
  - 10秒タイムアウト

- [ ] T016 [P] `router/src/api/cloud_models.rs` に fetch_anthropic_models() を実装
  - ANTHROPIC_API_KEY 環境変数チェック
  - `GET https://api.anthropic.com/v1/models`
  - x-api-key, anthropic-version ヘッダー
  - ISO 8601 → Unix タイムスタンプ変換

### 統合関数

- [ ] T017 `router/src/api/cloud_models.rs` に fetch_all_cloud_models() を実装
  - futures::join_all で3プロバイダー並列呼び出し
  - 結果をマージして Vec&lt;CloudModelInfo&gt; 返却

### キャッシュ管理

- [ ] T018 `router/src/api/cloud_models.rs` にグローバルキャッシュを実装
  - static CLOUD_MODELS_CACHE: OnceCell&lt;RwLock&lt;Option&lt;CloudModelsCache&gt;&gt;&gt;
  - get_or_refresh_cache() 関数
  - フォールバック: API失敗時は古いキャッシュ返却

## Phase 3.4: 統合

- [ ] T019 `router/src/api/openai.rs` の list_models() を拡張
  - cloud_models::get_cached_models() を呼び出し
  - クラウドモデルを data 配列に追加
  - 既存のローカルモデル処理は変更なし

- [ ] T020 `router/src/api/cloud_models.rs` に get_cached_models() 公開関数を追加
  - キャッシュ有効時はキャッシュ返却
  - キャッシュ無効時は fetch_all_cloud_models() + キャッシュ更新

## Phase 3.5: 仕上げ

- [ ] T021 全テスト実行・合格確認
  - `cargo test --package llm-router`
  - すべてのテストが成功することを確認

- [ ] T022 品質チェック実行
  - `cargo fmt --check`
  - `cargo clippy -- -D warnings`

- [ ] T023 [P] quickstart.md のシナリオを手動検証
  - 全プロバイダー設定時のレスポンス確認
  - 一部プロバイダーのみ設定時の動作確認
  - キャッシュ動作確認（2回目リクエストの高速化）

- [ ] T024 コミット作成
  - `feat(api): /v1/modelsでクラウドプロバイダーモデル一覧を統合`
  - 変更ファイル: cloud_models.rs (新規), mod.rs, openai.rs

## 依存関係

```text
T001 → T002 → T003-T010 (並列) → T011-T018 (テスト合格後) → T019-T020 → T021-T024
```

- T001, T002: セットアップ（順次）
- T003-T010: テスト作成（並列可能、異なるテスト関数）
- T011-T018: 実装（テスト失敗後に実行）
- T019-T020: 統合（実装完了後）
- T021-T024: 仕上げ（統合完了後）

## 並列実行例

```text
# T003-T008 を並列実行（異なるテスト関数、同一ファイルだが独立）:
Task: "cloud_models.rs に OpenAI パーステスト追加"
Task: "cloud_models.rs に Google パーステスト追加"
Task: "cloud_models.rs に Anthropic パーステスト追加"
Task: "cloud_models.rs に fetch_openai_models テスト追加"
Task: "cloud_models.rs に fetch_google_models テスト追加"
Task: "cloud_models.rs に fetch_anthropic_models テスト追加"

# T014-T016 を並列実行（独立した関数実装）:
Task: "cloud_models.rs に fetch_openai_models() 実装"
Task: "cloud_models.rs に fetch_google_models() 実装"
Task: "cloud_models.rs に fetch_anthropic_models() 実装"
```

## 検証チェックリスト

- [x] すべてのエンティティ (CloudModelInfo, CloudModelsCache) にタスクがある
- [x] すべてのプロバイダー (OpenAI, Google, Anthropic) に fetch 関数タスクがある
- [x] すべてのテストが実装より先にある (TDD順序)
- [x] 並列タスクは本当に独立している
- [x] 各タスクは正確なファイルパスを指定
- [x] 同じファイルを変更する [P] タスクは関数レベルで独立

## 注意事項

- [P] タスク = 異なるファイルまたは独立した関数
- 実装前にテストが失敗することを確認（RED フェーズ）
- 各フェーズ完了後にコミット推奨
- wiremock を使用して外部 API をモック（実 API は使用しない）

---

*Phase 3 タスク生成完了 - 合計 24 タスク*
