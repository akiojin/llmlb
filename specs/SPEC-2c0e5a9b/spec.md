# SPEC-2c0e5a9b: gpt-oss-20b safetensors 実行（GPU: Metal/CUDA）

## 背景 / 問題

- gpt-oss-20b は Hugging Face 上で safetensors（シャーディング + index）として配布される。
- 一部のモデルは、特定GPU向けに“公式の実行用アーティファクト”（例: Apple Silicon/Metal向けの事前変換済みファイル）を別途提供する場合がある。
- llm-router の Node は現状 GGUF（llama.cpp）を主前提としており、safetensors を正本として GPU で実行する“推論エンジン（線）”が不足している。
- 運用方針として safetensors を正本とし、GGUF は「GGUFしか存在しないモデル」のみで選択する（gpt-oss-20b は GGUF 前提にしない）。
- 実行環境は GPU 前提（Apple Silicon/Metal、NVIDIA/CUDA）。GPU 非搭載ノードは対象外。

## 目的

- `openai/gpt-oss-20b` を safetensors 系アーティファクトとして登録し、GPU 上でテキスト生成（OpenAI互換 `POST /v1/chat/completions`）が成立する。
- エンジン選択は Node 側抽象化（`SPEC-d7feaa2c`）を前提に、登録時の `format` と HF 由来メタデータ（`config.json` など）に従って決定する（`metadata.json` に依存しない）。
- `chat_template` が無いモデルはデフォルトテンプレートを用い、レスポンスは OpenAI 互換形式で返る。
- safetensors を正本（監査・説明責任の基準）としつつ、公式の GPU 最適化アーティファクトがある場合はそれを優先実行できる。

## スコープ

### スコープ内
- gpt-oss-20b のテキスト生成（通常応答 / ストリーミング）
- safetensors のシャーディング（`.safetensors.index.json` + shards）を 1 つのモデルとして扱うこと
- 登録時の選択（`format=safetensors`）を正として Node がロード可能であること
- Apple Silicon (Metal) / NVIDIA (CUDA) の GPU 実行を前提とした設計
- safetensors を正本（監査・説明責任の基準）としつつ、公式のGPU最適化アーティファクトが提供されている場合はそれを優先して実行できること

### スコープ外
- Nemotron 推論エンジン（本件では後回し / TBD）
- 画像生成・画像認識・音声生成・音声認識（別仕様）
- CPU のみでの推論フォールバック
- 品質/速度の最適化競争（成立後に段階的に扱う）

## ユーザーシナリオ＆テスト *(必須)*

### ユーザーストーリー1 - gpt-oss-20b を safetensors で登録し、GPUで推論したい (P1)
運用管理者として、`openai/gpt-oss-20b` を safetensors として登録し、Apple Silicon あるいは NVIDIA GPU のノードで推論できることを期待する。

**独立テスト**:
1. **前提** 対象ノードが GPU を検出済みで online、**実行** `openai/gpt-oss-20b` を `format=safetensors` で登録、**結果** `/v1/models` に当該モデルが ready として現れる。
2. **前提** 当該モデルが ready、**実行** `POST /v1/chat/completions` を実行、**結果** `choices[0].message.content` に非空文字列が返る。

### ユーザーストーリー2 - 不足ファイル/未対応環境は明確にエラーにしたい (P1)
運用管理者として、必要なメタデータや実行要件を満たさない場合に、原因が分かる形で即時に失敗してほしい。

**独立テスト**:
1. **前提** HFスナップショットに `config.json` または `tokenizer.json` が存在しない、**実行** `format=safetensors` で登録、**結果** 400 で不足ファイル名を含むエラー。
2. **前提** 対象ノードが gpt-oss safetensors 実行に未対応、**実行** `format=safetensors` で登録、**結果** モデルは `/v1/models` の ready 一覧に出ない（または明確な未対応エラー）。

### エッジケース
- `model.safetensors.index.json` が存在するが shard の一部が欠けている場合、モデルは ready にならず、欠けているファイル名を示す。
- `format=safetensors` で `.safetensors` が複数あるのに index が無い場合、曖昧としてエラーにする。
- 登録が重複する場合は既存を上書きせず、明確にエラーにする（既存仕様と整合）。

## 要件 *(必須)*

## エンジンアーキテクチャ（本仕様の範囲）

### 全体像（テキスト生成）
```
Client
  │  POST /v1/chat/completions
  ▼
Router
  │  (必要なら chat_template をレンダリング)
  ▼
Node
  ├─ ModelStorage: ローカル配置 + config.json から ModelDescriptor を生成
  ├─ EngineRegistry: runtime を解決
  └─ Engine(gpt-oss): GPUで推論（通常/ストリーミング）
       ├─ 優先1: 公式GPU最適化アーティファクト（allowlist対象）
       └─ 優先2: safetensors（index + shards）
```

### 役割の分離
- **safetensors（正本）**: 監査・説明責任の基準。常に保持し、必要なメタデータ（`config.json`, `tokenizer.json`）で一貫性を担保する。
- **公式GPU最適化アーティファクト（実行キャッシュ）**: GPU実行のために“公式が提供する最適化済みアーティファクト”。存在し、かつ許可リスト内なら実行で優先できる。
- **Engine**: gpt-oss の “線（推論ロジック）” を実装する実行単位。Metal/CUDA を内包し、OpenAI互換の生成結果を返す。

### 機能要件
- **FR-001**: gpt-oss-20b を `format=safetensors` として登録できる。
- **FR-002**: safetensors のシャーディング（index + shards）を 1 つのモデルとして扱える。
- **FR-003**: `format=safetensors` の登録では `config.json` と `tokenizer.json` を必須とし、不足時は明確なエラーを返す（既存仕様と整合）。
- **FR-004**: Node は登録時に確定した形式と `config.json` 等の HF 由来メタデータに基づき、gpt-oss 用エンジンを選択できる（`metadata.json` に依存しない）。
- **FR-005**: 対応エンジンが存在しない/要件を満たさない場合、モデルは ready として扱わず、運用者が判断できる情報を返す。
- **FR-006**: `POST /v1/chat/completions`（通常/ストリーミング）で 1 トークン以上の生成が成立する。
- **FR-007**: `chat_template` が無い場合はデフォルトテンプレートを利用し、レスポンスは OpenAI 互換形式で返す。
- **FR-008**: 公式のGPU最適化アーティファクトが利用可能な場合はそれを優先し、利用できない場合は safetensors を用いて実行する。
- **FR-009**: 公式のGPU最適化アーティファクトは「同一 publisher org（例: `openai`, `nvidia`）配下の別リポジトリ」から取得できる。取得可否は許可リストで管理する（初期値: `openai/*`, `nvidia/*`）。

### 非機能要件
- **NFR-001**: GPU 非搭載ノードを登録対象にしない（既存方針と整合）。
- **NFR-002**: Node は Python 依存なしで動作する（必須）。
- **NFR-003**: 失敗時のエラーメッセージは運用者が対処できる粒度（不足ファイル、未対応環境、等）である。

## 依存関係
- `SPEC-d7feaa2c`（Nodeエンジンローダー抽象化）
- `SPEC-a61b24f2`（登録時の形式選択: safetensors/GGUF）
- `SPEC-11106000`（HF URL 登録フロー、キャッシュ/ダウンロード）

## 成功基準 *(必須)*
1. `openai/gpt-oss-20b` を `format=safetensors` で登録し、GPU ノードで `POST /v1/chat/completions` が成功する。
2. 必須メタデータ不足・ファイル欠損・未対応環境のいずれも、運用者が原因を特定できる形で失敗する。
3. safetensors がシャーディングされていても 1 モデルとして一貫して扱える。
