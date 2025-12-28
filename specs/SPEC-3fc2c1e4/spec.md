# SPEC-3fc2c1e4: 実行エンジン（統合仕様）

**ステータス**: 計画中（統合）

## この仕様の役割（初見向け）
この仕様は**実行エンジン領域の入口ガイド**です。  
エンジン選択/抽象化の原則をまとめ、詳細は下記の個別SPECに委譲します。

## 背景 / 問題
推論エンジンに関する要件が分散し、モデル管理と混在することで責務が曖昧になっている。
実行エンジン領域を統合し、モデル管理とは明確に分離する必要がある。

## 目的
- Node側エンジン抽象化と推論責務を統合的に定義する
- GPU前提（Metal/CUDA）の実行要件を明確化する
- エンジン選択が登録時のアーティファクトに従うことを保証する

## スコープ
- Node側のエンジン抽象化（EngineRegistry/Engineの責務）
- 実行環境の前提（GPU必須）
- 登録時に確定した形式とHFメタデータに基づくエンジン選択

## 非ゴール
- モデル登録・形式選択・保存（モデル管理領域）
- 自動変換/量子化生成
- Nemotron推論エンジンの詳細設計（TBD）

## 原則
- `metadata.json` のような独自メタデータには依存しない
- エンジン選択は「登録時に確定したアーティファクト」を正とする
- safetensors と GGUF が共存する場合は **登録時に形式選択が必須**
  （実行時の自動判別やフォールバックは禁止）

## 決定事項（共有用サマリ）
- **責務分離**: 形式選択は登録時に確定し、実行時はその結果に従う（実行時の自動判別/フォールバック禁止）。
- **Node前提**: Node は Python 依存を導入しない。
- **GPU前提**: GPU 非搭載ノードは対象外（登録不可）。
- **対応OS/GPU**:
  - macOS: Apple Silicon のみ対象
  - Linux/Windows: CUDA (GeForce系を含む) を対象
  - WSL2: GPUが検出できる場合のみ対象
- **形式選択必須**: safetensors と GGUF が両方ある場合は登録時に format を指定する。
  safetensors は推奨だが、自動選択は行わない。
- **最適化アーティファクト**: 公式最適化アーティファクトの利用優先はエンジン領域の実行最適化として扱い、登録時の形式選択を置き換えない。
- **Nemotron**: 新エンジンの仕様/実装は後回し（TBD）。
- **内蔵エンジンはプラグイン形式**: Node 本体は Engine Host とし、各エンジンは動的プラグイン（.dylib/.so/.dll）で追加可能にする。
- **ABI固定**: プラグインは C ABI で互換性を保証し、`abi_version` を必須とする。

## 内蔵エンジンのアーキテクチャ（概念）

> 目的: **内蔵エンジン群の責務境界と選択フロー**を一枚で把握できるようにする。

### 構成図（概念）

```
┌──────────────┐            ┌───────────────────────────┐
│  Router      │            │           Node            │
│  - 登録/形式 │──manifest──▶  ModelStorage / Registry   │
│  - HF検証    │            │  - config/tokenizer検証   │
└──────────────┘            │  - safetensors/gguf解決    │
                             │             │
                             │             ▼
                             │     EngineRegistry
                             │  (RuntimeTypeで選択)
                             │             │
                             │             ▼
                             │  Engine Host (Plugin Loader)
                             │    ├─ GGUF → llama.cpp (plugin)
                             │    ├─ TTS  → ONNX Runtime (plugin)
                             │    └─ safetensors → 独自エンジン群 (plugins)
                             │          ├─ gpt-oss
                             │          ├─ nemotron
                             │          └─ その他（Whisper/SD など）
                             └───────────────────────────┘
```

### 主要コンポーネント

- **Router**
  - 登録時に **形式（safetensors/gguf）を確定**し、HF metadata を検証する。
  - 形式選択の結果を **manifest** として Node に配布する。
- **Node / ModelStorage**
  - 形式・ファイルの整合性（`config.json` / `tokenizer.json` / shard / index）を検証。
  - `ModelDescriptor` を生成（format / primary_path / runtime / capabilities）。
- **EngineRegistry**
  - `RuntimeType` に基づき **外側の推論エンジンを確定**する。
  - Node は登録時の形式と metadata を正とし、**実行時の自動判別や形式切替は行わない**。
- **Inference Engine（外側）**
  - 共通の推論インターフェース。内部で runtime に応じてプラグインを振り分ける。
  - GGUF → `llama.cpp`、TTS → `ONNX Runtime`、safetensors → 独自エンジン群（すべてプラグイン）。
  - 公式最適化アーティファクトは **実行キャッシュ**として利用可能だが、
    登録時の形式選択は上書きしない。

### プラグイン設計指針（Node）

- **配布単位**: 共有ライブラリ + manifest.json の 1 セット
- **manifest内容**:
  - engine_id / engine_version / abi_version
  - 対応 RuntimeType / 形式（safetensors, gguf, onnx 等）
  - 対応 capabilities（text / vision / asr / tts / image）
  - GPU 要件（Metal / CUDA）
- **互換性**: C ABI を固定し、ABI 互換を破る変更は abi_version を更新する
- **解決順序**: EngineRegistry が RuntimeType と format をキーにプラグインを解決する

### RuntimeType とエンジンの対応（現状）

| RuntimeType | 主用途 | 主要アーティファクト | 備考 |
|---|---|---|---|
| `LlamaCpp` | LLM / Embedding | GGUF | 登録時に `format=gguf` を選択した場合 |
| `GptOssCpp` | gpt-oss | safetensors + 公式最適化 | Metal/CUDA の最適化アーティファクトは補助 |
| `NemotronCpp` | Nemotron | safetensors | **CUDAのみが前提**、Metalは後回し |
| `WhisperCpp` | ASR | GGML/GGUF（当面） | safetensors正本 → 変換で運用、将来は独自エンジン |
| `StableDiffusion` | 画像生成 | safetensors（直接） | stable-diffusion.cpp を当面利用 |
| `OnnxRuntime` | TTS | ONNX | Python依存なしで運用する |

### 形式選択とエンジン選択の原則

1. **Router が登録時に形式を確定**（`format=safetensors` / `format=gguf`）。
2. **safetensors/GGUF が共存する場合、format は必須指定**（自動判別禁止）。
3. **Node は登録時の形式を尊重**し、runtime を metadata から決定する。
4. **safetensors は推奨**だが、format に従って実行する（実行時フォールバック禁止）。
5. **最適化アーティファクトは “実行キャッシュ”** であり、形式選択は上書きしない。

### Nemotron の位置づけ

- 内蔵エンジンの **一部として Nemotron 対応を含む**。
- **CUDAのみが前提**（Metal は将来対応）。
- Nemotron 専用の詳細設計は **TBD** として後段 SPEC に委譲。

## 詳細仕様（参照）
- **エンジン抽象化**: `SPEC-d7feaa2c`
- **gpt-oss-20b safetensors 実行**: `SPEC-2c0e5a9b`
- **gptossアーキテクチャエイリアス**: `SPEC-8a2d1d43`
- **Nemotron PoC**: `SPEC-efff1da7`

## 受け入れ条件
1. Nodeのエンジン選択は登録済みアーティファクトとHFメタデータに一致する。
2. GPU非搭載ノードは対象外とする。
3. モデル管理の仕様と矛盾しない。

## 依存関係
- `SPEC-08d2b908`（モデル管理統合）
- `SPEC-5cd7b614`（GPU必須ノード登録要件）
