# タスク: CLI インターフェース整備

**入力**: `/specs/SPEC-a7e6d40a/`の設計ドキュメント
**前提条件**: plan.md (必須)
**更新日**: 2025-12-10

## フォーマット: `[ID] [P?] 説明`

- **[P]**: 並列実行可能 (異なるファイル、依存関係なし)
- 説明には正確なファイルパスを含める

## Phase 3.1: セットアップ

- [x] T001 `llmlb/Cargo.toml` に clap 依存関係を追加
  - `clap = { version = "4", features = ["derive", "env"] }`
- [x] T002 `llmlb/src/cli/mod.rs` にCLIモジュールの基本構造を作成
  - Cliアーキタイプ構造体（clap derive）
  - ~~Commands enum（None/User）~~ → 廃止（Phase 4で削除）
- [x] ~~T003~~ **廃止** `llmlb/src/cli/user.rs` - Phase 4で完全削除
- [x] T004 `llmlb/src/lib.rs` に `pub mod cli;` を追加

## Phase 3.2: テストファースト (TDD)

**重要: これらのテストは記述され、実装前に失敗する必要がある**

- [x] T005-T010 `llmlb/tests/cli_tests.rs` にCLIパーステストを統合
  - `--help` / `--version` 表示テスト
  - ~~`user list` / `add` / `delete` コマンドパーステスト~~ → 廃止（Phase 4で削除）

## Phase 3.3: コア実装 (テストが失敗した後のみ)

- [x] T011 `llmlb/src/main.rs` を修正してCLI引数をパース
  - clap でコマンドライン引数を処理
  - 引数なし → サーバー起動（既存動作維持）
  - `--help` → ヘルプ表示
  - `--version` → バージョン表示
  - ~~`user` → サブコマンドへディスパッチ~~ → 廃止（Phase 4で削除）
- [x] T012 `llmlb/src/cli/mod.rs` にCli構造体を実装
  - `#[command(version, about)]` 属性
  - 環境変数一覧をhelpに表示
- [x] ~~T013~~ **廃止** user list - API `GET /api/users` で代替
- [x] ~~T014~~ **廃止** user add - API `POST /api/users` で代替
- [x] ~~T015~~ **廃止** user delete - API `DELETE /api/users/:id` で代替

## Phase 3.4: 環境変数統一

- [x] T016 `llmlb/src/config.rs` を新規作成（環境変数ヘルパー）
  - `get_env_with_fallback(new_name, old_name)` 関数
  - 旧変数使用時の警告ログ
- [x] T017 `llmlb/src/main.rs` の環境変数を更新
  - `LLMLB_HOST` → `LLMLB_HOST`（フォールバック付き）
  - `LLMLB_PORT` → `LLMLB_PORT`（フォールバック付き）
  - `JWT_SECRET` → `LLMLB_JWT_SECRET`（フォールバック付き）
  - `DATABASE_URL` → `LLMLB_DATABASE_URL`（フォールバック付き）
  - `HEALTH_CHECK_INTERVAL` → `LLMLB_HEALTH_CHECK_INTERVAL`
  - `NODE_TIMEOUT` → `LLMLB_NODE_TIMEOUT`
  - `LOAD_BALANCER_MODE` → `LLMLB_LOAD_BALANCER_MODE`
- [x] T018 `llmlb/src/auth/bootstrap.rs` の環境変数を更新
  - `ADMIN_USERNAME` → `LLMLB_ADMIN_USERNAME`（フォールバック付き）
  - `ADMIN_PASSWORD` → `LLMLB_ADMIN_PASSWORD`（フォールバック付き）
- [x] T019 `llmlb/src/logging.rs` の環境変数を更新
  - `LLMLB_LOG_LEVEL` を削除（レガシー）
  - `LLM_LOG_LEVEL` → `LLMLB_LOG_LEVEL`（フォールバック付き）
- [x] T020 Cloud API環境変数 - スコープ外（既存のままで良い）

## Phase 3.5: JWT_SECRET ファイル永続化

- [x] T021-T023 `llmlb/src/jwt_secret.rs` にJWT_SECRETファイル永続化実装
  - `get_or_create_jwt_secret()` 関数
  - UUIDv4生成
  - ファイル読み書き（パーミッション600）
  - テストも含めて実装済み

## Phase 3.6: Node（C++）CLI

- [x] T024 [P] `node/tests/unit/cli_test.cpp` に --help テスト
  - `xllm --help` の出力に環境変数の説明が含まれることを確認
- [x] T025 [P] `node/tests/unit/cli_test.cpp` に --version テスト
  - `xllm --version` の出力にバージョン番号が含まれることを確認
- [x] T026 `node/include/utils/version.h` を新規作成
  - `XLLM_VERSION` 定数
- [x] T027 `node/src/main.cpp` にCLI引数処理を追加
  - `--help` / `-h` → ヘルプ表示して終了
  - `--version` / `-V` → バージョン表示して終了
  - 引数なし → 既存動作（ノード起動）

## Phase 3.7: Node 環境変数統一

- [x] T028 [P] `node/tests/unit/utils_config_test.cpp` に環境変数テスト追加
  - `XLLM_MODELS_DIR` が使用されることを確認
  - 旧 `LLM_MODELS_DIR` 使用時に警告が出ることを確認
- [x] T029 `node/src/utils/config.cpp` の環境変数を更新
  - `LLM_MODELS_DIR` → `XLLM_MODELS_DIR`（フォールバック付き）
  - `LLM_HEARTBEAT_SECS` → `XLLM_HEARTBEAT_SECS`（フォールバック付き）
  - `LLM_ALLOW_NO_GPU` → `XLLM_ALLOW_NO_GPU`（削除）
  - `LLM_BIND_ADDRESS` → `XLLM_BIND_ADDRESS`（フォールバック付き）
- [x] T030 `node/src/utils/logger.cpp` の環境変数を更新
  - `LLM_LOG_DIR` → `XLLM_LOG_DIR`（フォールバック付き）
  - `LLM_LOG_LEVEL` → `XLLM_LOG_LEVEL`（フォールバック付き）
  - `LLM_LOG_RETENTION_DAYS` → `XLLM_LOG_RETENTION_DAYS`（フォールバック付き）

## Phase 3.8: 仕上げ

- [x] T031 [P] `docs/authentication.md` を更新
  - CLI user コマンドの使用例を追加
  - 環境変数 `LLMLB_ADMIN_PASSWORD` / `LLMLB_JWT_SECRET` を記載
- [x] T032 [P] `INSTALL.md` を更新
  - 新しい環境変数名を表形式で記載（Load Balancer: LLMLB_*, Node: XLLM_*）
- [x] T033 [P] `README.md` を更新
  - 環境変数セクションを更新済み
- [x] T034 すべてのテストを実行して合格を確認
  - Load Balancer: `cargo test` (121 tests) / `cargo clippy -- -D warnings` / `cargo fmt --check` ✓
  - Node: 91 unit tests ✓
- [x] T035 手動テスト実行
  - `xllm --help` → 環境変数一覧表示 ✓
  - `xllm --version` → "xllm 1.0.0" ✓

## Phase 4: CLI簡素化（2025-12-10追加）

**設計方針変更**: CLIはサーバー起動専用とし、すべての管理操作はAPI/Dashboard経由で行う。

- [x] T036 `llmlb/src/cli/user.rs` を削除
  - ユーザー管理はAPI経由で実行
- [x] T037 `llmlb/src/cli/model.rs` を削除
  - モデル管理はAPI経由で実行
- [x] T038 `llmlb/src/cli/mod.rs` を簡素化
  - Commands enum を削除
  - preload_models オプションを削除
  - Cli 構造体のみ残す（-h/-V のみ）
- [x] T039 `llmlb/src/main.rs` を簡素化
  - handle_user_command() を削除
  - model コマンド処理を削除
  - parse_repo_filename() を削除
  - preload_models 処理を削除
- [x] T040 `llmlb/tests/cli_tests.rs` を更新
  - user/model 関連テストを削除
  - -h/-V のみのテストに簡素化
- [x] T041 `llmlb/tests/cli/model_cli_test.rs` を削除
- [x] T042 `llmlb/src/main.rs` で全プラットフォームCLI対応
  - Windows/macOS でも -h/-V が動作するよう修正
- [x] T043 spec.md を更新
  - 廃止機能を明記
  - ステータスを「実装済み」に更新
- [x] T044 tasks.md を更新（本タスク）
  - Phase 4 を追加

## 依存関係

```text
# Load Balancer (Rust) - Phase 3
T001 → T002 → T004
T004 → T005-T010 (並列可能)
T005-T010 → T011, T012
T012 → T016
T016 → T017, T018, T019, T020
T017-T020 → T021-T023

# Load Balancer (Rust) - Phase 4 (CLI簡素化)
T023 → T036, T037, T038, T039
T038, T039 → T040, T041
T040, T041 → T042, T043, T044

# Node (C++)
T024-T025 (並列可能) → T026 → T027
T028 → T029, T030

# 仕上げ
T023, T030 → T031-T033 (並列可能)
T031-T034 → T035
```

## 並列実行例

```bash
# Phase 3.2 のテストを並列起動:
# T005-T010 を同時に実行可能（異なるファイル）

# Phase 3.6 のNode CLIテストを並列起動:
# T024-T025 を同時に実行可能（異なるファイル）

# Phase 3.8 のドキュメント更新を並列起動:
# T031-T033 を同時に実行可能（異なるファイル）

# Load Balancer と Node は独立して並列実行可能
```

## 検証チェックリスト

- [x] すべてのユーザーストーリーに対応するテストがある
- [x] すべてのテストが実装より先にある
- [x] 並列タスクは本当に独立している
- [x] 各タスクは正確なファイルパスを指定
- [x] 同じファイルを変更する[P]タスクがない
- [x] Node CLI要件が含まれている
- [x] JWT_SECRET永続化要件が含まれている
