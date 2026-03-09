#!/bin/bash

# Claude Code PreToolUse Hook: Block git branch operations
# このスクリプトは git checkout, git switch, git branch, git worktree コマンドをブロックします

# git branch コマンドが参照系かどうかを判定
# 許可リスト方式：参照系フラグのみ許可、それ以外はブロック
is_read_only_git_branch() {
    local branch_args="$1"

    # トリム
    branch_args=$(echo "$branch_args" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')

    # 引数なしは許可（ブランチ一覧表示）
    if [ -z "$branch_args" ]; then
        return 0
    fi

    # 参照系フラグのみの場合は許可
    # 許可リスト: --list, --show-current, --all, -a, --remotes, -r, --contains, --merged, --no-merged, --points-at, --format, --sort, --abbrev, -v, -vv, --verbose
    if echo "$branch_args" | grep -qE '^(--list|--show-current|--all|-a|--remotes|-r|--contains|--merged|--no-merged|--points-at|--format|--sort|--abbrev|-v|-vv|--verbose)'; then
        return 0
    fi

    # その他はブロック（破壊的操作、新規ブランチ作成等）
    return 1
}

# stdinからJSON入力を読み取り
json_input=$(cat)

# ツール名を確認
tool_name=$(echo "$json_input" | jq -r '.tool_name // empty')

# Bashツール以外は許可
if [ "$tool_name" != "Bash" ]; then
    exit 0
fi

# コマンドを取得
command=$(echo "$json_input" | jq -r '.tool_input.command // empty')

# 演算子で連結された各コマンドを個別にチェックするために分割
# &&, ||, ;, |, |&, &, 改行などで区切って先頭トークンを判定する
command_segments=$(printf '%s\n' "$command" | sed -E 's/\|&/\n/g; s/\|\|/\n/g; s/&&/\n/g; s/[;|&]/\n/g')

while IFS= read -r segment; do
    # リダイレクトやheredoc以降を落としてトリミング
    trimmed_segment=$(echo "$segment" | sed 's/[<>].*//; s/<<.*//' | xargs)

    # 空行はスキップ
    if [ -z "$trimmed_segment" ]; then
        continue
    fi

    # インタラクティブrebase禁止 (git rebase -i origin/main)
    if printf '%s' "$trimmed_segment" | grep -qE '^git[[:space:]]+rebase\b'; then
        if printf '%s' "$trimmed_segment" | grep -qE '(^|[[:space:]])(-i|--interactive)([[:space:]]|$)' &&
           printf '%s' "$trimmed_segment" | grep -qE '(^|[[:space:]])origin/main([[:space:]]|$)'; then
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

    # ブランチ切り替え/作成/worktreeコマンドをチェック（オプション付きgitコマンドにも対応）
    # git -C /path checkout, git --work-tree=/path checkout などを検出
    if echo "$trimmed_segment" | grep -qE '^git\b'; then
        # checkout/switchは無条件ブロック
        if echo "$trimmed_segment" | grep -qE '\b(checkout|switch)\b'; then
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
        # ファイル名にbranchを含む場合は許可（例: git diff .claude/hooks/block-git-branch-ops.sh）
        if echo "$trimmed_segment" | grep -qE '^git[[:space:]]+((-[a-zA-Z]|--[a-z-]+)[[:space:]]+)*branch\b'; then
            # git ... branch の後の引数を抽出（branchより前を全て除去）
            branch_args=$(echo "$trimmed_segment" | sed -E 's/^git[[:space:]]+((-[a-zA-Z]|--[a-z-]+)[[:space:]]+)*branch//')
            if is_read_only_git_branch "$branch_args"; then
                continue
            fi
            # 破壊的操作をブロック
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
        # ファイル名にworktreeを含む場合は許可（例: git add src/worktree.ts）
        if echo "$trimmed_segment" | grep -qE '^git[[:space:]]+((-[a-zA-Z]|--[a-z-]+)[[:space:]]+)*worktree\b'; then
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
    fi
done <<< "$command_segments"

# 許可
exit 0
