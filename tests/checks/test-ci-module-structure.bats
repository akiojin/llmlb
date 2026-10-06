#!/usr/bin/env bats

# module-structure（SPEC #699 FR-009/FR-010）の CI 配線テスト（Issue #771）
# pre-push だけで走っていたため、1,500 行超のファイルが CI 緑のまま develop に着地した。
# 必須チェック「Rust Format & Clippy」がこの検査を実行し、関連ファイルの変更で起動することを保証する。

setup() {
    LINT_WORKFLOW="$BATS_TEST_DIRNAME/../../.github/workflows/lint.yml"
    SCRIPT="$BATS_TEST_DIRNAME/../../scripts/checks/check-module-structure.sh"
    [ -f "$LINT_WORKFLOW" ]
}

# lint.yml から指定ジョブ（2 スペースインデントのキー）のブロックを抜き出す
job_block() {
    awk -v job="  $1:" '
        $0 == job { in_job = 1; print; next }
        in_job && /^  [A-Za-z0-9_-]+:/ { exit }
        in_job { print }
    ' "$LINT_WORKFLOW"
}

@test "必須チェック Rust Format & Clippy が module-structure を実行する" {
    block="$(job_block rust-lint)"
    [[ "$block" == *"name: Rust Format & Clippy"* ]] || false
    [[ "$block" == *"make module-structure"* ]] || false
}

@test "module-structure の検査スクリプト変更で Rust lint が起動する" {
    block="$(job_block changes)"
    [[ "$block" == *"'scripts/checks/check-module-structure.sh'"* ]] || false
}

@test "module-structure の許可リスト変更で Rust lint が起動する" {
    block="$(job_block changes)"
    [[ "$block" == *"'scripts/checks/module-structure-allowlist.txt'"* ]] || false
}

@test "上限超過ファイルがあると module-structure は失敗する" {
    src="$BATS_TEST_TMPDIR/src"
    mkdir -p "$src"
    printf '# empty\n' > "$BATS_TEST_TMPDIR/allowlist.txt"
    for _ in 1 2 3 4; do echo '// line'; done > "$src/big.rs"
    SRC_DIR="$src" ALLOWLIST="$BATS_TEST_TMPDIR/allowlist.txt" MAX_LINES=3 run bash "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"FR-009: big.rs"* ]] || false
}

# Issue #811: テストコードも同じ上限で守る（3,000 行超の tests.rs が CI 緑のまま残っていた）
@test "tests.rs も行数上限の対象である" {
    src="$BATS_TEST_TMPDIR/src"
    mkdir -p "$src"
    printf '# empty\n' > "$BATS_TEST_TMPDIR/allowlist.txt"
    for _ in 1 2 3 4; do echo '// line'; done > "$src/tests.rs"
    SRC_DIR="$src" ALLOWLIST="$BATS_TEST_TMPDIR/allowlist.txt" MAX_LINES=3 run bash "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"FR-009: tests.rs"* ]] || false
}

@test "tests/ 配下へ分割したテストファイルも行数上限の対象である" {
    src="$BATS_TEST_TMPDIR/src"
    mkdir -p "$src/foo/tests"
    printf '# empty\n' > "$BATS_TEST_TMPDIR/allowlist.txt"
    echo 'mod part;' > "$src/foo/tests.rs"
    for _ in 1 2 3 4; do echo '// line'; done > "$src/foo/tests/part.rs"
    SRC_DIR="$src" ALLOWLIST="$BATS_TEST_TMPDIR/allowlist.txt" MAX_LINES=3 run bash "$SCRIPT"
    [ "$status" -eq 1 ]
    [[ "$output" == *"FR-009: foo/tests/part.rs"* ]] || false
}

@test "上限内の tests.rs は通る" {
    src="$BATS_TEST_TMPDIR/src"
    mkdir -p "$src"
    printf '# empty\n' > "$BATS_TEST_TMPDIR/allowlist.txt"
    for _ in 1 2 3; do echo '// line'; done > "$src/tests.rs"
    SRC_DIR="$src" ALLOWLIST="$BATS_TEST_TMPDIR/allowlist.txt" MAX_LINES=3 run bash "$SCRIPT"
    [ "$status" -eq 0 ]
}

@test "リポジトリの llmlb/src に上限超過ファイルが無い" {
    cd "$BATS_TEST_DIRNAME/../.."
    run bash "$SCRIPT"
    [ "$status" -eq 0 ]
}
