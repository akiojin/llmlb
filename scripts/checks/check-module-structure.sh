#!/usr/bin/env bash

# check-module-structure.sh - Rust モジュール構造の標準化チェック（SPEC #699）
#
# 検証内容:
#   FR-009: 本体ソース (*.rs) が MAX_LINES 行以下であること。
#           テスト専用ファイル tests.rs は対象外（#699 PM 裁定）。
#   FR-010: mod.rs が宣言と re-export のみで、fn/struct/impl 等の実装を持たないこと。
#
# 分割が未完了のファイルは ALLOWLIST に `size <path>` / `mod <path>` で列挙する。
# 分割を終えたのに ALLOWLIST に残っているエントリも失敗として扱い、
# 許可リストが単調に縮むことを保証する。
#
# 戻り値:
#   0: 違反なし
#   1: 違反あり、または不要な許可リストエントリあり
#   2: エラー（対象ディレクトリや許可リストが見つからない等）

set -euo pipefail

SRC_DIR="${SRC_DIR:-llmlb/src}"
ALLOWLIST="${ALLOWLIST:-scripts/checks/module-structure-allowlist.txt}"
MAX_LINES="${MAX_LINES:-1500}"

# 行頭に置かれた実装アイテム（mod / use 以外）を検出する。
IMPL_ITEM_RE='^(pub(\([a-z]+\))? )?((async|unsafe|const) )*(fn|struct|enum|impl|trait|static|type|const|union|macro_rules!)[ <!]'

if [ ! -d "$SRC_DIR" ]; then
    echo "✗ ソースディレクトリが見つかりません: $SRC_DIR" >&2
    exit 2
fi
if [ ! -f "$ALLOWLIST" ]; then
    echo "✗ 許可リストが見つかりません: $ALLOWLIST" >&2
    exit 2
fi

allowed() {
    grep -qxF "$1 $2" "$ALLOWLIST"
}

status=0

while IFS= read -r file; do
    rel="${file#"$SRC_DIR"/}"

    lines=$(wc -l < "$file" | tr -d ' ')
    if [ "$lines" -gt "$MAX_LINES" ]; then
        if ! allowed size "$rel"; then
            echo "✗ FR-009: $rel が ${lines} 行です（上限 ${MAX_LINES} 行）" >&2
            status=1
        fi
    elif allowed size "$rel"; then
        echo "✗ 許可リスト不要: size $rel は ${lines} 行で上限内です。エントリを削除してください" >&2
        status=1
    fi

    if [ "$(basename "$file")" = "mod.rs" ]; then
        if grep -qE "$IMPL_ITEM_RE" "$file"; then
            if ! allowed mod "$rel"; then
                echo "✗ FR-010: $rel が実装を含みます（mod.rs は宣言と re-export のみ）" >&2
                grep -nE "$IMPL_ITEM_RE" "$file" | head -5 | sed 's/^/    /' >&2
                status=1
            fi
        elif allowed mod "$rel"; then
            echo "✗ 許可リスト不要: mod $rel は re-export のみです。エントリを削除してください" >&2
            status=1
        fi
    fi
done < <(find "$SRC_DIR" -name '*.rs' ! -name 'tests.rs' | sort)

# 実在しないファイルを指すエントリも不要として扱う
while read -r kind path; do
    case "$kind" in
        '' | '#'*) continue ;;
        size | mod) ;;
        *)
            echo "✗ 許可リストの種別が不正です: $kind $path" >&2
            status=1
            continue
            ;;
    esac
    if [ ! -f "$SRC_DIR/$path" ]; then
        echo "✗ 許可リスト不要: $kind $path は存在しません。エントリを削除してください" >&2
        status=1
    fi
done < "$ALLOWLIST"

if [ "$status" -eq 0 ]; then
    echo "✓ module structure OK (max ${MAX_LINES} lines, mod.rs re-export only)"
fi
exit "$status"
