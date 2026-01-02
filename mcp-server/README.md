# @llm-router/mcp-server

MCP (Model Context Protocol) server for LLM Router API operations.

## Installation

```bash
npm install -g @llm-router/mcp-server
# or
npx @llm-router/mcp-server
```

## Usage

### With Claude Code

Add to your `.mcp.json`:

```json
{
  "mcpServers": {
    "llm-router": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@llm-router/mcp-server"],
      "env": {
        "LLM_ROUTER_URL": "http://localhost:32768",
        "LLM_ROUTER_API_KEY": "sk_your_api_key"
      }
    }
  }
}
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `LLM_ROUTER_URL` | Base URL of the LLM Router | `http://localhost:32768` |
| `LLM_ROUTER_API_KEY` | API key for inference endpoints | - |
| `LLM_ROUTER_JWT_TOKEN` | JWT token for management APIs | - |
| `LLM_ROUTER_OPENAPI_PATH` | Path to custom OpenAPI spec | - |

## Tools

### execute_curl

Execute curl commands against the LLM Router API with automatic authentication.

**Parameters:**

- `command` (required): curl command to execute
- `auto_auth` (optional): Automatically inject auth headers (default: true)
- `timeout` (optional): Request timeout in seconds (default: 30, max: 300)

**Example:**

```bash
# List models
curl http://localhost:32768/v1/models

# Chat completion
curl -X POST http://localhost:32768/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"llama3.2:3b","messages":[{"role":"user","content":"Hello"}]}'
```

## Resources

The server provides API documentation as MCP resources:

- `llm-router-openapi`: OpenAPI specification (JSON)
- `llm-router-guide-inference`: Guide for inference endpoints
- `llm-router-guide-node-mgmt`: Guide for node management
- `llm-router-guide-model-mgmt`: Guide for model management
- `llm-router-guide-user-mgmt`: Guide for user and API key management
- `llm-router-guide-dashboard`: Guide for dashboard and monitoring

## Security

- Only requests to the configured router host are allowed
- Shell injection patterns are blocked
- Dangerous curl options are filtered
- Authentication is automatically injected

## License

MIT
