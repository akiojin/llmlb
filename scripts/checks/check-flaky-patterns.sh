#!/usr/bin/env bash
# SPEC #838: conservative candidate detection, scoped to test code. See docs/flaky-tests.md.
# Allowlist key: pattern|relative-path|line-content checksum|occurrence (line moves are harmless).
# --inventory prints candidates for individual review; never auto-allow new violations in CI.
set -euo pipefail
export LC_ALL=C
SCAN_ROOT="${SCAN_ROOT:-.}"
ALLOWLIST="${ALLOWLIST:-scripts/checks/flaky-patterns-allowlist.txt}"
mode="${1:-check}"
case "$mode" in check|--inventory) ;; *) echo 'Usage: check-flaky-patterns.sh [--inventory]' >&2; exit 2;; esac
[ -d "$SCAN_ROOT/llmlb" ] && [ -f "$ALLOWLIST" ] || { echo '対象ディレクトリ/allowlist がありません' >&2; exit 2; }
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
find "$SCAN_ROOT/llmlb" -type d \( -name node_modules -o -name static -o -name reports -o -name .git \) -prune -o -type f \( -name '*.rs' -o -name '*.ts' -o -name '*.tsx' \) -print | sort > "$tmp/files"
: > "$tmp/candidates"
while IFS= read -r file; do
    path="${file#"$SCAN_ROOT/"}"
    all=0
    case "$path" in */tests/*|*/tests.rs|*.test.ts|*.test.tsx|*.spec.ts|*.spec.tsx) all=1 ;; esac
    awk -v all="$all" '
        function emit(pattern) { text=$0; gsub(/^[ \t]+|[ \t]+$/, "", text); print pattern "|" FNR "|" text }
        FNR==NR { if ($0 ~ /fetch_add|\.expect\([0-9]+\)|assert_eq!.*(count|calls)/) strict=1; next }
        /^[ \t]*#\[cfg\(test\)\]/ { pending=1; next }
        pending && /^[ \t]*#\[/ { next }
        pending && $0 !~ /^[ \t]*(\/\/|$)/ { unit=1; pending=0; depth=0; block=0 }
        !all && !unit { next }
        /^[ \t]*(\/\/|\/\*|\*|$)/ { next }
        /(^|[^A-Za-z_])(sleep|waitForTimeout)[ \t]*\(|(^|[^A-Za-z_.])setTimeout[ \t]*\(/ { emit("fixed-wait") }
        /getComputedStyle[ \t]*\(|\.toHaveCSS[ \t]*\(/ { emit("computed-style") }
        temp_pending>0 { temp_pending-- }
        port_pending>0 { port_pending-- }
        /temp_dir\(\)/ { temp_pending=4 }
        /(bind|listen)[ \t]*\(/ { port_pending=4 }
        temp_pending && /\.join[ \t]*\("/ { emit("shared-resource") }
        port_pending && /(127\.0\.0\.1|localhost|0\.0\.0\.0):[1-9][0-9]*/ { emit("shared-resource") }
        /UpdateManager::new(_with_config)?[ \t]*\(|home_dir\(|data_dir\(|(API_BASE|baseURL).*(127\.0\.0\.1:[1-9][0-9]*|localhost:[1-9][0-9]*|0\.0\.0\.0:[1-9][0-9]*)|\.listen\([1-9][0-9]*/ { emit("shared-resource") }
        /waitForNavigation[ \t]*\(|(\*\*\/dashboard\/\*\*|\*\*\/login\*\*)|\.goto.*\/dashboard\/#/ { emit("navigation-race") }
        strict && /path[ \t]*\("\/[^\"]*"\)/ { emit("mock-count") }
        unit && !all {
            code=$0
            gsub(/"([^"\\]|\\.)*"/, "", code)
            sub(/\/\/.*$/, "", code)
            opened=gsub(/\{/, "{", code); closed=gsub(/\}/, "}", code)
            if (opened) block=1
            depth+=opened-closed
            if ((block && depth<=0) || (!block && code ~ /;/)) unit=0
        }
    ' "$file" "$file" | while IFS='|' read -r pattern line content; do
        checksum=$(printf '%s' "$content" | cksum | awk '{print $1 "-" $2}')
        printf '%s|%s|%s\t%s:%s %s\n' "$pattern" "$path" "$checksum" "$path" "$line" "$content" >> "$tmp/candidates"
    done
done < "$tmp/files"
awk -F '\t' '{ count[$1]++; print $1 "|" count[$1] "\t" $2 }' "$tmp/candidates" > "$tmp/numbered"
cut -f1 "$tmp/numbered" | sort > "$tmp/keys"
if [ "$mode" = --inventory ]; then cat "$tmp/keys"; exit 0; fi
awk 'NF && $0 !~ /^#/ { print }' "$ALLOWLIST" | sort > "$tmp/allowed"
status=0
while IFS= read -r key; do
    if ! grep -qxF "$key" "$tmp/allowed"; then
        echo "✗ フレーク誘発パターン: $key" >&2
        awk -F '\t' -v key="$key" '$1==key { print "    " $2 }' "$tmp/numbered" >&2
        status=1
    fi
done < "$tmp/keys"
while IFS= read -r key; do
    if ! grep -qxF "$key" "$tmp/keys"; then echo "✗ 許可リスト不要: $key" >&2; status=1; fi
done < "$tmp/allowed"
if [ "$status" -eq 0 ]; then echo "✓ flaky-patterns: 既存 $(wc -l < "$tmp/keys" | tr -d ' ') 件以外の違反なし"; fi
exit "$status"
