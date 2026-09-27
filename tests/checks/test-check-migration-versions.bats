#!/usr/bin/env bats

# check-migration-versions.sh の契約テスト（Issue #737）
# 並行ブランチのマイグレーション番号衝突をローカル検証で検出できることを確認する。

setup() {
    SCRIPT="$BATS_TEST_DIRNAME/../../scripts/checks/check-migration-versions.sh"
    [ -x "$SCRIPT" ]

    # git フック（pre-push 等）から継承した GIT_DIR 等が残っていると、
    # 一時リポジトリへの操作が実リポジトリに漏れるため必ず解除する。
    unset $(git rev-parse --local-env-vars)
    export GIT_AUTHOR_NAME=test GIT_AUTHOR_EMAIL=test@example.com
    export GIT_COMMITTER_NAME=test GIT_COMMITTER_EMAIL=test@example.com

    REPO="$BATS_TEST_TMPDIR/repo"
    mkdir -p "$REPO/llmlb/migrations"
    cd "$REPO"
    git init -q
    [ "$(git rev-parse --show-toplevel)" = "$(pwd -P)" ]
    touch llmlb/migrations/001_init.sql
    touch llmlb/migrations/032_add_password_reset_tokens.sql
    commit_all base
    export BASE_REF=HEAD
}

commit_all() {
    git add -A
    git commit -q -m "$1"
}

add_migration() {
    touch "llmlb/migrations/$1"
}

@test "旧形式の既存マイグレーションのみなら成功する" {
    run "$SCRIPT"
    [ "$status" -eq 0 ]
}

@test "タイムスタンプ形式の新規マイグレーションは成功する" {
    add_migration 20260928010203_add_feature.sql
    run "$SCRIPT"
    [ "$status" -eq 0 ]
}

@test "凍結上限を超える連番の新規マイグレーションは失敗する" {
    add_migration 033_add_feature.sql
    run "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"033_add_feature.sql"* ]]
}

@test "日付として不正なタイムスタンプは失敗する" {
    add_migration 20261328010203_add_feature.sql
    run "$SCRIPT"
    [ "$status" -eq 1 ]
}

@test "桁数が不正なバージョンは失敗する" {
    add_migration 2026092801020_add_feature.sql
    run "$SCRIPT"
    [ "$status" -eq 1 ]
}

@test "作業ツリー内のバージョン重複は失敗する" {
    add_migration 20260928010203_add_a.sql
    add_migration 20260928010203_add_b.sql
    run "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"20260928010203"* ]]
}

@test "ベースに先行着地した同一バージョンとの衝突を検出する" {
    # ベースに add_a が着地し、作業ツリーは着地前に分岐して add_b を追加した状態
    add_migration 20260928010203_add_a.sql
    commit_all landed
    rm llmlb/migrations/20260928010203_add_a.sql
    add_migration 20260928010203_add_b.sql
    run "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"20260928010203_add_a.sql"* ]]
}

@test "ベースと同名のファイルは衝突とみなさない" {
    add_migration 20260928010203_add_a.sql
    commit_all landed
    add_migration 20260928020000_add_b.sql
    run "$SCRIPT"
    [ "$status" -eq 0 ]
}

@test "ベース ref が解決できない場合はエラー終了する" {
    BASE_REF=no-such-ref run "$SCRIPT"
    [ "$status" -eq 2 ]
}
