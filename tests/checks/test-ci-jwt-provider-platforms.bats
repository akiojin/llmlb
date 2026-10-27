#!/usr/bin/env bats

# Issue #948: provider changes must compile, lint and test on all supported OSes.
setup() {
    ROOT="$BATS_TEST_DIRNAME/../.."
    WORKFLOW="$ROOT/.github/workflows/test.yml"
}

@test "Rust の観測ログ収集はテスト成功・失敗時だけ実行する" {
    for step in "Record Rust flaky tests" "Upload Rust flake observations"; do
        condition="$(awk -v step="$step" '
            index($0, "- name: " step) { active=1; next }
            active && /^      - / { exit }
            active && /^        if: / { sub(/^        if: /, ""); print; exit }
        ' "$WORKFLOW")"
        run node - "$condition" <<'JS'
const assert = require('node:assert/strict');
const condition = process.argv[2]
  .replaceAll('always()', 'true')
  .replaceAll('needs.changes.outputs.rust', 'changed')
  .replaceAll('steps.rust-tests.outcome', 'outcome');
const shouldRun = new Function('changed', 'outcome', `return (${condition});`);
for (const outcome of ['success', 'failure', 'skipped', 'cancelled', undefined]) {
  assert.equal(shouldRun('true', outcome), outcome === 'success' || outcome === 'failure', String(outcome));
  assert.equal(shouldRun('false', outcome), false, `no Rust changes: ${outcome}`);
}
JS
        if [ "$status" -ne 0 ]; then
            printf '%s: %s\n' "$step" "$output"
            return 1
        fi
    done
}

@test "JWT provider の Rust CI は Linux macOS Windows を実行する" {
    block="$(awk '/^  rust-test:/ { active=1; next } active && /^  [a-zA-Z_-]+:/ { exit } active { print }' "$WORKFLOW")"
    [[ "$block" == *'os: [ubuntu-latest, macos-latest, windows-latest]'* ]]
    [[ "$block" == *'cargo build --workspace --all-features --locked'* ]]
    [[ "$block" == *'cargo clippy --all-targets --all-features -- -D warnings'* ]]
    [[ "$block" == *'cargo test --all-features --workspace -- --test-threads=1'* ]]
    [[ "$block" == *'components: clippy'* ]]
}
