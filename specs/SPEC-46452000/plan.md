# 実装計画: LM Studioエンドポイントタイプの検出・分類・メタデータ取得

**機能ID**: `SPEC-46452000` | **日付**: 2026-02-13 | **仕様**: [spec.md](spec.md)
**入力**: `/specs/SPEC-46452000/spec.md` の機能仕様

## 概要

llmlbのエンドポイントタイプ自動検出にLM Studioを追加する。
LM Studio固有のREST API（`/api/v1/models`）を利用した複合判定で確実に識別し、
豊富なモデルメタデータを取得する。既存の検出アーキテクチャ（xLLM/Ollama/vLLM）
のパターンに準拠し、最小限の変更で統合する。

## 技術コンテキスト

**言語/バージョン**: Rust (edition 2021)
**主要依存関係**: reqwest (HTTP), serde/serde_json (JSON), sqlx (DB), tracing (logging)
**ストレージ**: SQLite (endpoint_models.max_tokens)
**テスト**: cargo test (contract/integration/unit)
**対象プラットフォーム**: macOS, Linux
**プロジェクトタイプ**: single (llmlb/)
**制約**: 検出タイムアウト 5秒以内、既存テスト回帰なし

## 憲章チェック

| 原則 | 適合状況 |
|---|---|
| I. Router-Nodeアーキテクチャ | 適合: Router側の検出・メタデータ取得のみ |
| II. HTTP/REST通信 | 適合: HTTP/JSON経由のAPI通信 |
| III. テストファースト | 適合: TDD RED→GREEN→REFACTORサイクル厳守 |
| IV. GPU必須 | N/A: 検出機能はGPU要件に影響しない |
| V. シンプルさ | 適合: 既存パターンに準拠、最小限の新規コード |
| VI. LLM最適化 | 適合: メタデータは既存構造を拡張 |
| VII. 可観測性 | 適合: tracing debug/infoログ追加 |
| VIII. 認証 | 適合: 既存のapi_keyパススルーを使用 |

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-46452000/
├── spec.md              # 機能仕様
├── plan.md              # このファイル
├── research.md          # API調査結果
├── data-model.md        # データモデル変更
├── quickstart.md        # 開発クイックスタート
└── tasks.md             # タスク分解（/speckit.tasksで生成）
```

### ソースコード変更

```text
llmlb/src/
├── types/
│   └── endpoint.rs          # [変更] EndpointType enum + supports_* メソッド
├── detection/
│   ├── mod.rs               # [変更] 検出順序にLM Studio追加
│   └── lm_studio.rs         # [新規] LM Studio検出ロジック
├── metadata/
│   ├── mod.rs               # [変更] ModelMetadata拡張 + ルーティング
│   └── lm_studio.rs         # [新規] メタデータ取得ロジック
├── sync/
│   └── mod.rs               # [変更] メタデータ取得対象追加
└── api/
    └── endpoints.rs         # [変更] EndpointResponse反映（既存パターン踏襲）

llmlb/tests/
├── contract/
│   └── endpoints_type_filter_test.rs  # [変更] LM Studioフィルタテスト追加
└── unit/
    └── detection_lm_studio_test.rs    # [新規] 検出ユニットテスト
```

## 実装フェーズ

### Phase 1: EndpointType拡張（基盤）

**対象**: `llmlb/src/types/endpoint.rs`

1. `EndpointType` enumに `LmStudio` バリアント追加
2. `as_str()` に `"lm_studio"` 追加
3. `from_str()` に `"lm_studio"` パターン追加
4. `supports_model_download()` → `false`
5. `supports_model_metadata()` に `Self::LmStudio` 追加 → `true`
6. `Display` traitの自動対応確認

### Phase 2: LM Studio検出実装

**対象**: `llmlb/src/detection/lm_studio.rs`（新規）, `llmlb/src/detection/mod.rs`

検出関数 `detect_lm_studio()` を実装:

1. **Primary判定**: `GET {base_url}/api/v1/models`
   - HTTP 200かつレスポンスJSONに `publisher` または `arch` または `state` フィールドが存在
   - 理由: `"LM Studio: /api/v1/models returned LM Studio format (publisher={publisher})"`
2. **Fallback 1**: Serverヘッダーチェック
   - `/v1/models` レスポンスのServerヘッダーに "lm-studio" or "lm studio" (case-insensitive)
   - 理由: `"LM Studio: Server header contains lm-studio ({header})"`
3. **Fallback 2**: owned_byフィールド
   - `/v1/models` レスポンスのdata配列のowned_byに "lm-studio" (case-insensitive)
   - 理由: `"LM Studio: owned_by field contains lm-studio"`

検出順序更新（`mod.rs`）:
xLLM → Ollama → **LM Studio** → vLLM → OpenAI互換 → Unknown

### Phase 3: ModelMetadata拡張

**対象**: `llmlb/src/metadata/mod.rs`

ModelMetadata structに追加:

- `format: Option<String>` - モデルフォーマット
- `supports_vision: Option<bool>` - ビジョン対応
- `supports_tool_use: Option<bool>` - ツール利用対応
- `quantization_bits: Option<f32>` - 量子化ビット数

全フィールドは `#[serde(skip_serializing_if = "Option::is_none")]` 付き。
既存テストへの影響: Default traitで自動的にNone初期化されるため回帰なし。

### Phase 4: LM Studioメタデータ取得

**対象**: `llmlb/src/metadata/lm_studio.rs`（新規）

`get_lm_studio_model_metadata()` 関数:

- `GET {base_url}/api/v1/models/{model}` を呼び出し
- レスポンスからModelMetadataへマッピング
- 認証: 既存のapi_keyをBearerトークンとして使用

ルーティング更新（`mod.rs`の`get_model_metadata()`）:

- `EndpointType::LmStudio` ブランチ追加

### Phase 5: モデル同期統合

**対象**: `llmlb/src/sync/mod.rs`

メタデータ取得条件の拡張:

- 既存: `ep_type == EndpointType::Xllm || ep_type == EndpointType::Ollama`
- 変更: `ep_type == EndpointType::Xllm || ep_type == EndpointType::Ollama || ep_type == EndpointType::LmStudio`

### Phase 6: APIレスポンス・フィルタリング

**対象**: `llmlb/src/api/endpoints.rs`, テストファイル

- `endpoint_type: "lm_studio"` がレスポンスに含まれることを確認
- `?type=lm_studio` フィルタの動作確認
- 手動指定 `endpoint_type: "lm_studio"` の受け入れ確認

（既存のserde/fromstr実装により、大部分は自動的に動作する）

## 複雑さトラッキング

> 憲章違反なし。既存パターンに完全準拠。

## テスト戦略

### ユニットテスト

- `EndpointType::LmStudio` の各メソッド（as_str, from_str, supports_*）
- 検出ロジックのヘッダー/フィールドマッチング
- ModelMetadata新フィールドのシリアライズ/デシリアライズ
- メタデータマッピングのフィールド変換

### コントラクトテスト

- LM Studioタイプフィルタリング（既存テストファイル拡張）
- エンドポイント登録時の手動タイプ指定
- レスポンスにendpoint_type: "lm_studio"が含まれること

### 統合テスト

- 検出フロー全体（mock server利用）
- メタデータ取得→モデル同期フロー
- ヘルスチェック時の再検出

### 実機検証

- LM Studio 0.4.0+での自動検出確認
- メタデータ取得の実データ検証
