# 実装計画: safetensors量子化対応

**機能ID**: `SPEC-afa11b7d` | **日付**: 2026-01-27 | **仕様**: [spec.md](./spec.md)
**入力**: `/specs/SPEC-afa11b7d/spec.md` の機能仕様

## 実行フロー (/speckit.plan コマンドのスコープ)
```
1. 入力パスから機能仕様を読み込み
2. 技術コンテキストを記入
3. 憲章チェックセクションを評価
4. Phase 0 を実行 → research.md
5. Phase 1 を実行 → contracts, data-model.md, quickstart.md
6. 憲章チェックセクションを再評価
7. Phase 2 を計画 → タスク生成アプローチを記述
8. 停止 - /speckit.tasks コマンドの準備完了
```

## 概要

safetensors形式モデルに対して量子化設定を指定・適用できるようにし、VRAM使用量を抑えつつ推論を可能にする。
GGUFの挙動は維持し、safetensors_cppエンジンでのみ量子化を扱う。

**主要要件**:
- safetensorsモデル向けの量子化方式を指定できる
- 量子化の状態を一覧/詳細で確認できる
- 未対応の量子化は明確なエラーで拒否する
- GGUFの挙動は変更しない

**技術アプローチ**:
- safetensors.cppの量子化機構に合わせた設定をxllm側で受け渡す
- 量子化の指定経路（CLI/API/設定）を整理し、モデルメタデータに反映する
- 既存のsafetensors_cppエンジンのロードフローに量子化設定を統合する

## 技術コンテキスト

**言語/バージョン**: C++17
**主要依存関係**:
- safetensors.cpp（xllm/third_party）
- ggml（safetensors.cppの依存）
- xllm core / engine registry
**ストレージ**: ファイルシステム（モデルディレクトリ、manifest.json）
**テスト**: xllm unit/integration/contract テスト
**対象プラットフォーム**: macOS (Metal), Windows/Linux (CUDA), Linux (ROCm), Vulkan
**プロジェクトタイプ**: single（xllm）
**パフォーマンス目標**: 量子化によりVRAM使用量を低減し、推論成功率を上げる
**制約**:
- safetensors_cppはGPUバックエンド必須（CPUは非対応）
- 量子化導入でGGUFの動作を変更しない
- 量子化方式の未対応は明確に拒否
**スケール/スコープ**: safetensorsモデルの量子化対応に限定

## 憲章チェック
*ゲート: Phase 0 research前に合格必須。Phase 1 design後に再チェック。*

**シンプルさ**:
- プロジェクト数: 1（xllm）
- フレームワークを直接使用? ✅ Yes（既存のsafetensors_cppエンジン）
- 単一データモデル? ✅ Yes（既存のModelDescriptor/metadataを拡張）
- パターン回避? ✅ Yes（新規レイヤー追加を避ける）

**アーキテクチャ**:
- すべての機能をライブラリとして? ✅ Yes（xllmエンジン内）
- ライブラリリスト: xllm（safetensors_cpp, model_storage, inference_engine）
- ライブラリごとのCLI: xllm CLI（既存）
- ライブラリドキュメント: specs/SPEC-afa11b7d/

**テスト (妥協不可)**:
- RED-GREEN-Refactorサイクルを強制? ✅ Yes
- Gitコミットはテストが実装より先に表示? ✅ Yes
- 順序: Contract→Integration→E2E→Unitを厳密に遵守? ✅ Yes
- 実依存関係を使用? ✅ Yes（safetensors.cpp実装）
- Integration testの対象: 量子化指定の解決、ロード、推論
- 禁止: テスト前の実装、REDフェーズのスキップ ✅

**可観測性**:
- 構造化ロギング含む? ✅ Yes（既存ログ）
- エラーコンテキスト十分? ✅ Yes（量子化未対応理由を明示）

**バージョニング**:
- バージョン番号割り当て済み? ✅ Yes（既存のリリースフロー）
- 破壊的変更を処理? ✅ Yes（既存APIの互換性維持）

## プロジェクト構造

### ドキュメント (この機能)
```
specs/SPEC-afa11b7d/
├── spec.md              # 機能仕様
├── plan.md              # このファイル (/speckit.plan)
├── research.md          # Phase 0 出力
├── data-model.md        # Phase 1 出力
├── quickstart.md        # Phase 1 出力
├── contracts/           # Phase 1 出力
│   └── cli.md           # CLI/表示の契約
└── tasks.md             # Phase 2 出力 (/speckit.tasks)
```

### ソースコード (リポジトリルート)
```
xllm/
├── engines/safetensors/         # safetensors_cppエンジン
├── src/models/model_storage.cpp # モデル解決とメタデータ
├── src/core/inference_engine.cpp# エンジン解決/ロード
├── src/api/openai_endpoints.cpp # モデル名パース/メタ情報
├── src/utils/cli.cpp            # CLIオプション/ヘルプ
└── src/cli/commands/            # pull/show/list など
```

**構造決定**: 既存のxllm構造内で完結し、新規サブプロジェクトは追加しない。

## Phase 0: リサーチ

- safetensors.cpp側で利用可能な量子化方式の洗い出し
- 量子化方式ごとのGPUバックエンド制約の確認
- 量子化の指定経路（CLI/API/設定ファイル/モデル名）の優先順位整理
- 量子化設定とモデルアーキテクチャの整合条件の定義
- 量子化適用時のVRAM計測方法の確定

詳細は [research.md](./research.md) を参照。

## Phase 1: 設計

### 主要コンポーネント

- **量子化指定の解決**: 量子化設定の入力経路を整理し、ModelDescriptorへ反映
- **safetensors_cpp拡張**: 量子化設定をsafetensors.cppに引き渡す
- **メタデータ可視化**: list/showで量子化状態を表示
- **エラー設計**: 未対応方式や不整合を明示的に拒否

### データモデル

- 量子化設定の正規化とメタデータ格納方針を定義
- 詳細は [data-model.md](./data-model.md) を参照

### 契約

- CLI表示およびモデルメタ情報の契約を定義
- 詳細は [contracts/cli.md](./contracts/cli.md) を参照

### テスト方針

- Contract: 量子化指定の入力/表示の契約
- Integration: 量子化指定 → ロード → 推論成功/失敗
- E2E: 実モデルでの量子化有効時の推論確認（可能な範囲）

## Phase 2: タスク分割方針

- Setup: 量子化方式のリサーチ結果反映
- Contract: CLI/APIの契約追加
- Core: safetensors_cppへの設定伝播、メタデータ更新
- Integration/E2E: 実モデル検証とVRAM差分確認
- Docs/Polish: quickstart、エラーメッセージ整備

**重要**: 詳細タスクは `/speckit.tasks` コマンドで生成する。
