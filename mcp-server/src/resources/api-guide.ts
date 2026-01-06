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
    description: "Model management endpoints: /v0/models/* (register, delete, manifest)",
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

### 1. API Key Authentication (Scoped)

**Header**: \`X-API-Key: sk_xxx\` or \`Authorization: Bearer sk_xxx\`

**Used for**:
- /v1/* (scope: \`api\`)
- /v0/nodes (scope: \`node\`)
- /v0/health (scope: \`node\`)
- /v0/models/registry/:model_name/manifest.json (scope: \`node\`)
- /v0/* admin endpoints (scope: \`admin\`)

### 2. JWT Authentication (Management APIs)

**Header**: \`Authorization: Bearer <jwt_token>\`

**Used for**:
- /v0/auth/* (me/logout)
- /v0/users (admin only)
- /v0/api-keys (admin only)
- /v0/dashboard/*, /v0/metrics/* (admin only)

**Obtain JWT via**:
\`\`\`bash
curl -X POST ${routerUrl}/v0/auth/login \\
  -H "Content-Type: application/json" \\
  -d '{"username":"admin","password":"your_password"}'
\`\`\`

### 3. Node Token (Node Communication)

**Header**: \`X-Node-Token: nt_xxx\`

**Used for**:
- /v0/health (internal node health reporting, requires API key too)
- /v1/models (node model sync)

For \`/v0/health\`, include **both** \`X-Node-Token\` and \`Authorization: Bearer <api_key>\`.

## API Categories

| Category | Base Path | Description |
|----------|-----------|-------------|
| OpenAI-Compatible | /v1/* | Chat, completions, embeddings, models |
| Node Management | /v0/nodes | Register/manage inference nodes |
| Model Management | /v0/models/* | Register/delete models, manifest |
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
curl ${routerUrl}/v0/nodes \\
  -H "Authorization: Bearer ADMIN_API_KEY"
\`\`\`

## Register Node

**Endpoint**: POST ${routerUrl}/v0/nodes

\`\`\`bash
curl -X POST ${routerUrl}/v0/nodes \\
  -H "Content-Type: application/json" \\
  -H "Authorization: Bearer NODE_API_KEY" \\
  -d '{
    "machine_name": "gpu-server-1",
    "ip_address": "192.168.1.100",
    "runtime_version": "0.1.0",
    "runtime_port": 32768,
    "gpu_available": true,
    "gpu_devices": [
      {"model": "NVIDIA RTX 4090", "count": 1}
    ]
  }'
\`\`\`

## Delete Node

**Endpoint**: DELETE ${routerUrl}/v0/nodes/:node_id

\`\`\`bash
curl -X DELETE ${routerUrl}/v0/nodes/NODE_ID \\
  -H "Authorization: Bearer ADMIN_API_KEY"
\`\`\`

## Disconnect Node

**Endpoint**: POST ${routerUrl}/v0/nodes/:node_id/disconnect

\`\`\`bash
curl -X POST ${routerUrl}/v0/nodes/NODE_ID/disconnect \\
  -H "Authorization: Bearer ADMIN_API_KEY"
\`\`\`

## Update Node Settings

**Endpoint**: PUT ${routerUrl}/v0/nodes/:node_id/settings

\`\`\`bash
curl -X PUT ${routerUrl}/v0/nodes/NODE_ID/settings \\
  -H "Content-Type: application/json" \\
  -H "Authorization: Bearer ADMIN_API_KEY" \\
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
curl ${routerUrl}/v0/nodes/NODE_ID/logs \\
  -H "Authorization: Bearer ADMIN_API_KEY"
\`\`\`
`;
}

function getModelManagementGuide(routerUrl: string): string {
  return `# Model Management API

## List Registered Models (Node Sync)

**Endpoint**: GET ${routerUrl}/v0/models

\`\`\`bash
curl ${routerUrl}/v0/models \
  -H "Authorization: Bearer NODE_API_KEY"
\`\`\`

## Register Model

**Endpoint**: POST ${routerUrl}/v0/models/register

\`\`\`bash
curl -X POST ${routerUrl}/v0/models/register \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer ADMIN_API_KEY" \
  -d '{
    "repo": "TheBloke/Llama-2-7B-GGUF",
    "filename": "llama-2-7b.Q4_K_M.gguf"
  }'
\`\`\`

If \`filename\` is omitted, the router stores the manifest for the repo and nodes choose compatible artifacts.
The router does **not** download binaries or run conversions.

## Delete Model

**Endpoint**: DELETE ${routerUrl}/v0/models/:model_name

\`\`\`bash
curl -X DELETE ${routerUrl}/v0/models/llama-2-7b \
  -H "Authorization: Bearer ADMIN_API_KEY"
\`\`\`

## Model Manifest

**Endpoint**: GET ${routerUrl}/v0/models/registry/:model_name/manifest.json

\`\`\`bash
curl ${routerUrl}/v0/models/registry/gpt-oss-20b/manifest.json \
  -H "Authorization: Bearer NODE_API_KEY"
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
curl ${routerUrl}/v0/dashboard/overview \
  -H "Authorization: Bearer ADMIN_API_KEY"
\`\`\`

## Dashboard Statistics

**Endpoint**: GET ${routerUrl}/v0/dashboard/stats

\`\`\`bash
curl ${routerUrl}/v0/dashboard/stats \
  -H "Authorization: Bearer ADMIN_API_KEY"
\`\`\`

## Node Information

**Endpoint**: GET ${routerUrl}/v0/dashboard/nodes

\`\`\`bash
curl ${routerUrl}/v0/dashboard/nodes \
  -H "Authorization: Bearer ADMIN_API_KEY"
\`\`\`

## Node Metrics History

**Endpoint**: GET ${routerUrl}/v0/dashboard/metrics/:node_id

\`\`\`bash
curl ${routerUrl}/v0/dashboard/metrics/NODE_ID \
  -H "Authorization: Bearer ADMIN_API_KEY"
\`\`\`

## Request/Response Logs

**Endpoint**: GET ${routerUrl}/v0/dashboard/request-responses

\`\`\`bash
curl ${routerUrl}/v0/dashboard/request-responses \
  -H "Authorization: Bearer ADMIN_API_KEY"
\`\`\`

## Export Request/Response Logs

**Endpoint**: GET ${routerUrl}/v0/dashboard/request-responses/export

\`\`\`bash
curl -L ${routerUrl}/v0/dashboard/request-responses/export \
  -H "Authorization: Bearer ADMIN_API_KEY" \
  -o request-responses.json
\`\`\`

## Request Detail

**Endpoint**: GET ${routerUrl}/v0/dashboard/request-responses/:id

\`\`\`bash
curl ${routerUrl}/v0/dashboard/request-responses/REQUEST_ID \
  -H "Authorization: Bearer ADMIN_API_KEY"
\`\`\`

## Router Logs

**Endpoint**: GET ${routerUrl}/v0/dashboard/logs/router

\`\`\`bash
curl ${routerUrl}/v0/dashboard/logs/router \
  -H "Authorization: Bearer ADMIN_API_KEY"
\`\`\`
`;
}
