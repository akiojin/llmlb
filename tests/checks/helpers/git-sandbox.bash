# git-sandbox.bash - テスト用一時 git リポジトリの隔離ヘルパー（Issue #737）
#
# git フック（pre-push 等）は GIT_DIR / GIT_WORK_TREE / GIT_INDEX_FILE などを
# export した状態で子プロセスを起動する。これを継承したまま一時ディレクトリで
# git を操作すると、実リポジトリにコミットや設定が書き込まれる。
# 一時リポジトリを扱うテストは必ず git_sandbox_init を経由すること。

# 継承した git のリポジトリ指定環境変数を解除し、author/committer を
# 環境変数で与える（git config には書き込まない）。
git_sandbox_isolate_env() {
    # shellcheck disable=SC2046
    unset $(git rev-parse --local-env-vars)
    export GIT_CONFIG_NOSYSTEM=1
    export GIT_AUTHOR_NAME=test GIT_AUTHOR_EMAIL=test@example.com
    export GIT_COMMITTER_NAME=test GIT_COMMITTER_EMAIL=test@example.com
}

# $1 が独立したリポジトリのトップレベルであることを検証する。
git_sandbox_assert_toplevel() {
    local dir toplevel
    dir=$(cd "$1" && pwd -P)
    toplevel=$(git -C "$dir" rev-parse --show-toplevel 2>/dev/null) || toplevel=""
    if [ "$toplevel" != "$dir" ]; then
        echo "✗ git sandbox が隔離されていません: $dir (toplevel: ${toplevel:-none})" >&2
        return 1
    fi
}

# $1 に隔離された一時リポジトリを作成し、そのディレクトリへ移動する。
git_sandbox_init() {
    git_sandbox_isolate_env
    mkdir -p "$1"
    cd "$1" || return 1
    git init -q
    git_sandbox_assert_toplevel "$1"
}
