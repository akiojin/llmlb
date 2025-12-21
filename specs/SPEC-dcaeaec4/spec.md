# SPEC-dcaeaec4: LLM-Router独自モデルストレージ

## 概要

llm-nodeがモデルファイルを `~/.llm-router/models/` 配下から読み込むことを基本としつつ、
ルーターが返す配布情報（共有パス or ダウンロードURL）を優先利用する。
LLM runtime固有のストレージ形式への暗黙フォールバックは撤廃する。

## 背景と動機

### 現状の問題

1. **LLM runtime依存**: 現在のLLM runtimeCompatクラスはLLM runtimeのストレージ形式に依存している
   - `~/.runtime/models/manifests/registry.runtime.ai/library/<name>/<tag>`
   - `~/.runtime/models/blobs/<sha256-digest>`
2. **複雑なパス解決**: LLM runtimeのmanifest→blob形式は本プロジェクトには過剰
3. **混乱**: ユーザーがモデルをどこに配置すべきか分かりにくい

### 解決策

シンプルな独自ディレクトリ構造を採用しつつ、ルーター主導で配布情報を返す：

```text
~/.llm-router/models/
  <model-name>/
    # 登録時に選択したアーティファクトを配置
    #
    # - safetensors を選択した場合:
    #   - model.safetensors もしくは model.safetensors.index.json + shard safetensors
    #   - 付随するメタデータ (config.json, tokenizer.json)
    #
    # - GGUF を選択した場合:
    #   - model.gguf
```

## 要件

### 機能要件

#### FR-1: モデルディレクトリ構造（ノードローカルキャッシュ）

- デフォルトのモデル保存先は `~/.llm-router/models/`
- 環境変数で上書き可能（推奨: `LLM_NODE_MODELS_DIR`、互換: `LLM_MODELS_DIR`）
- 各モデルは `<models_dir>/<model-name>/` に「登録時に選択したアーティファクト」を配置
  - safetensors の場合は `config.json` と `tokenizer.json` を必須とする（不足時はエラー）

#### FR-2: モデル名の形式

- モデル名（モデルID）はファイル名ベース形式または階層形式を許可
  - ファイル名ベース形式: `gpt-oss-20b`
  - 階層形式: `openai/gpt-oss-20b`（HuggingFace互換）
- ディレクトリ名はモデルIDをそのまま使用（小文字に正規化）
  - `gpt-oss-20b` → `gpt-oss-20b/model.gguf`
  - `openai/gpt-oss-20b` → `openai/gpt-oss-20b/model.gguf`（ネストディレクトリ）
- 危険な文字（`..`, `\0`等）は禁止、`/`はディレクトリセパレータとして許可

#### FR-3: モデルアーティファクト解決（多段フロー）

1. ルーター `/v0/models` 応答の対象モデルを取得し、`path` と `download_url` を参照できること。
   - **注意**: `/v1/models` はOpenAI互換APIのため拡張情報を含まない。ノード同期用は `/v0/models` を使用
2. ルーターは `download_url` をもつモデルについて **事前に自分の `~/.llm-router/models/` へキャッシュ** を試みる。成功すれば `path` を応答に含める。
3. ノードはまずローカル `~/.llm-router/models/<name>/` 配下で登録済みアーティファクトを探す。あれば採用。
4. ルーターから受け取った `path` が存在し読み取り可能なら、それを直接使用（共有ストレージ: NFS, S3等）。
5. `path` が不可なら、ルーターの配信APIからダウンロードし、`~/.llm-router/models` に保存。
6. いずれも不可なら、`download_url` を最後の手段としてダウンロードし、`~/.llm-router/models` に保存。
7. いずれも不可ならエラーを返す。LLM runtime固有形式への暗黙フォールバックは禁止。

※4の「直接使用」は共有ストレージ/NFS/S3を想定。不可の場合は5にフォールバックする。

#### FR-4: 利用可能モデル一覧

- `listAvailable()` は `models_dir` 配下の全ディレクトリを走査
- 登録済みアーティファクト（safetensors / GGUF）が存在するものをリスト

#### FR-5: 追加メタデータファイルは不要

- `metadata.json` のような llm-router 独自メタデータファイルは使用しない
- 必要な情報は Hugging Face の `config.json` / `tokenizer.json` 等のモデル由来メタデータで管理する

#### FR-6: ノード起動時同期

- ノードは起動時にルーターの `/v0/models` エンドポイントからモデル一覧を取得
- 各モデルについてFR-3のモデル解決フローに従って取得/参照
- ローカル → 共有パス → API経由ダウンロードの順で解決

#### FR-7: ルーターからのプッシュ通知

- ルーターは新しいモデル登録時に、オンライン状態の全ノードにプルリクエストを送信
- プルリクエストはノードの `/api/models/pull` エンドポイントをPOST
- ノードはリクエスト受信後、FR-3のモデル解決フローに従って取得
- リトライポリシー: 無限リトライ、指数バックオフ（1s, 2s, 4s, ... 最大60s）

#### FR-8: API設計

- `/v0/models` - 独自拡張API（`path`, `download_url`等を含む）- ノード同期用
- `/v1/models` - OpenAI互換API（標準形式のみ）
- `/api/models/registered` - **廃止**（`/v0/models`に統合）

### 非機能要件

#### NFR-1: 後方互換性

- 既存のテストは引き続きパスする（テストはモック/一時ディレクトリを使用）

#### NFR-2: シンプルさと安全性

- LLM runtimeのmanifest/blob形式のサポートは削除（他アプリの資産に依存しない）
- 参照パスは `~/.llm-router/models` とルーターが明示的に返す `path`/`download_url` のみ

## ディレクトリ構造の例

```text
~/.llm-router/
├── config.json          # 設定ファイル
├── router.db            # ルーターDB（SQLite）
└── models/
    ├── gpt-oss-20b/
    │   └── model.gguf   # GGUF（登録時に選択）
    ├── gpt-oss-7b/
    │   └── model.gguf
    └── nvidia-nemotron-3-nano-30b-a3b-bf16/
        ├── config.json
        ├── tokenizer.json
        ├── model.safetensors.index.json
        ├── model-00001-of-000NN.safetensors
        └── model-000NN-of-000NN.safetensors
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

## 受け入れ基準

1. 登録時に選択したアーティファクト（safetensors / GGUF）を `~/.llm-router/models/<model_name>/` から読み込める
2. モデルディレクトリを環境変数で上書きできる
3. モデルIDがディレクトリ名として安全に扱われる
4. 既存の単体テスト・統合テストがパスする
5. E2Eテストでモデル推論が成功する
