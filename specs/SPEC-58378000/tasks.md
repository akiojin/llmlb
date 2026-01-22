# タスク: llmlb CLIコマンド

**入力**: `/specs/SPEC-58378000/`の設計ドキュメント
**前提条件**: plan.md, research.md, data-model.md, contracts/cli-commands.md, quickstart.md

## 実行フロー

```text
1. 機能ディレクトリからplan.mdを読み込み ✓
2. オプション設計ドキュメントを読み込み:
   → data-model.md: 7エンティティ抽出
   → contracts/cli-commands.md: 11コマンド仕様
   → research.md: 9つの技術決定
   → quickstart.md: 4つの検証シナリオ
3. カテゴリ別にタスク生成完了
4. タスクルール適用完了
5. 検証チェックリスト合格
```

## フォーマット: `[ID] [P?] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)
- 説明には正確なファイルパスを含める

## Phase 3.1: セットアップ ✅

- [x] T001 `node/src/cli/` ディレクトリ構造を作成 (commands/, headers)
- [x] T002 `node/CMakeLists.txt` にcli/ソースファイルを追加
- [x] T003 [P] `node/include/utils/cli.h` にサブコマンド定義を追加

## Phase 3.2: テストファースト (TDD) ⚠️ 3.3の前に完了必須

**重要: これらのテストは記述され、実装前に失敗する必要がある**

### Contract Tests (CLIコマンド仕様) ✅ RED phase完了

- [x] T004 [P] `node/tests/contract/cli_serve_test.cpp` に `node serve` の contract test
- [x] T005 [P] `node/tests/contract/cli_run_test.cpp` に `node run` の contract test
- [x] T006 [P] `node/tests/contract/cli_pull_test.cpp` に `node pull` の contract test
- [x] T007 [P] `node/tests/contract/cli_list_test.cpp` に `node list` の contract test
- [x] T008 [P] `node/tests/contract/cli_show_test.cpp` に `node show` の contract test
- [x] T009 [P] `node/tests/contract/cli_rm_test.cpp` に `node rm` の contract test
- [x] T010 [P] `node/tests/contract/cli_stop_test.cpp` に `node stop` の contract test
- [x] T011 [P] `node/tests/contract/cli_ps_test.cpp` に `node ps` の contract test

### Integration Tests (ユーザーストーリー) ✅ RED phase完了

- [x] T012 [P] `node/tests/integration/cli_pull_list_test.cpp` にpull→list検証シナリオ
- [x] T013 [P] `node/tests/integration/cli_repl_test.cpp` にrun→prompt→/bye検証シナリオ
- [x] T014 [P] `node/tests/integration/cli_model_lifecycle_test.cpp` にshow→rm→list検証シナリオ
- [x] T015 [P] `node/tests/integration/cli_server_test.cpp` にserve+Ctrl+Cグレースフル終了シナリオ

## Phase 3.3: コア実装 (テストが失敗した後のみ)

### データモデル ✅

- [x] T016 [P] `node/include/cli/models/model_info.h` にModel構造体
- [x] T017 [P] `node/include/cli/models/node_info.h` にNode構造体
- [x] T018 [P] `node/include/cli/models/session.h` にSession/Message/SessionSettings構造体
- [x] T019 [P] `node/include/cli/models/download_progress.h` にDownloadProgress構造体
- [x] T020 [P] `node/include/cli/models/ollama_model.h` にOllamaModel構造体

### コアコンポーネント

- [x] T021 `node/src/utils/cli.cpp` にサブコマンドパーサー実装
- [x] T022 `node/src/main.cpp` にサブコマンド分岐ロジック追加
- [x] T023 [P] `node/include/cli/cli_client.h` + `node/src/cli/cli_client.cpp` にCLIClient
- [x] T024 [P] `node/include/cli/repl_session.h` + `node/src/cli/repl_session.cpp` にREPLSession
- [x] T025 [P] `node/include/cli/progress_renderer.h` + `node/src/cli/progress_renderer.cpp` にProgressRenderer
- [x] T026 [P] `node/include/cli/ollama_compat.h` + `node/src/cli/ollama_compat.cpp` にOllamaCompat

### Node サブコマンド実装

- [x] T027 [P] `node/src/cli/commands/serve.cpp` に `node serve` 実装
- [x] T028 [P] `node/src/cli/commands/run.cpp` に `node run` 実装 (REPL, --think, vision)
- [x] T029 [P] `node/src/cli/commands/pull.cpp` に `node pull` 実装 (HF download, progress)
- [x] T030 [P] `node/src/cli/commands/list.cpp` に `node list` 実装 (ollama参照含む)
- [x] T031 [P] `node/src/cli/commands/show.cpp` に `node show` 実装 (--license等オプション)
- [x] T032 [P] `node/src/cli/commands/rm.cpp` に `node rm` 実装
- [x] T033 [P] `node/src/cli/commands/stop.cpp` に `node stop` 実装
- [x] T034 [P] `node/src/cli/commands/ps.cpp` に `node ps` 実装 (VRAM, TEMP表示)

### Load Balancer サブコマンド実装

- [x] T035 [P] `node/src/cli/commands/router_nodes.cpp` に `router nodes` 実装
- [x] T036 [P] `node/src/cli/commands/router_models.cpp` に `router models` 実装
- [x] T037 [P] `node/src/cli/commands/router_status.cpp` に `router status` 実装

## Phase 3.4: 統合 ✅

- [x] T038 CLIClientをModelStorage/ModelSyncに接続
- [x] T039 REPLSessionを`/v1/chat/completions`エンドポイントに接続
- [x] T040 OllamaCompatを`~/.ollama/models/`に接続
- [x] T041 環境変数処理の統合 (LLMLB_HOST, LLMLB_DEBUG, HF_TOKEN)
- [x] T042 エラーハンドリングと終了コード統一 (0/1/2)

## Phase 3.5: 仕上げ ✅

### Unit Tests ✅

- [x] T043 [P] `node/tests/unit/cli_test.cpp` にサブコマンド解析の unit tests (既存)
- [x] T044 [P] `node/tests/unit/progress_renderer_test.cpp` にプログレス表示の unit tests
- [x] T045 [P] `node/tests/unit/ollama_compat_test.cpp` にollama参照の unit tests
- [x] T046 [P] `node/tests/unit/repl_session_test.cpp` にREPL処理の unit tests

### ドキュメント・検証 ✅

- [x] T047 [P] quickstart.mdの検証シナリオを手動実行 (サーバー起動・統合テスト完了)
- [x] T048 --helpメッセージの確認と修正 (既存実装で対応)
- [x] T049 エラーメッセージの英語確認 (実装時に英語で統一済み)

## 依存関係

```text
Setup (T001-T003)
    ↓
Contract Tests (T004-T011) [並列可能]
    ↓
Integration Tests (T012-T015) [並列可能]
    ↓
Data Models (T016-T020) [並列可能]
    ↓
Core Components:
  - T021 (parser) → T022 (main分岐)
  - T023-T026 [並列可能]
    ↓
Node Commands (T027-T034) [並列可能]
Load Balancer Commands (T035-T037) [並列可能]
    ↓
Integration (T038-T042) [順次]
    ↓
Polish (T043-T049) [並列可能]
```

## 並列実行例

```text
# Phase 3.2 Contract Tests を一緒に起動:
Task: "node/tests/contract/cli_serve_test.cpp に node serve の contract test"
Task: "node/tests/contract/cli_run_test.cpp に node run の contract test"
Task: "node/tests/contract/cli_pull_test.cpp に node pull の contract test"
Task: "node/tests/contract/cli_list_test.cpp に node list の contract test"
Task: "node/tests/contract/cli_show_test.cpp に node show の contract test"
Task: "node/tests/contract/cli_rm_test.cpp に node rm の contract test"
Task: "node/tests/contract/cli_stop_test.cpp に node stop の contract test"
Task: "node/tests/contract/cli_ps_test.cpp に node ps の contract test"

# Phase 3.3 Commands を一緒に起動:
Task: "node/src/cli/commands/serve.cpp に node serve 実装"
Task: "node/src/cli/commands/run.cpp に node run 実装"
Task: "node/src/cli/commands/pull.cpp に node pull 実装"
Task: "node/src/cli/commands/list.cpp に node list 実装"
Task: "node/src/cli/commands/show.cpp に node show 実装"
Task: "node/src/cli/commands/rm.cpp に node rm 実装"
Task: "node/src/cli/commands/stop.cpp に node stop 実装"
Task: "node/src/cli/commands/ps.cpp に node ps 実装"
```

## 注意事項

- [P] タスク = 異なるファイル、依存関係なし
- 実装前にテストが失敗することを確認 (TDD RED)
- 各タスク後にコミット
- 回避: 曖昧なタスク、同じファイルの競合
- 終了コード: 0=成功, 1=一般エラー, 2=接続エラー

## 検証チェックリスト

- [x] すべてのcontractsに対応するテストがある (T004-T011: 8コマンド)
- [x] すべてのentitiesにmodelタスクがある (T016-T020: 7エンティティ)
- [x] すべてのテストが実装より先にある (Phase 3.2 → Phase 3.3)
- [x] 並列タスクは本当に独立している (異なるファイル)
- [x] 各タスクは正確なファイルパスを指定
- [x] 同じファイルを変更する[P]タスクがない

## サマリー

| カテゴリ | タスク数 | 並列可能 |
|---------|---------|---------|
| Setup | 3 | 1 |
| Contract Tests | 8 | 8 |
| Integration Tests | 4 | 4 |
| Data Models | 5 | 5 |
| Core Components | 6 | 4 |
| Node Commands | 8 | 8 |
| Load Balancer Commands | 3 | 3 |
| Integration | 5 | 0 |
| Polish | 7 | 5 |
| **合計** | **49** | **38** |
