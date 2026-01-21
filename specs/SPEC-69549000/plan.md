# 実装計画: safetensors.cpp

**機能ID**: `SPEC-69549000` | **日付**: 2026-01-06 | **仕様**: [spec.md](./spec.md)
**入力**: `/specs/SPEC-69549000/spec.md`の機能仕様

## 概要

llama.cpp/stable-diffusion.cpp/whisper.cppと同様の独立C++プロジェクトとして、
safetensors形式のLLMを直接ロード・推論するライブラリを実装する。

ggmlバックエンドを使用し、Metal/CUDA/ROCm/Vulkanで統一的にGPU推論を実行可能にする。

**Phase 9でNemotron 3対応を追加**: Mamba-Transformer MoE（Mixture of Experts）ハイブリッドアーキテクチャに対応し、1M-tokenコンテキストをサポートする。

## 技術コンテキスト

**言語/バージョン**: C++17
**主要依存関係**: ggml（サブモジュール）
**ストレージ**: ファイルシステム（mmap）
**テスト**: CTest + catch2 または googletest
**対象プラットフォーム**: macOS (Metal), Windows/Linux (CUDA), Linux (ROCm), クロスプラットフォーム (Vulkan)
**プロジェクトタイプ**: single（独立ライブラリ）
**パフォーマンス目標**: HuggingFace transformers以上の推論速度
**制約**: GPU必須（CPU推論は初期スコープ外）、VRAMにモデルが収まること
**スケール/スコープ**: gpt-oss-20b対応済み、Phase 9でNemotron 3（Mamba-Transformer MoE）対応追加、将来的に他アーキテクチャ拡張

## 憲章チェック

**シンプルさ**:

- プロジェクト数: 1（safetensors.cpp本体のみ）
- フレームワークを直接使用? はい（ggmlを直接利用、独自ラッパーなし）
- 単一データモデル? はい（ggml_tensor構造体を中心に設計）
- パターン回避? はい（Repository/UoW不要、直接ファイルアクセス）

**アーキテクチャ**:

- すべての機能をライブラリとして? はい
- ライブラリリスト:
  - `safetensors.cpp`: コアライブラリ（safetensorsロード、推論）
- ライブラリごとのCLI: `examples/main.cpp`（--help/--version対応）
- ライブラリドキュメント: README.md + APIリファレンス

**テスト (妥協不可)**:

- RED-GREEN-Refactorサイクルを強制? はい
- Gitコミットはテストが実装より先に表示? はい
- 順序: Contract → Integration → E2E → Unit を厳密に遵守? はい
- 実依存関係を使用? はい（実際のsafetensorsファイル、モックなし）
- Integration testの対象: safetensorsローダー、GPUバックエンド、推論パイプライン
- 禁止: テスト前の実装、REDフェーズのスキップ

**可観測性**:

- 構造化ロギング含む? はい（DEBUG/INFO/WARN/ERRORレベル、設定可能）
- エラーコンテキスト十分? はい（コールバック方式でエラー詳細を提供）

**バージョニング**:

- バージョン番号割り当て済み? はい（0.1.0から開始）
- 変更ごとにBUILDインクリメント? はい
- 破壊的変更を処理? 初期開発中は0.x.xで管理

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-69549000/
├── spec.md              # 機能仕様書
├── plan.md              # このファイル
├── research.md          # 技術リサーチ結果
├── data-model.md        # データモデル定義
├── quickstart.md        # クイックスタートガイド
├── contracts/           # API契約定義
│   └── c-api.md         # C API仕様
└── tasks.md             # タスクリスト
```

### ソースコード (リポジトリルート)

```text
node/third_party/safetensors.cpp/
├── ggml/                      # サブモジュール（ggml本家）
├── include/
│   └── safetensors.h          # 公開C API (stcpp_* prefix)
├── src/
│   ├── safetensors.cpp        # コア実装
│   ├── safetensors-loader.cpp # safetensors→ggmlテンソル変換
│   ├── tokenizer.cpp          # HuggingFace tokenizer.json解析
│   ├── chat-template.cpp      # Jinja2 chat_template解析
│   ├── sampling.cpp           # サンプリング実装
│   ├── kv-cache.cpp           # KVキャッシュ管理
│   ├── batch.cpp              # continuous batching
│   ├── lora.cpp               # LoRA/QLoRAアダプター
│   └── arch/                  # アーキテクチャ別実装（in-process）
│       ├── gptoss.cpp         # gpt-oss-20b
│       ├── nemotron3.cpp      # Nemotron 3 (Mamba-Transformer MoE)
│       ├── mamba.cpp          # Mamba State Space Model
│       └── moe.cpp            # Mixture of Experts routing
├── examples/
│   └── main.cpp               # CLIサンプル
├── tests/
│   ├── contract/              # API契約テスト
│   ├── integration/           # 統合テスト
│   └── unit/                  # ユニットテスト
├── CMakeLists.txt
├── README.md
└── LICENSE                    # MIT
```

## Phase 0: リサーチ

### 完了済みリサーチ（インタビュー結果）

1. **safetensorsローダー選択**:
   - 決定: stable-diffusion.cppの実装を移植
   - 理由: 動作実績あり、コードベース内に既存、MIT互換
   - 代替案: syoyo/safetensors-cpp、hsnyder/safetensors.h

2. **ggmlバージョン**:
   - 決定: ggml本家（<https://github.com/ggml-org/ggml>）mainに追従
   - 理由: GPUバックエンド（Metal/CUDA/ROCm/Vulkan）がフル対応
   - 代替案: llama.cpp内蔵ggml（依存が発生）

3. **KVキャッシュ**:
   - 決定: llama.cppのKVキャッシュ実装を参考
   - 理由: 成熟した実装、INT8/FP8量子化対応
   - 代替案: 独自実装（工数大）

4. **Attention実装**:
   - 決定: ggml標準のみ使用（Flash Attentionは当面不要）
   - 理由: ggml標準で十分な性能、複雑性を避ける

5. **マルチGPU**:
   - 決定: Pipeline Parallelism
   - 理由: レイヤー単位の分割は実装が比較的シンプル
   - 代替案: Tensor Parallelism（将来検討）

6. **Nemotron 3アーキテクチャ** (Phase 9):
   - 決定: Mamba State Space Model + Transformer + MoE（Mixture of Experts）ハイブリッド実装
   - 理由: Nemotron 3はLlamaベースではない独自アーキテクチャ（1M-tokenコンテキスト対応）
   - 主要コンポーネント:
     - Mamba SSM: 効率的な長コンテキスト処理
     - Transformer: 標準的なアテンション機構
     - MoE: Top-K Expert routing
   - 実装参考: Mamba公式実装、MoE関連論文

詳細は [research.md](./research.md) を参照。

## Phase 1: 設計

### 主要コンポーネント

```text
┌─────────────────────────────────────────────────────────────┐
│                    safetensors.cpp                          │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │  C API      │  │  Model      │  │  Context    │         │
│  │  (stcpp_*)  │  │  Loader     │  │  Manager    │         │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘         │
│         │                │                │                 │
│  ┌──────▼────────────────▼────────────────▼──────┐         │
│  │              Inference Engine                 │         │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐       │         │
│  │  │Tokenizer│  │Sampling │  │KV Cache │       │         │
│  │  └─────────┘  └─────────┘  └─────────┘       │         │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐       │         │
│  │  │Batching │  │LoRA     │  │Prompt$  │       │         │
│  │  └─────────┘  └─────────┘  └─────────┘       │         │
│  └───────────────────────┬───────────────────────┘         │
│                          │                                  │
│  ┌───────────────────────▼───────────────────────┐         │
│  │           Architecture Plugins                │         │
│  │  ┌─────────┐  ┌──────────────┐  ┌─────────┐  │         │
│  │  │gpt-oss  │  │Nemotron 3    │  │ (future)│  │         │
│  │  │         │  │(Mamba+MoE)   │  │         │  │         │
│  │  └─────────┘  └──────────────┘  └─────────┘  │         │
│  └───────────────────────┬───────────────────────┘         │
│                          │                                  │
├──────────────────────────┼──────────────────────────────────┤
│                          │                                  │
│  ┌───────────────────────▼───────────────────────┐         │
│  │                  ggml (submodule)             │         │
│  │  ┌───────┐  ┌───────┐  ┌───────┐  ┌───────┐  │         │
│  │  │ Metal │  │ CUDA  │  │ ROCm  │  │Vulkan │  │         │
│  │  └───────┘  └───────┘  └───────┘  └───────┘  │         │
│  └───────────────────────────────────────────────┘         │
└─────────────────────────────────────────────────────────────┘
```

### C API設計（stcpp_*プレフィックス）

詳細は [contracts/c-api.md](./contracts/c-api.md) を参照。

主要関数:

- `stcpp_model_load()` - モデルロード
- `stcpp_model_free()` - モデル解放
- `stcpp_context_new()` - コンテキスト作成
- `stcpp_context_free()` - コンテキスト解放
- `stcpp_generate()` - テキスト生成
- `stcpp_generate_stream()` - ストリーミング生成
- `stcpp_embeddings()` - 埋め込み生成
- `stcpp_tokenize()` - トークン化
- `stcpp_detokenize()` - デトークン化
- `stcpp_lora_load()` - LoRAロード
- `stcpp_lora_apply()` - LoRA適用
- `stcpp_batch_add()` - バッチ追加
- `stcpp_batch_decode()` - バッチ推論

### データモデル

詳細は [data-model.md](./data-model.md) を参照。

## Phase 2: タスク計画アプローチ

**タスク生成戦略**:

- MVP（単一GPU推論+ストリーミング）を最優先
- TDD順序: テストが実装より先
- 依存関係順序: ggmlセットアップ → safetensorsローダー → 推論エンジン → API
- 並列実行可能なタスクに[P]マーク

**フェーズ分割**:

1. **Phase 1: 基盤** - ggmlサブモジュール、CMake、基本構造
2. **Phase 2: ローダー** - safetensorsパーサー、テンソル変換
3. **Phase 3: トークナイザー** - tokenizer.json解析、BPE
4. **Phase 4: 推論MVP** - GPUバックエンド、生成パイプライン
5. **Phase 5-8: 拡張** - KVキャッシュ、batching、埋め込み、LoRA
6. **Phase 9: Nemotron 3** - Mamba SSM、MoE、ハイブリッドアーキテクチャ
7. **Phase 10-12: 仕上げ** - 高度な機能、マルチGPU、E2E、ドキュメント

**推定タスク数**: 64個（Phase 9追加により6タスク増加）

**重要**: 詳細タスクは `/speckit.tasks` コマンドで生成

## 複雑さトラッキング

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|--------------------------------|
| なし | - | - |

## 進捗トラッキング

**フェーズステータス**:

- [x] Phase 0: Research完了（Nemotron 3追加調査含む）
- [x] Phase 1: Design完了
- [x] Phase 2: Task planning完了
- [x] Phase 3: Tasks生成済み（Phase 9: Nemotron 3追加）
- [x] Phase 4-8: 実装完了（MVP達成）
- [ ] Phase 9: Nemotron 3実装中
- [x] Phase 10-12: 実装完了（Nemotron 3以外）
- [ ] Phase 全体: 検証合格（Phase 9待ち）

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み

---
*憲章 v2.1.1 に基づく - `/memory/constitution.md` 参照*
