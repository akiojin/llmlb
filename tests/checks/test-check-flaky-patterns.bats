#!/usr/bin/env bats

setup() {
    ROOT="$BATS_TEST_DIRNAME/../.."
    SCRIPT="$ROOT/scripts/checks/check-flaky-patterns.sh"
    SRC="$BATS_TEST_TMPDIR/repo"
    ALLOW="$BATS_TEST_TMPDIR/allowlist.txt"
    mkdir -p "$SRC/llmlb/tests" "$SRC/llmlb/src"
    printf '# empty\n' > "$ALLOW"
}

check() {
    SCAN_ROOT="$SRC" ALLOWLIST="$ALLOW" run bash "$SCRIPT"
}

@test "a: 固定sleepとsetTimeoutとwaitForTimeoutは失敗する" {
    cat > "$SRC/llmlb/tests/bad.rs" <<'RS'
async fn test_transition() {
    tokio::time::sleep(Duration::from_millis(100)).await;
}
RS
    printf 'await new Promise(resolve => setTimeout(resolve, 100));\nawait page.waitForTimeout(100);\n' > "$SRC/llmlb/tests/bad.spec.ts"
    check
    [ "$status" -eq 1 ]
    [[ "$output" == *'fixed-wait'* ]] || false
}

@test "b: computed styleとtoHaveCSSの観測点を失敗にする" {
    cat > "$SRC/llmlb/tests/bad.spec.ts" <<'TS'
const bg = await badge.evaluate(el => getComputedStyle(el).backgroundColor);
expect(parseFloat(bg)).toBeLessThan(0.95);
await expect(badge).toHaveCSS('opacity', '0.5');
TS
    check
    [ "$status" -eq 1 ]
    [[ "$output" == *'computed-style'* ]] || false
}

@test "c: 固定名の共有temp・dataディレクトリとポートは失敗する" {
    cat > "$SRC/llmlb/tests/bad.rs" <<'RS'
let tmp = std::env::temp_dir().join("shared-test");
let data = dirs::home_dir().unwrap().join(".llmlb");
let listener = TcpListener::bind("127.0.0.1:8080").await;
RS
    check
    [ "$status" -eq 1 ]
    [[ "$output" == *'shared-resource'* ]] || false
}

@test "d: 広すぎるURL待ちとクリック後のnavigation event待ちは失敗する" {
    cat > "$SRC/llmlb/tests/bad.spec.ts" <<'TS'
await page.waitForURL('**/dashboard/**');
await page.waitForNavigation();
TS
    check
    [ "$status" -eq 1 ]
    [[ "$output" == *'navigation-race'* ]] || false
}

@test "動的tempdir・状態poll・production timeoutは許可する" {
    cat > "$SRC/llmlb/tests/good.rs" <<'RS'
let temp = tempfile::tempdir().unwrap();
let listener = TcpListener::bind("127.0.0.1:0").await;
RS
    printf 'await expect.poll(readStatus).toBe("Online");\nawait page.waitForURL("**/dashboard/#endpoints");\n' > "$SRC/llmlb/tests/good.spec.ts"
    printf 'pub async fn delay() { tokio::time::sleep(Duration::from_secs(1)).await; }\n' > "$SRC/llmlb/src/production.rs"
    check
    [ "$status" -eq 0 ]
}

@test "cfg(test) 内の固定sleepを見逃さない" {
    cat > "$SRC/llmlb/src/unit.rs" <<'RS'
pub fn production() {}
#[cfg(test)]
mod tests {
    async fn check() { sleep(Duration::from_secs(1)).await; }
}
RS
    check
    [ "$status" -eq 1 ]
}

@test "allowlist は内容ごとに許可し削除済み・追加違反を拒否する" {
    printf 'await page.waitForTimeout(100);\n' > "$SRC/llmlb/tests/bad.spec.ts"
    SCAN_ROOT="$SRC" ALLOWLIST="$ALLOW" run bash "$SCRIPT" --inventory
    [ "$status" -eq 0 ]
    printf '%s\n' "$output" > "$ALLOW"
    check
    [ "$status" -eq 0 ]
    printf 'await page.waitForTimeout(200);\n' >> "$SRC/llmlb/tests/bad.spec.ts"
    check
    [ "$status" -eq 1 ]
    printf '' > "$SRC/llmlb/tests/bad.spec.ts"
    check
    [ "$status" -eq 1 ]
    [[ "$output" == *'許可リスト不要'* ]] || false
}

@test "quality-checksと既存必須lintが検査を実行する" {
    deps="$(grep '^quality-checks:' "$ROOT/Makefile")"
    [[ " $deps " == *' flaky-patterns '* ]] || false
    grep -q 'make flaky-patterns' "$ROOT/.github/workflows/lint.yml"
    grep -q 'scripts/checks/check-flaky-patterns.sh' "$ROOT/.github/workflows/lint.yml"
}

@test "e: 固定mockルートと厳密なI/O回数assertの組合せは失敗する" {
    cat > "$SRC/llmlb/tests/bad.rs" <<'RS'
Mock::given(method("GET"))
    .and(path("/v1/models"))
    .respond_with(move |_| {
        call_count.fetch_add(1, Ordering::SeqCst);
        ResponseTemplate::new(200)
    });
assert_eq!(call_count.load(Ordering::SeqCst), 0);
RS
    check
    [ "$status" -eq 1 ]
    [[ "$output" == *'mock-count'* ]] || false
}

@test "test.setTimeoutは待機ではなくdeadlineで固定ポートの例示URLも共有資源ではない" {
    cat > "$SRC/llmlb/tests/good.spec.ts" <<'TS'
test.setTimeout(240_000);
const exampleEndpoint = { base_url: 'http://127.0.0.1:8080' };
TS
    check
    [ "$status" -eq 0 ]
}

@test "cfg(test) importや閉じたtest moduleの後のproductionは走査しない" {
    cat > "$SRC/llmlb/src/production.rs" <<'RS'
#[cfg(test)]
use std::time::Duration;
#[cfg(test)]
mod tests {
    fn safe() { let x = "{"; }
}
pub async fn production() { tokio::time::sleep(Duration::from_secs(1)).await; }
RS
    check
    [ "$status" -eq 0 ]
}

@test "c: 実data dirを暗黙利用するupdate constructorを捕捉する" {
    printf 'let manager = UpdateManager::new_with_config(config).await;\n' > "$SRC/llmlb/tests/old.rs"
    check
    [ "$status" -eq 1 ]
    [[ "$output" == *'shared-resource'* ]] || false
}

@test "整形で引数が改行された共有temp・port・広いURL待ちも失敗する" {
    cat > "$SRC/llmlb/tests/bad.rs" <<'RS'
let tmp = std::env::temp_dir()
    .join("shared-test");
let listener = TcpListener::bind(
    "127.0.0.1:8080"
).await;
RS
    cat > "$SRC/llmlb/tests/bad.spec.ts" <<'TS'
await page.waitForURL(
    '**/dashboard/**'
);
TS
    check
    [ "$status" -eq 1 ]
    [[ "$output" == *'shared-resource'* ]] || false
    [[ "$output" == *'navigation-race'* ]] || false
}
