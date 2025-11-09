#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$PROJECT_ROOT"

echo "==========================================="
echo "  リリースブランチ作成ワークフロー"
echo "==========================================="
echo ""

if ! command -v gh >/dev/null 2>&1; then
  echo "[ERROR] GitHub CLI (gh) がインストールされていません" >&2
  exit 1
fi

if ! gh auth status >/dev/null 2>&1; then
  echo "[ERROR] GitHub CLI が未認証です。以下を実行してください:" >&2
  echo "       gh auth login" >&2
  exit 1
fi

echo "[1/2] GitHub Actions の create-release.yml を起動します..."
gh workflow run create-release.yml --ref develop
echo "✓ ワークフローを起動しました"
echo ""

echo "[2/2] 直近の実行状況を表示します..."
sleep 5
gh run list --workflow create-release.yml --limit 3
echo ""

echo "==========================================="
echo "  次のステップ"
echo "==========================================="
echo "1. 実行状況を監視:"
echo "   gh run watch \$(gh run list --workflow=create-release.yml --limit 1 --json databaseId --jq '.[0].databaseId')"
echo ""
echo "2. ワークフロー完了後、release/vX.Y.Z ブランチが作成されます"
echo "3. push を契機に release.yml が動作し、semantic-release とマージ処理が実行されます"
echo "4. main へのマージ後、publish.yml がバイナリを添付します"
echo "==========================================="
