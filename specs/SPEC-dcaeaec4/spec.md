# SPEC-dcaeaec4: LLM-Router独自モデルストレージ

## 概要

llm-node がモデルファイルを `~/.llm-router/models/` 配下から読み込むことを基本としつつ、
**モデルキャッシュはNode主導**とする。ルーターは登録情報とファイル一覧（マニフェスト）を提示し、
Node が GPU 差分に応じて必要アーティファクトを選択・取得する。
LLM runtime固有のストレージ形式への暗黙フォールバックは撤廃する。

## 背景と動機

### 現状の問題

1. **LLM runtime依存**: 現在のLLM runtimeCompatクラスはLLM runtimeのストレージ形式に依存している
   - `~/.runtime/models/manifests/registry.runtime.ai/library/<name>/<tag>`
   - `~/.runtime/models/blobs/<sha256-digest>`
2. **複雑なパス解決**: LLM runtimeのmanifest→blob形式は本プロジェクトには過剰
3. **混乱**: ユーザーがモデルをどこに配置すべきか分かりにくい

### 解決策

シンプルな独自ディレクトリ構造を採用しつつ、ルーターは登録情報とマニフェストを提示する：

```text
~/.llm-router/models/
  <model-name>/
    model.gguf または model.safetensors.*
```

## 要件

### 機能要件

#### FR-1: モデルディレクトリ構造（ノードローカルキャッシュ）

- デフォルトのモデル保存先は `~/.llm-router/models/`
- 環境変数で上書き可能（推奨: `LLM_NODE_MODELS_DIR`、互換: `LLM_MODELS_DIR`）
- 各モデルは `<models_dir>/<model-name>/model.gguf` に配置

#### FR-2: モデル名の形式

- モデル名（モデルID）はファイル名ベース形式または階層形式を許可
  - ファイル名ベース形式: `gpt-oss-20b`
  - 階層形式: `openai/gpt-oss-20b`（HuggingFace互換）
- ディレクトリ名はモデルIDをそのまま使用（小文字に正規化）
  - `gpt-oss-20b` → `gpt-oss-20b/model.gguf`
  - `openai/gpt-oss-20b` → `openai/gpt-oss-20b/model.gguf`（ネストディレクトリ）
- 危険な文字（`..`, `\0`等）は禁止、`/`はディレクトリセパレータとして許可

#### FR-3: モデルアーティファクト解決（Node主導）

1. ルーターは登録済みモデルの**ファイル一覧（マニフェスト）**を提示する。
   - 例: `/v0/models/registry/:model_name/manifest.json`
2. Node は登録時に確定した形式と GPU バックエンド（Metal/DirectML）に応じて**必要アーティファクトを選択**する。
3. Node はローカル `~/.llm-router/models/<name>/` を確認し、必要アーティファクトが揃っていれば採用する。
4. 共有パスが設定済みでアクセス可能な場合、共有パスを直接参照する（コピーしない）。
5. 共有パスが使えない場合、Node は**許可リスト内の外部ソース（HuggingFace等）**から必要アーティファクトを取得し、ローカルに保存する。
   - ルーターは必要に応じてプロキシ（`/v0/models/registry/.../files/...` 等）として利用できるが、事前キャッシュは前提としない。
6. いずれも不可ならエラーを返す。LLM runtime固有形式への暗黙フォールバックは禁止。

※4の「直接使用」は共有ストレージ/NFS/S3を想定する。

#### FR-4: 利用可能モデル一覧

- `listAvailable()` は `models_dir` 配下の全ディレクトリを走査
- 各ディレクトリ内に `model.gguf` もしくは safetensors（index + shards）が存在するものをリスト

#### FR-5: メタデータ（オプション）

- `metadata.json` が存在する場合、モデル情報を読み込む
- 必須フィールドなし（存在しなくても動作する）

#### FR-6: ノード起動時同期

- ノードは起動時にルーターのモデル一覧（`/v1/models` または `/v0/models`）を取得
- 各モデルについて **マニフェストを参照**し、FR-3の解決フローに従って取得/参照
- ローカル → 共有パス → 外部ソース/プロキシの順で解決

#### FR-7: ルーターからの同期通知

- ルーターは新しいモデル登録時に、オンライン状態の全ノードに同期通知を送信
- 同期通知はノードの `/api/models/sync` エンドポイントをPOST（モデル名のみ含む）
- ノードは通知受信後、ルーターの `/v1/models` からモデル一覧を取得し、FR-3のモデル解決フローに従って同期
- **重要**: これはPull型の同期。ルーターはモデルファイルをプッシュせず、通知のみ送信。ノード側が自発的に取得する
- リトライポリシー: 無限リトライ、指数バックオフ（1s, 2s, 4s, ... 最大60s）

#### FR-8: API設計

外部クライアント向けと Node 同期向けで用途を分ける：

- `GET /v1/models` - 外部クライアント向け一覧（OpenAI互換 + ダッシュボード拡張フィールド）
- `POST /v1/models/register` - モデル登録（HuggingFace URL等）
- `DELETE /v1/models/:model_name` - モデル削除
- `POST /v1/models/discover-gguf` - GGUF検索
- `GET /v0/models` - Node 同期向けメタデータ（format / repo / filename など）
- `GET /v0/models/registry/:model_name/manifest.json` - Node 向けファイル一覧
- `GET /v0/models/registry/:model_name/files/:file` - ルーター経由のファイル取得（プロキシとして任意利用）

**廃止済みエンドポイント**:
- `/api/models/registered` - **廃止**（`/v1/models`に統合）

#### FR-9: 全ノード全モデル対応の原則

- すべてのノードは、ルーターに登録された全対応モデルに対応する
- 個別ノードへのモデル割り当て機能は本仕様のスコープ外
- ノードは起動時および同期通知受信時に、ルーターの全対応モデルを取得する

### 非機能要件

#### NFR-1: 後方互換性

- 既存のテストは引き続きパスする（テストはモック/一時ディレクトリを使用）

#### NFR-2: シンプルさと安全性

- LLM runtimeのmanifest/blob形式のサポートは削除（他アプリの資産に依存しない）
- 参照パスは `~/.llm-router/models` と共有パス（設定時）のみ
- ノードのダウンロード先は **許可リスト内の外部ソース** または **ルーターのプロキシ** に限定する

## ディレクトリ構造の例

```text
~/.llm-router/
├── config.json          # 設定ファイル
├── router.db            # ルーターDB（SQLite）
└── models/
    ├── gpt-oss-20b/
    │   ├── config.json
    │   ├── tokenizer.json
    │   ├── model.safetensors.index.json
    │   ├── model-00001-of-0000X.safetensors
    │   └── model.metal.bin  # (optional, Metal最適化)
    ├── gpt-oss-7b/
    │   └── model.gguf
    └── qwen3-coder-30b/
        └── model.gguf
```

## 影響範囲

### 変更対象ファイル

1. `node/src/models/runtime_compat.cpp` → `model_storage.cpp` にリネーム
2. `node/include/models/runtime_compat.h` → `model_storage.h` にリネーム
3. `node/src/utils/config.cpp` - デフォルトパス変更
4. `node/src/utils/cli.cpp` - ヘルプメッセージ更新
5. `node/src/main.cpp` - クラス名変更に対応

### 削除される機能

- LLM runtimeのmanifest/blob解析ロジック
- `registry.runtime.ai` パス構造のサポート

## Clarifications

### Session 2025-12-24

仕様を精査した結果、重大な曖昧さは検出されませんでした。

**確認済み事項**:

- デフォルトパス: ~/.llm-router/models/（FR-1で明記）
- 環境変数: LLM_NODE_MODELS_DIR（推奨）、LLM_MODELS_DIR（互換）（FR-1で明記）
- モデル名形式: ファイル名ベースまたは階層形式（FR-2で明記）
- 解決フロー: ローカル → 共有パス → 外部ソース/プロキシ（FR-3で明記）
- API設計: 外部は `/v1/models`、Node同期は `/v0/models` と registry API（FR-8で明記）
- 全ノード全モデル対応: すべてのノードがルーターの全対応モデルに対応（FR-9で明記）

**削除される機能**:

- LLM runtimeのmanifest/blob形式サポート
- registry.runtime.aiパス構造
 

**重要な設計判断（2025-12-24追加）**:

- Node は許可リスト内の外部ソースから直接取得できる（FR-3）
- 同期はPull型: ルーターは通知のみ送信し、ノードが自発的に取得（FR-7）
- モデル割り当ては行わず、全ノードが全モデルに対応（FR-9）

---

## 受け入れ基準

1. `~/.llm-router/models/<model_name>/model.gguf` からモデルを読み込める
2. モデルディレクトリを環境変数で上書きできる
3. モデルIDがディレクトリ名として安全に扱われる
4. 既存の単体テスト・統合テストがパスする
5. E2Eテストでモデル推論が成功する
