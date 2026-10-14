#!/usr/bin/env bash

# check-coverage-gate.sh - Rust カバレッジ閾値ゲートの整合チェック
#
# SPEC #585 FR-032（ユニットテストカバレッジ80%以上）を担保するため、
# 以下を検証する（Issue #697）:
#   - Makefile の coverage ターゲットが cargo llvm-cov --fail-under-lines N を実行し、N >= 80
#   - CI ワークフローの coverage-rust ジョブが make coverage を実行する（ローカルと同一コマンド）
#
# 戻り値:
#   0: 整合
#   1: 不整合あり
#   2: エラー（ファイルが見つからない等）

set -euo pipefail

MAKEFILE="${MAKEFILE:-Makefile}"
CI_WORKFLOW="${CI_WORKFLOW:-.github/workflows/ci.yml}"
REQUIRED_MIN=80

for f in "$MAKEFILE" "$CI_WORKFLOW"; do
    if [ ! -f "$f" ]; then
        echo "✗ ファイルが見つかりません: $f" >&2
        exit 2
    fi
done

status=0

threshold=$(sed -nE 's/.*cargo llvm-cov .*--fail-under-lines[ =]+([0-9]+).*/\1/p' "$MAKEFILE" | head -1)
if [ -z "$threshold" ]; then
    echo "✗ Makefile に cargo llvm-cov --fail-under-lines が見つかりません: $MAKEFILE" >&2
    status=1
elif [ "$threshold" -lt "$REQUIRED_MIN" ]; then
    echo "✗ カバレッジ閾値が ${REQUIRED_MIN}% 未満です: ${threshold}%" >&2
    status=1
fi

if ! grep -qE '^coverage:' "$MAKEFILE"; then
    echo "✗ Makefile に coverage ターゲットがありません: $MAKEFILE" >&2
    status=1
fi

if ! grep -qE '^[[:space:]]*(run:[[:space:]]*)?make coverage[[:space:]]*$' "$CI_WORKFLOW"; then
    echo "✗ CI ワークフローが make coverage を実行していません: $CI_WORKFLOW" >&2
    status=1
fi

if [ "$status" -eq 0 ]; then
    echo "✓ coverage gate OK: --fail-under-lines ${threshold}"
fi
exit "$status"
