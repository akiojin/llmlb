#!/usr/bin/env bash

# 前提条件チェック統合スクリプト
#
# このスクリプトはSpec駆動開発ワークフローの前提条件チェックを統一的に提供します。
# 従来複数のスクリプトに分散していた機能を統合しています。
#
# 使用法: ./check-prerequisites.sh [OPTIONS]
#
# オプション:
#   --json              JSON形式で出力
#   --require-tasks     tasks.mdの存在を必須とする（実装フェーズ用）
#   --include-tasks     tasks.mdをAVAILABLE_DOCSリストに含める
#   --paths-only        パス変数のみ出力（バリデーションなし）
#   --help, -h          ヘルプメッセージを表示
#
# 出力:
#   JSONモード: {"FEATURE_DIR":"...", "AVAILABLE_DOCS":["..."]}
#   テキストモード: FEATURE_DIR:... \n AVAILABLE_DOCS: \n ✓/✗ file.md
#   パスのみ: REPO_ROOT: ... \n BRANCH: ... \n FEATURE_DIR: ... など

set -e

# コマンドライン引数の解析
JSON_MODE=false
REQUIRE_TASKS=false
INCLUDE_TASKS=false
PATHS_ONLY=false
SPEC_ID=""

for arg in "$@"; do
    case "$arg" in
        --json)
            JSON_MODE=true
            ;;
        --require-tasks)
            REQUIRE_TASKS=true
            ;;
        --include-tasks)
            INCLUDE_TASKS=true
            ;;
        --paths-only)
            PATHS_ONLY=true
            ;;
        --help|-h)
            cat << 'EOF'
使用法: check-prerequisites.sh [OPTIONS] [SPEC-ID]

Spec駆動開発ワークフローの前提条件チェックを統一的に行います。

オプション:
  --json              JSON形式で出力
  --require-tasks     tasks.mdの存在を必須とする（実装フェーズ用）
  --include-tasks     tasks.mdをAVAILABLE_DOCSリストに含める
  --paths-only        パス変数のみ出力（前提条件のバリデーションなし）
  --help, -h          このヘルプメッセージを表示
  SPEC-ID             オプションのSPEC ID（例: SPEC-ea015fbb）
                      未指定の場合は現在のブランチから導出

使用例:
  # タスクの前提条件をチェック（plan.md必須）
  ./check-prerequisites.sh --json

  # 特定のSPECの前提条件をチェック
  ./check-prerequisites.sh --json SPEC-ea015fbb

  # 実装の前提条件をチェック（plan.md + tasks.md必須）
  ./check-prerequisites.sh --json --require-tasks --include-tasks

  # 機能パスのみ取得（バリデーションなし）
  ./check-prerequisites.sh --paths-only

EOF
            exit 0
            ;;
        SPEC-*)
            SPEC_ID="$arg"
            ;;
        *)
            echo "エラー: 不明なオプション '$arg'。使用方法は --help を参照してください。" >&2
            exit 1
            ;;
    esac
done

# 共通関数を読み込み
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

# 機能パスを取得しブランチを検証
# SPEC_IDが指定されている場合は機能検索をオーバーライド
if [[ -n "$SPEC_ID" ]]; then
    eval $(get_feature_paths "$SPEC_ID")
else
    eval $(get_feature_paths)
fi
check_feature_branch "$CURRENT_BRANCH" "$HAS_GIT" || exit 1

# パスのみモードの場合はパスを出力して終了（JSON + パスのみの組み合わせもサポート）
if $PATHS_ONLY; then
    if $JSON_MODE; then
        # 最小限のJSONパスペイロード（バリデーションなし）
        printf '{"REPO_ROOT":"%s","BRANCH":"%s","FEATURE_DIR":"%s","FEATURE_SPEC":"%s","IMPL_PLAN":"%s","TASKS":"%s"}\n' \
            "$REPO_ROOT" "$CURRENT_BRANCH" "$FEATURE_DIR" "$FEATURE_SPEC" "$IMPL_PLAN" "$TASKS"
    else
        echo "REPO_ROOT: $REPO_ROOT"
        echo "BRANCH: $CURRENT_BRANCH"
        echo "FEATURE_DIR: $FEATURE_DIR"
        echo "FEATURE_SPEC: $FEATURE_SPEC"
        echo "IMPL_PLAN: $IMPL_PLAN"
        echo "TASKS: $TASKS"
    fi
    exit 0
fi

# 必須ディレクトリとファイルを検証
if [[ ! -d "$FEATURE_DIR" ]]; then
    echo "エラー: 機能ディレクトリが見つかりません: $FEATURE_DIR" >&2
    echo "まず /speckit.specify を実行して機能構造を作成してください。" >&2
    exit 1
fi

if [[ ! -f "$IMPL_PLAN" ]]; then
    echo "エラー: plan.md が $FEATURE_DIR に見つかりません" >&2
    echo "まず /speckit.plan を実行して実装計画を作成してください。" >&2
    exit 1
fi

# tasks.mdが必須の場合はチェック
if $REQUIRE_TASKS && [[ ! -f "$TASKS" ]]; then
    echo "エラー: tasks.md が $FEATURE_DIR に見つかりません" >&2
    echo "まず /speckit.tasks を実行してタスクリストを作成してください。" >&2
    exit 1
fi

# 利用可能なドキュメントリストを構築
docs=()

# これらのオプションドキュメントを常にチェック
[[ -f "$RESEARCH" ]] && docs+=("research.md")
[[ -f "$DATA_MODEL" ]] && docs+=("data-model.md")

# contractsディレクトリをチェック（存在しファイルがある場合のみ）
if [[ -d "$CONTRACTS_DIR" ]] && [[ -n "$(ls -A "$CONTRACTS_DIR" 2>/dev/null)" ]]; then
    docs+=("contracts/")
fi

[[ -f "$QUICKSTART" ]] && docs+=("quickstart.md")

# 要求された場合、tasks.mdが存在すれば含める
if $INCLUDE_TASKS && [[ -f "$TASKS" ]]; then
    docs+=("tasks.md")
fi

# 結果を出力
if $JSON_MODE; then
    # ドキュメントのJSON配列を構築
    if [[ ${#docs[@]} -eq 0 ]]; then
        json_docs="[]"
    else
        json_docs=$(printf '"%s",' "${docs[@]}")
        json_docs="[${json_docs%,}]"
    fi

    printf '{"FEATURE_DIR":"%s","AVAILABLE_DOCS":%s}\n' "$FEATURE_DIR" "$json_docs"
else
    # テキスト出力
    echo "FEATURE_DIR:$FEATURE_DIR"
    echo "AVAILABLE_DOCS:"

    # 各候補ドキュメントのステータスを表示
    check_file "$RESEARCH" "research.md"
    check_file "$DATA_MODEL" "data-model.md"
    check_dir "$CONTRACTS_DIR" "contracts/"
    check_file "$QUICKSTART" "quickstart.md"

    if $INCLUDE_TASKS; then
        check_file "$TASKS" "tasks.md"
    fi
fi
