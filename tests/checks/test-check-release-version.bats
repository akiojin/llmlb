#!/usr/bin/env bats

# check-release-version.sh の契約テスト（Issue #813）
# develop が main より進んでいるのにワークスペースバージョンが main と同じままだと、
# リリースしても release.yml が既存タグを見つけてタグ・Release・配布をすべて skip する。
# その状態をリリース前（ローカル検証）に検出できることを確認する。

setup() {
    SCRIPT="$BATS_TEST_DIRNAME/../../scripts/checks/check-release-version.sh"
    MAKEFILE="$BATS_TEST_DIRNAME/../../Makefile"
    [ -x "$SCRIPT" ]

    # 一時リポジトリは必ず git_sandbox_init 経由で作る（実リポジトリへの書き込み防止）
    load helpers/git-sandbox
    REPO="$BATS_TEST_TMPDIR/repo"
    git_sandbox_init "$REPO"
    write_manifest 6.1.0
    commit_all "release 6.1.0"
    git tag main-ref
    export MAIN_REF=main-ref DEVELOP_REF=develop-ref HEAD_REF=HEAD
}

# $1: [workspace.package] の version、$2: 依存クレートの version（省略可）
write_manifest() {
    cat > Cargo.toml <<EOF
[workspace]
members = ["llmlb"]

[workspace.package]
version = "$1"
edition = "2021"

[workspace.dependencies]
serde = { version = "1.0" }

[workspace.metadata.example]
version = "${2:-0.0.1}"
EOF
}

commit_all() {
    git add -A
    git commit -q -m "$1"
}

add_feature() {
    echo "$1" >> feature.txt
    commit_all "feat: $1"
}

# 現在の HEAD を develop の着地済み状態として記録する
land_on_develop() {
    git tag -f develop-ref >/dev/null
}

@test "develop が main と同じ内容なら成功する" {
    land_on_develop
    run "$SCRIPT"
    [ "$status" -eq 0 ]
}

@test "develop が進んでいるのにバージョンが main と同じなら失敗する" {
    add_feature one
    land_on_develop
    run "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"6.1.0"* ]] || false
    [[ "$output" == *"v6.1.0"* ]] || false
}

@test "着地候補の HEAD がバージョンを上げていれば成功する" {
    add_feature one
    land_on_develop
    write_manifest 6.2.0
    commit_all "chore(release): v6.2.0"
    run "$SCRIPT"
    [ "$status" -eq 0 ]
}

@test "develop がバージョンを上げていれば古い作業ブランチでも成功する" {
    git tag stale-base
    add_feature one
    write_manifest 6.2.0
    commit_all "chore(release): v6.2.0"
    land_on_develop
    git reset -q --hard stale-base
    add_feature two
    run "$SCRIPT"
    [ "$status" -eq 0 ]
}

@test "リリース直後に develop へ着地する最初の変更もバージョン更新が必要" {
    land_on_develop
    add_feature one
    run "$SCRIPT"
    [ "$status" -eq 1 ]
}

@test "main が develop をマージ済みなら成功する" {
    add_feature one
    write_manifest 6.2.0
    commit_all "chore(release): v6.2.0"
    land_on_develop
    git tag -f main-ref >/dev/null
    run "$SCRIPT"
    [ "$status" -eq 0 ]
}

@test "[workspace.package] 以外の version の変更はバージョン更新とみなさない" {
    add_feature one
    write_manifest 6.1.0 9.9.9
    commit_all "chore: bump unrelated version"
    land_on_develop
    run "$SCRIPT"
    [ "$status" -eq 1 ]
}

@test "ref を解決できなければエラー終了する" {
    land_on_develop
    MAIN_REF=no-such-ref run "$SCRIPT"
    [ "$status" -eq 2 ]
    [[ "$output" == *"no-such-ref"* ]] || false
}

@test "ワークスペースバージョンを読めなければエラー終了する" {
    printf '[workspace]\nmembers = ["llmlb"]\n' > Cargo.toml
    commit_all "chore: drop workspace version"
    land_on_develop
    run "$SCRIPT"
    [ "$status" -eq 2 ]
}

@test "make quality-checks が release-version を実行する" {
    prerequisites="$(awk '/^quality-checks:/ { print; exit }' "$MAKEFILE")"
    [[ " $prerequisites " == *" release-version "* ]] || false
    recipe="$(awk '/^release-version:/ { getline; print; exit }' "$MAKEFILE")"
    [[ "$recipe" == *"scripts/checks/check-release-version.sh"* ]] || false
}
