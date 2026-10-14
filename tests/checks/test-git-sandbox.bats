#!/usr/bin/env bats

# git_sandbox の隔離契約テスト（Issue #737）
# git フック（pre-push 等）から GIT_DIR / GIT_WORK_TREE / GIT_INDEX_FILE を
# 継承した状態でも、テスト用一時リポジトリへの操作が外部リポジトリに一切
# 書き込まれないことを保証する。囮リポジトリは BATS_TEST_TMPDIR 内に作る。

setup() {
    load helpers/git-sandbox
    git_sandbox_isolate_env

    DECOY="$BATS_TEST_TMPDIR/decoy"
    git init -q "$DECOY"
    git -C "$DECOY" commit -q --allow-empty -m decoy
    DECOY_HEAD=$(git -C "$DECOY" rev-parse HEAD)
    DECOY_CONFIG=$(cat "$DECOY/.git/config")
    DECOY_INDEX=$(git -C "$DECOY" ls-files --stage)
}

@test "継承した git 環境変数下でも外部リポジトリに書き込まない" {
    run env \
        GIT_DIR="$DECOY/.git" \
        GIT_WORK_TREE="$DECOY" \
        GIT_INDEX_FILE="$DECOY/.git/index" \
        HELPER="$BATS_TEST_DIRNAME/helpers/git-sandbox.bash" \
        SANDBOX="$BATS_TEST_TMPDIR/sandbox" \
        bash -c '
            set -e
            source "$HELPER"
            git_sandbox_init "$SANDBOX"
            touch file.sql
            git add -A
            git commit -q -m sandbox
            git log --format=%s -1
        '
    [ "$status" -eq 0 ]
    [ "${lines[${#lines[@]}-1]}" = "sandbox" ]

    [ "$(git -C "$DECOY" rev-parse HEAD)" = "$DECOY_HEAD" ]
    [ "$(cat "$DECOY/.git/config")" = "$DECOY_CONFIG" ]
    [ "$(git -C "$DECOY" ls-files --stage)" = "$DECOY_INDEX" ]
    [ "$(git -C "$BATS_TEST_TMPDIR/sandbox" log --format=%s -1)" = "sandbox" ]
}

@test "サンドボックスは git config に user を書き込まない" {
    git_sandbox_init "$BATS_TEST_TMPDIR/sandbox"
    run git config --local --get user.email
    [ "$status" -ne 0 ]
}

@test "サンドボックス外のリポジトリを検出した場合は失敗する" {
    mkdir -p "$BATS_TEST_TMPDIR/outer/inner"
    git init -q "$BATS_TEST_TMPDIR/outer"
    run git_sandbox_assert_toplevel "$BATS_TEST_TMPDIR/outer/inner"
    [ "$status" -ne 0 ]
}
