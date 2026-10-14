#!/usr/bin/env bats

# Issue #889 AC-3: bash 3.2 の Bats がUTF-8テスト名を実行せず終了しないこと。

setup() {
    REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
    FIXTURE="$BATS_TEST_TMPDIR/repo"
    mkdir -p "$FIXTURE/node_modules" "$FIXTURE/tests/checks" \
        "$FIXTURE/scripts/checks/tests" "$FIXTURE/tests/hooks"
    ln -s "$REPO_ROOT/node_modules/bats" "$FIXTURE/node_modules/bats"
    cat > "$FIXTURE/tests/checks/unicode.bats" <<'EOF'
@test "日本語のテストを実行する" {
    run env -u LC_ALL LANG=C.UTF-8 LC_CTYPE=C.UTF-8 /bin/bash -uc 'value=ok; echo "${value}）"'
    [ "$status" -eq 0 ]
    [ "$output" = 'ok）' ]
}
EOF
    cp "$FIXTURE/tests/checks/unicode.bats" "$FIXTURE/tests/hooks/test-block-git-branch-ops.bats"
    cp "$FIXTURE/tests/checks/unicode.bats" "$FIXTURE/tests/hooks/test-block-cd-command.bats"
}

@test "test-checks executes Japanese test names under UTF-8 on system bash" {
    run env -u LC_ALL LANG=C.UTF-8 LC_CTYPE=C.UTF-8 \
        make -C "$FIXTURE" -f "$REPO_ROOT/Makefile" test-checks
    [ "$status" -eq 0 ]
    [[ "$output" == *"ok 1 日本語のテストを実行する"* ]] || false
    [[ "$output" != *"unknown test name"* ]] || false
}

@test "test-hooks executes Japanese test names under UTF-8 on system bash" {
    run env -u LC_ALL LANG=C.UTF-8 LC_CTYPE=C.UTF-8 \
        make -C "$FIXTURE" -f "$REPO_ROOT/Makefile" test-hooks
    [ "$status" -eq 0 ]
    [[ "$output" == *"ok 1 日本語のテストを実行する"* ]] || false
    [[ "$output" == *"ok 2 日本語のテストを実行する"* ]] || false
    [[ "$output" != *"unknown test name"* ]] || false
}
