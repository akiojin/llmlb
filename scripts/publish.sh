#!/usr/bin/env bash
set -euo pipefail
[ "${PUBLISH_DEBUG:-0}" = "1" ] && set -x

# publish.sh <major|minor|patch> [--tags-only|--no-push] [--remote <name>]
# 単一入口で以下を実施:
# 1) Cargo workspace version の更新
# 2) タグ付けとコミット＆プッシュ

usage() { echo "Usage: $0 <major|minor|patch> [--tags-only|--no-push] [--remote <name>]"; exit 1; }

LEVEL=${1-}
[[ "$LEVEL" =~ ^(major|minor|patch)$ ]] || usage
shift || true

PUSH_MODE=${PUBLISH_PUSH:-all}

while [ $# -gt 0 ]; do
  case "$1" in
    --tags-only)
      PUSH_MODE=tags
      ;;
    --no-push)
      PUSH_MODE=none
      ;;
    --remote)
      shift
      [ $# -gt 0 ] || { echo "[error] --remote requires a value" >&2; exit 1; }
      REMOTE="$1"
      ;;
    *)
      echo "[warn] unknown option: $1" >&2
      ;;
  esac
  shift || true
done

ROOT_DIR=$(cd "$(dirname "$0")/.." && pwd)
REMOTE=${REMOTE:-origin}
cd "$ROOT_DIR"

CUR_VER=$(
  awk '
    /^\[workspace.package\]$/ { in_ws = 1; next }
    /^\[/ && $0 != "[workspace.package]" { in_ws = 0 }
    in_ws && /^version = / {
      gsub(/"/, "", $3)
      print $3
      exit
    }
  ' Cargo.toml
)

if [ -z "$CUR_VER" ]; then
  echo "[error] failed to read workspace version from Cargo.toml" >&2
  exit 2
fi

echo "[info] current version: $CUR_VER"

IFS='.' read -r MAJOR MINOR PATCH <<<"$CUR_VER"
case "$LEVEL" in
  major)
    MAJOR=$((MAJOR + 1))
    MINOR=0
    PATCH=0
    ;;
  minor)
    MINOR=$((MINOR + 1))
    PATCH=0
    ;;
  patch)
    PATCH=$((PATCH + 1))
    ;;
esac

NEW_VER="${MAJOR}.${MINOR}.${PATCH}"
TAG="v${NEW_VER}"

echo "[step] update Cargo workspace version -> $NEW_VER"
awk -v new_ver="$NEW_VER" '
  /^\[workspace.package\]$/ { in_ws = 1; print; next }
  /^\[/ && $0 != "[workspace.package]" { in_ws = 0 }
  in_ws && /^version = / {
    print "version = \"" new_ver "\""
    next
  }
  { print }
' Cargo.toml > Cargo.toml.tmp
mv Cargo.toml.tmp Cargo.toml

git add Cargo.toml
if ! git diff --cached --quiet; then
  git commit -m "chore(release): $TAG"
fi

if git rev-parse -q --verify "$TAG" >/dev/null; then
  echo "[info] tag exists: $TAG"
else
  git tag -a "$TAG" -m "$TAG"
fi

if ! git ls-remote --exit-code "$REMOTE" >/dev/null 2>&1; then
  echo "[error] remote not accessible: $REMOTE" >&2
  exit 2
fi

case "$PUSH_MODE" in
  all)
    echo "[step] push commits and tag (mode=all)"
    git push --follow-tags "$REMOTE" || echo "[warn] git push --follow-tags failed; will try explicit tag push"
    git push "$REMOTE" "$TAG" || true
    ;;
  tags)
    echo "[step] push tag only (mode=tags)"
    git push "$REMOTE" "$TAG" || true
    ;;
  none)
    echo "[step] skip push (mode=none)"
    ;;
  *)
    echo "[error] unknown PUSH_MODE: $PUSH_MODE" >&2
    exit 2
    ;;
esac

echo "[step] verify tag on remote: $TAG"
if [ "$PUSH_MODE" = "none" ]; then
  echo "[skip] verification skipped (no push)"
elif git ls-remote --tags "$REMOTE" | awk '{print $2}' | grep -qx "refs/tags/$TAG"; then
  echo "[ok] tag exists on remote: $TAG"
else
  echo "[warn] tag not found on remote; retrying explicit push"
  for i in 1 2 3; do
    sleep $((i * 2))
    git push "$REMOTE" "$TAG" && break || true
  done
  if git ls-remote --tags "$REMOTE" | awk '{print $2}' | grep -qx "refs/tags/$TAG"; then
    echo "[ok] tag exists on remote after retry: $TAG"
  else
    echo "[error] failed to push tag $TAG to $REMOTE" >&2
    exit 3
  fi
fi

echo "[done] release tag prepared: $TAG"
