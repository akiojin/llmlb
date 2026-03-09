#!/bin/bash

# Claude Code PreToolUse Hook: Block git branch operations.
# Blocks branch switching and branch/worktree mutation commands while allowing
# read-only `git branch` queries.

is_read_only_git_branch() {
    local branch_args="$1"

    # Trim
    branch_args=$(echo "$branch_args" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')

    # Empty means `git branch` list output
    if [ -z "$branch_args" ]; then
        return 0
    fi

    local -a tokens
    read -r -a tokens <<< "$branch_args"

    local i=0
    local token_count=${#tokens[@]}

    while [ $i -lt $token_count ]; do
        local token="${tokens[$i]}"
        case "$token" in
            --list|--show-current|--all|-a|--remotes|-r|-v|-vv|--verbose)
                i=$((i + 1))
                ;;
            --contains|--merged|--no-merged|--points-at|--format|--sort|--abbrev)
                # Option may be followed by a value
                if [ $((i + 1)) -lt $token_count ] && [[ ! "${tokens[$((i + 1))]}" =~ ^- ]]; then
                    i=$((i + 2))
                else
                    i=$((i + 1))
                fi
                ;;
            *)
                return 1
                ;;
        esac
    done

    return 0
}

# Parse a `git` command segment and extract `<subcommand> [args...]`.
# Handles global options such as `-C <path>` and `--work-tree=<path>`.
extract_git_subcommand_and_args() {
    local git_cmd="$1"
    local -a tokens
    read -r -a tokens <<< "$git_cmd"

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
                local subcommand="${tokens[$i]}"
                local args="${tokens[*]:$((i + 1))}"
                if [ -n "$args" ]; then
                    echo "$subcommand $args"
                else
                    echo "$subcommand"
                fi
                return 0
                ;;
        esac
    done

    return 1
}

emit_block() {
    local reason="$1"
    local stop_reason="$2"

    cat <<EOF
{
  "decision": "block",
  "reason": "$reason",
  "stopReason": "$stop_reason"
}
EOF
}

# Prefer jq, fallback to jq.exe on Windows Git Bash.
if command -v jq >/dev/null 2>&1; then
    JQ_BIN="jq"
elif command -v jq.exe >/dev/null 2>&1; then
    JQ_BIN="jq.exe"
else
    emit_block \
      "[blocked] jq command is required" \
      "Cannot evaluate hook policy because jq is not available in the shell environment. Install jq and retry."
    exit 2
fi

# Read JSON input from stdin
json_input=$(cat)

# Tool name
tool_name=$(echo "$json_input" | "$JQ_BIN" -r '.tool_name // empty' | tr -d '\r')

# Only enforce for Bash tool
if [ "$tool_name" != "Bash" ]; then
    exit 0
fi

# Command text
command=$(echo "$json_input" | "$JQ_BIN" -r '.tool_input.command // empty' | tr -d '\r')

# Split compound commands for independent checks
command_segments=$(printf '%s\n' "$command" | sed -E 's/\|&/\n/g; s/\|\|/\n/g; s/&&/\n/g; s/[;|&]/\n/g')

while IFS= read -r segment; do
    trimmed_segment=$(echo "$segment" | sed 's/[<>].*//; s/<<.*//' | xargs)

    if [ -z "$trimmed_segment" ]; then
        continue
    fi

    # Block interactive rebase against origin/main
    if printf '%s' "$trimmed_segment" | grep -qE '^git[[:space:]]+rebase\b'; then
        if printf '%s' "$trimmed_segment" | grep -qE '(^|[[:space:]])(-i|--interactive)([[:space:]]|$)' &&
           printf '%s' "$trimmed_segment" | grep -qE '(^|[[:space:]])origin/main([[:space:]]|$)'; then
            emit_block \
              "[blocked] Interactive rebase against origin/main is not allowed" \
              "Interactive rebase against origin/main initiated by LLMs is blocked because it frequently fails and disrupts sessions.\n\nBlocked command: $command"
            echo "[blocked] $command" >&2
            exit 2
        fi
    fi

    if echo "$trimmed_segment" | grep -qE '^git\b'; then
        git_parsed=$(extract_git_subcommand_and_args "$trimmed_segment")
        if [ -z "$git_parsed" ]; then
            continue
        fi

        git_subcommand=$(printf '%s' "$git_parsed" | awk '{print $1}')
        git_subcommand_args=$(printf '%s' "$git_parsed" | cut -d' ' -f2-)
        if [ "$git_subcommand_args" = "$git_parsed" ]; then
            git_subcommand_args=""
        fi

        # Block checkout/switch
        if [ "$git_subcommand" = "checkout" ] || [ "$git_subcommand" = "switch" ]; then
            emit_block \
              "[blocked] Branch switching commands (checkout/switch) are not allowed" \
              "Worktree is designed to complete work on the launched branch. Branch operations such as git checkout and git switch cannot be executed.\n\nBlocked command: $command"
            echo "[blocked] $command" >&2
            exit 2
        fi

        # Block destructive branch operations (allow read-only list/query flags)
        if [ "$git_subcommand" = "branch" ]; then
            if is_read_only_git_branch "$git_subcommand_args"; then
                continue
            fi

            emit_block \
              "[blocked] Branch modification commands are not allowed" \
              "Worktree is designed to complete work on the launched branch. Destructive branch operations such as git branch -d, git branch -m cannot be executed.\n\nBlocked command: $command"
            echo "[blocked] $command" >&2
            exit 2
        fi

        # Block worktree commands
        if [ "$git_subcommand" = "worktree" ]; then
            emit_block \
              "[blocked] Worktree commands are not allowed" \
              "Worktree management operations such as git worktree add/remove cannot be executed from within a worktree.\n\nBlocked command: $command"
            echo "[blocked] $command" >&2
            exit 2
        fi
    fi
done <<< "$command_segments"

exit 0
