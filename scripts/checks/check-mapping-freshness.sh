#!/usr/bin/env bash

# check-mapping-freshness.sh - canonical モデルマッピングの鮮度チェック（Issue #776）
#
# BUILTIN_MAPPINGS は人手で編集する静的テーブルで、モデルの廃止・改名や追従漏れを
# 自動では検知できない。各 canonical が持つ最終確認日（last_verified）を検査し、
# 「確認が古くなったこと」を検出する。
#
# ネットワークには一切アクセスしない（上流障害で CI を落とさないため）。Hugging Face
# 上の実在確認は last_verified を更新する人手の作業で、手順は
# docs/model-catalog-policy.md の「canonical テーブルの更新手順」を参照。
#
# 検証内容:
#   1. 全 canonical に last_verified があること。
#   2. last_verified が実在する日付（YYYY-MM-DD）で、未来日でないこと。
#   3. last_verified からの経過日数が MAPPING_MAX_AGE_DAYS（既定 180）以内であること。
#
# 戻り値:
#   0: 違反なし
#   1: 違反あり
#   2: エラー（ファイルが見つからない、テーブルを読み取れない等）

set -euo pipefail

MAPPING_FILE="${MAPPING_FILE:-llmlb/src/models/mapping.rs}"
MAX_AGE_DAYS="${MAPPING_MAX_AGE_DAYS:-180}"

if [ ! -f "$MAPPING_FILE" ]; then
    echo "✗ マッピングファイルが見つかりません: $MAPPING_FILE" >&2
    exit 2
fi
if [[ ! "$MAX_AGE_DAYS" =~ ^[0-9]+$ ]]; then
    echo "✗ MAPPING_MAX_AGE_DAYS は 0 以上の整数で指定してください: $MAX_AGE_DAYS" >&2
    exit 2
fi

# 1970-01-01 からの通算日数（先発グレゴリオ暦）。date コマンドの BSD/GNU 差を避ける。
days_from_civil() {
    local y=$1 m=$2 d=$3
    ((m <= 2)) && y=$((y - 1))
    local era=$(((y >= 0 ? y : y - 399) / 400))
    local yoe=$((y - era * 400))
    local doy=$(((153 * (m + (m > 2 ? -3 : 9)) + 2) / 5 + d - 1))
    echo $((era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468))
}

# YYYY-MM-DD を通算日数へ変換する。日付として不正なら失敗する。
parse_date() {
    [[ "$1" =~ ^([0-9]{4})-([0-9]{2})-([0-9]{2})$ ]] || return 1
    local y=$((10#${BASH_REMATCH[1]})) m=$((10#${BASH_REMATCH[2]})) d=$((10#${BASH_REMATCH[3]}))
    ((m >= 1 && m <= 12)) || return 1
    local days_in_month=(31 28 31 30 31 30 31 31 30 31 30 31)
    if ((y % 4 == 0 && (y % 100 != 0 || y % 400 == 0))); then
        days_in_month[1]=29
    fi
    ((d >= 1 && d <= days_in_month[m - 1])) || return 1
    days_from_civil "$y" "$m" "$d"
}

# BUILTIN_MAPPINGS 内の "<canonical>\t<last_verified>" を出現順に出力する。
# last_verified を持たないエントリは "-" とする。
entries=$(awk '
    /^pub static BUILTIN_MAPPINGS/ { in_table = 1; next }
    in_table && /^\];/ { in_table = 0 }
    !in_table { next }
    /^[[:space:]]*canonical: "/ {
        if (pending != "") print pending "\t-"
        pending = $0
        sub(/^[[:space:]]*canonical: "/, "", pending)
        sub(/".*$/, "", pending)
        next
    }
    /^[[:space:]]*last_verified: "/ {
        verified = $0
        sub(/^[[:space:]]*last_verified: "/, "", verified)
        sub(/".*$/, "", verified)
        if (pending != "") print pending "\t" verified
        pending = ""
    }
    END { if (pending != "") print pending "\t-" }
' "$MAPPING_FILE")

if [ -z "$entries" ]; then
    echo "✗ $MAPPING_FILE から BUILTIN_MAPPINGS の canonical を読み取れません" >&2
    exit 2
fi

today=$(date -u +%Y-%m-%d)
today_days=$(parse_date "$today")

violations=0
total=0
oldest_age=-1
oldest_name=""
fail() {
    echo "✗ $1"
    violations=$((violations + 1))
}

while IFS=$'\t' read -r canonical verified; do
    total=$((total + 1))
    if [ "$verified" = "-" ]; then
        fail "$canonical: last_verified がありません"
        continue
    fi
    if ! verified_days=$(parse_date "$verified"); then
        fail "$canonical: last_verified が日付（YYYY-MM-DD）として不正です: $verified"
        continue
    fi
    age=$((today_days - verified_days))
    if ((age < 0)); then
        fail "$canonical: last_verified が未来日です: $verified（今日: $today）"
        continue
    fi
    if ((age > MAX_AGE_DAYS)); then
        fail "$canonical: 最終確認から ${age} 日経過しています（last_verified: $verified、閾値: ${MAX_AGE_DAYS} 日）"
    fi
    if ((age > oldest_age)); then
        oldest_age=$age
        oldest_name=$canonical
    fi
done <<<"$entries"

if [ "$violations" -gt 0 ]; then
    echo "✗ canonical マッピング鮮度チェック失敗: ${total} 件中 ${violations} 件"
    echo "  Hugging Face 上の実在を確認し、last_verified を更新してください。"
    echo "  手順: docs/model-catalog-policy.md「canonical テーブルの更新手順」"
    exit 1
fi
echo "✓ canonical マッピング鮮度チェック OK（${total} 件、閾値 ${MAX_AGE_DAYS} 日、次の失効まで $((MAX_AGE_DAYS - oldest_age)) 日: ${oldest_name}）"
