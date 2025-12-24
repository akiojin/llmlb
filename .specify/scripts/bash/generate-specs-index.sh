#!/usr/bin/env bash
#
# generate-specs-index.sh - Generate specs/specs.md from SPEC directories
#
# Usage: ./generate-specs-index.sh [--dry-run]
#
# Options:
#   --dry-run  Print to stdout instead of writing to specs/specs.md
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
SPECS_DIR="$REPO_ROOT/specs"
OUTPUT_FILE="$SPECS_DIR/specs.md"
DRY_RUN=false

for arg in "$@"; do
    case "$arg" in
        --dry-run)
            DRY_RUN=true
            ;;
        --help|-h)
            echo "Usage: $0 [--dry-run]"
            echo "  --dry-run  Print to stdout instead of writing to specs/specs.md"
            exit 0
            ;;
    esac
done

# Categorize spec based on title (more accurate than content matching)
categorize_spec() {
    local title="$1"

    # CI/CD・自動化 (最優先)
    if echo "$title" | grep -qE "リリース|マージ|Worktree|自動化"; then
        echo "cicd"
        return
    fi

    # 認証・セキュリティ
    if echo "$title" | grep -qE "認証|アクセス制御|APIキー"; then
        echo "auth"
        return
    fi

    # マルチモーダル対応 (モデル管理より先に評価)
    if echo "$title" | grep -qE "音声|TTS|ASR|画像生成|Image Generation|Playground Chat マルチモーダル"; then
        echo "multimodal"
        return
    fi

    # ルーティング (モデル管理より先に評価)
    if echo "$title" | grep -qE "Routing|ルーティング|capabilities|Playground Multi-Modal|クラウドモデルプレフィックス"; then
        echo "routing"
        return
    fi

    # UI・ダッシュボード
    if echo "$title" | grep -qE "ダッシュボード|CLI|ページネーション"; then
        echo "ui"
        return
    fi

    # ログ・履歴
    if echo "$title" | grep -qE "ログ|履歴|トレース|Log Retrieval|ロギング"; then
        echo "log"
        return
    fi

    # モデル管理
    if echo "$title" | grep -qE "モデル|Models|GGUF|ストレージ|Hugging Face|自動配布|自動解決|gptoss"; then
        echo "model"
        return
    fi

    # コアシステム
    if echo "$title" | grep -qE "ノード|GPU|プロキシ|ヘルスチェック|負荷|バランシング|LLM Router System"; then
        echo "core"
        return
    fi

    echo "other"
}

# Create temp files for each category
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

for cat in core auth model routing multimodal ui log cicd other; do
    touch "$TMP_DIR/$cat.txt"
done

# Counters
total_specs=0
deprecated_count=0
missing_plan_count=0

# Process each spec
for spec_dir in "$SPECS_DIR"/SPEC-*/; do
    [ -d "$spec_dir" ] || continue
    spec_file="$spec_dir/spec.md"
    [ -f "$spec_file" ] || continue

    spec_id=$(basename "$spec_dir")
    title=$(grep -m1 "^# " "$spec_file" | sed 's/^# //' | sed 's/機能仕様書: //')

    is_deprecated=false
    if echo "$title" | grep -qE "^廃止"; then
        is_deprecated=true
        deprecated_count=$((deprecated_count + 1))
    fi

    has_plan=false
    plan_file="$spec_dir/plan.md"
    if [ -f "$plan_file" ]; then
        has_plan=true
    else
        missing_plan_count=$((missing_plan_count + 1))
    fi

    # Status icon
    status_icon="📋"
    if [ "$is_deprecated" = "true" ]; then
        status_icon="🗑️"
    elif [ "$has_plan" = "true" ]; then
        status_icon="✅"
    fi

    category=$(categorize_spec "$title")

    echo "| \`$spec_id\` | $title | $status_icon |" >> "$TMP_DIR/$category.txt"
    total_specs=$((total_specs + 1))
done

# Generate output
generate_output() {
    local timestamp
    timestamp=$(date "+%Y-%m-%d %H:%M:%S")

    cat << EOF
# 機能仕様一覧

> 自動生成: $timestamp
>
> 総SPEC数: **$total_specs** | 廃止: $deprecated_count | plan.md欠損: $missing_plan_count

**凡例:** ✅ plan.md有り | 📋 plan.md無し | 🗑️ 廃止

EOF

    # Output each category
    print_category() {
        local cat_file="$1"
        local emoji="$2"
        local cat_name="$3"

        if [ -s "$cat_file" ]; then
            echo "## $emoji $cat_name"
            echo ""
            echo "| SPEC ID | 機能名 | Status |"
            echo "|---------|--------|--------|"
            cat "$cat_file"
            echo ""
        fi
    }

    print_category "$TMP_DIR/core.txt" "🔧" "コアシステム"
    print_category "$TMP_DIR/auth.txt" "🔐" "認証・セキュリティ"
    print_category "$TMP_DIR/model.txt" "📦" "モデル管理"
    print_category "$TMP_DIR/routing.txt" "🛤️" "ルーティング"
    print_category "$TMP_DIR/multimodal.txt" "🎨" "マルチモーダル対応"
    print_category "$TMP_DIR/ui.txt" "🖥️" "UI・ダッシュボード"
    print_category "$TMP_DIR/log.txt" "📊" "ログ・履歴"
    print_category "$TMP_DIR/cicd.txt" "🚀" "CI/CD・自動化"
    print_category "$TMP_DIR/other.txt" "📁" "その他"
}

# Execute
if [ "$DRY_RUN" = "true" ]; then
    generate_output
else
    generate_output > "$OUTPUT_FILE"
    echo "Generated: $OUTPUT_FILE"
fi
