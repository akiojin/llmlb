#!/usr/bin/env bash

# check-event-publishers.sh - canonical ダッシュボードイベントの publisher 検査
# （SPEC #582 FR-048e / Issue #781）
#
# `DashboardEvent` の各バリアントについて、production コードに発行呼び出し
# （`publish(...)` / `publish_coalesced(...)` の引数として構築している箇所）があることを検証する。
# enum 定義とシリアライズテストだけが存在し production から一度も発行されないイベントは、
# ダッシュボードがポーリングでも更新されるために症状が出ず、気付けないまま残る。
#
# publisher として数えないもの:
#   - イベント定義モジュール（EVENTS_DIR 配下）。バス自身の実装は発行元ではない。
#   - テストコード（tests.rs と、各ファイルの最初の `#[cfg(test)]` 以降）。
#   - `publish` の引数ではない参照（match 式など）。
#     イベントを変数に束縛してから publish する書き方も検出できないため、引数で直接構築すること。
#
# publisher が別 Issue の担当で未着地のバリアントは ALLOWLIST に 1 行 1 バリアントで列挙する。
# publisher が存在するのに ALLOWLIST に残っているエントリと、enum に存在しないエントリも
# 失敗として扱い、許可リストが単調に縮むことを保証する。
#
# 戻り値:
#   0: 違反なし
#   1: publisher の無いバリアントあり、または不要な許可リストエントリあり
#   2: エラー（対象ディレクトリ・許可リストが見つからない、enum を解析できない等）

set -euo pipefail

SRC_DIR="${SRC_DIR:-llmlb/src}"
EVENTS_DIR="${EVENTS_DIR:-$SRC_DIR/events}"
ENUM_FILE="${ENUM_FILE:-$EVENTS_DIR/bus.rs}"
ALLOWLIST="${ALLOWLIST:-scripts/checks/event-publishers-allowlist.txt}"

if [ ! -d "$SRC_DIR" ]; then
    echo "✗ ソースディレクトリが見つかりません: $SRC_DIR" >&2
    exit 2
fi
if [ ! -f "$ENUM_FILE" ]; then
    echo "✗ イベント定義が見つかりません: $ENUM_FILE" >&2
    exit 2
fi
if [ ! -f "$ALLOWLIST" ]; then
    echo "✗ 許可リストが見つかりません: $ALLOWLIST" >&2
    exit 2
fi

# enum 本体の直下（4 スペースインデント）にあるバリアント名を抜き出す
variants=$(awk '
    /^pub enum DashboardEvent \{/ { in_enum = 1; next }
    in_enum && /^\}/ { exit }
    in_enum && /^    [A-Z][A-Za-z0-9]*[ ,({]/ {
        name = $1
        sub(/[,({].*$/, "", name)
        print name
    }
' "$ENUM_FILE")

if [ -z "$variants" ]; then
    echo "✗ $ENUM_FILE から DashboardEvent のバリアントを抽出できません" >&2
    exit 2
fi

# production コードを 1 行に連結する。テストコードを落とし、空白を 1 つに畳むことで、
# rustfmt が `publish(` の後で改行した呼び出しも同じ正規表現で検出できる。
production=$(
    find "$SRC_DIR" -name '*.rs' ! -name 'tests.rs' ! -path "$EVENTS_DIR/*" | sort |
        while IFS= read -r file; do
            awk '/^[[:space:]]*#\[cfg\(test\)\]/ { exit } { print }' "$file"
        done | tr -s '[:space:]' ' '
)

# grep -q は一致した時点で終了するため、パイプで渡すと pipefail が SIGPIPE を失敗として扱う。
# here-string で渡して終了コードを grep のものだけにする。
has_publisher() {
    grep -Eq "publish(_coalesced)?\( ?([A-Za-z_]+::)*DashboardEvent::$1([^A-Za-z0-9_]|\$)" \
        <<< "$production"
}

allowed() {
    grep -qxF "$1" "$ALLOWLIST"
}

status=0
count=0

for variant in $variants; do
    count=$((count + 1))
    if has_publisher "$variant"; then
        if allowed "$variant"; then
            echo "✗ 許可リスト不要: $variant には publisher があります。エントリを削除してください" >&2
            status=1
        fi
    elif ! allowed "$variant"; then
        echo "✗ DashboardEvent::$variant に production の publisher がありません" >&2
        echo "    $SRC_DIR（$EVENTS_DIR とテストコードを除く）で publish(DashboardEvent::$variant ...) を呼び出してください" >&2
        status=1
    fi
done

# enum に存在しないバリアントを指すエントリも不要として扱う
while read -r entry _; do
    case "$entry" in
        '' | '#'*) continue ;;
    esac
    if ! grep -qxF "$entry" <<< "$variants"; then
        echo "✗ 許可リスト不要: $entry は DashboardEvent に存在しません。エントリを削除してください" >&2
        status=1
    fi
done < "$ALLOWLIST"

if [ "$status" -eq 0 ]; then
    echo "✓ event-publishers: ${count} 種のイベントすべてに publisher があります（許可リスト分を除く）"
fi

exit "$status"
