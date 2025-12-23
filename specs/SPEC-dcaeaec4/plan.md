# 実装計画: LLM-Router独自モデルストレージ

**機能ID**: `SPEC-dcaeaec4` | **日付**: 2025-12-23 | **仕様**: [spec.md](./spec.md)
**ステータス**: 計画中

## 概要

llm-nodeがモデルファイルを `~/.llm-router/models/` 配下から読み込むことを基本としつつ、
ルーターが返す配布情報（共有パス or ダウンロードURL）を優先利用する。
LLM runtime固有のストレージ形式への暗黙フォールバックは撤廃する。

## 技術コンテキスト

**言語/バージョン**: C++17, Rust 1.75+
**主要依存関係**: llama.cpp, Axum
**ストレージ**: ファイルシステム (`~/.llm-router/models/`)
**テスト**: Google Test, cargo test
**対象プラットフォーム**: Linux/macOS
**プロジェクトタイプ**: web (node/, router/)

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
~/.llm-router/
├── config.json          # 設定ファイル
├── router.db            # ルーターDB（SQLite）
└── models/
    ├── gpt-oss-20b/
    │   ├── model.gguf   # モデルファイル
    │   └── metadata.json # (optional)
    └── qwen3-coder-30b/
        └── model.gguf
```

## GGUFファイル解決フロー

```text
1. ローカル ~/.llm-router/models/<name>/model.gguf を探す → あれば採用
2. ルーター /v0/models の path が存在し読み取り可能 → 直接使用（共有ストレージ）
3. path 不可 → ルーター /v0/models/blob/:model_name からダウンロード
4. いずれも不可 → download_url からダウンロード
5. いずれも不可 → エラー
```

## Phase 2: タスク計画アプローチ

**タスク生成戦略**:

1. runtime_compat.cpp → model_storage.cpp リネーム
2. LLM runtime manifest/blob解析ロジック削除
3. 独自ディレクトリ構造実装
4. ルーターAPI連携（/v0/models, /v0/models/blob）
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
