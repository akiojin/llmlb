#!/usr/bin/env bash

# check-dashboard-data-hooks.sh - ダッシュボードコンポーネントの React Query 直接呼び出し検査
# （SPEC #821 T016 / Issue #824）
#
# `components/` 配下のコンポーネントが `useQuery` / `useMutation` / `useQueryClient` を
# 直接呼ぶと、データ所有がコンポーネントに散り MVVM 移行（ViewModel 側へのデータ所有の集約）が
# 後退する。移行中に新しい直接呼び出しが混入しても気付けないため、検査で検出する。
#
# 判定:
#   - 対象は SRC_DIR 配下の *.ts / *.tsx（テストファイル *.test.ts / *.test.tsx は除く）。
#   - 識別子として `useQuery` / `useMutation` / `useQueryClient` が現れたファイルを違反とする
#     （import 文も含む。前方一致する別名 `useQueryClientRef` 等は識別子境界で除外する）。
#
# 移行待ちのファイルは ALLOWLIST に 1 行 1 ファイル（SRC_DIR からの相対パス）で列挙する。
# 直接呼び出しが無くなったのに ALLOWLIST に残っているエントリと、存在しないファイルを指す
# エントリも失敗として扱い、許可リストが単調に縮むことを保証する。
#
# 戻り値:
#   0: 違反なし
#   1: 許可リストに無い直接呼び出しあり、または不要な許可リストエントリあり
#   2: エラー（対象ディレクトリ・許可リストが見つからない）

set -euo pipefail

SRC_DIR="${SRC_DIR:-llmlb/src/web/dashboard/src/components}"
ALLOWLIST="${ALLOWLIST:-scripts/checks/dashboard-data-hooks-allowlist.txt}"

if [ ! -d "$SRC_DIR" ]; then
    echo "✗ 対象ディレクトリが見つかりません: $SRC_DIR" >&2
    exit 2
fi
if [ ! -f "$ALLOWLIST" ]; then
    echo "✗ 許可リストが見つかりません: $ALLOWLIST" >&2
    exit 2
fi

# JS/TS の識別子境界（英数字・_・$ 以外）で囲まれたフック名だけを一致させる
HOOK_PATTERN='(^|[^A-Za-z0-9_$])use(Query|Mutation|QueryClient)([^A-Za-z0-9_$]|$)'

allowed() {
    grep -qxF "$1" "$ALLOWLIST"
}

status=0
count=0

while IFS= read -r file; do
    count=$((count + 1))
    rel="${file#"$SRC_DIR"/}"
    if grep -Eq "$HOOK_PATTERN" "$file"; then
        if ! allowed "$rel"; then
            echo "✗ $file が React Query のフックを直接呼んでいます" >&2
            echo "    useQuery / useMutation / useQueryClient は ViewModel 側に置き、コンポーネントからは ViewModel を呼び出してください" >&2
            status=1
        fi
    elif allowed "$rel"; then
        echo "✗ 許可リスト不要: $rel に直接呼び出しはありません。エントリを削除してください" >&2
        status=1
    fi
done < <(
    find "$SRC_DIR" -type f \( -name '*.ts' -o -name '*.tsx' \) \
        ! -name '*.test.ts' ! -name '*.test.tsx' | sort
)

# 存在しないファイルを指すエントリも不要として扱う
while read -r entry _; do
    case "$entry" in
        '' | '#'*) continue ;;
    esac
    if [ ! -f "$SRC_DIR/$entry" ]; then
        echo "✗ 許可リスト不要: $entry は $SRC_DIR に存在しません。エントリを削除してください" >&2
        status=1
    fi
done < "$ALLOWLIST"

if [ "$status" -eq 0 ]; then
    echo "✓ dashboard-data-hooks: ${count} ファイルに許可リスト外の React Query 直接呼び出しはありません"
fi

exit "$status"
