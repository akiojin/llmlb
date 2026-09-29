#!/usr/bin/env bats

# check-dependabot-subjects.sh の契約テスト（Issue #727）
# Dependabot のグループ更新 PR の件名が commitlint（header-max-length 72）を
# 通ることを、dependabot.yml の group 設定からローカルで検証できることを確認する。

setup() {
    SCRIPT="$BATS_TEST_DIRNAME/../../scripts/checks/check-dependabot-subjects.sh"
    REPO_CONFIG="$BATS_TEST_DIRNAME/../../.github/dependabot.yml"
    [ -x "$SCRIPT" ]
    export DEPENDABOT_CONFIG="$BATS_TEST_TMPDIR/dependabot.yml"
}

write_config() {
    # $1: commit-message prefix, $2..: group 名
    local prefix="$1"
    shift
    {
        echo 'version: 2'
        echo 'updates:'
        echo '  - package-ecosystem: "npm"'
        echo '    directory: "/"'
        echo '    commit-message:'
        echo "      prefix: \"$prefix\""
        echo '    groups:'
        for group in "$@"; do
            echo "      $group:"
            echo '        patterns:'
            echo '          - "*"'
        done
    } >"$DEPENDABOT_CONFIG"
}

@test "リポジトリの dependabot.yml は全 group の件名が commitlint を通る" {
    DEPENDABOT_CONFIG="$REPO_CONFIG" run "$SCRIPT"
    [ "$status" -eq 0 ]
}

@test "リポジトリの dependabot.yml は npm の group を複数に分割している" {
    DEPENDABOT_CONFIG="$REPO_CONFIG" run "$SCRIPT"
    [ "$status" -eq 0 ]
    npm_groups=$(echo "$output" | grep -c 'chore(deps): bump the npm-')
    [ "$npm_groups" -ge 2 ]
}

@test "短い group 名は成功する" {
    write_config "chore(deps)" npm-prod npm-dev
    run "$SCRIPT"
    [ "$status" -eq 0 ]
    [[ "$output" == *"chore(deps): bump the npm-prod group across 1 directory with 99 updates"* ]]
    [[ "$output" == *"chore(deps): bump the npm-dev group across 1 directory with 99 updates"* ]]
}

@test "npm_and_yarn group（#720 の件名）は header-max-length 超過で失敗する" {
    write_config "chore(deps)" npm_and_yarn
    run "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"chore(deps): bump the npm_and_yarn group across 1 directory with 99 updates"* ]]
    [[ "$output" == *"header-max-length"* ]]
}

@test "commit-message の prefix を件名に反映する" {
    write_config "chore(dependencies-long)" npm-dev
    run "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"chore(dependencies-long): bump the npm-dev group"* ]]
}

@test "group が 1 つも無い設定はエラー終了する" {
    printf 'version: 2\nupdates:\n  - package-ecosystem: "npm"\n    directory: "/"\n' >"$DEPENDABOT_CONFIG"
    run "$SCRIPT"
    [ "$status" -eq 2 ]
}

@test "設定ファイルが無い場合はエラー終了する" {
    rm -f "$DEPENDABOT_CONFIG"
    run "$SCRIPT"
    [ "$status" -eq 2 ]
}

# --- Issue #751: dependabot.yml の prefix と commitlint.config.js の除外条件の整合 ---

@test "リポジトリの dependabot.yml は全エントリで commit-message の prefix を明示している" {
    DEPENDABOT_CONFIG="$REPO_CONFIG" run "$SCRIPT"
    [ "$status" -eq 0 ]
    [[ "$output" == *"gitsubmodule"* ]] || false
    [[ "$output" != *"prefix が未設定"* ]] || false
}

@test "commit-message の prefix が未設定のエントリがあると失敗する" {
    write_config "chore(deps)" npm-dev
    {
        echo '  - package-ecosystem: "gitsubmodule"'
        echo '    directory: "/"'
    } >>"$DEPENDABOT_CONFIG"
    run "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"gitsubmodule"*"prefix が未設定"* ]] || false
}

@test "commitlint の除外対象外の prefix は 1 件更新の長い件名で失敗する" {
    write_config "build(deps)" npm-dev
    run "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"npm: build(deps): bump "* ]] || false
    [[ "$output" == *"header-max-length"* ]] || false
}

@test "除外対象の prefix は 1 件更新の長い件名でも Dependabot コミットとして通る" {
    write_config "chore(deps)" npm-dev
    run "$SCRIPT"
    [ "$status" -eq 0 ]
    [[ "$output" == *"✓ npm: chore(deps): bump "* ]] || false
}
