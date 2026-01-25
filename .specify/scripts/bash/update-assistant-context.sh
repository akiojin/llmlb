#!/usr/bin/env bash

# AIアシスタントコンテキストファイル更新スクリプト
#
# このスクリプトはplan.mdの情報を使用してAIアシスタントコンテキストファイルを管理します。
# 機能仕様を解析し、プロジェクト情報でアシスタント固有の設定ファイルを更新します。
#
# 主要機能:
# 1. 環境検証
#    - Gitリポジトリ構造とブランチ情報を確認
#    - 必要なplan.mdファイルとテンプレートをチェック
#    - ファイル権限とアクセシビリティを検証
#
# 2. Planデータ抽出
#    - plan.mdファイルを解析してプロジェクトメタデータを抽出
#    - 言語/バージョン、フレームワーク、データベース、プロジェクトタイプを特定
#    - 欠落または不完全な仕様データを適切に処理
#
# 3. アシスタントファイル管理
#    - 必要に応じてテンプレートから新しいアシスタントコンテキストファイルを作成
#    - 既存のアシスタントファイルを新しいプロジェクト情報で更新
#    - 手動追加やカスタム設定を保持
#    - 複数のAIエージェント形式とディレクトリ構造をサポート
#
# 4. コンテンツ生成
#    - 言語固有のビルド/テストコマンドを生成
#    - 適切なプロジェクトディレクトリ構造を作成
#    - 技術スタックと最近の変更セクションを更新
#    - 一貫したフォーマットとタイムスタンプを維持
#
# 5. マルチアシスタントサポート
#    - アシスタント固有のファイルパスと命名規則を処理
#    - サポート: Claude, Gemini, Copilot, Cursor, Qwen, opencode, Codex, Windsurf, Kilo Code, Auggie CLI, Roo Code, CodeBuddy CLI, Amp, Amazon Q Developer CLI
#    - 単一のアシスタントまたは既存のすべてのアシスタントファイルを更新可能
#    - アシスタントファイルが存在しない場合はデフォルトでClaudeファイルを作成
#
# 使用法: ./update-assistant-context.sh [assistant_type]
# アシスタントタイプ: claude|gemini|copilot|cursor|qwen|opencode|codex|windsurf|kilocode|auggie|q
# 空にすると既存のすべてのアシスタントファイルを更新

set -e

# 厳密なエラーハンドリングを有効化
set -u
set -o pipefail

#==============================================================================
# 設定とグローバル変数
#==============================================================================

# スクリプトディレクトリを取得し共通関数を読み込み
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"

# 共通関数から全パスと変数を取得
eval $(get_feature_paths)

NEW_PLAN="$IMPL_PLAN"  # 既存コードとの互換性のためのエイリアス
AGENT_TYPE="${1:-}"

# アシスタント固有のファイルパス
CLAUDE_FILE="$REPO_ROOT/CLAUDE.md"
GEMINI_FILE="$REPO_ROOT/GEMINI.md"
COPILOT_FILE="$REPO_ROOT/.github/copilot-instructions.md"
CURSOR_FILE="$REPO_ROOT/.cursor/rules/specify-rules.mdc"
QWEN_FILE="$REPO_ROOT/QWEN.md"
AGENTS_FILE="$REPO_ROOT/AGENTS.md"
WINDSURF_FILE="$REPO_ROOT/.windsurf/rules/specify-rules.md"
KILOCODE_FILE="$REPO_ROOT/.kilocode/rules/specify-rules.md"
AUGGIE_FILE="$REPO_ROOT/.augment/rules/specify-rules.md"
ROO_FILE="$REPO_ROOT/.roo/rules/specify-rules.md"
CODEBUDDY_FILE="$REPO_ROOT/CODEBUDDY.md"
AMP_FILE="$REPO_ROOT/AGENTS.md"
Q_FILE="$REPO_ROOT/AGENTS.md"

# テンプレートファイル
TEMPLATE_FILE="$REPO_ROOT/.specify/templates/assistant-file-template.md"

# 解析されたplanデータ用のグローバル変数
NEW_LANG=""
NEW_FRAMEWORK=""
NEW_DB=""
NEW_PROJECT_TYPE=""

#==============================================================================
# ユーティリティ関数
#==============================================================================

log_info() {
    echo "情報: $1"
}

log_success() {
    echo "✓ $1"
}

log_error() {
    echo "エラー: $1" >&2
}

log_warning() {
    echo "警告: $1" >&2
}

# 一時ファイルのクリーンアップ関数
cleanup() {
    local exit_code=$?
    rm -f /tmp/assistant_update_*_$$
    rm -f /tmp/manual_additions_$$
    exit $exit_code
}

# クリーンアップトラップを設定
trap cleanup EXIT INT TERM

#==============================================================================
# 検証関数
#==============================================================================

validate_environment() {
    # 現在のブランチ/機能があるかチェック（git/非git両対応）
    if [[ -z "$CURRENT_BRANCH" ]]; then
        log_error "現在の機能を特定できません"
        if [[ "$HAS_GIT" == "true" ]]; then
            log_info "featureブランチにいることを確認してください"
        else
            log_info "SPECIFY_FEATURE環境変数を設定するか、まず機能を作成してください"
        fi
        exit 1
    fi

    # plan.mdが存在するかチェック
    if [[ ! -f "$NEW_PLAN" ]]; then
        log_error "plan.mdが見つかりません: $NEW_PLAN"
        log_info "対応するspecディレクトリがある機能で作業していることを確認してください"
        if [[ "$HAS_GIT" != "true" ]]; then
            log_info "使用: export SPECIFY_FEATURE=your-feature-name または新しい機能を作成してください"
        fi
        exit 1
    fi

    # テンプレートが存在するかチェック（新規ファイル作成に必要）
    if [[ ! -f "$TEMPLATE_FILE" ]]; then
        log_warning "テンプレートファイルが見つかりません: $TEMPLATE_FILE"
        log_warning "新しいエージェントファイルの作成は失敗します"
    fi
}

#==============================================================================
# Plan解析関数
#==============================================================================

extract_plan_field() {
    local field_pattern="$1"
    local plan_file="$2"

    grep "^\*\*${field_pattern}\*\*: " "$plan_file" 2>/dev/null | \
        head -1 | \
        sed "s|^\*\*${field_pattern}\*\*: ||" | \
        sed 's/^[ \t]*//;s/[ \t]*$//' | \
        grep -v "要明確化" | \
        grep -v "^N/A$" || echo ""
}

parse_plan_data() {
    local plan_file="$1"

    if [[ ! -f "$plan_file" ]]; then
        log_error "Planファイルが見つかりません: $plan_file"
        return 1
    fi

    if [[ ! -r "$plan_file" ]]; then
        log_error "Planファイルを読み取れません: $plan_file"
        return 1
    fi

    log_info "Planデータを解析中: $plan_file"

    NEW_LANG=$(extract_plan_field "言語/バージョン" "$plan_file")
    NEW_FRAMEWORK=$(extract_plan_field "主要依存関係" "$plan_file")
    NEW_DB=$(extract_plan_field "ストレージ" "$plan_file")
    NEW_PROJECT_TYPE=$(extract_plan_field "プロジェクトタイプ" "$plan_file")

    # 見つかった情報をログ出力
    if [[ -n "$NEW_LANG" ]]; then
        log_info "言語を検出: $NEW_LANG"
    else
        log_warning "planに言語情報が見つかりません"
    fi

    if [[ -n "$NEW_FRAMEWORK" ]]; then
        log_info "フレームワークを検出: $NEW_FRAMEWORK"
    fi

    if [[ -n "$NEW_DB" ]] && [[ "$NEW_DB" != "N/A" ]]; then
        log_info "データベースを検出: $NEW_DB"
    fi

    if [[ -n "$NEW_PROJECT_TYPE" ]]; then
        log_info "プロジェクトタイプを検出: $NEW_PROJECT_TYPE"
    fi
}

format_technology_stack() {
    local lang="$1"
    local framework="$2"
    local parts=()

    # 空でないパーツを追加
    [[ -n "$lang" && "$lang" != "要明確化" ]] && parts+=("$lang")
    [[ -n "$framework" && "$framework" != "要明確化" && "$framework" != "N/A" ]] && parts+=("$framework")

    # 適切なフォーマットで結合
    if [[ ${#parts[@]} -eq 0 ]]; then
        echo ""
    elif [[ ${#parts[@]} -eq 1 ]]; then
        echo "${parts[0]}"
    else
        # 複数パーツを " + " で結合
        local result="${parts[0]}"
        for ((i=1; i<${#parts[@]}; i++)); do
            result="$result + ${parts[i]}"
        done
        echo "$result"
    fi
}

#==============================================================================
# テンプレートとコンテンツ生成関数
#==============================================================================

get_project_structure() {
    local project_type="$1"

    if [[ "$project_type" == *"web"* ]]; then
        echo "backend/\\nfrontend/\\ntests/"
    else
        echo "src/\\ntests/"
    fi
}

get_commands_for_language() {
    local lang="$1"

    case "$lang" in
        *"Python"*)
            echo "cd src && pytest && ruff check ."
            ;;
        *"Rust"*)
            echo "cargo test && cargo clippy"
            ;;
        *"JavaScript"*|*"TypeScript"*)
            echo "npm test \\&\\& npm run lint"
            ;;
        *)
            echo "# $lang 用のコマンドを追加"
            ;;
    esac
}

get_language_conventions() {
    local lang="$1"
    echo "$lang: 標準的なコーディング規約に従う"
}

create_new_agent_file() {
    local target_file="$1"
    local temp_file="$2"
    local project_name="$3"
    local current_date="$4"

    if [[ ! -f "$TEMPLATE_FILE" ]]; then
        log_error "テンプレートが見つかりません: $TEMPLATE_FILE"
        return 1
    fi

    if [[ ! -r "$TEMPLATE_FILE" ]]; then
        log_error "テンプレートファイルを読み取れません: $TEMPLATE_FILE"
        return 1
    fi

    log_info "テンプレートから新しいエージェントコンテキストファイルを作成中..."

    if ! cp "$TEMPLATE_FILE" "$temp_file"; then
        log_error "テンプレートファイルのコピーに失敗しました"
        return 1
    fi

    # テンプレートのプレースホルダーを置換
    local project_structure
    project_structure=$(get_project_structure "$NEW_PROJECT_TYPE")

    local commands
    commands=$(get_commands_for_language "$NEW_LANG")

    local language_conventions
    language_conventions=$(get_language_conventions "$NEW_LANG")

    # より安全なアプローチでエラーチェック付きの置換を実行
    # sed用に特殊文字をエスケープ
    local escaped_lang=$(printf '%s\n' "$NEW_LANG" | sed 's/[\[\.*^$()+{}|]/\\&/g')
    local escaped_framework=$(printf '%s\n' "$NEW_FRAMEWORK" | sed 's/[\[\.*^$()+{}|]/\\&/g')
    local escaped_branch=$(printf '%s\n' "$CURRENT_BRANCH" | sed 's/[\[\.*^$()+{}|]/\\&/g')

    # 技術スタックと最近の変更文字列を条件付きで構築
    local tech_stack
    if [[ -n "$escaped_lang" && -n "$escaped_framework" ]]; then
        tech_stack="- $escaped_lang + $escaped_framework ($escaped_branch)"
    elif [[ -n "$escaped_lang" ]]; then
        tech_stack="- $escaped_lang ($escaped_branch)"
    elif [[ -n "$escaped_framework" ]]; then
        tech_stack="- $escaped_framework ($escaped_branch)"
    else
        tech_stack="- ($escaped_branch)"
    fi

    local recent_change
    if [[ -n "$escaped_lang" && -n "$escaped_framework" ]]; then
        recent_change="- $escaped_branch: $escaped_lang + $escaped_framework を追加"
    elif [[ -n "$escaped_lang" ]]; then
        recent_change="- $escaped_branch: $escaped_lang を追加"
    elif [[ -n "$escaped_framework" ]]; then
        recent_change="- $escaped_branch: $escaped_framework を追加"
    else
        recent_change="- $escaped_branch: 追加"
    fi

    local substitutions=(
        "s|\[PROJECT NAME\]|$project_name|"
        "s|\[DATE\]|$current_date|"
        "s|\[EXTRACTED FROM ALL PLAN.MD FILES\]|$tech_stack|"
        "s|\[ACTUAL STRUCTURE FROM PLANS\]|$project_structure|g"
        "s|\[ONLY COMMANDS FOR ACTIVE TECHNOLOGIES\]|$commands|"
        "s|\[LANGUAGE-SPECIFIC, ONLY FOR LANGUAGES IN USE\]|$language_conventions|"
        "s|\[LAST 3 FEATURES AND WHAT THEY ADDED\]|$recent_change|"
    )

    for substitution in "${substitutions[@]}"; do
        if ! sed -i.bak -e "$substitution" "$temp_file"; then
            log_error "置換の実行に失敗しました: $substitution"
            rm -f "$temp_file" "$temp_file.bak"
            return 1
        fi
    done

    # \nシーケンスを実際の改行に変換
    newline=$(printf '\n')
    sed -i.bak2 "s/\\\\n/${newline}/g" "$temp_file"

    # バックアップファイルをクリーンアップ
    rm -f "$temp_file.bak" "$temp_file.bak2"

    return 0
}




update_existing_agent_file() {
    local target_file="$1"
    local current_date="$2"

    log_info "既存のエージェントコンテキストファイルを更新中..."

    # アトミック更新用の単一一時ファイルを使用
    local temp_file
    temp_file=$(mktemp) || {
        log_error "一時ファイルの作成に失敗しました"
        return 1
    }

    # ファイルを1パスで処理
    local tech_stack=$(format_technology_stack "$NEW_LANG" "$NEW_FRAMEWORK")
    local new_tech_entries=()
    local new_change_entry=""

    # 新しい技術エントリを準備
    if [[ -n "$tech_stack" ]] && ! grep -q "$tech_stack" "$target_file"; then
        new_tech_entries+=("- $tech_stack ($CURRENT_BRANCH)")
    fi

    if [[ -n "$NEW_DB" ]] && [[ "$NEW_DB" != "N/A" ]] && [[ "$NEW_DB" != "要明確化" ]] && ! grep -q "$NEW_DB" "$target_file"; then
        new_tech_entries+=("- $NEW_DB ($CURRENT_BRANCH)")
    fi

    # 新しい変更エントリを準備
    if [[ -n "$tech_stack" ]]; then
        new_change_entry="- $CURRENT_BRANCH: $tech_stack を追加"
    elif [[ -n "$NEW_DB" ]] && [[ "$NEW_DB" != "N/A" ]] && [[ "$NEW_DB" != "要明確化" ]]; then
        new_change_entry="- $CURRENT_BRANCH: $NEW_DB を追加"
    fi

    # ファイル内にセクションが存在するかチェック
    local has_active_technologies=0
    local has_recent_changes=0

    if grep -q "^## Active Technologies" "$target_file" 2>/dev/null; then
        has_active_technologies=1
    fi

    if grep -q "^## Recent Changes" "$target_file" 2>/dev/null; then
        has_recent_changes=1
    fi

    # ファイルを行ごとに処理
    local in_tech_section=false
    local in_changes_section=false
    local tech_entries_added=false
    local changes_entries_added=false
    local existing_changes_count=0
    local file_ended=false

    while IFS= read -r line || [[ -n "$line" ]]; do
        # Active Technologiesセクションを処理
        if [[ "$line" == "## Active Technologies" ]]; then
            echo "$line" >> "$temp_file"
            in_tech_section=true
            continue
        elif [[ $in_tech_section == true ]] && [[ "$line" =~ ^##[[:space:]] ]]; then
            # セクション終了前に新しい技術エントリを追加
            if [[ $tech_entries_added == false ]] && [[ ${#new_tech_entries[@]} -gt 0 ]]; then
                printf '%s\n' "${new_tech_entries[@]}" >> "$temp_file"
                tech_entries_added=true
            fi
            echo "$line" >> "$temp_file"
            in_tech_section=false
            continue
        elif [[ $in_tech_section == true ]] && [[ -z "$line" ]]; then
            # 技術セクション内の空行前に新しいエントリを追加
            if [[ $tech_entries_added == false ]] && [[ ${#new_tech_entries[@]} -gt 0 ]]; then
                printf '%s\n' "${new_tech_entries[@]}" >> "$temp_file"
                tech_entries_added=true
            fi
            echo "$line" >> "$temp_file"
            continue
        fi

        # Recent Changesセクションを処理
        if [[ "$line" == "## Recent Changes" ]]; then
            echo "$line" >> "$temp_file"
            # 見出し直後に新しい変更エントリを追加
            if [[ -n "$new_change_entry" ]]; then
                echo "$new_change_entry" >> "$temp_file"
            fi
            in_changes_section=true
            changes_entries_added=true
            continue
        elif [[ $in_changes_section == true ]] && [[ "$line" =~ ^##[[:space:]] ]]; then
            echo "$line" >> "$temp_file"
            in_changes_section=false
            continue
        elif [[ $in_changes_section == true ]] && [[ "$line" == "- "* ]]; then
            # 既存の変更は最初の2つのみ保持
            if [[ $existing_changes_count -lt 2 ]]; then
                echo "$line" >> "$temp_file"
                ((existing_changes_count++))
            fi
            continue
        fi

        # タイムスタンプを更新
        if [[ "$line" =~ \*\*Last\ updated\*\*:.*[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9] ]]; then
            echo "$line" | sed "s/[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]/$current_date/" >> "$temp_file"
        else
            echo "$line" >> "$temp_file"
        fi
    done < "$target_file"

    # ループ後のチェック: まだActive Technologiesセクション内で新しいエントリを追加していない場合
    if [[ $in_tech_section == true ]] && [[ $tech_entries_added == false ]] && [[ ${#new_tech_entries[@]} -gt 0 ]]; then
        printf '%s\n' "${new_tech_entries[@]}" >> "$temp_file"
        tech_entries_added=true
    fi

    # セクションが存在しない場合はファイル末尾に追加
    if [[ $has_active_technologies -eq 0 ]] && [[ ${#new_tech_entries[@]} -gt 0 ]]; then
        echo "" >> "$temp_file"
        echo "## Active Technologies" >> "$temp_file"
        printf '%s\n' "${new_tech_entries[@]}" >> "$temp_file"
        tech_entries_added=true
    fi

    if [[ $has_recent_changes -eq 0 ]] && [[ -n "$new_change_entry" ]]; then
        echo "" >> "$temp_file"
        echo "## Recent Changes" >> "$temp_file"
        echo "$new_change_entry" >> "$temp_file"
        changes_entries_added=true
    fi

    # 一時ファイルをターゲットにアトミックに移動
    if ! mv "$temp_file" "$target_file"; then
        log_error "ターゲットファイルの更新に失敗しました"
        rm -f "$temp_file"
        return 1
    fi

    return 0
}
#==============================================================================
# メインエージェントファイル更新関数
#==============================================================================

update_agent_file() {
    local target_file="$1"
    local agent_name="$2"

    if [[ -z "$target_file" ]] || [[ -z "$agent_name" ]]; then
        log_error "update_agent_fileにはtarget_fileとagent_nameパラメータが必要です"
        return 1
    fi

    log_info "$agent_name コンテキストファイルを更新中: $target_file"

    local project_name
    project_name=$(basename "$REPO_ROOT")
    local current_date
    current_date=$(date +%Y-%m-%d)

    # ディレクトリが存在しない場合は作成
    local target_dir
    target_dir=$(dirname "$target_file")
    if [[ ! -d "$target_dir" ]]; then
        if ! mkdir -p "$target_dir"; then
            log_error "ディレクトリの作成に失敗しました: $target_dir"
            return 1
        fi
    fi

    if [[ ! -f "$target_file" ]]; then
        # テンプレートから新しいファイルを作成
        local temp_file
        temp_file=$(mktemp) || {
            log_error "一時ファイルの作成に失敗しました"
            return 1
        }

        if create_new_agent_file "$target_file" "$temp_file" "$project_name" "$current_date"; then
            if mv "$temp_file" "$target_file"; then
                log_success "新しい $agent_name コンテキストファイルを作成しました"
            else
                log_error "一時ファイルを $target_file に移動できませんでした"
                rm -f "$temp_file"
                return 1
            fi
        else
            log_error "新しいエージェントファイルの作成に失敗しました"
            rm -f "$temp_file"
            return 1
        fi
    else
        # 既存ファイルを更新
        if [[ ! -r "$target_file" ]]; then
            log_error "既存ファイルを読み取れません: $target_file"
            return 1
        fi

        if [[ ! -w "$target_file" ]]; then
            log_error "既存ファイルに書き込めません: $target_file"
            return 1
        fi

        if update_existing_agent_file "$target_file" "$current_date"; then
            log_success "既存の $agent_name コンテキストファイルを更新しました"
        else
            log_error "既存のエージェントファイルの更新に失敗しました"
            return 1
        fi
    fi

    return 0
}

#==============================================================================
# エージェント選択と処理
#==============================================================================

update_specific_agent() {
    local agent_type="$1"

    case "$agent_type" in
        claude)
            update_agent_file "$CLAUDE_FILE" "Claude Code"
            ;;
        gemini)
            update_agent_file "$GEMINI_FILE" "Gemini CLI"
            ;;
        copilot)
            update_agent_file "$COPILOT_FILE" "GitHub Copilot"
            ;;
        cursor|cursor-agent)
            update_agent_file "$CURSOR_FILE" "Cursor IDE"
            ;;
        qwen)
            update_agent_file "$QWEN_FILE" "Qwen Code"
            ;;
        opencode)
            update_agent_file "$AGENTS_FILE" "opencode"
            ;;
        codex)
            update_agent_file "$AGENTS_FILE" "Codex CLI"
            ;;
        windsurf)
            update_agent_file "$WINDSURF_FILE" "Windsurf"
            ;;
        kilocode)
            update_agent_file "$KILOCODE_FILE" "Kilo Code"
            ;;
        auggie)
            update_agent_file "$AUGGIE_FILE" "Auggie CLI"
            ;;
        roo)
            update_agent_file "$ROO_FILE" "Roo Code"
            ;;
        codebuddy)
            update_agent_file "$CODEBUDDY_FILE" "CodeBuddy CLI"
            ;;
        amp)
            update_agent_file "$AMP_FILE" "Amp"
            ;;
        q)
            update_agent_file "$Q_FILE" "Amazon Q Developer CLI"
            ;;
        *)
            log_error "不明なエージェントタイプ '$agent_type'"
            log_error "指定可能: claude|gemini|copilot|cursor|qwen|opencode|codex|windsurf|kilocode|auggie|roo|amp|q"
            exit 1
            ;;
    esac
}

update_all_existing_agents() {
    local found_agent=false

    # 各エージェントファイルをチェックし、存在すれば更新
    if [[ -f "$CLAUDE_FILE" ]]; then
        update_agent_file "$CLAUDE_FILE" "Claude Code"
        found_agent=true
    fi

    if [[ -f "$GEMINI_FILE" ]]; then
        update_agent_file "$GEMINI_FILE" "Gemini CLI"
        found_agent=true
    fi

    if [[ -f "$COPILOT_FILE" ]]; then
        update_agent_file "$COPILOT_FILE" "GitHub Copilot"
        found_agent=true
    fi

    if [[ -f "$CURSOR_FILE" ]]; then
        update_agent_file "$CURSOR_FILE" "Cursor IDE"
        found_agent=true
    fi

    if [[ -f "$QWEN_FILE" ]]; then
        update_agent_file "$QWEN_FILE" "Qwen Code"
        found_agent=true
    fi

    if [[ -f "$AGENTS_FILE" ]]; then
        update_agent_file "$AGENTS_FILE" "Codex/opencode"
        found_agent=true
    fi

    if [[ -f "$WINDSURF_FILE" ]]; then
        update_agent_file "$WINDSURF_FILE" "Windsurf"
        found_agent=true
    fi

    if [[ -f "$KILOCODE_FILE" ]]; then
        update_agent_file "$KILOCODE_FILE" "Kilo Code"
        found_agent=true
    fi

    if [[ -f "$AUGGIE_FILE" ]]; then
        update_agent_file "$AUGGIE_FILE" "Auggie CLI"
        found_agent=true
    fi

    if [[ -f "$ROO_FILE" ]]; then
        update_agent_file "$ROO_FILE" "Roo Code"
        found_agent=true
    fi

    if [[ -f "$CODEBUDDY_FILE" ]]; then
        update_agent_file "$CODEBUDDY_FILE" "CodeBuddy CLI"
        found_agent=true
    fi

    if [[ -f "$Q_FILE" ]]; then
        update_agent_file "$Q_FILE" "Amazon Q Developer CLI"
        found_agent=true
    fi

    # エージェントファイルが存在しない場合はデフォルトでClaudeファイルを作成
    if [[ "$found_agent" == false ]]; then
        log_info "既存のエージェントファイルが見つかりません。デフォルトのClaudeファイルを作成します..."
        update_agent_file "$CLAUDE_FILE" "Claude Code"
    fi
}
print_summary() {
    echo
    log_info "変更サマリー:"

    if [[ -n "$NEW_LANG" ]]; then
        echo "  - 言語を追加: $NEW_LANG"
    fi

    if [[ -n "$NEW_FRAMEWORK" ]]; then
        echo "  - フレームワークを追加: $NEW_FRAMEWORK"
    fi

    if [[ -n "$NEW_DB" ]] && [[ "$NEW_DB" != "N/A" ]]; then
        echo "  - データベースを追加: $NEW_DB"
    fi

    echo

    log_info "使用法: $0 [claude|gemini|copilot|cursor|qwen|opencode|codex|windsurf|kilocode|auggie|codebuddy|q]"
}

#==============================================================================
# メイン実行
#==============================================================================

main() {
    # 処理前に環境を検証
    validate_environment

    log_info "=== 機能 $CURRENT_BRANCH のアシスタントコンテキストファイルを更新中 ==="

    # planファイルを解析してプロジェクト情報を抽出
    if ! parse_plan_data "$NEW_PLAN"; then
        log_error "Planデータの解析に失敗しました"
        exit 1
    fi

    # アシスタントタイプ引数に基づいて処理
    local success=true

    if [[ -z "$AGENT_TYPE" ]]; then
        # 特定のアシスタントが指定されていない場合 - 既存のすべてのアシスタントファイルを更新
        log_info "アシスタントが指定されていません。既存のすべてのアシスタントファイルを更新します..."
        if ! update_all_existing_agents; then
            success=false
        fi
    else
        # 特定のアシスタントが指定された場合 - そのアシスタントのみ更新
        log_info "特定のアシスタントを更新: $AGENT_TYPE"
        if ! update_specific_agent "$AGENT_TYPE"; then
            success=false
        fi
    fi

    # サマリーを出力
    print_summary

    if [[ "$success" == true ]]; then
        log_success "アシスタントコンテキストの更新が正常に完了しました"
        exit 0
    else
        log_error "アシスタントコンテキストの更新がエラーで完了しました"
        exit 1
    fi
}

# スクリプトが直接実行された場合にmain関数を実行
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi
