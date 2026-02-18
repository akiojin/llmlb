# 実装計画: モデル形式選択（safetensors/GGUF）とGGUF選択ポリシー

**機能ID**: `SPEC-a61b24f2`  
**作成日**: 2025-12-21  
**ステータス**: 廃止（2025-12-31）

## 廃止理由
- ロードバランサーでの形式選択/gguf_policyは廃止。
- NodeがHFから直接取得し、実行環境に応じてアーティファクトを選択する方針に統一。

## 代替仕様
- `SPEC-68551ec8`（HF URL登録）
- `SPEC-08d2b908`（モデル管理 統合仕様）
- `SPEC-dcaeaec4` / `SPEC-48678000 (moved to xLLM repo)`（Node主導のモデル解決）
