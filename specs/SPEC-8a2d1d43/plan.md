# 実装計画: gptossアーキテクチャエイリアスサポート

**機能ID**: `SPEC-8a2d1d43` | **日付**: 2025-12-23 | **仕様**: [spec.md](./spec.md)
**ステータス**: 実装済み（事後文書化）

## 概要

llama.cppが`gptoss`アーキテクチャ名（ハイフンなし）を認識できるようにする。
LLM runtimeでプルしたgpt-ossモデルをC++ Nodeで使用可能にする。

## 技術コンテキスト

**言語/バージョン**: C++17
**主要依存関係**: llama.cpp (フォーク版)
**ストレージ**: N/A
**テスト**: 手動テスト（モデルロード確認）
**対象プラットフォーム**: Linux/macOS
**プロジェクトタイプ**: single (node/third_party/llama.cpp)

## 憲章チェック

**シンプルさ**: ✅ 合格

- 最小限の変更（アーキテクチャ名マッピング追加のみ）
- 後方互換性維持

**テスト**: ✅ 合格（手動検証）

- gptoss/gpt-oss両方のモデルがロードできることを確認

## 実装済み変更

### llama-arch.cpp

```cpp
// 修正前
{ LLM_ARCH_OPENAI_MOE,       "gpt-oss"          },

// 修正後
{ LLM_ARCH_OPENAI_MOE,       "gptoss"           },
// + llm_arch_from_string で "gpt-oss" エイリアスも認識
```

### 追加テンソル

- `LLM_TENSOR_ATTN_POST_NORM`
- `LLM_TENSOR_ATTN_SINKS`
- バイアステンソル（bq, bk, bv, bo, ffn_*_b）

### グラフビルダー

- `llm_build_openai_moe_iswa` 使用
- SWAパターン設定追加

## 進捗トラッキング

**フェーズステータス**:

- [x] Phase 0: Research完了
- [x] Phase 1: Design完了
- [x] Phase 2: Task planning完了
- [x] Phase 3: Tasks生成済み
- [x] Phase 4: 実装完了
- [x] Phase 5: 検証合格

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み (なし)

---

*憲章 v1.0.0 に基づく - `/memory/constitution.md` 参照*
