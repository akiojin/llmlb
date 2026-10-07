#!/usr/bin/env bats

setup() {
    ROOT="$BATS_TEST_DIRNAME/../.."
    COLLECTOR="$ROOT/scripts/checks/collect-flaky-tests.mjs"
    OUT="$BATS_TEST_TMPDIR/flaky-tests.md"
}

collect() {
    run node "$COLLECTOR" --output "$OUT" "$@"
}

@test "Playwright retry 成功だけを共通台帳と件数に記録する" {
    cat > "$BATS_TEST_TMPDIR/results.json" <<'JSON'
{"stats":{"flaky":1,"expected":1,"unexpected":0,"skipped":0,"startTime":"2026-10-08T00:00:00Z","duration":100},"suites":[{"title":"dashboard","suites":[{"specs":[{"title":"Offline alpha","file":"status.spec.ts","tests":[{"projectName":"chromium","status":"flaky","results":[{"status":"failed"},{"status":"passed","retry":1}]},{"projectName":"webkit","status":"expected","results":[{"status":"passed"}]}]}]}]}]}
JSON
    collect --playwright "$BATS_TEST_TMPDIR/results.json" --date 2026-10-08
    [ "$status" -eq 0 ]
    [[ "$output" == *'"playwright_flaky":1'* ]] || false
    grep -q 'status.spec.ts.*Offline alpha.*2026-10-08' "$OUT"
    ! grep -q webkit "$OUT"
}

@test "Rust は初回 FAILED から同じ harness の ok に変わったテストだけ記録する" {
    cat > "$BATS_TEST_TMPDIR/first.log" <<'LOG'
     Running unittests src/lib.rs (target/debug/deps/llmlb-ab12)
test update::transition ... FAILED
test still_broken ... FAILED
test stable ... ok
test result: FAILED. 1 passed; 2 failed;
     Running tests/other.rs (target/debug/deps/other-cd34)
test update::transition ... ok
test result: ok. 1 passed; 0 failed;
LOG
    cat > "$BATS_TEST_TMPDIR/retry.log" <<'LOG'
     Running unittests src/lib.rs (target/debug/deps/llmlb-ab12)
test update::transition ... ok
test still_broken ... FAILED
test stable ... ok
test result: FAILED. 1 passed; 2 failed;
     Running tests/other.rs (target/debug/deps/other-cd34)
test update::transition ... ok
test result: ok. 1 passed; 0 failed;
LOG
    collect --rust-first "$BATS_TEST_TMPDIR/first.log" --rust-retry "$BATS_TEST_TMPDIR/retry.log" --date 2026-10-08
    [ "$status" -eq 0 ]
    [[ "$output" == *'"rust_flaky":1'* ]] || false
    grep -q 'src/lib.rs.*update::transition' "$OUT"
    ! grep -q still_broken "$OUT"
    ! grep -q tests/other.rs "$OUT"
}

@test "Rust の別 harness の同名成功を再実行成功と誤認しない" {
    printf '     Running unittests src/lib.rs (target/debug/deps/lib-a)\ntest same ... FAILED\ntest result: FAILED. 0 passed; 1 failed;\n' > "$BATS_TEST_TMPDIR/first.log"
    printf '     Running tests/other.rs (target/debug/deps/other-b)\ntest same ... ok\ntest result: ok. 1 passed; 0 failed;\n' > "$BATS_TEST_TMPDIR/retry.log"
    collect --rust-first "$BATS_TEST_TMPDIR/first.log" --rust-retry "$BATS_TEST_TMPDIR/retry.log"
    [ "$status" -eq 0 ]
    [[ "$output" == *'"rust_flaky":0'* ]] || false
}

@test "欠損・壊れた Playwright report をゼロ件に見せない" {
    collect --playwright "$BATS_TEST_TMPDIR/missing.json"
    [ "$status" -ne 0 ]
    printf '{"stats":{"flaky":1,"expected":1,"unexpected":0,"skipped":0,"startTime":"2026-10-08T00:00:00Z","duration":100}}' > "$BATS_TEST_TMPDIR/results.json"
    collect --playwright "$BATS_TEST_TMPDIR/results.json"
    [ "$status" -ne 0 ]
}

@test "CI は reporter を上書きせず成功時もフレーク台帳を公開する" {
    workflow="$ROOT/.github/workflows/test.yml"
    ! grep -q -- '--reporter=list' "$workflow"
    grep -q 'collect-flaky-tests.mjs.*--playwright' "$workflow"
    grep -q 'collect-flaky-tests.mjs.*--rust-first' "$workflow"
    grep -q 'name: flaky-tests-rust-' "$workflow"
    grep -q 'name: flaky-tests-playwright' "$workflow"
    grep -qF 'cargo test --all-features --workspace -- --test-threads=1' "$workflow"
}

@test "途中・空のrunner結果をフレークゼロと報告しない" {
    printf '{"suites":[],"stats":{"flaky":0}}' > "$BATS_TEST_TMPDIR/results.json"
    collect --playwright "$BATS_TEST_TMPDIR/results.json"
    [ "$status" -ne 0 ]
    printf '' > "$BATS_TEST_TMPDIR/first.log"
    collect --rust-first "$BATS_TEST_TMPDIR/first.log"
    [ "$status" -ne 0 ]
}

@test "最後のRust harnessが途中なら前harnessのsummaryで完了扱いしない" {
    cat > "$BATS_TEST_TMPDIR/first.log" <<'LOG'
     Running unittests src/lib.rs (target/debug/deps/lib-a)
test good ... ok
test result: ok. 1 passed; 0 failed;
     Running tests/other.rs (target/debug/deps/other-b)
test incomplete ... FAILED
LOG
    collect --rust-first "$BATS_TEST_TMPDIR/first.log"
    [ "$status" -ne 0 ]
}

@test "実Playwright reporterのretry成功を共通台帳へ変換する（ブラウザ不要）" {
    cat > "$BATS_TEST_TMPDIR/playwright.config.cjs" <<CONFIG
module.exports = {
  testDir: '$BATS_TEST_TMPDIR',
  testMatch: 'reporter-fixture.spec.cjs',
  retries: 1,
  reporter: [['json', { outputFile: '$BATS_TEST_TMPDIR/real-results.json' }]],
};
CONFIG
    cat > "$BATS_TEST_TMPDIR/reporter-fixture.spec.cjs" <<TEST
const { test, expect } = require('$ROOT/node_modules/@playwright/test');
test('diagnostic retry fixture', async ({}, info) => { expect(info.retry).toBe(1); });
TEST
    run "$ROOT/node_modules/.bin/playwright" test --config "$BATS_TEST_TMPDIR/playwright.config.cjs"
    [ "$status" -eq 0 ]
    collect --playwright "$BATS_TEST_TMPDIR/real-results.json"
    [ "$status" -eq 0 ]
    [[ "$output" == *'"playwright_flaky":1'* ]] || false
    grep -q 'diagnostic retry fixture' "$OUT"
}

@test "Rust初回ログのteeはcargo失敗を隠さない" {
    body="$(awk '
        /name: Run Rust tests/ { step=1; next }
        step && /^        run: \|/ { body=1; next }
        body && /^          / { sub(/^          /, ""); print; next }
        body { exit }
    ' "$ROOT/.github/workflows/test.yml" | sed 's/${{ matrix.os }}/fixture/g')"
    cargo() { printf 'test diagnostic ... FAILED\n'; return 1; }
    export -f cargo
    # Exercise GitHub Actions explicit bash fail-fast/pipefail semantics in an isolated directory.
    run bash -e -o pipefail -c 'cd "$1"; eval "$2"' -- "$BATS_TEST_TMPDIR" "$body"
    [ "$status" -eq 1 ]
    grep -q '^test diagnostic .* FAILED' "$BATS_TEST_TMPDIR/reports/flaky/rust-first.log"
}

@test "CI再実行は前attemptのartifactを上書きせず観測runもattemptを指す" {
    workflow="$ROOT/.github/workflows/test.yml"
    grep -qF 'name: flaky-tests-rust-${{ matrix.os }}-${{ github.run_attempt }}' "$workflow"
    grep -qF 'name: flaky-tests-playwright-${{ github.run_attempt }}' "$workflow"
    grep -qF '/attempts/${{ github.run_attempt }}' "$workflow"
}
