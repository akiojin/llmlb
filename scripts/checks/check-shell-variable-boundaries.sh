#!/usr/bin/env bash

# Issue #889: bash 3.2 はUTF-8で変数直後の非ASCII文字を変数名に取り込む。
# scripts/ のシェルでこの曖昧な境界を禁止し、波括弧形式への修正を促す。
# 戻り値: 0=違反なし、1=違反あり、2=検査エラー。

set -euo pipefail

SCRIPTS_DIR="${SCRIPTS_DIR:-scripts}"
if [ ! -d "$SCRIPTS_DIR" ]; then
    echo "shell variable boundaries: directory not found: $SCRIPTS_DIR" >&2
    exit 2
fi

file_list=$(mktemp)
trap 'rm -f "$file_list"' EXIT
if ! find "$SCRIPTS_DIR" -type f \( -name '*.sh' -o -name '*.bash' -o -name '*.bats' \) -print0 > "$file_list"; then
    exit 2
fi

# BSD/GNU grep共通のバイト範囲。ロケール固定はgrepだけに限定する。
pattern=$(printf '\\$[A-Za-z_][A-Za-z0-9_]*[\200-\377]')
status=0
while IFS= read -r -d '' file; do
    if LC_ALL=C grep -nHE "$pattern" "$file"; then
        status=1
    else
        grep_status=$?
        if [ "$grep_status" -ne 1 ]; then
            exit 2
        fi
    fi
done < "$file_list"

if [ "$status" -ne 0 ]; then
    echo 'shell variable boundaries: use ${name} before non-ASCII characters.' >&2
else
    echo 'shell variable boundaries OK'
fi
exit "$status"
