# 実装計画: Nemotron CUDA PoC

**機能ID**: `SPEC-83825900` | **日付**: 2025-12-24 | **仕様**: [spec.md](./spec.md)
**入力**: `/specs/SPEC-83825900/spec.md`の機能仕様

## 概要

safetensors形式のNemotronモデルをllama.cppに依存せずにCUDAで直接ロード・推論し、テキスト生成が成立することを検証するPoC。既存のSPEC-efff1da7（safetensors mmap PoC）を基盤とし、CUDA演算パスを追加する。

**主要要件**:

- FR-001〜008: safetensors → CUDAロード → テキスト生成（1トークン以上）
- 段階的検証: Nemotron-Mini（4B）→ Nemotron-Medium（15B）
- 配置場所: `poc/nemotron-cuda-cpp/`

## 技術コンテキスト

**言語/バージョン**: C++17, CUDA 12.x
**主要依存関係**: safetensors-cpp（ヘッダーオンリー）, cuBLAS, CUDA Runtime
**ストレージ**: N/A（ファイルシステム直接アクセス）
**テスト**: 手動実行による動作検証（PoCのため自動テストは最小限）
**対象プラットフォーム**: Linux (x86_64), Windows (x86_64) - CUDA対応GPU必須
**プロジェクトタイプ**: single（独立PoC）
**パフォーマンス目標**: トークン生成速度を測定（目標値は設定せず、ベースライン取得が目的）
**制約**: GPUメモリに収まるモデルのみ（Mini: 8GB+, Medium: 24GB+）
**スケール/スコープ**: 単一GPU、単一モデル、単一セッション

## 憲章チェック

**シンプルさ**:

- プロジェクト数: 1（poc/nemotron-cuda-cpp）✓
- フレームワークを直接使用? ✓（CUDA Runtime直接使用）
- 単一データモデル? ✓（safetensors → GPU tensor）
- パターン回避? ✓（最小限の抽象化）

**アーキテクチャ**:

- すべての機能をライブラリとして? N/A（PoCのため単一実行ファイル）
- ライブラリリスト: N/A
- ライブラリごとのCLI: N/A
- ライブラリドキュメント: READMEのみ

**テスト (PoCのため簡略化)**:

- RED-GREEN-Refactorサイクル: 手動検証（PoCフェーズ）
- 順序: 動作検証優先
- 実依存関係を使用: ✓（実GPU、実モデル）

**可観測性**:

- 構造化ロギング: 標準出力へのログ出力
- エラーコンテキスト: CUDAエラーコード + 説明

**バージョニング**: N/A（PoCのためバージョン管理対象外）

## プロジェクト構造

### ドキュメント (この機能)

```text
specs/SPEC-83825900/
├── spec.md              # 機能仕様
├── plan.md              # このファイル
├── research.md          # Phase 0 出力
└── tasks.md             # Phase 2 出力
```

### ソースコード (リポジトリルート)

```text
poc/nemotron-cuda-cpp/
├── CMakeLists.txt       # CUDAビルド設定
├── README.md            # ビルド・実行手順
├── src/
│   ├── main.cpp         # エントリポイント
│   ├── safetensors_loader.cpp  # safetensors読み込み
│   ├── cuda_memory.cu   # CUDAメモリ管理
│   ├── transformer.cu   # Transformer演算（Attention, FFN）
│   ├── tokenizer.cpp    # トークナイザー（簡易実装）
│   └── inference.cpp    # 推論ループ
├── include/
│   ├── safetensors.hh   # safetensors-cpp（コピー）
│   └── *.h              # 各モジュールヘッダー
└── test/
    └── test_basic.cpp   # 基本動作テスト
```

## Phase 0: アウトライン＆リサーチ

### リサーチタスク

1. **Nemotronアーキテクチャ調査**
   - config.jsonからのパラメータ抽出方法
   - レイヤー構成（Attention, MLP, LayerNorm）
   - MoE（Mixture of Experts）の有無と処理方法

2. **CUDA演算パターン調査**
   - BF16/FP16テンソルのGPUロード方法
   - cuBLASによるGEMM演算
   - Attention演算の実装パターン（Flash Attention等）

3. **トークナイザー処理調査**
   - tokenizer.jsonの解析方法
   - BPE/SentencePieceのC++実装オプション

4. **既存実装の活用調査**
   - poc/nemotron-safetensors-cpp: safetensors mmapロード
   - node/engines/: NemotronEngine検証コード

**出力**: research.md

## Phase 1: 設計＆契約

### データモデル

```text
┌─────────────────────────────────────────────────────────────┐
│ SafetensorsFile                                             │
│ ├── index.json (シャーディング時)                            │
│ └── *.safetensors (1つ以上のシャード)                        │
└─────────────────────────────────────────────────────────────┘
                         ↓ mmap + 解析
┌─────────────────────────────────────────────────────────────┐
│ ModelWeights (CPU)                                          │
│ ├── embed_tokens [vocab_size, hidden_dim]                   │
│ ├── layers[] (N layers)                                     │
│ │   ├── self_attn.{q,k,v,o}_proj                            │
│ │   ├── mlp.{gate,up,down}_proj                             │
│ │   └── input/post_attention_layernorm                      │
│ └── lm_head [hidden_dim, vocab_size]                        │
└─────────────────────────────────────────────────────────────┘
                         ↓ cudaMemcpy
┌─────────────────────────────────────────────────────────────┐
│ CUDAModel (GPU)                                             │
│ ├── d_embed_tokens                                          │
│ ├── d_layers[]                                              │
│ └── d_lm_head                                               │
└─────────────────────────────────────────────────────────────┘
```

### 推論フロー

```text
Input: "Hello"
    ↓
[Tokenizer] → token_ids: [1, 15043]
    ↓
[Embedding Lookup] → hidden_states [seq_len, hidden_dim]
    ↓
[Transformer Layers x N]
    ├── LayerNorm
    ├── Self-Attention (Q, K, V projections + scaled dot-product)
    ├── Residual Add
    ├── LayerNorm
    ├── MLP (gate_proj, up_proj, down_proj + SiLU)
    └── Residual Add
    ↓
[Final LayerNorm]
    ↓
[LM Head] → logits [vocab_size]
    ↓
[Sampling] → next_token_id
    ↓
[Detokenize] → "World"
```

### CLIインターフェース

```bash
# 基本実行
./nemotron-cuda-poc --model /path/to/nemotron-mini --prompt "Hello"

# オプション
--model PATH      # モデルディレクトリ（必須）
--prompt TEXT     # 入力プロンプト（必須）
--max-tokens N    # 最大生成トークン数（デフォルト: 100）
--device N        # CUDAデバイスID（デフォルト: 0）
--verbose         # 詳細ログ出力
```

**出力**: data-model.md, quickstart.md

## Phase 2: タスク計画アプローチ

**タスク生成戦略**:

1. **Setup**: ディレクトリ作成、CMakeLists.txt、依存関係配置
2. **Core - Loader**: safetensors読み込み + CUDAメモリ転送
3. **Core - Tokenizer**: 簡易トークナイザー実装
4. **Core - Inference**: Transformer演算 + 生成ループ
5. **Integration**: E2Eテスト（Nemotron-Mini）
6. **Validation**: Nemotron-Medium検証
7. **Docs**: README、実行手順

**順序戦略**:

- 依存関係順: Loader → Tokenizer → Inference
- 段階的検証: Mini → Medium

**推定出力**: tasks.mdに15-20個の番号付き、順序付きタスク

## 複雑さトラッキング

| 違反 | 必要な理由 | より単純な代替案が却下された理由 |
|------|-----------|--------------------------------|
| なし | - | - |

## 進捗トラッキング

**フェーズステータス**:

- [x] Phase 0: Research完了
- [x] Phase 1: Design完了
- [x] Phase 2: Task planning完了（アプローチのみ記述）
- [x] Phase 3: Tasks生成済み (/speckit.tasks コマンド)
- [ ] Phase 4: 実装中 ← **現在**
- [ ] Phase 5: 検証合格

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み（なし）

## 実装進捗（Phase 4）

**最終更新**: 2025-12-24

| フェーズ | タスク範囲 | 完了 | 状態 |
|---------|-----------|------|------|
| 3.1 セットアップ | T001-T005 | 0/5 | 未着手 |
| 3.2 コアローダー | T006-T011 | 0/6 | 未着手 |
| 3.3 CUDAカーネル | T012-T017 | 0/6 | 未着手 |
| 3.4 トークナイザー | T018-T019 | 0/2 | 未着手 |
| 3.5 Transformerレイヤー | T020-T022 | 0/3 | 未着手 |
| 3.6 推論ループ | T023-T026 | 0/4 | 未着手 |
| 3.7 メインエントリ | T027-T029 | 0/3 | 未着手 |
| 3.8 統合テスト(Mini) | T030-T033 | 0/4 | 未着手 |
| 3.9 拡張検証(Medium) | T034-T035 | 0/2 | 未着手 |
| 3.10 ドキュメント | T036-T037 | 0/2 | 未着手 |
| **合計** | T001-T037 | **0/37** | **0%** |

**次のアクション**: T001から順次実装開始

---
*憲章 v1.0.0 に基づく - `/memory/constitution.md` 参照*
