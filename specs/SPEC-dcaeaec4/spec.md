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
    model.gguf
    metadata.json (optional)
```

## 要件

### 機能要件

#### FR-1: モデルディレクトリ構造（ノードローカルキャッシュ）

- デフォルトのモデル保存先は `~/.llm-router/models/`
- 環境変数で上書き可能（推奨: `LLM_NODE_MODELS_DIR`、互換: `LLM_MODELS_DIR`）
- 各モデルは `<models_dir>/<model-name>/model.gguf` に配置

#### FR-2: モデル名の形式

- モデル名（モデルID）はファイル名ベース形式（例: `gpt-oss-20b`）
- ディレクトリ名はモデルIDをそのまま使用（小文字に正規化し、危険な文字は `_` に置換）
  - `gpt-oss-20b` → `gpt-oss-20b/model.gguf`

#### FR-3: GGUFファイル解決（多段フロー）

1. ルーター `/v1/models` 応答の対象モデルを取得し、`path` と `download_url` を参照できること。
2. ルーターは `download_url` をもつモデルについて **事前に自分の `~/.llm-router/models/` へキャッシュ** を試みる。成功すれば `path` を応答に含める。
3. ノードはまずローカル `~/.llm-router/models/<name>/model.gguf` を探す。あれば採用。
4. ルーターから受け取った `path` が存在し読み取り可能なら、それを直接使用（コピー可）。
5. `path` が不可なら、ルーターの `/v0/models/blob/:model_name` からダウンロードし、`~/.llm-router/models` に保存。
6. いずれも不可なら、`download_url` を最後の手段としてダウンロードし、`~/.llm-router/models` に保存。
7. いずれも不可ならエラーを返す。LLM runtime固有形式への暗黙フォールバックは禁止。

※3の「直接使用」は共有ストレージ/NFSを想定。不可の場合でも4にフォールバックする。

#### FR-4: 利用可能モデル一覧

- `listAvailable()` は `models_dir` 配下の全ディレクトリを走査
- 各ディレクトリ内に `model.gguf` が存在するものをリスト

#### FR-5: メタデータ（オプション）

- `metadata.json` が存在する場合、モデル情報を読み込む
- 必須フィールドなし（存在しなくても動作する）

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
    │   ├── model.gguf   # モデルファイル
    │   └── metadata.json # (optional)
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

## 受け入れ基準

1. `~/.llm-router/models/<model_name>/model.gguf` からモデルを読み込める
2. モデルディレクトリを環境変数で上書きできる
3. モデルIDがディレクトリ名として安全に扱われる
4. 既存の単体テスト・統合テストがパスする
5. E2Eテストでモデル推論が成功する
