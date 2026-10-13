#!/usr/bin/env bats

# Issue #889: bash 3.2 が非ASCII文字を変数名に取り込む書き方を再発させない。

setup() {
    REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/../.." && pwd)"
    SCRIPT="$REPO_ROOT/scripts/checks/check-shell-variable-boundaries.sh"
    SCRIPTS="$BATS_TEST_TMPDIR/scripts with spaces"
    mkdir -p "$SCRIPTS/nested"
}

run_check() {
    SCRIPTS_DIR="$SCRIPTS" run /bin/bash "$SCRIPT"
}

@test "非ASCII文字が直後に続く変数を全箇所診断して拒否する" {
    cat > "$SCRIPTS/nested/unsafe.sh" <<'EOF'
#!/bin/bash
echo "$name）"
echo "$BASE_REF、"
echo "$value日本語"
EOF
    run_check
    [ "$status" -eq 1 ]
    [[ "$output" == *"nested/unsafe.sh:2:"* ]] || false
    [[ "$output" == *"nested/unsafe.sh:3:"* ]] || false
    [[ "$output" == *"nested/unsafe.sh:4:"* ]] || false
    [[ "$output" == *'${name}'* ]] || false
}

@test "波括弧形式とASCII境界は拒否しない" {
    cat > "$SCRIPTS/safe.sh" <<'EOF'
#!/bin/bash
echo "${name}） ${value}日本語 $name: $name/path $name.txt $name suffix"
EOF
    run_check
    [ "$status" -eq 0 ]
    [[ "$output" == *"shell variable boundaries OK"* ]] || false
}

@test "bashとbats拡張子も検査する" {
    printf 'echo "$name）"\n' > "$SCRIPTS/helper.bash"
    printf 'echo "$name、"\n' > "$SCRIPTS/check.bats"
    run_check
    [ "$status" -eq 1 ]
    [[ "$output" == *"helper.bash:1:"* ]] || false
    [[ "$output" == *"check.bats:1:"* ]] || false
}

@test "シェル以外のファイルは拒否しない" {
    printf 'Example: $name）\n' > "$SCRIPTS/notes.md"
    run_check
    [ "$status" -eq 0 ]
}

@test "UTF-8とCロケールで同じ違反を検出する" {
    printf 'echo "$name）"\n' > "$SCRIPTS/unsafe.sh"
    LC_ALL=C run_check
    [ "$status" -eq 1 ]
    c_output="$output"
    LC_ALL=C.UTF-8 run_check
    [ "$status" -eq 1 ]
    [ "$output" = "$c_output" ]
}

@test "対象ディレクトリがなければエラーにする" {
    SCRIPTS_DIR="$BATS_TEST_TMPDIR/missing" run /bin/bash "$SCRIPT"
    [ "$status" -eq 2 ]
    [[ "$output" == *"missing"* ]] || false
}

@test "make quality-checksが変数境界検査を実行する" {
    targets="$(grep -E '^quality-checks:' "$REPO_ROOT/Makefile")"
    [[ " $targets " == *" shell-variable-boundaries "* ]] || false
    run make -n -C "$REPO_ROOT" shell-variable-boundaries
    [ "$status" -eq 0 ]
    [[ "$output" == *"bash scripts/checks/check-shell-variable-boundaries.sh"* ]] || false
}

@test "Linux必須ジョブがパス条件なしで変数境界検査を実行する" {
    block="$(awk '
        /^  commitlint:/ { in_job = 1 }
        in_job && /^  [A-Za-z0-9_-]+:/ && !/^  commitlint:/ { exit }
        in_job { print }
    ' "$REPO_ROOT/.github/workflows/lint.yml")"
    [[ "$block" == *"name: Commit Message Lint"* ]] || false
    [[ "$block" == *"runs-on: ubuntu-latest"* ]] || false
    [[ "$block" == *"run: make shell-variable-boundaries"* ]] || false
    step="$(printf '%s\n' "$block" | awk '
        /^      - name:/ { step = $0 ORS; next }
        { step = step $0 ORS }
        /run: make shell-variable-boundaries/ { print step }
    ')"
    [[ "$step" != *"if:"* ]] || false
    [[ "$step" != *"continue-on-error:"* ]] || false
    grep -q '"context": "Commit Message Lint"' "$REPO_ROOT/.github/config/branch-protection/develop.json"
}

@test "リポジトリの全scriptsシェルが変数境界検査を通過する" {
    SCRIPTS_DIR="$REPO_ROOT/scripts" run /bin/bash "$SCRIPT"
    [ "$status" -eq 0 ]
}
