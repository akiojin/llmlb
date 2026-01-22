# 実装計画: LLM-Load Balancer独自モデルストレージ

**機能ID**: `SPEC-dcaeaec4` | **日付**: 2025-12-23 | **仕様**: [spec.md](./spec.md)
**ステータス**: 計画中

## 概要

xllmがモデルファイルを `~/.llmlb/models/` 配下から読み込むことを基本としつつ、
ロードバランサーは**登録情報とマニフェストのみ**を提供する。
モデルバイナリは保持せず、NodeがHF等の外部ソースから**直接ダウンロード**してキャッシュする。
LLM runtime固有のストレージ形式への暗黙フォールバックは撤廃する。

## 技術コンテキスト

**言語/バージョン**: C++17, Rust 1.75+  
**主要依存関係**: llama.cpp, Axum  
**ストレージ**: ファイルシステム (`~/.llmlb/models/`)  
**テスト**: Google Test, cargo test  
**対象プラットフォーム**: Linux/macOS  
**プロジェクトタイプ**: web (node/, llmlb/)

## 憲章チェック

**シンプルさ**: ✅ 合格

- プロジェクト数: 2 (node, router)
- シンプルなディレクトリ構造（manifest/blob不要）
- 外部アプリ依存の撤廃

**テスト**: ✅ 合格

- TDD順序: Contract→Integration→E2E→Unit
- モック/一時ディレクトリ使用

## ディレクトリ構造

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
    │   └── model.metal.bin  # (optional)
    └── qwen3-coder-30b/
        └── model.gguf
```

## モデル解決フロー（Node主導）

```text
1. ローカル ~/.llmlb/models/<name>/ を確認（必要アーティファクトが揃っていれば採用）
2. ロードバランサーのマニフェストを取得（/v0/models/registry/:model/manifest.json）
3. Nodeがruntime/GPU要件に合うアーティファクトを選択
4. HF等の外部ソースから直接ダウンロードして保存
5. いずれも不可 → エラー
```

## Phase 2: タスク計画アプローチ

**タスク生成戦略**:

1. runtime_compat.cpp → model_storage.cpp リネーム
2. LLM runtime manifest/blob解析ロジック削除
3. 独自ディレクトリ構造実装
4. ロードバランサーAPI連携（/v0/models, manifest）
5. ノード起動時同期ロジック
6. プッシュ通知受信ハンドラ
7. 統合テスト

**順序戦略**:

- まず削除（LLM runtime依存）
- 次に新規実装（独自ストレージ）
- 最後に統合テスト

## 変更対象ファイル

| ファイル | 変更内容 |
|----------|----------|
| `node/src/models/runtime_compat.cpp` | `model_storage.cpp` にリネーム |
| `node/include/models/runtime_compat.h` | `model_storage.h` にリネーム |
| `node/src/utils/config.cpp` | デフォルトパス変更 |
| `node/src/utils/cli.cpp` | ヘルプメッセージ更新 |
| `node/src/main.cpp` | クラス名変更に対応 |

## 進捗トラッキング

**フェーズステータス**:

- [x] Phase 0: Research完了
- [x] Phase 1: Design完了
- [x] Phase 2: Task planning完了
- [ ] Phase 3: Tasks生成済み
- [ ] Phase 4: 実装完了
- [ ] Phase 5: 検証合格

**ゲートステータス**:

- [x] 初期憲章チェック: 合格
- [x] 設計後憲章チェック: 合格
- [x] すべての要明確化解決済み
- [ ] 複雑さの逸脱を文書化済み

---

*憲章 v1.0.0 に基づく - `/memory/constitution.md` 参照*
