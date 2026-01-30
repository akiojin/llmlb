# SPEC-dcaeaec4: LLM-Load Balancer独自モデルストレージ

## 概要

xllm がモデルファイルを `~/.llmlb/models/` 配下から読み込むことを基本としつつ、
**モデルキャッシュはNode主導**とする。ロードバランサーは登録情報とファイル一覧（マニフェスト）を提示し、
Node が GPU 差分に応じて必要アーティファクトを選択・取得する。
ロードバランサーは**モデルバイナリをキャッシュしない**（登録メタデータのみ保持）。
LLM runtime固有のストレージ形式への暗黙フォールバックは撤廃する。

## 背景と動機

### 現状の問題

1. **LLM runtime依存**: 現在のLLM runtimeCompatクラスはLLM runtimeのストレージ形式に依存している
   - `~/.runtime/models/manifests/registry.runtime.ai/library/<name>/<tag>`
   - `~/.runtime/models/blobs/<sha256-digest>`
2. **複雑なパス解決**: LLM runtimeのmanifest→blob形式は本プロジェクトには過剰
3. **混乱**: ユーザーがモデルをどこに配置すべきか分かりにくい

### 解決策

シンプルな独自ディレクトリ構造を採用しつつ、ロードバランサーは登録情報とマニフェストを提示する：

```text
~/.llmlb/models/
  <model-name>/
    model.safetensors.*（index + shards）/ model.gguf / model.metal.bin など
```

## 要件

### 機能要件

#### FR-1: モデルディレクトリ構造（ノードローカルキャッシュ）

- デフォルトのモデル保存先は `~/.llmlb/models/`
- 環境変数で上書き可能（推奨: `XLLM_MODELS_DIR`、互換: `LLM_MODELS_DIR`）
- 各モデルは `<models_dir>/<model-name>/` 配下に配置し、形式に応じたアーティファクトを保持する

#### FR-2: モデル名の形式

- モデル名（モデルID）はファイル名ベース形式または階層形式を許可
  - ファイル名ベース形式: `gpt-oss-20b`
  - 階層形式: `openai/gpt-oss-20b`（HuggingFace互換）
- ディレクトリ名はモデルIDをそのまま使用（小文字に正規化）
  - `gpt-oss-20b` → `gpt-oss-20b/` 配下に必要アーティファクト
  - `openai/gpt-oss-20b` → `openai/gpt-oss-20b/` 配下に各アーティファクト（ネストディレクトリ）
- 危険な文字（`..`, `\0`等）は禁止、`/`はディレクトリセパレータとして許可

#### FR-3: モデルアーティファクト解決（Node主導）

1. ロードバランサーは登録済みモデルの**ファイル一覧（マニフェスト）**を提示する。
   - 例: `/api/models/registry/:model_name/manifest.json`
2. Node はマニフェストと GPU バックエンド（Metal/DirectML）に応じて**必要アーティファクトを選択**する。
3. Node はローカル `~/.llmlb/models/<name>/` を確認し、必要アーティファクトが揃っていれば採用する。
4. 共有パスは本仕様では扱わない（廃止）。
5. ローカルに無ければ、Node は**許可リスト内の外部ソース（Hugging Face 等）から直接取得**し、ローカルに保存する。
   - ロードバランサーは**モデルバイナリを保持しない**ため、プロキシは必須ではない。
6. いずれも不可ならエラーを返す。LLM runtime固有形式への暗黙フォールバックは禁止。

#### FR-4: 利用可能モデル一覧

- `listAvailable()` は `models_dir` 配下の全ディレクトリを走査
- 各ディレクトリ内に **対応アーティファクト（safetensors/gguf/metal等）** が存在するものをリスト

#### FR-5: メタデータ（オプション）

- `metadata.json` が存在する場合、モデル情報を読み込む
- 必須フィールドなし（存在しなくても動作する）

#### FR-6: ノード起動時同期

- ノードは起動時にロードバランサーのモデル一覧（`/v1/models` または `/api/models`）を取得
- 各モデルについて **マニフェストを参照**し、FR-3の解決フローに従って取得/参照
- ローカル → 外部ソース（HF等）の順で解決

#### FR-7: ロードバランサーからの同期通知

- ロードバランサーは新しいモデル登録時に、オンライン状態の全ノードに同期通知を送信
- 同期通知はノードの `/api/models/sync` エンドポイントをPOST（モデル名のみ含む）
- ノードは通知受信後、ロードバランサーの `/v1/models` からモデル一覧を取得し、FR-3のモデル解決フローに従って同期
- **重要**: これはPull型の同期。ロードバランサーはモデルファイルをプッシュせず、通知のみ送信。ノード側が自発的に取得する
- リトライポリシー: 無限リトライ、指数バックオフ（1s, 2s, 4s, ... 最大60s）

#### FR-8: API設計

外部クライアント向けと Node 同期向けで用途を分ける：

- `GET /v1/models` - 外部クライアント向け一覧（OpenAI互換 + ダッシュボード拡張フィールド）
- `POST /v1/models/register` - モデル登録（HuggingFace URL等）
- `DELETE /v1/models/:model_name` - モデル削除
- `GET /api/models` - Node 同期向けメタデータ（format / repo / filename など）
- `GET /api/models/registry/:model_name/manifest.json` - Node 向けファイル一覧

**廃止済みエンドポイント**:
- `/api/models/registered` - **廃止**（`/v1/models`に統合）

#### FR-9: 全ノード全モデル対応の原則（廃止）

**廃止理由**: SPEC-93536000 により、モデル対応はノードが `executable_models` として報告する方式へ移行。

- すべてのノードが全モデルに対応する前提は廃止
- 個別ノードへのモデル割り当て機能は引き続きスコープ外
- ノードは起動時および同期通知受信時に、ロードバランサーから全モデルを取得する前提は廃止

### 非機能要件

#### NFR-1: 後方互換性

- 既存のテストは引き続きパスする（テストはモック/一時ディレクトリを使用）

#### NFR-2: シンプルさと安全性

- LLM runtimeのmanifest/blob形式のサポートは削除（他アプリの資産に依存しない）
- 参照パスは `~/.llmlb/models` と共有パス（設定時）のみ
- ノードのダウンロード先は **許可リスト内の外部ソース** に限定する

## ディレクトリ構造の例

```text
~/.llmlb/
├── config.json          # 設定ファイル
├── lb.db            # ロードバランサーDB（SQLite）
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
        └── model.safetensors.index.json
            (shards...)
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

- デフォルトパス: ~/.llmlb/models/（FR-1で明記）
- 環境変数: XLLM_MODELS_DIR（推奨）、LLM_MODELS_DIR（互換）（FR-1で明記）
- モデル名形式: ファイル名ベースまたは階層形式（FR-2で明記）
- 解決フロー: ローカル → 外部ソース（FR-3で明記）
- API設計: 外部は `/v1/models`、Node同期は `/api/models` と manifest API（FR-8で明記）
- 全ノード全モデル対応: 廃止（SPEC-93536000 でノード別対応へ移行）

**削除される機能**:

- LLM runtimeのmanifest/blob形式サポート
- registry.runtime.aiパス構造
 

**重要な設計判断（2025-12-24追加）**:

- Node は許可リスト内の外部ソースから直接取得できる（FR-3）
- 同期はPull型: ロードバランサーは通知のみ送信し、ノードが自発的に取得（FR-7）
- モデル割り当ては行わず、全ノードが全モデルに対応（FR-9）は廃止（SPEC-93536000へ移行）

### Session 2025-12-31

- **ロードバランサーはモデルバイナリを保持しない**（登録メタデータのみ）
- **Node は外部ソース（HF等）から直接ダウンロード**してキャッシュ
- **変換パイプラインは廃止**（GGUF変換も行わない）

---

## 受け入れ基準

1. `~/.llmlb/models/<model_name>/` 配下のアーティファクト（gguf/safetensors）からモデルを読み込める
2. モデルディレクトリを環境変数で上書きできる
3. モデルIDがディレクトリ名として安全に扱われる
4. 既存の単体テスト・統合テストがパスする
5. E2Eテストでモデル推論が成功する
