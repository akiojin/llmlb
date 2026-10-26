#!/usr/bin/env bats

# Issue #948: provider changes must compile, lint and test on all supported OSes.
setup() {
    ROOT="$BATS_TEST_DIRNAME/../.."
    WORKFLOW="$ROOT/.github/workflows/test.yml"
}

@test "JWT provider の Rust CI は Linux macOS Windows を実行する" {
    block="$(awk '/^  rust-test:/ { active=1; next } active && /^  [a-zA-Z_-]+:/ { exit } active { print }' "$WORKFLOW")"
    [[ "$block" == *'os: [ubuntu-latest, macos-latest, windows-latest]'* ]]
    [[ "$block" == *'cargo build --workspace --all-features --locked'* ]]
    [[ "$block" == *'cargo clippy --all-targets --all-features -- -D warnings'* ]]
    [[ "$block" == *'cargo test --all-features --workspace -- --test-threads=1'* ]]
    [[ "$block" == *'components: clippy'* ]]
}
