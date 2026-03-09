#!/bin/bash

# Claude Code PreToolUse Hook: Block git branch operations
# このスクリプトは git checkout, git switch, git branch, git worktree コマンドをブロックします

contains_element() {
    local needle="$1"
    shift
    for element in "$@"; do
        if [ "$element" = "$needle" ]; then
            return 0
        fi
    done
    return 1
}

split_shell_words() {
    local input="$1"
    local python_bin=""

    if command -v python >/dev/null 2>&1; then
        python_bin="python"
    elif command -v python3 >/dev/null 2>&1; then
        python_bin="python3"
    fi

    if [ -n "$python_bin" ]; then
        SHELL_WORDS_INPUT="$input" "$python_bin" - <<'PY' 2>/dev/null
import os
import shlex

text = os.environ.get("SHELL_WORDS_INPUT", "")
try:
    tokens = shlex.split(text)
except ValueError:
    tokens = []

for token in tokens:
    print(token)
PY
        return
    fi

    local -a fallback_tokens=()
    read -r -a fallback_tokens <<< "$input"
    printf '%s\n' "${fallback_tokens[@]}"
}

GIT_SUBCOMMAND=""
GIT_SUBCOMMAND_ARGS=()

extract_git_subcommand_and_args() {
    local git_cmd="$1"
    local -a tokens=()

    while IFS= read -r token; do
        tokens+=("$token")
    done < <(split_shell_words "$git_cmd")

    GIT_SUBCOMMAND=""
    GIT_SUBCOMMAND_ARGS=()

    local i=1
    local token_count=${#tokens[@]}

    while [ $i -lt $token_count ]; do
        local token="${tokens[$i]}"
        case "$token" in
            --)
                i=$((i + 1))
                break
                ;;
            -C|--git-dir|--work-tree|--namespace|--super-prefix|--exec-path|--config-env|-c)
                i=$((i + 2))
                continue
                ;;
            -C*|-c*|--git-dir=*|--work-tree=*|--namespace=*|--super-prefix=*|--exec-path=*|--config-env=*)
                i=$((i + 1))
                continue
                ;;
            -p|--paginate|--no-pager|--bare|--no-optional-locks|--no-replace-objects|--literal-pathspecs|--glob-pathspecs|--noglob-pathspecs|--icase-pathspecs|--help|--version|--html-path|--man-path|--info-path)
                i=$((i + 1))
                continue
                ;;
            -*)
                i=$((i + 1))
                continue
                ;;
            *)
                GIT_SUBCOMMAND="${tokens[$i]}"
                if [ $((i + 1)) -lt $token_count ]; then
                    GIT_SUBCOMMAND_ARGS=("${tokens[@]:$((i + 1))}")
                fi
                return 0
                ;;
        esac
    done

    return 1
}

# git branch コマンドが参照系かどうかを判定
# 許可リスト方式：参照系フラグのみ許可、それ以外はブロック
is_read_only_git_branch() {
    if [ $# -eq 0 ]; then
        return 0
    fi

    local dangerous_flags=(-d -D --delete -m -M --move -c -C --copy --create-reflog --set-upstream-to --unset-upstream --track --no-track --edit-description -f --force)
    local expect_value_flags=(--list -l --contains --merged --no-merged --points-at --format --sort --abbrev)
    local expect_value=""

    local token
    for token in "$@"; do
        if [ -z "$token" ]; then
            continue
        fi

        if [ -n "$expect_value" ]; then
            if [[ "$token" == -* ]]; then
                expect_value=""
            else
                expect_value=""
                continue
            fi
        fi

        if [ "$token" = "--" ]; then
            return 1
        fi

        if [[ "$token" == -* ]]; then
            local option_name="$token"
            local inline_value=""

            if [[ "$token" == *=* ]]; then
                option_name="${token%%=*}"
                inline_value="${token#*=}"
            fi

            if [[ "$option_name" == -* && "$option_name" != --* && ${#option_name} -gt 2 && "$option_name" != -*=* ]]; then
                local short_flags="${option_name#-}"
                local i
                for ((i = 0; i < ${#short_flags}; i++)); do
                    local short_flag="-${short_flags:i:1}"
                    if contains_element "$short_flag" "${dangerous_flags[@]}"; then
                        return 1
                    fi
                    if contains_element "$short_flag" "${expect_value_flags[@]}"; then
                        expect_value="$short_flag"
                    fi
                done
                continue
            fi

            if contains_element "$option_name" "${dangerous_flags[@]}"; then
                return 1
            fi

            if contains_element "$option_name" "${expect_value_flags[@]}"; then
                if [ -z "$inline_value" ]; then
                    expect_value="$option_name"
                fi
                continue
            fi

            continue
        fi

        return 1
    done

    return 0
}

# stdinからJSON入力を読み取り
json_input=$(cat)

get_json_value() {
    local query="$1"

    if command -v jq >/dev/null 2>&1; then
        printf '%s' "$json_input" | jq -r "$query" 2>/dev/null
        return
    fi

    if command -v python >/dev/null 2>&1; then
        JSON_INPUT="$json_input" QUERY="$query" python - <<'PY' 2>/dev/null
import json
import os

data = os.environ.get("JSON_INPUT", "").lstrip("\ufeff").strip()
query = os.environ.get("QUERY", "")
try:
    obj = json.loads(data)
except Exception:
    print("")
    raise SystemExit

if query.startswith(".tool_name"):
    value = obj.get("tool_name", "")
elif query.startswith(".tool_input.command"):
    value = (obj.get("tool_input") or {}).get("command", "")
else:
    value = ""

print("" if value is None else value)
PY
        return
    fi

    if command -v python3 >/dev/null 2>&1; then
        JSON_INPUT="$json_input" QUERY="$query" python3 - <<'PY' 2>/dev/null
import json
import os

data = os.environ.get("JSON_INPUT", "").lstrip("\ufeff").strip()
query = os.environ.get("QUERY", "")
try:
    obj = json.loads(data)
except Exception:
    print("")
    raise SystemExit

if query.startswith(".tool_name"):
    value = obj.get("tool_name", "")
elif query.startswith(".tool_input.command"):
    value = (obj.get("tool_input") or {}).get("command", "")
else:
    value = ""

print("" if value is None else value)
PY
        return
    fi

    printf '%s' ""
}

# ツール名を確認
tool_name=$(get_json_value '.tool_name // empty')

# Bashツール以外は許可
if [ "$tool_name" != "Bash" ]; then
    exit 0
fi

# コマンドを取得
command=$(get_json_value '.tool_input.command // empty')

# 演算子で連結された各コマンドを個別にチェックするために分割
# &&, ||, ;, |, |&, &, 改行などで区切って先頭トークンを判定する
command_segments=$(printf '%s\n' "$command" | sed -E 's/\|&/\n/g; s/\|\|/\n/g; s/&&/\n/g; s/[;|&]/\n/g')

while IFS= read -r segment; do
    # リダイレクトやheredoc以降を落として、引用符は維持したまま前後の空白だけ削る
    trimmed_segment=$(printf '%s' "$segment" | sed -E 's/[<>].*//; s/<<.*//; s/^[[:space:]]+//; s/[[:space:]]+$//')

    # 空行はスキップ
    if [ -z "$trimmed_segment" ]; then
        continue
    fi

    if ! printf '%s' "$trimmed_segment" | grep -qE '^git\b'; then
        continue
    fi

    if ! extract_git_subcommand_and_args "$trimmed_segment"; then
        continue
    fi

    # インタラクティブrebase禁止 (git rebase -i origin/main)
    if [ "$GIT_SUBCOMMAND" = "rebase" ]; then
        has_interactive_flag=0
        has_origin_main=0
        for git_arg in "${GIT_SUBCOMMAND_ARGS[@]}"; do
            if [ "$git_arg" = "-i" ] || [ "$git_arg" = "--interactive" ]; then
                has_interactive_flag=1
            fi
            if [ "$git_arg" = "origin/main" ]; then
                has_origin_main=1
            fi
        done

        if [ $has_interactive_flag -eq 1 ] && [ $has_origin_main -eq 1 ]; then
            cat <<EOF
{
  "decision": "block",
  "reason": "🚫 Interactive rebase against origin/main is not allowed",
  "stopReason": "Interactive rebase against origin/main initiated by LLMs is blocked because it frequently fails and disrupts sessions.\n\nBlocked command: $command"
}
EOF
            echo "🚫 Blocked: $command" >&2
            echo "Reason: Interactive rebase against origin/main is not allowed in Worktree." >&2
            exit 2
        fi
    fi

    # checkout/switchは無条件ブロック
    if [ "$GIT_SUBCOMMAND" = "checkout" ] || [ "$GIT_SUBCOMMAND" = "switch" ]; then
        cat <<EOF
{
  "decision": "block",
  "reason": "🚫 Branch switching commands (checkout/switch) are not allowed",
  "stopReason": "Worktree is designed to complete work on the launched branch. Branch operations such as git checkout and git switch cannot be executed.\n\nBlocked command: $command"
}
EOF
        echo "🚫 Blocked: $command" >&2
        echo "Reason: Branch switching (checkout/switch) is not allowed in Worktree." >&2
        exit 2
    fi

    # branchサブコマンドは参照系のみ許可
    if [ "$GIT_SUBCOMMAND" = "branch" ]; then
        if is_read_only_git_branch "${GIT_SUBCOMMAND_ARGS[@]}"; then
            continue
        fi

        cat <<EOF
{
  "decision": "block",
  "reason": "🚫 Branch modification commands are not allowed",
  "stopReason": "Worktree is designed to complete work on the launched branch. Destructive branch operations such as git branch -d, git branch -m cannot be executed.\n\nBlocked command: $command"
}
EOF
        echo "🚫 Blocked: $command" >&2
        echo "Reason: Branch modification is not allowed in Worktree." >&2
        exit 2
    fi

    # worktreeサブコマンドをブロック（git worktree add/remove等）
    if [ "$GIT_SUBCOMMAND" = "worktree" ]; then
        cat <<EOF
{
  "decision": "block",
  "reason": "🚫 Worktree commands are not allowed",
  "stopReason": "Worktree management operations such as git worktree add/remove cannot be executed from within a worktree.\n\nBlocked command: $command"
}
EOF
        echo "🚫 Blocked: $command" >&2
        echo "Reason: Worktree management is not allowed in Worktree." >&2
        exit 2
    fi
done <<< "$command_segments"

# 許可
exit 0
