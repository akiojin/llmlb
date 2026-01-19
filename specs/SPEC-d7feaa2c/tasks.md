# SPEC-d7feaa2c: Tasks

## Note

- 2026-01-19: 動的プラグインシステムを廃止し、モダリティ別マネージャー方式に移行
- 以前のタスク（プラグイン関連）は廃止セクションに移動

---

## Phase 1: マネージャー層の実装

### TextManager

- [ ] T001 TextManagerクラスの定義
  - `allm/include/core/text_manager.h` を作成
  - load_model(), generate() インターフェース定義
- [ ] T002 形式判定ロジックの実装
  - GGUF/safetensors形式を自動判定
  - config.json / ファイル拡張子による判定
- [ ] T003 llama.cpp統合
  - 既存LlamaManagerとの統合
  - GGUFモデルの推論実行
- [ ] T004 safetensors.cpp統合
  - safetensors.cppライブラリとの統合
  - safetensorsモデルの推論実行
- [ ] T005 TextManagerテストの作成
  - 形式判定テスト
  - GGUF/safetensors両形式での推論テスト

### AudioManager

- [ ] T006 AudioManagerクラスの定義
  - 既存whisper_managerをリファクタ
  - `allm/include/core/audio_manager.h` を作成
- [ ] T007 AudioManagerテストの作成
  - 音声認識基本機能テスト

### ImageManager

- [ ] T008 ImageManagerクラスの定義
  - 既存sd_managerをリファクタ
  - `allm/include/core/image_manager.h` を作成
- [ ] T009 ImageManagerテストの作成
  - 画像生成基本機能テスト

---

## Phase 2: プラグインシステムの削除

### コード削除

- [ ] T010 `allm/engines/` ディレクトリの削除
  - llama_cpp/ プラグイン削除
  - safetensors/ プラグイン削除
- [ ] T011 EngineRegistry関連コードの削除
  - `engine_registry.h/cpp` 削除
  - 関連する参照の削除
- [ ] T012 EngineHost関連コードの削除
  - `engine_host.h/cpp` 削除
  - プラグインローディング関連コード削除
- [ ] T013 プラグインABI定義の削除
  - `engine_plugin_api.h` 削除
  - manifest.json関連コード削除

### ビルド更新

- [ ] T014 CMakeLists.txtの更新
  - engines/ サブディレクトリ参照削除
  - プラグイン関連ビルド設定削除
- [ ] T015 ビルド確認
  - 全ターゲットのビルド成功確認
  - テスト実行確認

---

## Phase 3: API層の整備

### Responses API

- [ ] T016 Responses APIエンドポイントの実装
  - `/v1/responses` エンドポイント追加
  - リクエスト/レスポンス形式の実装
- [ ] T017 TextManagerとの統合
  - Responses API → TextManager呼び出し
- [ ] T018 Responses APIテストの作成
  - 基本リクエスト/レスポンステスト
  - ストリーミングテスト

### Chat Completion API（後方互換）

- [ ] T019 Chat Completion APIの維持確認
  - 既存機能の動作確認
  - TextManagerとの統合確認
- [ ] T020 Chat Completion APIテストの更新
  - マネージャー方式での動作確認

---

## Phase 4: safetensors.cppアーキテクチャ対応

### 対応済みアーキテクチャ

- [x] Llama
- [x] Mistral
- [x] Qwen/Qwen2
- [x] Phi
- [x] Gemma
- [x] Nemotron (2026-01-19 完了)
- [x] GPT-OSS

### 追加予定

- [ ] T021 GLMアーキテクチャ対応
  - config.json model_type: "glm"
  - テンソル名マッピング実装

---

## Tests

### Unit Tests

- [ ] T022 TextManager形式判定テスト
- [ ] T023 AudioManager基本機能テスト
- [ ] T024 ImageManager基本機能テスト

### Integration Tests

- [ ] T025 GGUF形式でのE2Eテスト
- [ ] T026 safetensors形式でのE2Eテスト
- [ ] T027 Responses API E2Eテスト
- [ ] T028 Chat Completion API E2Eテスト

### 削除確認

- [ ] T029 プラグイン関連コードが存在しないことの確認
- [ ] T030 `allm/engines/`が削除されていることの確認

---

## Docs

- [ ] T031 SPEC-d7feaa2cの更新完了確認
- [ ] T032 CLAUDE.mdの更新（必要に応じて）

---

## 廃止タスク（2026-01-19以前）

以下のタスクは動的プラグインシステム廃止により不要：

- ~~EngineHost（プラグインローダー）を導入する~~
- ~~Engine ABI/manifest の必須項目・ABI一致を検証する~~
- ~~llama.cpp を plugin 化してロードできるようにする~~
- ~~プラグインのシャドウロード機能を実装~~
- ~~プラグイン manifest 検証テスト~~
- ~~ホットリロードテスト~~
- ~~サードパーティプラグインサポート~~

---

## 変更履歴

### 2026-01-19

- 動的プラグインシステム関連タスクを廃止
- モダリティ別マネージャー方式のタスクを新規追加
- Responses API関連タスクを追加
