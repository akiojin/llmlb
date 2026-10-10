#!/usr/bin/env bats

# Issue #878 AC-2: データ取得 hook 検査を push / PR の両方で直接実行する。
setup() {
    WORKFLOW="$BATS_TEST_DIRNAME/../../.github/workflows/lint.yml"
}

@test "Lint は develop の push と pull_request で実行される" {
    triggers="$(awk '/^on:/ { found = 1; next } found && /^jobs:/ { exit } found { print }' "$WORKFLOW")"
    [[ "$triggers" == *"  push:"* ]] || false
    [[ "$triggers" == *"  pull_request:"* ]] || false
    [ "$(printf '%s\n' "$triggers" | grep -c '      - develop')" -eq 2 ]
}

@test "dashboard-lint がイベントやパス条件なしで dashboard-data-hooks を直接実行する" {
    block="$(awk '
        /^  dashboard-lint:/ { found = 1; next }
        found && /^  [A-Za-z0-9_-]+:/ { exit }
        found { print }
    ' "$WORKFLOW")"
    [[ "$block" == *"run: make dashboard-data-hooks"* ]] || {
        echo "dashboard-lint に直接実行stepがありません"
        false
    }
    ! printf '%s\n' "$block" | grep -qE '^[[:space:]]+if:'
}
