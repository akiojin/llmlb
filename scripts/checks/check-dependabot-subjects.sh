#!/usr/bin/env bash

# check-dependabot-subjects.sh - Dependabot グループ更新の件名長チェック（Issue #727）
#
# Dependabot のグループ更新 PR は、件名を次の形式で自動生成する。
#   <prefix>: bump the <group> group across 1 directory with <N> updates
# group 名が長いと、更新数が 2 桁になった時点で commitlint の header-max-length (72) を
# 超え、必須チェック Commit Message Lint が必ず失敗する（#715 / #720）。
#
# dependabot.yml の各 group について、更新数 99 件の最悪ケースの件名を組み立て、
# リポジトリの commitlint 設定で検証する。
#
# group 内の更新が 1 件だけの場合、件名は依存名で長さが決まり（#666 で 81 文字）、
# 設定側で 72 文字以内を保証できない。そのため commitlint.config.js は Dependabot の
# コミットを除外している（Issue #751）。本スクリプトは dependabot.yml の全エントリが
# commit-message.prefix を明示し、その prefix の長い件名が Dependabot コミットとして
# 除外されること（dependabot.yml と commitlint.config.js の整合）も検証する。
#
# 環境変数:
#   DEPENDABOT_CONFIG: 検査する設定ファイル（既定 .github/dependabot.yml）
#
# 戻り値:
#   0: 全 group の件名が commitlint を通る
#   1: commitlint に違反する group がある
#   2: エラー（設定ファイルや commitlint が見つからない、group が 1 つも無い等）

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEPENDABOT_CONFIG="${DEPENDABOT_CONFIG:-$ROOT/.github/dependabot.yml}"
COMMITLINT="$ROOT/node_modules/.bin/commitlint"
DEFAULT_PREFIX="chore(deps)"
WORST_CASE_UPDATES=99
DEPENDABOT_SIGN_OFF="Signed-off-by: dependabot[bot] <support@github.com>"
# 1 件更新の件名に使う長い依存名とバージョン（prefix を付けると 72 文字を超える）
LONG_DEPENDENCY="bump @opentelemetry/instrumentation-http from 0.200.0 to 0.201.0"

if [ ! -f "$DEPENDABOT_CONFIG" ]; then
    echo "✗ Dependabot 設定ファイルが見つかりません: $DEPENDABOT_CONFIG" >&2
    exit 2
fi
if [ ! -x "$COMMITLINT" ]; then
    echo "✗ commitlint が見つかりません。'pnpm install' を実行してください: $COMMITLINT" >&2
    exit 2
fi

# updates の各エントリから "<prefix>\t<group>" を列挙する。
# group は groups: 直下（1 段深いインデント）のキーとして定義される。
extract_groups() {
    awk -v default_prefix="$DEFAULT_PREFIX" '
        function indent_of(line) { match(line, /^ */); return RLENGTH }
        function flush(   i) {
            for (i = 1; i <= n; i++) printf "%s\t%s\n", prefix, names[i]
            n = 0; prefix = default_prefix; groups_indent = -1
        }
        BEGIN { n = 0; prefix = default_prefix; groups_indent = -1 }
        /^[[:space:]]*(#|$)/ { next }
        /^ *- package-ecosystem:/ { flush(); next }
        {
            indent = indent_of($0)
            if (groups_indent >= 0 && indent <= groups_indent) groups_indent = -1
            if (groups_indent >= 0) {
                if (group_indent < 0) group_indent = indent
                if (indent == group_indent && $0 ~ /^ *[A-Za-z0-9_.-]+: *$/) {
                    name = $0; sub(/^ */, "", name); sub(/: *$/, "", name)
                    names[++n] = name
                }
                next
            }
            if ($0 ~ /^ *groups: *$/) { groups_indent = indent; group_indent = -1; next }
            if ($0 ~ /^ *prefix:/) {
                value = $0; sub(/^ *prefix: */, "", value); gsub(/^["\x27]|["\x27] *$/, "", value)
                prefix = value
            }
        }
        END { flush() }
    ' "$DEPENDABOT_CONFIG"
}

# updates の各エントリから "<package-ecosystem>\t<prefix>" を列挙する（prefix 未設定は空）。
extract_prefixes() {
    awk '
        function flush() { if (ecosystem != "") printf "%s\t%s\n", ecosystem, prefix }
        /^[[:space:]]*(#|$)/ { next }
        /^ *- package-ecosystem:/ {
            flush()
            ecosystem = $0; sub(/^ *- package-ecosystem: */, "", ecosystem); gsub(/["\x27]/, "", ecosystem)
            prefix = ""
            next
        }
        /^ *prefix:/ {
            prefix = $0; sub(/^ *prefix: */, "", prefix); gsub(/^["\x27]|["\x27] *$/, "", prefix)
        }
        END { flush() }
    ' "$DEPENDABOT_CONFIG"
}

# 件名と本文を commitlint で検証し、結果を表示する。違反時は violations を加算する。
lint_subject() {
    local label="$1" subject="$2" message="$3" result
    if result="$(printf '%s\n' "$message" | "$COMMITLINT" --cwd "$ROOT" 2>&1)"; then
        echo "✓ $label: $subject (${#subject} 文字)"
    else
        echo "✗ $label: $subject (${#subject} 文字)"
        echo "$result" | grep '✖' | sed 's/^/    /'
        violations=$((violations + 1))
    fi
}

groups="$(extract_groups)"
if [ -z "$groups" ]; then
    echo "✗ group が 1 つも定義されていません: $DEPENDABOT_CONFIG" >&2
    exit 2
fi

violations=0
while IFS=$'\t' read -r prefix group; do
    subject="$prefix: bump the $group group across 1 directory with $WORST_CASE_UPDATES updates"
    lint_subject "$group" "$subject" "$subject"
done <<<"$groups"

while IFS=$'\t' read -r ecosystem prefix; do
    if [ -z "$prefix" ]; then
        echo "✗ $ecosystem: commit-message の prefix が未設定です（commitlint の Dependabot 除外と整合しません）"
        violations=$((violations + 1))
        continue
    fi
    subject="$prefix: $LONG_DEPENDENCY"
    lint_subject "$ecosystem" "$subject" "$(printf '%s\n\n%s' "$subject" "$DEPENDABOT_SIGN_OFF")"
done <<<"$(extract_prefixes)"

if [ "$violations" -gt 0 ]; then
    echo ""
    echo "✗ $violations 件の Dependabot の件名が commitlint に違反します。"
    echo "  group 名を短くするか group を分割し、commit-message.prefix を commitlint.config.js の"
    echo "  Dependabot 除外条件と揃えてください（header-max-length は緩めない）。"
    exit 1
fi
echo "✓ 全エントリの Dependabot 件名が commitlint を通ります"
