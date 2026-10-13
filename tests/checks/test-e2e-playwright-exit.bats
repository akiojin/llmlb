#!/usr/bin/env bats

setup() {
    ROOT="$BATS_TEST_DIRNAME/../.."
    FIXTURE="$BATS_TEST_TMPDIR/worktree"
    mkdir -p "$FIXTURE/llmlb/tests/e2e-playwright" "$BATS_TEST_TMPDIR/bin"
    export E2E_FIXTURE_RUNNER="$ROOT/node_modules/.bin/playwright"
    export E2E_FIXTURE_CONFIG="$BATS_TEST_TMPDIR/playwright.config.cjs"
    export E2E_FIXTURE_REPORT="$BATS_TEST_TMPDIR/results.json"
    export E2E_FIXTURE_MODE=success
    export PLAYWRIGHT_HTML_OPEN=never
    export PATH="$BATS_TEST_TMPDIR/bin:$PATH"

    # Run the real CLI without a browser/server; only redirect pnpm to this fixture.
    cat > "$BATS_TEST_TMPDIR/bin/pnpm" <<'SH'
#!/bin/sh
if [ "$*" != 'exec playwright test --project=chromium --grep-invert @real-runtimes' ]; then
    echo "Unexpected Playwright command: $*" >&2
    exit 64
fi
shift 2
exec "$E2E_FIXTURE_RUNNER" "$@" --config "$E2E_FIXTURE_CONFIG"
SH
    chmod +x "$BATS_TEST_TMPDIR/bin/pnpm"
    cat > "$E2E_FIXTURE_CONFIG" <<'JS'
module.exports = {
  testDir: __dirname,
  testMatch: 'exit-fixture.spec.cjs',
  retries: 1,
  workers: 1,
  projects: [{ name: 'chromium' }],
  reporter: [['json', { outputFile: process.env.E2E_FIXTURE_REPORT }]],
};
JS
    cat > "$BATS_TEST_TMPDIR/exit-fixture.spec.cjs" <<JS
const { test, expect } = require('$ROOT/node_modules/@playwright/test');
test('exit propagation fixture', async ({}, info) => {
  if (process.env.E2E_FIXTURE_MODE === 'failure') expect('actual').toBe('expected');
  if (process.env.E2E_FIXTURE_MODE === 'flaky') expect(info.retry).toBe(1);
});
JS
}

run_e2e() {
    run make --no-print-directory -C "$FIXTURE" -f "$ROOT/Makefile" e2e-playwright
}

@test "make e2e-playwright preserves Playwright success" {
    run_e2e
    [ "$status" -eq 0 ]
    run node -e 'const r = require(process.argv[1]); process.exit(r.stats.expected === 1 && r.stats.unexpected === 0 ? 0 : 1)' "$E2E_FIXTURE_REPORT"
    [ "$status" -eq 0 ]
}

@test "make e2e-playwright rejects Playwright failure after retries" {
    export E2E_FIXTURE_MODE=failure
    run_e2e
    [ "$status" -ne 0 ]
    run node -e 'const r = require(process.argv[1]); process.exit(r.stats.unexpected === 1 ? 0 : 1)' "$E2E_FIXTURE_REPORT"
    [ "$status" -eq 0 ]
}

@test "make e2e-playwright accepts real retry success as flaky" {
    export E2E_FIXTURE_MODE=flaky
    run_e2e
    [ "$status" -eq 0 ]
    run node -e 'const r = require(process.argv[1]); process.exit(r.stats.flaky === 1 && r.stats.unexpected === 0 ? 0 : 1)' "$E2E_FIXTURE_REPORT"
    [ "$status" -eq 0 ]
}

@test "make quality-checks propagates failure from its real e2e-playwright prerequisite" {
    export E2E_FIXTURE_MODE=failure
    # Isolate unrelated prerequisites; keep the real quality-checks dependency graph.
    awk '/^quality-checks:/ {
        for (i = 2; i <= NF; i++) if ($i != "e2e-playwright") print $i ":\n\t@:"
    }' "$ROOT/Makefile" > "$BATS_TEST_TMPDIR/unrelated-checks.mk"
    run make --no-print-directory -C "$FIXTURE" -f "$ROOT/Makefile" -f "$BATS_TEST_TMPDIR/unrelated-checks.mk" quality-checks
    [ "$status" -ne 0 ]
    run node -e 'const r = require(process.argv[1]); process.exit(r.stats.unexpected === 1 ? 0 : 1)' "$E2E_FIXTURE_REPORT"
    [ "$status" -eq 0 ]
}
