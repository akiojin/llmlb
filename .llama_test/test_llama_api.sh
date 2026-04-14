#!/bin/bash

# llama.cpp OpenAI互換API テストスクリプト
# 公開されているllama.cppサーバーをテスト対象

LLAMA_ENDPOINT="${1:-http://localhost:8000}"
echo "Testing llama.cpp at: $LLAMA_ENDPOINT"
echo ""

# 1. User-Agent 確認（サーバー側から返される情報）
echo "=== Test 1: /v1/models エンドポイント ==="
curl -s "$LLAMA_ENDPOINT/v1/models" | python -m json.tool 2>/dev/null || echo "Failed to connect"
echo ""

# 2. /v1/version エンドポイント確認（llama.cpp固有）
echo "=== Test 2: /v1/version エンドポイント ==="
curl -s "$LLAMA_ENDPOINT/v1/version" -H "User-Agent: test" | python -m json.tool 2>/dev/null || echo "Endpoint not available"
echo ""

# 3. /v1/chat/completions のテスト（簡単なリクエスト）
echo "=== Test 3: /v1/chat/completions (stream=false) ==="
curl -s -X POST "$LLAMA_ENDPOINT/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "test",
    "messages": [{"role": "user", "content": "Hi"}],
    "stream": false,
    "max_tokens": 10
  }' | python -m json.tool 2>/dev/null | head -20 || echo "Failed"
echo ""

# 4. User-Agent ヘッダー確認
echo "=== Test 4: Request Headers ==="
curl -s -v "$LLAMA_ENDPOINT/v1/models" 2>&1 | grep -i "user-agent\|server" | head -5
echo ""

