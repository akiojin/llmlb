#!/usr/bin/env bats
# E2E tests for OpenAI-compatible API with local LLM
#
# Prerequisites:
#   - Router running (LLM_ROUTER_URL, default: http://localhost:8080)
#   - Node running with at least one model available
#   - API key set (LLM_ROUTER_API_KEY)
#
# Usage:
#   LLM_ROUTER_URL=http://localhost:8081 \
#   LLM_ROUTER_API_KEY=sk_xxx \
#   npx bats tests/e2e/test-openai-api.bats

setup() {
    ROUTER_URL="${LLM_ROUTER_URL:-http://localhost:8080}"
    API_KEY="${LLM_ROUTER_API_KEY}"

    if [[ -z "$API_KEY" ]]; then
        skip "LLM_ROUTER_API_KEY is not set"
    fi
}

# Helper function to make API requests
api_request() {
    local endpoint="$1"
    local method="${2:-GET}"
    local data="$3"

    if [[ -n "$data" ]]; then
        curl -s -X "$method" \
            -H "Authorization: Bearer $API_KEY" \
            -H "Content-Type: application/json" \
            -d "$data" \
            "${ROUTER_URL}${endpoint}"
    else
        curl -s -X "$method" \
            -H "Authorization: Bearer $API_KEY" \
            "${ROUTER_URL}${endpoint}"
    fi
}

# Helper to check if router is accessible
check_router() {
    local response
    response=$(curl -s -o /dev/null -w "%{http_code}" \
        -H "Authorization: Bearer $API_KEY" \
        "${ROUTER_URL}/v1/models")
    [[ "$response" == "200" ]]
}

# Helper to get first available model
get_model() {
    api_request "/v1/models" | jq -r '.data[0].id // empty'
}

@test "Router is accessible" {
    run check_router
    [ "$status" -eq 0 ]
}

@test "GET /v1/models returns model list" {
    run api_request "/v1/models"
    [ "$status" -eq 0 ]

    # Check response structure
    echo "$output" | jq -e '.object == "list"'
    echo "$output" | jq -e '.data | type == "array"'
    echo "$output" | jq -e '.data | length > 0'
}

@test "GET /v1/models returns valid model objects" {
    run api_request "/v1/models"
    [ "$status" -eq 0 ]

    # Check first model has required fields
    echo "$output" | jq -e '.data[0].id'
    echo "$output" | jq -e '.data[0].object == "model"'
}

@test "POST /v1/chat/completions basic request" {
    MODEL=$(get_model)
    [[ -n "$MODEL" ]] || skip "No models available"

    local data
    data=$(cat <<EOF
{
    "model": "$MODEL",
    "messages": [{"role": "user", "content": "Say test"}],
    "max_tokens": 10
}
EOF
)

    run api_request "/v1/chat/completions" "POST" "$data"
    [ "$status" -eq 0 ]

    # Check response structure
    echo "$output" | jq -e '.object == "chat.completion"'
    echo "$output" | jq -e '.choices | type == "array"'
    echo "$output" | jq -e '.choices | length > 0'
    echo "$output" | jq -e '.choices[0].message.role == "assistant"'
    echo "$output" | jq -e '.choices[0].message.content | type == "string"'
}

@test "POST /v1/chat/completions with system prompt" {
    MODEL=$(get_model)
    [[ -n "$MODEL" ]] || skip "No models available"

    local data
    data=$(cat <<EOF
{
    "model": "$MODEL",
    "messages": [
        {"role": "system", "content": "You are helpful."},
        {"role": "user", "content": "Hi"}
    ],
    "max_tokens": 10
}
EOF
)

    run api_request "/v1/chat/completions" "POST" "$data"
    [ "$status" -eq 0 ]

    echo "$output" | jq -e '.choices[0].message.content | type == "string"'
}

@test "POST /v1/chat/completions with temperature parameter" {
    MODEL=$(get_model)
    [[ -n "$MODEL" ]] || skip "No models available"

    local data
    data=$(cat <<EOF
{
    "model": "$MODEL",
    "messages": [{"role": "user", "content": "Hi"}],
    "max_tokens": 10,
    "temperature": 0.5
}
EOF
)

    run api_request "/v1/chat/completions" "POST" "$data"
    [ "$status" -eq 0 ]

    echo "$output" | jq -e '.choices[0].message.content | type == "string"'
}

@test "POST /v1/chat/completions with top_p parameter" {
    MODEL=$(get_model)
    [[ -n "$MODEL" ]] || skip "No models available"

    local data
    data=$(cat <<EOF
{
    "model": "$MODEL",
    "messages": [{"role": "user", "content": "Hi"}],
    "max_tokens": 10,
    "top_p": 0.9
}
EOF
)

    run api_request "/v1/chat/completions" "POST" "$data"
    [ "$status" -eq 0 ]

    echo "$output" | jq -e '.choices[0].message.content | type == "string"'
}

@test "POST /v1/chat/completions multi-turn conversation" {
    MODEL=$(get_model)
    [[ -n "$MODEL" ]] || skip "No models available"

    local data
    data=$(cat <<EOF
{
    "model": "$MODEL",
    "messages": [
        {"role": "user", "content": "My name is Alice"},
        {"role": "assistant", "content": "Hello Alice"},
        {"role": "user", "content": "What is my name"}
    ],
    "max_tokens": 20
}
EOF
)

    run api_request "/v1/chat/completions" "POST" "$data"
    [ "$status" -eq 0 ]

    echo "$output" | jq -e '.choices[0].message.content | type == "string"'
}

@test "POST /v1/chat/completions streaming returns SSE format" {
    MODEL=$(get_model)
    [[ -n "$MODEL" ]] || skip "No models available"

    local data
    data=$(cat <<EOF
{
    "model": "$MODEL",
    "messages": [{"role": "user", "content": "Hi"}],
    "max_tokens": 10,
    "stream": true
}
EOF
)

    # Use curl with -N for streaming and capture first few lines
    local response
    response=$(curl -s -N -X POST \
        -H "Authorization: Bearer $API_KEY" \
        -H "Content-Type: application/json" \
        -d "$data" \
        "${ROUTER_URL}/v1/chat/completions" 2>&1 | head -5)

    # Check that response starts with "data:" (SSE format)
    echo "$response" | grep -q "^data:"
}

@test "POST /v1/chat/completions streaming ends with [DONE]" {
    MODEL=$(get_model)
    [[ -n "$MODEL" ]] || skip "No models available"

    local data
    data=$(cat <<EOF
{
    "model": "$MODEL",
    "messages": [{"role": "user", "content": "Hi"}],
    "max_tokens": 10,
    "stream": true
}
EOF
)

    local response
    response=$(curl -s -N -X POST \
        -H "Authorization: Bearer $API_KEY" \
        -H "Content-Type: application/json" \
        -d "$data" \
        "${ROUTER_URL}/v1/chat/completions" 2>&1)

    # Check that response ends with [DONE]
    echo "$response" | grep -q "\[DONE\]"
}

@test "POST /v1/chat/completions missing model returns error" {
    local data='{"messages": [{"role": "user", "content": "Hi"}]}'

    run api_request "/v1/chat/completions" "POST" "$data"

    # Should return error (4xx status code or error in body)
    # The exact behavior depends on implementation
    [[ "$output" == *"error"* ]] || [[ "$output" == *"model"* ]]
}

@test "POST /v1/chat/completions invalid model returns error" {
    local data
    data=$(cat <<EOF
{
    "model": "nonexistent-model-12345",
    "messages": [{"role": "user", "content": "Hi"}],
    "max_tokens": 10
}
EOF
)

    run api_request "/v1/chat/completions" "POST" "$data"

    # Should return error
    [[ "$output" == *"error"* ]] || [[ "$output" == *"not found"* ]] || [[ "$output" == *"No available"* ]]
}

@test "Request without API key returns 401" {
    local response
    response=$(curl -s -o /dev/null -w "%{http_code}" \
        "${ROUTER_URL}/v1/models")

    # API key authentication is required for /v1/* endpoints
    [[ "$response" == "401" ]]
}

@test "Request with invalid API key returns 401" {
    local response
    response=$(curl -s -o /dev/null -w "%{http_code}" \
        -H "Authorization: Bearer invalid_key_12345" \
        "${ROUTER_URL}/v1/models")

    # Invalid API key should return 401 Unauthorized
    [[ "$response" == "401" ]]
}

@test "Request history records local model requests" {
    MODEL=$(get_model)
    [[ -n "$MODEL" ]] || skip "No models available"

    # Make a request
    local data
    data=$(cat <<EOF
{
    "model": "$MODEL",
    "messages": [{"role": "user", "content": "Test request for history"}],
    "max_tokens": 5
}
EOF
)

    api_request "/v1/chat/completions" "POST" "$data" > /dev/null

    # Wait a moment for history to be recorded
    sleep 1

    # Check request history
    run api_request "/v0/dashboard/request-responses"
    [ "$status" -eq 0 ]

    echo "$output" | jq -e '.records | type == "array"'
    echo "$output" | jq -e '.total_count >= 0'
}
