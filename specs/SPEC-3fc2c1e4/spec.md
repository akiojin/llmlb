# SPEC-3fc2c1e4: 実行エンジン統合仕様

**ステータス**: 実装完了

## この仕様の役割
本仕様は**実行エンジン領域の入口ガイド**です。エンジン選択・抽象化の原則をまとめ、詳細は個別 SPEC に委譲します。

## 背景 / 問題
- 推論エンジンに関する要件が分散し、モデル管理との責務が混在している。
- エンジン選択規則が曖昧で、登録時の形式と実行時の選択がぶれていた。

## 目的
- Node 側のエンジン抽象化と推論責務を統合的に定義する。
- GPU 前提（Metal/CUDA）での実行要件を明確化する。
- エンジン選択が登録時の `format` とアーティファクトに従うことを保証する。

## スコープ
- Node 側の EngineRegistry / EngineHost / plugin ロード設計
- マニフェストとローカル実体に基づくエンジン選択
- GPU バックエンド前提の実行要件

## 非ゴール
- モデル登録・保存（モデル管理領域）
- 自動変換/量子化生成
- Nemotron 推論エンジンの詳細設計（TBD）

## 原則
- `metadata.json` のような独自メタデータには依存しない。
- 形式選択は Router では行わず、Node が runtime/GPU 要件に応じて判断する。
- 登録時に確定した `format` を尊重し、実行時の形式変換は行わない。

## 決定事項（要約）
- **責務分離**: Router は manifest を提供し、Node が runtime/アーティファクト選択を行う。
- **Node 前提**: Node は Python 依存を持たない。
- **GPU 前提**: GPU 非搭載ノードは対象外。
- **llama.cpp fork**: Upstream fixes are pending; operate on `akiojin/llama.cpp` until they land upstream.
- **対応 OS/GPU**:
  - macOS: Apple Silicon / Metal
  - Windows: CUDA
  - Linux: 当面は非対応（CUDA は実験扱い）
  - WSL2: 対象外
- **CUDA 移行理由**: Windows は **CUDA が再現性・安定性で優位**なため主経路とし、
  DirectML は **最適化アーティファクト不足とドライバ差分の影響が大きい**ため凍結。
- **形式固定**: `format` は登録時に確定し、実行時の形式切替は行わない。
- **最適化アーティファクト**: 公式最適化アーティファクトは実行キャッシュとして利用可能だが、
  Node が選択したアーティファクトを上書きしない。
- **Nemotron**: 新エンジンの詳細設計は後回し（TBD）。

## 内蔵エンジン要件（単一要件）

**REQ-IE-001**: 内蔵エンジンは **RuntimeType / format / capabilities** に基づく単一の選択規約を持つ。
以下の条件を **一つの要件** として満たすこと:
- **プラグイン形式**: Node 本体は Engine Host とし、エンジンは動的プラグイン（.dylib/.so/.dll）で追加可能。
- **ABI 固定**: C ABI で互換性を保証し、`abi_version` を明示する。
- **選択ソース**: 登録時に確定した `format` と HF 由来メタデータ（`config.json` 等）を正とする。
- **自動フォールバック禁止**: safetensors/GGUF が共存しても `format` を優先し、実行時に切替しない。
- **GPU 前提**: macOS=Metal / Windows=CUDA（Linux CUDA は実験扱い）。
- **可否判定**: EngineRegistry / EngineHost / `/v1/models` の可否判定に反映し、未対応は ready 対象外とする。

## アーキテクチャ概念

```
Router
  - 登録/メタデータ検証
  - manifest 作成（ファイル一覧）
        │
        ▼
Node
  - ModelStorage / Resolver
  - runtime/アーティファクト選択
        │
        ▼
EngineRegistry
  - RuntimeType で解決
        │
        ▼
Engine Host (Plugin Loader)
  - GGUF: llama.cpp
  - safetensors: gpt-oss / nemotron (TBD)
  - ASR/TTS/画像: whisper / onnx / stable-diffusion
```

## GPU バックエンド
- **Metal**: macOS / Apple Silicon
- **CUDA**: Windows / NVIDIA
- **DirectML**: 凍結

## 現状の対応状況（2026-01-06）
- safetensors 系 LLM で安定動作が確認できているのは **gpt-oss（Metal/macOS）** のみ。
- Windows は CUDA 主経路。DirectML は凍結。Nemotron は TBD。
- 詳細な検証状況は `specs/SPEC-6cd7f960/verified-models.md` を正とする。

## プラグイン設計指針
- **配布単位**: 共有ライブラリ + `manifest.json`
- **manifest.json**
  - engine_id / engine_version / abi_version
  - runtimes / formats / architectures / capabilities / modalities
- gpu_targets（metal / cuda / rocm / vulkan）
  - library（共有ライブラリ名）
- **互換性**: ABI 互換を破る変更は `abi_version` を更新する。

## RuntimeType 対応（現状）

| RuntimeType | 主用途 | 主要アーティファクト | 備考 |
|---|---|---|---|
| `LlamaCpp` | LLM / Embedding | GGUF | llama.cpp が複数アーキテクチャを横断対応 |
| `GptOssCpp` | gpt-oss | safetensors + 公式最適化 | macOS=Metal、Windows=CUDA |
| `NemotronCpp` | Nemotron | safetensors | **TBD**（Windows CUDA 想定） |
| `WhisperCpp` | ASR | GGML/GGUF | safetensors 対応は未前提 |
| `StableDiffusion` | 画像生成 | safetensors | stable-diffusion.cpp を当面利用 |
| `OnnxRuntime` | TTS | ONNX | Python 依存なし運用 |

## アーティファクト選択ルール
1. Router は形式を確定せず、manifest のみを提供する。
2. Node が runtime/GPU 要件に応じてアーティファクトを選択する。
3. 形式変換は行わない（safetensors/GGUF/Metal/CUDA はそのまま扱う）。
4. 公式最適化アーティファクトは実行キャッシュとして利用可能。

## 性能 / メモリ要件（指針）
- 登録時またはエンジン更新時に簡易ベンチマークを実施。
- 指標: tokens/sec、TTFT、VRAM 使用量（ピーク/平均）。
- VRAM 使用率が 90% 超または OOM は失敗扱い。

## Nemotron の位置づけ
- 統合エンジン群の一部として扱う。
- Windows CUDA を想定し、Linux CUDA は実験扱い。
- 具体仕様は後続 SPEC で定義する。
