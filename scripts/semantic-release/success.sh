#!/usr/bin/env bash
# Record release information for downstream packaging jobs.

set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "Usage: $0 <version> <git-tag>" >&2
  exit 1
fi

VERSION="$1"
TAG="$2"

# releaseディレクトリが存在しない場合は作成
mkdir -p release || {
  echo "⚠️ Failed to create release directory" >&2
  exit 0  # ディレクトリ作成失敗でもワークフローは継続
}

# エラー時もワークフローを継続させるため、エラーをログに記録
if ! cat <<EOF > release/semantic-release.json
{
  "version": "${VERSION}",
  "tag": "${TAG}"
}
EOF
then
  echo "⚠️ Failed to write release/semantic-release.json, but release was created" >&2
  exit 0  # ファイル書き込み失敗でもワークフローは継続
fi

echo "✓ Release information recorded: ${TAG}"
