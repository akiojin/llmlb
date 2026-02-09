import type { Resource } from "@modelcontextprotocol/sdk/types.js";

const GUIDE_BASE_URI = "llmlb://guide";

/**
 * API Guide categories
 *
 * NOTE:
 * llmlb has migrated away from legacy `/v0/*` "node" APIs to `/api/*` (management)
 * and `/v1/*` (OpenAI-compatible inference).
 */
const GUIDE_CATEGORIES = [
  {
    id: "overview",
    name: "llmlb-api-overview",
    description:
      "Overview of llmlb API categories, auth methods, and base URL configuration",
  },
  {
    id: "openai-compatible",
    name: "llmlb-openai-api",
    description:
      "OpenAI-compatible endpoints: /v1/chat/completions, /v1/completions, /v1/embeddings, /v1/models",
  },
  {
    id: "endpoint-management",
    name: "llmlb-endpoint-api",
    description: "Endpoint management endpoints: /api/endpoints",
  },
  {
    id: "model-management",
    name: "llmlb-model-api",
    description: "Model management endpoints: /api/models/*",
  },
  {
    id: "dashboard",
    name: "llmlb-dashboard-api",
    description: "Dashboard and monitoring endpoints: /api/dashboard/*",
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
    case "endpoint-management":
      return getEndpointManagementGuide(routerUrl);
    case "model-management":
      return getModelManagementGuide(routerUrl);
    case "dashboard":
      return getDashboardGuide(routerUrl);
    default:
      return null;
  }
}

function getOverviewGuide(routerUrl: string): string {
  return `# llmlb API Overview

## Base URL

\`\`\`
${routerUrl}
\`\`\`

## API Categories

| Category | Base Path | Notes |
|----------|-----------|-------|
| OpenAI-Compatible | /v1/* | Inference APIs. Requires an API key with \`api\` scope. |
| Management | /api/* | Endpoint/model/dashboard/admin APIs. Prefer an API key with \`admin\` scope. |
| Dashboard UI | /dashboard | Browser UI. Uses HttpOnly cookies after login. |

## Authentication

### API Key Authentication (recommended for programmatic access)

**Header**: \`X-API-Key: sk_xxx\` (or \`Authorization: Bearer sk_xxx\`)

Scopes (examples):
- \`api\`: /v1/* inference endpoints
- \`admin\`: /api/* management endpoints

This MCP server can auto-inject:
- \`LLMLB_API_KEY\` for /v1/*
- \`LLMLB_ADMIN_API_KEY\` for /api/* (preferred)

### Dashboard Session (browser UI)

The dashboard uses **HttpOnly cookies** for JWT sessions. This MCP server does not manage browser cookies.
Use scoped API keys for automation.`;
}

function getOpenAiGuide(routerUrl: string): string {
  return `# OpenAI-Compatible API (/v1/*)

## Chat Completions

**Endpoint**: POST ${routerUrl}/v1/chat/completions

\`\`\`bash
curl -X POST ${routerUrl}/v1/chat/completions \\
  -H "Content-Type: application/json" \\
  -H "X-API-Key: YOUR_API_KEY" \\
  -d '{
    "model": "llama3.2:3b",
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "Hello!"}
    ],
    "stream": false
  }'
\`\`\`

## Cloud Routing (model prefix)

\`\`\`bash
curl -X POST ${routerUrl}/v1/chat/completions \\
  -H "Content-Type: application/json" \\
  -H "X-API-Key: YOUR_API_KEY" \\
  -d '{"model": "openai:gpt-4o", "messages": [{"role":"user","content":"Hello"}]}'
\`\`\`

Supported prefixes:
- \`openai:\`
- \`google:\`
- \`anthropic:\`

## List Models

**Endpoint**: GET ${routerUrl}/v1/models

\`\`\`bash
curl ${routerUrl}/v1/models \\
  -H "X-API-Key: YOUR_API_KEY"
\`\`\`

## Embeddings

**Endpoint**: POST ${routerUrl}/v1/embeddings

\`\`\`bash
curl -X POST ${routerUrl}/v1/embeddings \\
  -H "Content-Type: application/json" \\
  -H "X-API-Key: YOUR_API_KEY" \\
  -d '{"model": "nomic-embed-text-v1.5", "input": "Hello world"}'
\`\`\`
`;
}

function getEndpointManagementGuide(routerUrl: string): string {
  return `# Endpoint Management API (/api/endpoints)

## List Endpoints

**Endpoint**: GET ${routerUrl}/api/endpoints

\`\`\`bash
curl ${routerUrl}/api/endpoints \\
  -H "X-API-Key: ADMIN_API_KEY"
\`\`\`

## Create Endpoint

**Endpoint**: POST ${routerUrl}/api/endpoints

\`\`\`bash
curl -X POST ${routerUrl}/api/endpoints \\
  -H "Content-Type: application/json" \\
  -H "X-API-Key: ADMIN_API_KEY" \\
  -d '{
    "name": "xllm-local",
    "base_url": "http://127.0.0.1:8080",
    "api_key": null
  }'
\`\`\`

Notes:
- \`endpoint_type\` can be provided to override auto-detection (xllm/ollama/vllm/openai_compatible).
- If omitted, llmlb will auto-detect the endpoint type (when reachable).`;
}

function getModelManagementGuide(routerUrl: string): string {
  return `# Model Management API (/api/models/*)

## List Models (management view)

**Endpoint**: GET ${routerUrl}/api/models/hub

\`\`\`bash
curl ${routerUrl}/api/models/hub \\
  -H "X-API-Key: ADMIN_API_KEY"
\`\`\`

## Register Model (admin only)

**Endpoint**: POST ${routerUrl}/api/models/register

\`\`\`bash
curl -X POST ${routerUrl}/api/models/register \\
  -H "Content-Type: application/json" \\
  -H "X-API-Key: ADMIN_API_KEY" \\
  -d '{
    "repo": "TheBloke/Llama-2-7B-GGUF",
    "filename": "llama-2-7b.Q4_K_M.gguf"
  }'
\`\`\`

## Delete Model (admin only)

**Endpoint**: DELETE ${routerUrl}/api/models/:model_name

\`\`\`bash
curl -X DELETE ${routerUrl}/api/models/gpt-oss-20b \\
  -H "X-API-Key: ADMIN_API_KEY"
\`\`\`

Note:
- llmlb does not push binaries to runtimes. Runtimes fetch manifests and artifacts as needed.`;
}

function getDashboardGuide(routerUrl: string): string {
  return `# Dashboard API (/api/dashboard/*)

## Overview

**Endpoint**: GET ${routerUrl}/api/dashboard/overview

\`\`\`bash
curl ${routerUrl}/api/dashboard/overview \\
  -H "X-API-Key: ADMIN_API_KEY"
\`\`\`

## Stats

**Endpoint**: GET ${routerUrl}/api/dashboard/stats

\`\`\`bash
curl ${routerUrl}/api/dashboard/stats \\
  -H "X-API-Key: ADMIN_API_KEY"
\`\`\`

## Request/Response History (API)

**Endpoint**: GET ${routerUrl}/api/dashboard/request-responses

\`\`\`bash
curl ${routerUrl}/api/dashboard/request-responses \\
  -H "X-API-Key: ADMIN_API_KEY"
\`\`\`

## Router Logs

**Endpoint**: GET ${routerUrl}/api/dashboard/logs/lb

\`\`\`bash
curl ${routerUrl}/api/dashboard/logs/lb \\
  -H "X-API-Key: ADMIN_API_KEY"
\`\`\`
`;
}
