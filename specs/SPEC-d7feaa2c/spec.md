# SPEC-d7feaa2c: Nodeエンジンローダー抽象化とNemotron直接ロード

## 背景 / 問題
Nodeは現在 llama.cpp (GGUF) を前提にしており、モデル形式の多様化（safetensors等）に対応できない。
Nemotron 3 Nano 30B A3B(BF16) はGGUF変換が失敗するため、safetensorsを直接実行できる新エンジンが必要。
ただし、Nemotron向けの新エンジン（推論エンジン）の仕様と実装は後回しにし、後で決める（TBD）。

## 目的
本仕様は統合仕様 `SPEC-3fc2c1e4`（実行エンジン）の**詳細仕様**として扱う。

- Node側でエンジンローダーを抽象化し、複数エンジンを共存可能にする
- 内蔵エンジンは **プラグイン形式（動的ロード）** とし、後から同一ABIで追加可能にする
- エンジン選択は「登録時に選択したアーティファクト（safetensors/GGUF）」と
  Hugging Face の `config.json` 等のモデル由来メタデータを正として判定する

## ゴール
- Nodeにエンジン抽象化レイヤーが導入され、llama.cpp 等の複数エンジンを選択できる
- エンジンは **動的プラグイン** として追加・更新できる
- `metadata.json` のような llm-router 独自メタデータファイルに依存せず、エンジン選択が実装される

## アーキテクチャ（Nodeエンジン層）

### 目的
- **モデル形式や実行方式が増えても**、Node側のモデル検出・ロード・推論の責務が破綻しない構造にする。
- **“正（登録で確定したアーティファクト）” と “実行（Engine）”** を分離し、拡張を容易にする。

### コンポーネント分解（概念）

```
Router（登録・配布）                Node（取得・検証・実行）
───────────────────               ─────────────────────────
register(format=...)  ───────▶    ModelStorage
  ├─ 形式確定                           ├─ ローカル配置検出
  ├─ 必須ファイル検証                    ├─ HF由来メタデータ検証
  └─ マニフェスト確定                    └─ ModelDescriptor生成
                                           │
                                           ▼
                                    EngineRegistry
                                      └─ runtime → Engine を解決
                                           │
                                           ▼
                                   EngineHost (Plugin Loader)
                                     └─ Pluginへ推論を委譲
                                           │
                                           ▼
                                       Engine Plugins（複数）
                                     ├─ llama.cpp（GGUF）
                                     └─ safetensors系（将来拡張）
```

### Key concepts
- **ModelDescriptor**: Nodeが推論を開始するために必要な最小情報（例: `format`, `runtime`, `model_dir`, `primary` など）。
- **runtime**: “どのEngineで実行すべきか” を表す識別子（例: `llama_cpp` など）。
- **Engine**: 推論の実体（GPU実行・サンプリング・ストリーミングなど）を担う差し替え可能ユニット。
- **Engine Plugin**: 共有ライブラリ + manifest.json で提供される実行ユニット。
- **Engine Host**: プラグインの発見・ロード・ライフサイクル管理のみ担当するホスト層。

### エンジン選択の入力（Single source of truth）
- **登録時に確定したアーティファクト（format/必要ファイル）**
- **Hugging Face 由来のメタデータ（`config.json` 等）**

※ Nodeローカルに複数形式が同居していることを理由に自動フォールバックは行わない（登録で確定させる）。

## 非ゴール
- Nemotron向けの新エンジン（推論エンジン）の仕様策定・実装（後回し / TBDとして別途扱う）
- Nemotron 推論の高速化・最適化
- Python依存の導入
- GGUF変換の改善
- プラグインサンドボックス/権限分離

## ユーザーストーリー
- 開発者として、モデル形式が増えてもNodeのエンジン選択が壊れない構造にしたい

## 保留事項（TBD）
- Nemotron向けの新エンジン（推論エンジン）は別SPECとして後日仕様化する（後回し / TBD。Metal/CUDA対応方針、chat_template互換、dtype戦略など）。

## 受け入れ条件
- Nodeが登録時の選択（safetensors/GGUF）と `config.json` に従ってエンジンを選択する
- `metadata.json` に依存しない
- エンジン判定結果は /v1/models の応答に影響し、未対応モデルは登録対象から除外できる
- Python依存は導入しない
- Engine は動的プラグインであり、ABIバージョン一致のもののみロードされる
