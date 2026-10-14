#!/usr/bin/env bats

# 運用通知の受け入れテスト（SPEC #777 AC-8）の CI 配線テスト
# AC-1〜AC-7 の検証（llmlb/tests/notification_tests.rs）が、CI の必須チェックと
# make quality-checks の両方で実行されることを保証する。

setup() {
    ROOT="$BATS_TEST_DIRNAME/../.."
    TEST_WORKFLOW="$ROOT/.github/workflows/test.yml"
    MAKEFILE="$ROOT/Makefile"
    ENTRYPOINT="$ROOT/llmlb/tests/notification_tests.rs"
    TEST_DIR="$ROOT/llmlb/tests/notifications"
    [ -f "$TEST_WORKFLOW" ]
    [ -f "$MAKEFILE" ]
}

# test.yml から指定ジョブ（2 スペースインデントのキー）のブロックを抜き出す
job_block() {
    awk -v job="  $1:" '
        $0 == job { in_job = 1; print; next }
        in_job && /^  [A-Za-z0-9_-]+:/ { exit }
        in_job { print }
    ' "$TEST_WORKFLOW"
}

@test "notification-tests ターゲットが受け入れテストのバイナリを実行する" {
    recipe="$(awk '
        /^notification-tests:/ { found = 1; next }
        found && /^\t/ { print; next }
        found { exit }
    ' "$MAKEFILE")"
    [[ "$recipe" == *"cargo test -p llmlb --test notification_tests"* ]] || false
}

@test "make quality-checks が notification-tests を実行する" {
    deps="$(grep -E '^quality-checks:' "$MAKEFILE")"
    [[ " $deps " == *" notification-tests "* ]] || false
}

@test "必須チェック Rust Tests が notification-tests を実行する" {
    block="$(job_block rust-test)"
    [[ "$block" == *'name: Rust Tests (${{ matrix.os }})'* ]] || false
    [[ "$block" == *"make notification-tests"* ]] || false
}

@test "受け入れテストのモジュールはすべて entrypoint に登録されている" {
    [ -f "$ENTRYPOINT" ]
    count=0
    for file in "$TEST_DIR"/*_test.rs; do
        [ -f "$file" ]
        name="$(basename "$file")"
        grep -qF "notifications/$name" "$ENTRYPOINT" || {
            echo "$name が $ENTRYPOINT に登録されていません"
            false
        }
        count=$((count + 1))
    done
    [ "$count" -gt 0 ]
}

@test "受け入れテストが実装済みの受け入れ基準をすべて含む" {
    # AC-3（即時通知）は PR #728 の着地が前提のため T-006 で追加する。
    for ac in 1 2 4 5 6 7; do
        grep -rqE "^async fn ac${ac}_" "$TEST_DIR" || {
            echo "AC-${ac} のテストがありません"
            false
        }
    done
}
