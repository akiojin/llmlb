#!/usr/bin/env bash

# 実装計画セットアップスクリプト
#
# このスクリプトは機能の実装計画をセットアップします。
# テンプレートからplan.mdを作成し、必要なパス情報を出力します。
#
# 使用法: ./setup-plan.sh [--json] [SPEC-ID]
#
# オプション:
#   --json    JSON形式で結果を出力
#   SPEC-ID   オプションのSPEC ID（例: SPEC-ea015fbb）
#             未指定の場合は現在のブランチから導出
#   --help    このヘルプメッセージを表示

set -e

# コマンドライン引数の解析
JSON_MODE=false
SPEC_ID=""
ARGS=()

for arg in "$@"; do
    case "$arg" in
        --json)
            JSON_MODE=true
            ;;
        --help|-h)
            echo "使用法: $0 [--json] [SPEC-ID]"
            echo "  --json    JSON形式で結果を出力"
            echo "  SPEC-ID   オプションのSPEC ID（例: SPEC-ea015fbb）"
            echo "            未指定の場合は現在のブランチから導出"
            echo "  --help    このヘルプメッセージを表示"
            exit 0
            ;;
        SPEC-*)
            SPEC_ID="$arg"
            ;;
        *)
            ARGS+=("$arg")
            ;;
    esac
done

# スクリプトディレクトリを取得し共通関数を読み込み
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

# 共通関数から全パスと変数を取得
# SPEC_IDが指定されている場合は機能検索をオーバーライド
if [[ -n "$SPEC_ID" ]]; then
    eval $(get_feature_paths "$SPEC_ID")
else
    eval $(get_feature_paths)
fi

# 適切な機能ブランチにいるかチェック（Gitリポジトリの場合のみ）
check_feature_branch "$CURRENT_BRANCH" "$HAS_GIT" || exit 1

# 機能ディレクトリが存在することを確認
mkdir -p "$FEATURE_DIR"

# テンプレートが存在する場合はplan.mdにコピー
TEMPLATE="$REPO_ROOT/.specify/templates/plan-template.md"
if [[ -f "$TEMPLATE" ]]; then
    cp "$TEMPLATE" "$IMPL_PLAN"
    echo "plan.mdテンプレートを $IMPL_PLAN にコピーしました"
else
    echo "警告: planテンプレートが $TEMPLATE に見つかりません"
    # テンプレートが存在しない場合は空のplanファイルを作成
    touch "$IMPL_PLAN"
fi

# 結果を出力
if $JSON_MODE; then
    printf '{"FEATURE_SPEC":"%s","IMPL_PLAN":"%s","SPECS_DIR":"%s","BRANCH":"%s","HAS_GIT":"%s"}\n' \
        "$FEATURE_SPEC" "$IMPL_PLAN" "$FEATURE_DIR" "$CURRENT_BRANCH" "$HAS_GIT"
else
    echo "FEATURE_SPEC: $FEATURE_SPEC"
    echo "IMPL_PLAN: $IMPL_PLAN"
    echo "SPECS_DIR: $FEATURE_DIR"
    echo "BRANCH: $CURRENT_BRANCH"
    echo "HAS_GIT: $HAS_GIT"
fi
