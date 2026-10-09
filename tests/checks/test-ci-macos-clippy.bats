#!/usr/bin/env bats

# Issue #851: Linux の cfg 対象外になる GUI の警告を macOS CI で検出する。
setup() {
    LINT_WORKFLOW="$BATS_TEST_DIRNAME/../../.github/workflows/lint.yml"
}

job_block() {
    awk -v job="  $1:" '
        $0 == job { in_job = 1; print; next }
        in_job && /^  [A-Za-z0-9_-]+:/ { exit }
        in_job { print }
    ' "$LINT_WORKFLOW"
}

macos_condition() {
    job_block macos-clippy | awk '
        /^    if: >-/ { in_condition = 1; next }
        in_condition && /^    [A-Za-z]/ { exit }
        in_condition { printf "%s ", $0 }
    '
}

check_condition() {
    expression="$(macos_condition)"
    [ -n "$expression" ]
    run node -e '
        const vm = require("node:vm");
        const [expression, event, ref, changed] = process.argv.slice(1);
        console.log(vm.runInNewContext(expression, {
            github: { event_name: event, ref },
            needs: { changes: { outputs: { macos_clippy: changed } } }
        }));
    ' "$expression" "$1" "$2" "$3"
    [ "$status" -eq 0 ]
    [ "$output" = "$4" ]
}

@test "macOS CI が全ターゲット・全 feature を warnings 拒否で検査する" {
    block="$(job_block macos-clippy)"
    [[ "$block" == *"name: macOS Clippy"* ]]
    [[ "$block" == *"needs: changes"* ]]
    [[ "$block" == *"runs-on: macos-latest"* ]]
    [[ "$block" == *"components: clippy"* ]]
    [[ "$block" == *"cargo clippy --all-targets --all-features -- -D warnings"* ]]
    [[ "$block" != *"continue-on-error"* ]]
}

@test "macOS 専用フィルタは GUI とルート Cargo ファイルだけを対象にする" {
    block="$(job_block changes)"
    [[ "$block" == *'macos_clippy: ${{ steps.filter.outputs.macos_clippy }}'* ]]
    paths="$(printf '%s\n' "$block" | awk '
        /^            macos_clippy:/ { in_filter = 1; next }
        in_filter && /^            [A-Za-z]/ { exit }
        in_filter && /^              - / { sub(/^              - /, ""); print }
    ')"
    expected="$(printf "'%s'\n" 'llmlb/src/gui/**' 'Cargo.toml' 'Cargo.lock')"
    [ "$paths" = "$expected" ]
}

@test "develop push は変更パスにかかわらず macOS clippy を実行する" {
    trigger="$(awk '/^  push:/ { active = 1; next } active && /^  [A-Za-z_]+:/ { exit } active { print }' "$LINT_WORKFLOW")"
    [[ "$trigger" == *"- develop"* ]]
    [[ "$trigger" != *"paths:"* ]]
    check_condition push refs/heads/develop false true
    check_condition push refs/heads/develop true true
    check_condition push refs/heads/main true false
}

@test "GUI または Cargo を変更する PR で macOS clippy を実行する" {
    check_condition pull_request refs/pull/851/merge true true
}

@test "その他の PR で macOS runner を消費しない" {
    check_condition pull_request refs/pull/851/merge false false
    check_condition pull_request refs/pull/851/merge '' false
}

@test "手動実行では専用フィルタが true でも macOS runner を消費しない" {
    check_condition workflow_dispatch refs/heads/develop true false
    check_condition workflow_dispatch refs/heads/develop false false
}

@test "既存 Ubuntu clippy の名前と完全なコマンドを維持する" {
    block="$(job_block rust-lint)"
    [[ "$block" == *"name: Rust Format & Clippy"* ]]
    [[ "$block" == *"runs-on: ubuntu-latest"* ]]
    [[ "$block" == *"cargo clippy --all-targets --all-features -- -D warnings"* ]]
}
