#!/usr/bin/env bash
# 全スクリプト共通の関数と変数

# リポジトリルートを取得（非Gitリポジトリのフォールバックあり）
get_repo_root() {
    if git rev-parse --show-toplevel >/dev/null 2>&1; then
        git rev-parse --show-toplevel
    else
        # 非Gitリポジトリの場合はスクリプト位置にフォールバック
        local script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
        (cd "$script_dir/../../.." && pwd)
    fi
}

# 現在のブランチを取得（非Gitリポジトリのフォールバックあり）
get_current_branch() {
    # まずSPECIFY_FEATURE環境変数をチェック
    if [[ -n "${SPECIFY_FEATURE:-}" ]]; then
        echo "$SPECIFY_FEATURE"
        return
    fi

    # 次にgitが利用可能かチェック
    if git rev-parse --abbrev-ref HEAD >/dev/null 2>&1; then
        git rev-parse --abbrev-ref HEAD
        return
    fi

    # 非Gitリポジトリの場合は.specify/.current-featureファイルをチェック
    local repo_root=$(get_repo_root)
    local current_feature_file="$repo_root/.specify/.current-feature"

    if [[ -f "$current_feature_file" ]]; then
        local branch_name=$(cat "$current_feature_file")
        if [[ -n "$branch_name" ]]; then
            echo "$branch_name"
            return
        fi
    fi

    echo "main"  # 最終フォールバック
}

# gitが利用可能かチェック
has_git() {
    git rev-parse --show-toplevel >/dev/null 2>&1
}

check_feature_branch() {
    local branch="$1"
    local has_git_repo="$2"

    # 非Gitリポジトリではブランチ命名規則を強制できないが、出力は提供
    if [[ "$has_git_repo" != "true" ]]; then
        echo "[specify] 警告: Gitリポジトリが検出されません。ブランチ検証をスキップしました" >&2
        return 0
    fi

    # feature/SPEC-UUID8桁形式をチェック（ブランチ＆Worktreeワークフロー）
    if [[ "$branch" =~ ^feature/SPEC-[a-z0-9]{8}$ ]]; then
        return 0
    fi

    # mainブランチを許可
    if [[ "$branch" == "main" ]] || [[ "$branch" == "master" ]]; then
        return 0
    fi

    # 他のブランチは警告するが続行を許可
    echo "[specify] 警告: ブランチ '$branch' は feature/SPEC-[UUID8桁] 形式ではありませんが、続行します" >&2
    return 0
}

get_feature_dir() { echo "$1/specs/$2"; }

# ブランチ＆Worktreeワークフロー用の機能ディレクトリを検索
# ブランチ名からSPEC-IDを抽出し、対応するWorktreeを検索
find_feature_dir_by_prefix() {
    local repo_root="$1"
    local branch_name="$2"

    # feature/SPEC-xxxブランチ名からSPEC-IDを抽出
    if [[ "$branch_name" =~ ^feature/(SPEC-[a-z0-9]{8})$ ]]; then
        local spec_id="${BASH_REMATCH[1]}"
        local worktree_dir="$repo_root/.worktrees/$spec_id"

        # Worktreeが存在するかチェック
        if [[ -d "$worktree_dir" ]]; then
            echo "$worktree_dir/specs/$spec_id"
            return
        fi

        # Worktreeが存在しない場合はメインリポジトリにフォールバック
        echo "$repo_root/specs/$spec_id"
        return
    fi

    # mainブランチまたは他のブランチの場合はメインリポジトリのspecsディレクトリを使用
    local specs_dir="$repo_root/specs"

    # branch_nameが直接SPEC-IDに見える場合
    if [[ "$branch_name" =~ ^SPEC-[a-z0-9]{8}$ ]]; then
        echo "$specs_dir/$branch_name"
        return
    fi

    # フォールバック: ブランチ名をそのまま使用
    echo "$specs_dir/$branch_name"
}

get_feature_paths() {
    local spec_id_override="${1:-}"  # オプションのSPEC-ID引数
    local repo_root=$(get_repo_root)
    local current_branch=$(get_current_branch)
    local has_git_repo="false"

    if has_git; then
        has_git_repo="true"
    fi

    local feature_dir
    if [[ -n "$spec_id_override" ]]; then
        # 指定されたSPEC-IDを直接使用
        feature_dir="$repo_root/specs/$spec_id_override"
    else
        # プレフィックスベースの検索を使用（spec単位で複数ブランチをサポート）
        feature_dir=$(find_feature_dir_by_prefix "$repo_root" "$current_branch")
    fi

    cat <<EOF
REPO_ROOT='$repo_root'
CURRENT_BRANCH='$current_branch'
HAS_GIT='$has_git_repo'
FEATURE_DIR='$feature_dir'
FEATURE_SPEC='$feature_dir/spec.md'
IMPL_PLAN='$feature_dir/plan.md'
TASKS='$feature_dir/tasks.md'
RESEARCH='$feature_dir/research.md'
DATA_MODEL='$feature_dir/data-model.md'
QUICKSTART='$feature_dir/quickstart.md'
CONTRACTS_DIR='$feature_dir/contracts'
EOF
}

check_file() { [[ -f "$1" ]] && echo "  ✓ $2" || echo "  ✗ $2"; }
check_dir() { [[ -d "$1" && -n $(ls -A "$1" 2>/dev/null) ]] && echo "  ✓ $2" || echo "  ✗ $2"; }
