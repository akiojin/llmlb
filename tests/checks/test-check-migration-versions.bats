#!/usr/bin/env bats

# check-migration-versions.sh の契約テスト（Issue #737）
# 並行ブランチのマイグレーション番号衝突をローカル検証で検出できることを確認する。

setup() {
    SCRIPT="$BATS_TEST_DIRNAME/../../scripts/checks/check-migration-versions.sh"
    [ -x "$SCRIPT" ]

    REPO="$BATS_TEST_TMPDIR/repo"
    mkdir -p "$REPO/llmlb/migrations"
    cd "$REPO"
    git init -q -b develop
    git config user.email test@example.com
    git config user.name test
    touch llmlb/migrations/001_init.sql
    touch llmlb/migrations/032_add_password_reset_tokens.sql
    git add -A
    git commit -q -m base
    export BASE_REF=develop
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
    git checkout -q -b feature
    git checkout -q develop
    add_migration 20260928010203_add_a.sql
    git add -A
    git commit -q -m landed
    git checkout -q feature
    add_migration 20260928010203_add_b.sql
    run "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"20260928010203_add_a.sql"* ]]
}

@test "ベースと同名のファイルは衝突とみなさない" {
    git checkout -q -b feature
    git checkout -q develop
    add_migration 20260928010203_add_a.sql
    git add -A
    git commit -q -m landed
    git checkout -q feature
    git merge -q develop
    add_migration 20260928020000_add_b.sql
    run "$SCRIPT"
    [ "$status" -eq 0 ]
}

@test "ベース ref が解決できない場合はエラー終了する" {
    BASE_REF=no-such-ref run "$SCRIPT"
    [ "$status" -eq 2 ]
}
