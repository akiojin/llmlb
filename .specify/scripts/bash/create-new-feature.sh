#!/usr/bin/env bash

# 新規機能ディレクトリ作成スクリプト
#
# 使用法: ./create-new-feature.sh [--json] <機能説明>
#
# オプション:
#   --json      JSON形式で出力
#   --help, -h  ヘルプメッセージを表示
#
# 注意: このスクリプトはブランチやWorktreeを作成しません。
# 現在のブランチ・ディレクトリのまま specs/SPEC-[UUID8桁]/ を作成します。

set -e

JSON_MODE=false
ARGS=()

for arg in "$@"; do
    case "$arg" in
        --json)
            JSON_MODE=true
            ;;
        --help|-h)
            echo "使用法: $0 [--json] <機能説明>"
            echo "  --json      JSON形式で出力"
            echo "  --help, -h  このヘルプメッセージを表示"
            echo ""
            echo "注意: ブランチやWorktreeは作成しません。"
            echo "      現在のブランチ上に specs/SPEC-[UUID8桁]/ を作成します。"
            exit 0
            ;;
        *)
            ARGS+=("$arg")
            ;;
    esac
done

FEATURE_DESCRIPTION="${ARGS[*]}"
if [ -z "$FEATURE_DESCRIPTION" ]; then
    echo "使用法: $0 [--json] <機能説明>" >&2
    exit 1
fi

# SPEC ID (SPEC-xxxxxxxx) をUUID8桁形式で生成
generate_spec_id() {
    local uuid=""
    # macOS/Linux両対応: uuidgen を優先、/proc にフォールバック
    if command -v uuidgen >/dev/null 2>&1; then
        uuid=$(uuidgen)
    elif [ -f /proc/sys/kernel/random/uuid ]; then
        uuid=$(cat /proc/sys/kernel/random/uuid)
    fi
    if [ -n "$uuid" ]; then
        local short
        short=$(echo "$uuid" | tr '[:upper:]' '[:lower:]' | tr -d '-' | cut -c1-8)
        echo "SPEC-$short"
        return
    fi
    # UUID生成手段がない場合はランダムバイトでフォールバック
    local hex
    hex=$(od -An -tx1 -N4 /dev/urandom 2>/dev/null | tr -d ' \n' | cut -c1-8)
    if [ -n "$hex" ]; then
        echo "SPEC-$hex"
        return
    fi
    echo "エラー: UUID生成手段が見つかりません (uuidgen, /proc/sys/kernel/random/uuid, /dev/urandom のいずれも利用不可)" >&2
    exit 1
}

# リポジトリルートを検索する関数
find_repo_root() {
    local dir="$1"
    while [ "$dir" != "/" ]; do
        if [ -d "$dir/.git" ] || [ -d "$dir/.specify" ]; then
            echo "$dir"
            return 0
        fi
        dir="$(dirname "$dir")"
    done
    return 1
}

# リポジトリルートを解決
# Gitが利用可能な場合はGit情報を優先、そうでなければリポジトリマーカーを検索
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if git rev-parse --show-toplevel >/dev/null 2>&1; then
    # Worktreeにいるかチェック
    if git rev-parse --git-dir 2>/dev/null | grep -q "\.git/worktrees"; then
        # Worktree内: カレントディレクトリからWorktreeルートを検索
        REPO_ROOT="$(find_repo_root "$PWD")"
        if [ -z "$REPO_ROOT" ]; then
            # マーカーが見つからない場合はPWDにフォールバック
            REPO_ROOT="$PWD"
        fi
    else
        # メインリポジトリ内: 標準のGitルートを使用
        REPO_ROOT=$(git rev-parse --show-toplevel)
    fi
    HAS_GIT=true
else
    REPO_ROOT="$(find_repo_root "$SCRIPT_DIR")"
    if [ -z "$REPO_ROOT" ]; then
        echo "エラー: リポジトリルートを特定できません。リポジトリ内で実行してください。" >&2
        exit 1
    fi
    HAS_GIT=false
fi

cd "$REPO_ROOT"

SPECS_DIR="$REPO_ROOT/specs"
mkdir -p "$SPECS_DIR"

# 一意のSPEC IDを生成
FEATURE_ID=""
while :; do
    candidate=$(generate_spec_id)
    if [ ! -d "$SPECS_DIR/$candidate" ]; then
        FEATURE_ID="$candidate"
        break
    fi
done

# ブランチ名を feature/ プレフィックス付きで作成（参照用のみ）
BRANCH_NAME="feature/$FEATURE_ID"

# Gitが利用可能な場合は現在のブランチを使用（新規ブランチは作成しない）
if [ "$HAS_GIT" = true ]; then
    CURRENT_BRANCH=$(git branch --show-current)
    echo "[specify] 現在のブランチを使用: $CURRENT_BRANCH"

    # 現在のWorktree/リポジトリにspecディレクトリを作成
    FEATURE_DIR="$REPO_ROOT/specs/$FEATURE_ID"
    echo "[specify] ブランチ作成はスキップ（ユーザーが手動で作成）"
else
    # 非Gitリポジトリのフォールバック
    FEATURE_DIR="$SPECS_DIR/$FEATURE_ID"
    echo "[specify] 警告: Gitリポジトリが検出されません。Worktreeなしでローカルディレクトリを使用"
fi

mkdir -p "$FEATURE_DIR"

# テンプレートからspecファイルを初期化
TEMPLATE="$REPO_ROOT/.specify/templates/spec-template.md"
SPEC_FILE="$FEATURE_DIR/spec.md"
if [ -f "$TEMPLATE" ]; then
    cp "$TEMPLATE" "$SPEC_FILE"
else
    touch "$SPEC_FILE"
fi

# 品質検証用のchecklistsサブディレクトリを作成
mkdir -p "$FEATURE_DIR/checklists"

# 現在のセッション用にSPECIFY_FEATURE環境変数を設定
if [ "$HAS_GIT" = true ]; then
    export SPECIFY_FEATURE="$CURRENT_BRANCH"
    mkdir -p "$REPO_ROOT/.specify"
    echo "$SPECIFY_FEATURE" > "$REPO_ROOT/.specify/.current-feature"
fi

echo "[specify] 機能ディレクトリを作成: $FEATURE_ID"
echo "[specify] 説明: $FEATURE_DESCRIPTION"

if $JSON_MODE; then
    if [ "$HAS_GIT" = true ]; then
        printf '{"FEATURE_ID":"%s","CURRENT_BRANCH":"%s","SPEC_FILE":"%s","FEATURE_DIR":"%s"}\n' \
            "$FEATURE_ID" "$CURRENT_BRANCH" "$SPEC_FILE" "$FEATURE_DIR"
    else
        printf '{"FEATURE_ID":"%s","SPEC_FILE":"%s","FEATURE_DIR":"%s"}\n' \
            "$FEATURE_ID" "$SPEC_FILE" "$FEATURE_DIR"
    fi
else
    echo "FEATURE_ID: $FEATURE_ID"
    if [ "$HAS_GIT" = true ]; then
        echo "CURRENT_BRANCH: $CURRENT_BRANCH"
    fi
    echo "SPEC_FILE: $SPEC_FILE"
    echo "FEATURE_DIR: $FEATURE_DIR"
fi
