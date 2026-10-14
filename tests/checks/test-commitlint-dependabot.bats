#!/usr/bin/env bats

# commitlint.config.js の Dependabot 除外に対する回帰テスト（Issue #751）
# Dependabot が生成したコミットだけを header-max-length から外し、
# 人間が書くコミットには 72 文字制限を維持することを固定サンプルで確認する。

setup() {
    ROOT="$BATS_TEST_DIRNAME/../.."
    COMMITLINT="$ROOT/node_modules/.bin/commitlint"
    [ -x "$COMMITLINT" ]
    SIGN_OFF='Signed-off-by: dependabot[bot] <support@github.com>'
    # PR #720 で実測された 75 文字の件名（Commit Message Lint が失敗した実物）
    OBSERVED_SUBJECT='chore(deps): bump the npm_and_yarn group across 1 directory with 38 updates'
    # PR #666 で実測された、group 内 1 件更新時の 81 文字の件名
    SINGLE_UPDATE_SUBJECT='chore(deps): bump tar from 0.4.45 to 0.4.46 in the cargo group across 1 directory'
}

lint() {
    run bash -c 'printf "%s\n" "$1" | "$2" --cwd "$3"' _ "$1" "$COMMITLINT" "$ROOT"
}

dependabot_message() {
    printf '%s\n\nBumps the group with updates in the / directory.\n\n---\nupdated-dependencies:\n- dependency-name: tar\n...\n\n%s' "$1" "$SIGN_OFF"
}

@test "実測 75 文字の件名は 72 文字を超えている" {
    [ "${#OBSERVED_SUBJECT}" -eq 75 ]
}

@test "Dependabot が生成した実測 75 文字の件名のコミットは通る" {
    lint "$(dependabot_message "$OBSERVED_SUBJECT")"
    [ "$status" -eq 0 ]
}

@test "Dependabot が生成した group 内 1 件更新の 81 文字の件名のコミットは通る" {
    lint "$(dependabot_message "$SINGLE_UPDATE_SUBJECT")"
    [ "$status" -eq 0 ]
}

@test "Dependabot の署名が無い同じ件名（人間のコミット）は header-max-length で失敗する" {
    lint "$(printf '%s\n\nhand written body' "$OBSERVED_SUBJECT")"
    [ "$status" -ne 0 ]
    [[ "$output" == *"header-max-length"* ]] || false
}

@test "Dependabot の署名があっても chore(deps) 以外の件名は除外しない" {
    lint "$(dependabot_message 'feat(api): add a very long subject line that clearly exceeds seventy two chars')"
    [ "$status" -ne 0 ]
    [[ "$output" == *"header-max-length"* ]] || false
}

@test "署名が本文中の引用で末尾トレーラーでない場合は除外しない" {
    lint "$(printf '%s\n\n%s\n\nhand written trailer-less body' "$OBSERVED_SUBJECT" "$SIGN_OFF")"
    [ "$status" -ne 0 ]
    [[ "$output" == *"header-max-length"* ]] || false
}

@test "人間の 72 文字以内の conventional commit は引き続き通る" {
    lint 'fix(ci): dependabot コミットを commitlint から限定除外する'
    [ "$status" -eq 0 ]
}
