# 実装計画: LLM runtimeモデルストレージ形式サポート

**機能ID**: `SPEC-0c4f3e5c` | **日付**: 2025-12-23 | **仕様**: [spec.md](./spec.md)
**ステータス**: 計画中（SPEC-dcaeaec4により方針変更）

## 概要

C++ NodeのLlamaManagerがLLM runtimeのネイティブモデルストレージ形式（blobファイル）を
認識・ロードできるようにする。ただし、SPEC-dcaeaec4にて独自ストレージ形式への移行が
決定されたため、本機能は廃止予定。

## 技術コンテキスト

**言語/バージョン**: C++17
**主要依存関係**: llama.cpp, nlohmann/json
**ストレージ**: ファイルシステム (`~/.runtime/models/`)
**テスト**: Google Test
**対象プラットフォーム**: Linux/macOS
**プロジェクトタイプ**: single (node/)

## 憲章チェック

**シンプルさ**: ⚠️ 要検討

- 外部アプリ（LLM runtime）のストレージ形式に依存
- manifest/blob解析ロジックが複雑
- SPEC-dcaeaec4で独自形式へ移行予定のため、本機能は廃止方針

**テスト**: ✅ 合格

- TDD要件を満たすテストケース定義済み
- isLLM runtimeBlobFile, loadModel, resolveModelPath のテスト

## 方針変更

SPEC-dcaeaec4「LLM-Load Balancer独自モデルストレージ」にて以下の決定:

- LLM runtime固有形式への暗黙フォールバックは禁止
- 独自ディレクトリ構造 `~/.llmlb/models/` を採用
- manifest/blob解析ロジックは削除対象

**結論**: 本SPECは実装せず、SPEC-dcaeaec4に統合

## 進捗トラッキング

**フェーズステータス**:

- [x] Phase 0: Research完了
- [x] Phase 1: Design完了（方針変更により廃止決定）
- [ ] Phase 2: Task planning（実施しない）
- [ ] Phase 3-5: （実施しない）

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 方針変更により廃止
- [x] すべての要明確化解決済み
- [x] 複雑さの逸脱を文書化済み

---

*憲章 v1.0.0 に基づく - `/memory/constitution.md` 参照*
