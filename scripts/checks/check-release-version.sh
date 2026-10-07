#!/usr/bin/env bash

# check-release-version.sh - 未リリース変更のバージョン更新漏れチェック（Issue #813）
#
# develop が main より進んでいるのにワークスペースバージョンが main と同じままだと、
# develop -> main をマージしても release.yml が既存タグ v<version> を見つけて
# タグ作成・GitHub Release 作成・publish.yml の配布をすべて skip する。
# 公開済みのタグが main の内容と一致しなくなるため、リリース前に検出する。
#
# 検証内容:
#   MAIN_REF に無いコミットを持つ側（DEVELOP_REF または着地候補の HEAD_REF）があるとき、
#   DEVELOP_REF か HEAD_REF のどちらかが [workspace.package] version を
#   MAIN_REF と異なる値に更新済みであること。
#   （HEAD_REF も見るのは、バージョンを更新する変更自身が着地前に検証を通れるようにするため。
#     DEVELOP_REF が更新済みなら、それを取り込む前の古い作業ブランチも通す。）
#
# 戻り値:
#   0: 違反なし
#   1: 違反あり
#   2: エラー（ref やバージョンを解決できない等）

set -euo pipefail

MAIN_REF="${MAIN_REF:-origin/main}"
DEVELOP_REF="${DEVELOP_REF:-origin/develop}"
HEAD_REF="${HEAD_REF:-HEAD}"

# release.yml の「Get version from Cargo.toml」と同じく [workspace.package] の version を読む
workspace_version() {
    local version
    version=$(git show "$1:Cargo.toml" 2>/dev/null | awk '
        /^\[/ { in_section = ($0 == "[workspace.package]") }
        in_section && /^version[[:space:]]*=/ {
            gsub(/^version[[:space:]]*=[[:space:]]*"|".*$/, "")
            print
            exit
        }
    ')
    if [ -z "$version" ]; then
        echo "✗ $1 の Cargo.toml から [workspace.package] version を読めません" >&2
        exit 2
    fi
    echo "$version"
}

for ref in "$MAIN_REF" "$DEVELOP_REF"; do
    case "$ref" in
        origin/*)
            git fetch --quiet origin "${ref#origin/}" 2>/dev/null ||
                echo "⚠ $ref の fetch に失敗しました。ローカルの ref で検査します" >&2
            ;;
    esac
done
for ref in "$MAIN_REF" "$DEVELOP_REF" "$HEAD_REF"; do
    if ! git rev-parse --verify --quiet "$ref^{commit}" >/dev/null; then
        echo "✗ ref を解決できません: $ref" >&2
        exit 2
    fi
done

main_version=$(workspace_version "$MAIN_REF")
develop_version=$(workspace_version "$DEVELOP_REF")
head_version=$(workspace_version "$HEAD_REF")

develop_ahead=$(git rev-list --count "$MAIN_REF..$DEVELOP_REF")
head_ahead=$(git rev-list --count "$MAIN_REF..$HEAD_REF")

if [ "$develop_ahead" -eq 0 ] && [ "$head_ahead" -eq 0 ]; then
    echo "✓ リリースバージョンチェック OK（$MAIN_REF に未リリースの変更なし）"
    exit 0
fi

if [ "$develop_version" != "$main_version" ] || [ "$head_version" != "$main_version" ]; then
    echo "✓ リリースバージョンチェック OK（$MAIN_REF: $main_version / $DEVELOP_REF: $develop_version / $HEAD_REF: $head_version）"
    exit 0
fi

echo "✗ 未リリースの変更があるのにバージョンが $MAIN_REF と同じ $main_version のままです"
echo "  $MAIN_REF..$DEVELOP_REF: ${develop_ahead} commit / $MAIN_REF..$HEAD_REF: ${head_ahead} commit"
echo "  このままリリースすると release.yml が既存タグ v$main_version を見つけ、タグ・Release・配布を skip します。"
echo "  Cargo.toml の [workspace.package] version と Cargo.lock を次のバージョンへ更新してください"
echo "  （$DEVELOP_REF が更新済みなら取り込むだけでよい）。"
exit 1
