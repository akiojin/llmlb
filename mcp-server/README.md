# @llmlb/mcp-server

MCP (Model Context Protocol) server for LLM Router API operations.

## Installation

```bash
npm install -g @llmlb/mcp-server
# or
npx @llmlb/mcp-server
```

## Usage

### With Claude Code

Add to your `.mcp.json`:

```json
{
  "mcpServers": {
    "llmlb": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@llmlb/mcp-server"],
      "env": {
        "LLMLB_URL": "http://localhost:32768",
        "LLMLB_API_KEY": "sk_your_api_key"
      }
    }
  }
}
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `LLMLB_URL` | Base URL of the LLM Router | `http://localhost:32768` |
| `LLMLB_API_KEY` | API key for inference endpoints | - |
| `LLMLB_JWT_TOKEN` | JWT token for management APIs | - |
| `LLMLB_OPENAPI_PATH` | Path to custom OpenAPI spec | - |

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

- `llmlb-openapi`: OpenAPI specification (JSON)
- `llmlb-guide-inference`: Guide for inference endpoints
- `llmlb-guide-node-mgmt`: Guide for node management
- `llmlb-guide-model-mgmt`: Guide for model management
- `llmlb-guide-user-mgmt`: Guide for user and API key management
- `llmlb-guide-dashboard`: Guide for dashboard and monitoring

## Security

- Only requests to the configured router host are allowed
- Shell injection patterns are blocked
- Dangerous curl options are filtered
- Authentication is automatically injected

## License

MIT
