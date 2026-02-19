# タスク: LM Studioエンドポイントタイプの検出・分類・メタデータ取得

**入力**: `/specs/SPEC-af1ec86d/` の設計ドキュメント
**前提条件**: plan.md, spec.md, research.md, data-model.md

## フォーマット: `[ID] [P?] [Story] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)
- **[Story]**: このタスクが属するユーザーストーリー (US1, US2, US3, US4)

## Phase 1: 基盤（EndpointType拡張）

**目的**: LmStudioバリアントを追加し、全後続タスクの前提条件を満たす

### テスト（RED）

- [X] T001 [US1] `llmlb/src/types/endpoint.rs` のテストモジュールに `EndpointType::LmStudio` のユニットテストを追加。`as_str()` が `"lm_studio"` を返すこと、`from_str("lm_studio")` が `LmStudio` を返すこと、`supports_model_download()` が `false` を返すこと、`supports_model_metadata()` が `true` を返すこと、serde シリアライズ/デシリアライズが `"lm_studio"` 文字列と対応することを検証するテストを書く。テストが失敗することを確認（RED）

### 実装（GREEN）

- [X] T002 [US1] `llmlb/src/types/endpoint.rs` の `EndpointType` enumに `LmStudio` バリアントを追加。`/// LM Studioサーバー` のdocコメント付き。`as_str()` に `Self::LmStudio => "lm_studio"` を追加。`from_str()` に `"lm_studio" => Self::LmStudio` を追加。`supports_model_download()` は変更不要（falseがデフォルト）。`supports_model_metadata()` のmatchパターンに `Self::LmStudio` を追加。T001のテストが通ることを確認（GREEN）

**チェックポイント**: `cargo test` で `EndpointType::LmStudio` 関連テストがすべて通過

---

## Phase 2: ユーザーストーリー1 - LM Studioエンドポイントの自動識別 (優先度: P1)

**目標**: LM Studioサーバーを登録すると、自動的にlm_studioタイプとして検出される
**独立テスト**: LM StudioのURLを登録し、endpoint_typeが"lm_studio"で返ること

### テスト（RED）

- [X] T003 [P] [US1] `llmlb/src/detection/lm_studio.rs` を新規作成し、`detect_lm_studio()` 関数のユニットテストを追加。以下をテスト: (1) `/api/v1/models` がLM Studio形式のJSONを返す場合にSome(reason)を返す、(2) publisher/arch/stateフィールドがない場合にNoneを返す、(3) HTTPエラー時にNoneを返す。テスト用のJSONは research.md のレスポンス形式を使用。テストが失敗することを確認（RED）

- [X] T004 [P] [US1] `llmlb/src/detection/mod.rs` のテストモジュールに、検出順序テストを追加。LM Studioが xLLM→Ollama の後、vLLM の前に検出されることを確認するテスト。`DETECTION_TIMEOUT` が5秒以内であることの既存テストは変更不要

### 実装（GREEN）

- [X] T005 [US1] `llmlb/src/detection/lm_studio.rs` に `detect_lm_studio()` 関数を実装。シグネチャ: `pub async fn detect_lm_studio(client: &Client, base_url: &str, api_key: Option<&str>) -> Option<String>`。以下の複合判定ロジックを実装:
  1. Primary: `GET {base_url}/api/v1/models` にリクエスト。HTTP 200かつJSONに `publisher` or `arch` or `state` フィールドが存在すれば `Some("LM Studio: /api/v1/models returned LM Studio format")` を返す
  2. Fallback 1: `/v1/models` のServerヘッダーに "lm-studio" or "lm studio" (case-insensitive) が含まれれば `Some("LM Studio: Server header contains lm-studio ({header})")` を返す
  3. Fallback 2: `/v1/models` のdata配列内のowned_byに "lm-studio" (case-insensitive) が含まれれば `Some("LM Studio: owned_by field contains lm-studio")` を返す
  4. すべて失敗で `None` を返す。api_keyが指定されていればBearerヘッダーを付与。tracing debugログを各ステップに追加

- [X] T006 [US1] `llmlb/src/detection/mod.rs` を更新。(1) `mod lm_studio;` を追加、(2) `pub use lm_studio::detect_lm_studio;` を追加、(3) `detect_endpoint_type_with_client()` 内のPriority 2（Ollama）とPriority 3（vLLM）の間にLM Studio検出を挿入: `if let Some(reason) = detect_lm_studio(client, base_url, api_key).await { return EndpointTypeDetection::new(EndpointType::LmStudio, Some(reason)); }`、(4) docコメントの検出順序を更新: `/// 1. xLLM > 2. Ollama > 3. LM Studio > 4. vLLM > 5. OpenAI-compatible`。T003, T004のテストが通ることを確認（GREEN）

**チェックポイント**: `cargo test` でLM Studio検出テストがすべて通過。既存の検出テストに回帰なし

---

## Phase 3: ユーザーストーリー2 - LM Studioモデルの詳細メタデータ表示 (優先度: P1)

**目標**: LM Studioからモデルメタデータ（コンテキスト長、アーキテクチャ、量子化等）を取得し保存
**独立テスト**: モデル同期後にmax_context_lengthが保存されていること

### テスト（RED）

- [X] T007 [P] [US2] `llmlb/src/metadata/mod.rs` のテストモジュールに、ModelMetadata新フィールドのテストを追加。(1) `format`, `supports_vision`, `supports_tool_use`, `quantization_bits` がデフォルトでNoneであること、(2) 新フィールドを含むJSONのシリアライズ/デシリアライズ（`"format":"gguf"`, `"supports_vision":true`, `"supports_tool_use":false`, `"quantization_bits":4.5`）、(3) skip_serializing_if により Noneフィールドが出力されないこと。テストが失敗することを確認（RED）

- [X] T008 [P] [US2] `llmlb/src/metadata/lm_studio.rs` を新規作成し、`get_lm_studio_model_metadata()` のユニットテストを追加。LM Studioの `/api/v1/models/{model}` レスポンスJSON（research.md参照: `{"id":"test","type":"llm","publisher":"lmstudio-community","arch":"llama","compatibility_type":"gguf","quantization":"Q4_K_M","state":"loaded","max_context_length":131072}`）から ModelMetadata への正しいマッピングをテスト。context_length=131072, family="llama", quantization="Q4_K_M", format="gguf" であること。テストが失敗することを確認（RED）

### 実装（GREEN）

- [X] T009 [US2] `llmlb/src/metadata/mod.rs` の `ModelMetadata` structに4フィールドを追加。すべて `#[serde(skip_serializing_if = "Option::is_none")]` 付き:
  - `pub format: Option<String>` - モデルフォーマット（"gguf", "mlx"等）
  - `pub supports_vision: Option<bool>` - ビジョン対応
  - `pub supports_tool_use: Option<bool>` - ツール利用対応
  - `pub quantization_bits: Option<f32>` - 量子化ビット数
  T007のテストが通ることを確認（GREEN）

- [X] T010 [US2] `llmlb/src/metadata/lm_studio.rs` に `get_lm_studio_model_metadata()` を実装。シグネチャ: `pub async fn get_lm_studio_model_metadata(client: &Client, base_url: &str, api_key: Option<&str>, model: &str) -> Result<ModelMetadata, MetadataError>`。`GET {base_url}/api/v1/models/{model}` を呼び出し、レスポンスJSONからModelMetadataにマッピング。マッピング: `max_context_length`→`context_length`, `arch`→`family`, `quantization`(文字列の場合)→`quantization`, `compatibility_type`→`format`, `params_string`→`parameter_size`, `size_bytes`→`size_bytes`, `capabilities.vision`→`supports_vision`, `capabilities.trained_for_tool_use`→`supports_tool_use`, `quantization.bits_per_weight`→`quantization_bits`。認証はapi_keyをBearerトークンとして使用。T008のテストが通ることを確認（GREEN）

- [X] T011 [US2] `llmlb/src/metadata/mod.rs` を更新。(1) `pub mod lm_studio;` を追加、(2) `get_model_metadata()` のmatch式に `EndpointType::LmStudio => lm_studio::get_lm_studio_model_metadata(client, base_url, api_key, model).await` ブランチを追加。コンパイルが通ることを確認

- [X] T012 [US2] `llmlb/src/sync/mod.rs` のメタデータ取得条件を更新。既存の `if ep_type == EndpointType::Xllm || ep_type == EndpointType::Ollama` に `|| ep_type == EndpointType::LmStudio` を追加。これによりLM Studioエンドポイントのモデル同期時にmax_tokensが自動取得される

**チェックポイント**: `cargo test` でメタデータ関連テストがすべて通過。既存のシリアライズテストに回帰なし

---

## Phase 4: ユーザーストーリー3 - フィルタリングと手動指定 (優先度: P2)

**目標**: `?type=lm_studio` フィルタと手動タイプ指定が動作する
**独立テスト**: `GET /api/endpoints?type=lm_studio` でLM Studioのみが返ること

### テスト（RED）

- [X] T013 [US3] `llmlb/tests/contract/endpoints_type_filter_test.rs` にLM Studioフィルタテストを追加。既存のxLLM/Ollama/vLLM/OpenAI互換フィルタテストのパターンに従い、(1) `?type=lm_studio` でLM Studioエンドポイントのみが返ること、(2) エンドポイント登録時に `endpoint_type: "lm_studio"` を手動指定できること、(3) 手動指定時の `endpoint_type_source` が `"manual"` であること、(4) レスポンスの `endpoint_type` フィールドに `"lm_studio"` が含まれることを検証するテストを追加。テストが失敗することを確認（RED）

### 実装（GREEN）

- [X] T014 [US3] Phase 1のEndpointType拡張（T002）により、serde/FromStr実装を通じてフィルタリングと手動指定は自動的に動作する。T013のテストが通ることを確認（GREEN）。もし追加の変更が必要な箇所があれば対応する（`llmlb/src/api/endpoints.rs` のドキュメントコメント更新等）

**チェックポイント**: `cargo test` でフィルタリングテストがすべて通過

---

## Phase 5: ユーザーストーリー4 - 他タイプとの誤検出防止 (優先度: P2)

**目標**: LM Studio検出が他タイプに影響しないことを保証
**独立テスト**: 各タイプのエンドポイントが正しく検出されること

### テスト（RED）

- [X] T015 [P] [US4] `llmlb/src/detection/lm_studio.rs` のテストモジュールに誤検出防止テストを追加。(1) vLLMのレスポンス（Serverヘッダー: "vLLM/0.4.0"、owned_by: "vllm"）でLM Studio検出がNoneを返すこと、(2) 標準的なOpenAI互換レスポンス（dataフィールドのみ、publisher/arch/stateなし）でNoneを返すこと、(3) Ollamaの `/api/tags` レスポンスでNoneを返すこと。テストが失敗することを確認（RED）

### 実装（GREEN）

- [X] T016 [US4] T005で実装した `detect_lm_studio()` の判定ロジックがすでに正しく動作していることを確認。LM Studio固有フィールド（publisher/arch/state）の存在チェックにより、他タイプのレスポンスでは検出されない設計。T015のテストが通ることを確認（GREEN）。必要に応じてフォールバック判定のowned_byチェックで "lm-studio" の完全一致条件を厳密化

**チェックポイント**: `cargo test` で全既存テストに回帰がないことを確認

---

## Phase 6: 仕上げ＆横断的関心事

**目的**: 品質チェック、ドキュメント整合性、最終検証

- [X] T017 [P] `specs/SPEC-af1ec86d/spec.md` のステータスを「下書き」から「実装中」に更新
- [X] T018 [P] `cargo fmt --check` でフォーマット確認。問題があれば `cargo fmt` で修正
- [X] T019 [P] `cargo clippy -- -D warnings` で警告がないことを確認。問題があれば修正
- [X] T020 `cargo test` で全テストが通過することを確認（timeout: 600000ms）
- [X] T021 `pnpm dlx markdownlint-cli2 "**/*.md" "!node_modules" "!.git" "!.github" "!.worktrees"` でmarkdownlintが通過することを確認
- [X] T022 `.specify/scripts/checks/check-tasks.sh` でタスクチェックが通過することを確認
- [X] T023 `make quality-checks` で全品質チェックが通過することを確認（timeout: 900000ms）

**チェックポイント**: 全品質チェック通過。コミット＆プッシュ可能な状態

---

## 依存関係＆実行順序

### フェーズ依存関係

- **Phase 1（基盤）**: 依存なし - 最初に実行
- **Phase 2（US1: 検出）**: Phase 1 完了に依存
- **Phase 3（US2: メタデータ）**: Phase 1 完了に依存。Phase 2と並列可能
- **Phase 4（US3: フィルタ）**: Phase 1 完了に依存。Phase 2/3と並列可能
- **Phase 5（US4: 誤検出防止）**: Phase 2 完了に依存（検出ロジックのテスト）
- **Phase 6（仕上げ）**: Phase 1-5 すべて完了に依存

### 並列実行可能なタスク

```text
Phase 1完了後:
├── T003, T004 (US1テスト) ← 並列
├── T007, T008 (US2テスト) ← 並列
└── T013 (US3テスト) ← Phase 2/3と並列可能

Phase 2完了後:
├── T015 (US4テスト) ← Phase 3/4と並列可能
```

### TDDサイクル

各Phaseは RED→GREEN→REFACTOR の順序を厳守:

1. テストタスク（RED）を先に実施し、失敗を確認
2. 実装タスク（GREEN）でテストを通す
3. 次のPhaseに進む前にリファクタリング
