#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-http://127.0.0.1:32769/health}"
DURATION="${DURATION:-30s}"
CONNECTIONS="${CONNECTIONS:-128}"
THREADS="${THREADS:-8}"
RATE="${RATE:-0}"

if ! command -v wrk >/dev/null 2>&1; then
  echo "wrk is required. Install: brew install wrk (mac) / apt-get install wrk (linux)" >&2
  exit 1
fi

echo "Running wrk against ${TARGET}"
if [ "$RATE" -gt 0 ]; then
  wrk -t"${THREADS}" -c"${CONNECTIONS}" -d"${DURATION}" -R "${RATE}" "${TARGET}"
else
  wrk -t"${THREADS}" -c"${CONNECTIONS}" -d"${DURATION}" "${TARGET}"
fi
