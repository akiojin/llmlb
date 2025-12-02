# SPEC-dcaeaec4: LLM-Router独自モデルストレージ

## 概要

llm-nodeがモデルファイルを `~/.llm-router/models/` 配下から読み込むようにし、
LLM runtime固有のストレージ形式への依存を排除する。

## 背景と動機

### 現状の問題

1. **LLM runtime依存**: 現在のLLM runtimeCompatクラスはLLM runtimeのストレージ形式に依存している
   - `~/.runtime/models/manifests/registry.runtime.ai/library/<name>/<tag>`
   - `~/.runtime/models/blobs/<sha256-digest>`
2. **複雑なパス解決**: LLM runtimeのmanifest→blob形式は本プロジェクトには過剰
3. **混乱**: ユーザーがモデルをどこに配置すべきか分かりにくい

### 解決策

シンプルな独自ディレクトリ構造を採用：

```text
~/.llm-router/models/
  <model-name>/
    model.gguf
    metadata.json (optional)
```

## 要件

### 機能要件

#### FR-1: モデルディレクトリ構造

- デフォルトのモデル保存先は `~/.llm-router/models/`
- 環境変数 `LLM_NODE_MODELS_DIR` でカスタマイズ可能
- 各モデルは `<models_dir>/<model-name>/model.gguf` に配置

#### FR-2: モデル名の形式

- モデル名は `<base>:<tag>` 形式（例: `gpt-oss:20b`）
- ディレクトリ名への変換: コロンをアンダースコアに置換
  - `gpt-oss:20b` → `gpt-oss_20b/model.gguf`
- tagがない場合は `latest` として扱う
  - `gpt-oss` → `gpt-oss_latest/model.gguf`

#### FR-3: GGUFファイル解決

- `resolveGguf("gpt-oss:20b")` は `~/.llm-router/models/gpt-oss_20b/model.gguf` を返す
- ファイルが存在しない場合は空文字列を返す

#### FR-4: 利用可能モデル一覧

- `listAvailable()` は `models_dir` 配下の全ディレクトリを走査
- 各ディレクトリ内に `model.gguf` が存在するものをリスト

#### FR-5: メタデータ（オプション）

- `metadata.json` が存在する場合、モデル情報を読み込む
- 必須フィールドなし（存在しなくても動作する）

### 非機能要件

#### NFR-1: 後方互換性

- 既存のテストは引き続きパスする（テストはモック/一時ディレクトリを使用）

#### NFR-2: シンプルさ

- LLM runtimeのmanifest/blob形式のサポートは削除

## ディレクトリ構造の例

```text
~/.llm-router/
├── config.json          # 設定ファイル
├── router.db            # ルーターDB（SQLite）
└── models/
    ├── gpt-oss_20b/
    │   ├── model.gguf   # モデルファイル
    │   └── metadata.json # (optional)
    ├── gpt-oss_7b/
    │   └── model.gguf
    └── qwen3-coder_30b/
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
2. 環境変数 `LLM_NODE_MODELS_DIR` でパスをカスタマイズできる
3. モデル名の `:` が `_` に変換される
4. 既存の単体テスト・統合テストがパスする
5. E2Eテストでモデル推論が成功する
