#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RELEASE_SCRIPT="$SCRIPT_DIR/release/create-release-branch.sh"

if [[ ! -x "$RELEASE_SCRIPT" ]]; then
  echo "[ERROR] $RELEASE_SCRIPT が見つからないか実行できません" >&2
  exit 1
fi

exec "$RELEASE_SCRIPT" "$@"
