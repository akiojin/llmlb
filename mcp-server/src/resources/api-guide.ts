import type { Resource } from "@modelcontextprotocol/sdk/types.js";

const GUIDE_BASE_URI = "llm-router://guide";

/**
 * API Guide categories
 */
const GUIDE_CATEGORIES = [
  {
    id: "overview",
    name: "llm-router-api-overview",
    description: "Overview of LLM Router API categories, authentication methods, and base URL configuration",
  },
  {
    id: "openai-compatible",
    name: "llm-router-openai-api",
    description: "OpenAI-compatible endpoints: /v1/chat/completions, /v1/completions, /v1/embeddings, /v1/models",
  },
  {
    id: "node-management",
    name: "llm-router-node-api",
    description: "Node management endpoints: /v0/nodes (list, register, delete, configure)",
  },
  {
    id: "model-management",
    name: "llm-router-model-api",
    description: "Model management endpoints: /v0/models/* (register, convert, list)",
  },
  {
    id: "dashboard",
    name: "llm-router-dashboard-api",
    description: "Dashboard and monitoring endpoints: /v0/dashboard/* (stats, overview, metrics)",
  },
];

/**
 * Get all API guide resource definitions
 */
export function getApiGuideResources(): Resource[] {
  return GUIDE_CATEGORIES.map((cat) => ({
    uri: `${GUIDE_BASE_URI}/${cat.id}`,
    name: cat.name,
    description: cat.description,
    mimeType: "text/markdown",
  }));
}

/**
 * Get API guide content for a category
 */
export function getApiGuideContent(
  category: string,
  routerUrl: string
): string | null {
  switch (category) {
    case "overview":
      return getOverviewGuide(routerUrl);
    case "openai-compatible":
      return getOpenAiGuide(routerUrl);
    case "node-management":
      return getNodeManagementGuide(routerUrl);
    case "model-management":
      return getModelManagementGuide(routerUrl);
    case "dashboard":
      return getDashboardGuide(routerUrl);
    default:
      return null;
  }
}

function getOverviewGuide(routerUrl: string): string {
  return `# LLM Router API Overview

## Base URL

\`\`\`
${routerUrl}
\`\`\`

## Authentication Methods

### 1. API Key Authentication (Inference APIs)

**Header**: \`X-API-Key: sk_xxx\` or \`Authorization: Bearer sk_xxx\`

**Used for**:
- /v1/chat/completions
- /v1/completions
- /v1/embeddings
- /v1/models

### 2. JWT Authentication (Management APIs)

**Header**: \`Authorization: Bearer <jwt_token>\`

**Used for**:
- /v0/users (user management)
- /v0/api-keys (API key management)

**Obtain JWT via**:
\`\`\`bash
curl -X POST ${routerUrl}/v0/auth/login \\
  -H "Content-Type: application/json" \\
  -d '{"username":"admin","password":"your_password"}'
\`\`\`

### 3. Node Token (Node Communication)

**Header**: \`X-Node-Token: nt_xxx\`

**Used for**:
- /v0/health (internal node health reporting)
- /v1/models (node model sync)

## API Categories

| Category | Base Path | Description |
|----------|-----------|-------------|
| OpenAI-Compatible | /v1/* | Chat, completions, embeddings, models |
| Node Management | /v0/nodes | Register/manage inference nodes |
| Model Management | /v0/models/* | Convert, register models |
| Dashboard | /v0/dashboard/* | Stats, metrics, overview |
| Authentication | /v0/auth/* | Login, logout, user info |

## Cloud Model Routing

Use prefixes to route to cloud providers:
- \`openai:gpt-4o\` → OpenAI API
- \`google:gemini-pro\` → Google AI API
- \`anthropic:claude-3-opus\` → Anthropic API
- No prefix → Local inference node
`;
}

function getOpenAiGuide(routerUrl: string): string {
  return `# OpenAI-Compatible API

## Chat Completions

**Endpoint**: POST ${routerUrl}/v1/chat/completions

\`\`\`bash
curl -X POST ${routerUrl}/v1/chat/completions \\
  -H "Content-Type: application/json" \\
  -H "X-API-Key: YOUR_API_KEY" \\
  -d '{
    "model": "llama2",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "Hello!"}
    ],
    "stream": false
  }'
\`\`\`

**Cloud routing**:
\`\`\`bash
curl -X POST ${routerUrl}/v1/chat/completions \\
  -H "Content-Type: application/json" \\
  -d '{"model": "openai:gpt-4o", "messages": [...]}'
\`\`\`

## List Models

**Endpoint**: GET ${routerUrl}/v1/models

\`\`\`bash
curl ${routerUrl}/v1/models -H "X-API-Key: YOUR_API_KEY"
\`\`\`

## Embeddings

**Endpoint**: POST ${routerUrl}/v1/embeddings

\`\`\`bash
curl -X POST ${routerUrl}/v1/embeddings \\
  -H "Content-Type: application/json" \\
  -H "X-API-Key: YOUR_API_KEY" \\
  -d '{"model": "nomic-embed-text", "input": "Hello world"}'
\`\`\`

## Text Completions (Legacy)

**Endpoint**: POST ${routerUrl}/v1/completions

\`\`\`bash
curl -X POST ${routerUrl}/v1/completions \\
  -H "Content-Type: application/json" \\
  -H "X-API-Key: YOUR_API_KEY" \\
  -d '{"model": "llama2", "prompt": "Once upon a time"}'
\`\`\`
`;
}

function getNodeManagementGuide(routerUrl: string): string {
  return `# Node Management API

## List Nodes

**Endpoint**: GET ${routerUrl}/v0/nodes

\`\`\`bash
curl ${routerUrl}/v0/nodes
\`\`\`

## Register Node

**Endpoint**: POST ${routerUrl}/v0/nodes

\`\`\`bash
curl -X POST ${routerUrl}/v0/nodes \\
  -H "Content-Type: application/json" \\
  -d '{
    "machine_name": "gpu-server-1",
    "ip_address": "192.168.1.100",
    "runtime_version": "0.1.0",
    "runtime_port": 11434,
    "gpu_available": true,
    "gpu_devices": [
      {"model": "NVIDIA RTX 4090", "count": 1}
    ]
  }'
\`\`\`

## Delete Node

**Endpoint**: DELETE ${routerUrl}/v0/nodes/:node_id

\`\`\`bash
curl -X DELETE ${routerUrl}/v0/nodes/NODE_ID
\`\`\`

## Disconnect Node

**Endpoint**: POST ${routerUrl}/v0/nodes/:node_id/disconnect

\`\`\`bash
curl -X POST ${routerUrl}/v0/nodes/NODE_ID/disconnect
\`\`\`

## Update Node Settings

**Endpoint**: PUT ${routerUrl}/v0/nodes/:node_id/settings

\`\`\`bash
curl -X PUT ${routerUrl}/v0/nodes/NODE_ID/settings \\
  -H "Content-Type: application/json" \\
  -d '{
    "custom_name": "Primary",
    "tags": ["gpu", "primary"],
    "notes": "Keep online"
  }'
\`\`\`

To clear a nullable field, send \`null\` (for example, \`{"custom_name": null}\`).

## Node Logs

**Endpoint**: GET ${routerUrl}/v0/nodes/:node_id/logs

\`\`\`bash
curl ${routerUrl}/v0/nodes/NODE_ID/logs
\`\`\`
`;
}

function getModelManagementGuide(routerUrl: string): string {
  return `# Model Management API

## List Available Models (HuggingFace)

**Endpoint**: GET ${routerUrl}/v0/models/available?source=hf

\`\`\`bash
curl "${routerUrl}/v0/models/available?source=hf"
\`\`\`

## List Registered Models

**Endpoint**: GET ${routerUrl}/v0/models/registered

\`\`\`bash
curl ${routerUrl}/v0/models/registered
\`\`\`

## Register Model

**Endpoint**: POST ${routerUrl}/v0/models/register

\`\`\`bash
curl -X POST ${routerUrl}/v0/models/register \\
  -H "Content-Type: application/json" \\
  -d '{
    "repo": "TheBloke/Llama-2-7B-GGUF",
    "filename": "llama-2-7b.Q4_K_M.gguf"
  }'
\`\`\`

If \`filename\` is omitted, the router tries to find a GGUF file in the repo. If none exists, it will queue a conversion task.
Track progress via **Convert Tasks** (\`GET ${routerUrl}/v0/models/convert\`). When completed, the model becomes available in \`GET ${routerUrl}/v1/models\`.

## Delete Model

**Endpoint**: DELETE ${routerUrl}/v0/models/:model_name

\`\`\`bash
curl -X DELETE ${routerUrl}/v0/models/llama-2-7b
\`\`\`

## Discover GGUF Files

**Endpoint**: POST ${routerUrl}/v0/models/discover-gguf

\`\`\`bash
curl -X POST ${routerUrl}/v0/models/discover-gguf \\
  -H "Content-Type: application/json" \\
  -d '{"model": "openai/gpt-oss-20b"}'
\`\`\`

## Convert Model

**Endpoint**: POST ${routerUrl}/v0/models/convert

\`\`\`bash
curl -X POST ${routerUrl}/v0/models/convert \\
  -H "Content-Type: application/json" \\
  -d '{
    "repo": "openai/gpt-oss-20b",
    "filename": "model.bin",
    "revision": "main"
  }'
\`\`\`

## Convert Tasks

- List tasks: GET ${routerUrl}/v0/models/convert
- Get task: GET ${routerUrl}/v0/models/convert/:task_id
- Delete task: DELETE ${routerUrl}/v0/models/convert/:task_id

## Model Blob Download

**Endpoint**: GET ${routerUrl}/v0/models/blob/:model_name

\`\`\`bash
curl -L ${routerUrl}/v0/models/blob/gpt-oss-20b -o model.gguf
\`\`\`

## Model ID Format

Model IDs are normalized to a filename-based format (for example \`gpt-oss-20b\`). Colons and slashes are not used.
`;
}

function getDashboardGuide(routerUrl: string): string {
  return `# Dashboard API

## Dashboard Overview

**Endpoint**: GET ${routerUrl}/v0/dashboard/overview

\`\`\`bash
curl ${routerUrl}/v0/dashboard/overview
\`\`\`

## Dashboard Statistics

**Endpoint**: GET ${routerUrl}/v0/dashboard/stats

\`\`\`bash
curl ${routerUrl}/v0/dashboard/stats
\`\`\`

## Node Information

**Endpoint**: GET ${routerUrl}/v0/dashboard/nodes

\`\`\`bash
curl ${routerUrl}/v0/dashboard/nodes
\`\`\`

## Node Metrics History

**Endpoint**: GET ${routerUrl}/v0/dashboard/metrics/:node_id

\`\`\`bash
curl ${routerUrl}/v0/dashboard/metrics/NODE_ID
\`\`\`

## Request/Response Logs

**Endpoint**: GET ${routerUrl}/v0/dashboard/request-responses

\`\`\`bash
curl ${routerUrl}/v0/dashboard/request-responses
\`\`\`

## Export Request/Response Logs

**Endpoint**: GET ${routerUrl}/v0/dashboard/request-responses/export

\`\`\`bash
curl -L ${routerUrl}/v0/dashboard/request-responses/export -o request-responses.json
\`\`\`

## Request Detail

**Endpoint**: GET ${routerUrl}/v0/dashboard/request-responses/:id

\`\`\`bash
curl ${routerUrl}/v0/dashboard/request-responses/REQUEST_ID
\`\`\`

## Router Logs

**Endpoint**: GET ${routerUrl}/v0/dashboard/logs/router

\`\`\`bash
curl ${routerUrl}/v0/dashboard/logs/router
\`\`\`
`;
}
