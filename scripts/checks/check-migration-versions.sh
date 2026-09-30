#!/usr/bin/env bash

# check-migration-versions.sh - マイグレーション採番の衝突チェック（Issue #737）
#
# 並行ブランチが同じマイグレーション番号を採番すると、後から develop へ着地した側で
# _sqlx_migrations.version の UNIQUE 違反が起きる。これを CI より前に検出する。
#
# 採番規則:
#   - 001〜LEGACY_MAX_VERSION (032) は旧連番として凍結する（既存 DB との互換維持）。
#   - 以降の新規マイグレーションは UTC タイムスタンプ YYYYMMDDHHMMSS_<説明>.sql とする。
#
# 検証内容:
#   1. ファイル名が上記規則に従うこと。
#   2. 作業ツリー内でバージョンが重複しないこと。
#   3. BASE_REF（既定 origin/develop、fetch して最新化）に先行着地した別名の
#      マイグレーションとバージョンが重複しないこと。
#
# 戻り値:
#   0: 違反なし
#   1: 違反あり
#   2: エラー（ディレクトリや BASE_REF が見つからない等）

set -euo pipefail

MIGRATIONS_DIR="${MIGRATIONS_DIR:-llmlb/migrations}"
BASE_REF="${BASE_REF:-origin/develop}"
LEGACY_MAX_VERSION=32

NAME_RE='^([0-9]+)_[a-z0-9_]+\.sql$'
TIMESTAMP_RE='^[0-9]{4}(0[1-9]|1[0-2])(0[1-9]|[12][0-9]|3[01])([01][0-9]|2[0-3])[0-5][0-9][0-5][0-9]$'

if [ ! -d "$MIGRATIONS_DIR" ]; then
    echo "✗ マイグレーションディレクトリが見つかりません: $MIGRATIONS_DIR" >&2
    exit 2
fi

case "$BASE_REF" in
    origin/*)
        git fetch --quiet origin "${BASE_REF#origin/}" 2>/dev/null ||
            echo "⚠ $BASE_REF の fetch に失敗しました。ローカルの ref で検査します" >&2
        ;;
esac
if ! git rev-parse --verify --quiet "$BASE_REF^{commit}" >/dev/null; then
    echo "✗ BASE_REF を解決できません: $BASE_REF" >&2
    exit 2
fi

violations=0
fail() {
    echo "✗ $1"
    violations=$((violations + 1))
}

version_of() {
    [[ "$1" =~ $NAME_RE ]] && echo "${BASH_REMATCH[1]}"
}

# 1. 命名規則
local_files=$(cd "$MIGRATIONS_DIR" && ls -1 -- *.sql 2>/dev/null | sort || true)
for file in $local_files; do
    version=$(version_of "$file") || {
        fail "$file: ファイル名は <version>_<snake_case>.sql 形式にしてください"
        continue
    }
    if [[ "$version" =~ ^[0-9]{3}$ ]] && ((10#$version <= LEGACY_MAX_VERSION)); then
        continue
    fi
    if [[ ! "$version" =~ $TIMESTAMP_RE ]]; then
        fail "$file: 新規マイグレーションは UTC タイムスタンプ YYYYMMDDHHMMSS_<説明>.sql で採番してください（連番は $(printf '%03d' "$LEGACY_MAX_VERSION") で凍結）"
    fi
done

# 2. 作業ツリー内の重複
duplicates=$(for file in $local_files; do
    version=$(version_of "$file") && echo "$((10#$version)) $file"
done | awk '{ files[$1] = files[$1] " " $2; count[$1]++ } END { for (v in count) if (count[v] > 1) print v ":" files[v] }')
while IFS= read -r dup; do
    [ -n "$dup" ] && fail "バージョン ${dup%%:*} が重複しています:${dup#*:}"
done <<<"$duplicates"

# 3. BASE_REF に先行着地したマイグレーションとの重複
base_files=$(git ls-tree --name-only "$BASE_REF" -- "$MIGRATIONS_DIR/" | xargs -n1 basename 2>/dev/null | grep '\.sql$' || true)
for file in $local_files; do
    grep -qxF "$file" <<<"$base_files" && continue
    version=$(version_of "$file") || continue
    for base_file in $base_files; do
        base_version=$(version_of "$base_file") || continue
        if [ "$((10#$version))" = "$((10#$base_version))" ]; then
            fail "$file: $BASE_REF に着地済みの $base_file とバージョンが衝突しています。新しいタイムスタンプで採番し直してください"
        fi
    done
done

if [ "$violations" -gt 0 ]; then
    echo "✗ マイグレーション採番チェック失敗: ${violations} 件"
    exit 1
fi
echo "✓ マイグレーション採番チェック OK（BASE_REF: $BASE_REF）"
