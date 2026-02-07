import { readFileSync, existsSync } from "node:fs";
import { parse as parseYaml } from "yaml";
import type { Resource } from "@modelcontextprotocol/sdk/types.js";

const OPENAPI_URI = "llmlb://api/openapi";

/**
 * Get OpenAPI resource definition
 */
export function getOpenApiResource(): Resource {
  return {
    uri: OPENAPI_URI,
    name: "llmlb-openapi",
    description:
      "OpenAPI specification for LLM Router API. Contains all endpoint definitions, request/response schemas, and authentication requirements.",
    mimeType: "application/json",
  };
}

/**
 * Get OpenAPI content from file or return default spec
 */
export async function getOpenApiContent(
  openapiPath?: string
): Promise<string> {
  // Try to load from file
  if (openapiPath && existsSync(openapiPath)) {
    try {
      const content = readFileSync(openapiPath, "utf-8");
      // Parse YAML and convert to JSON
      const spec = parseYaml(content);
      return JSON.stringify(spec, null, 2);
    } catch {
      // Fall through to default
    }
  }

  // Return embedded default spec
  return JSON.stringify(DEFAULT_OPENAPI_SPEC, null, 2);
}

/**
 * Default OpenAPI spec when file is not available
 */
const DEFAULT_OPENAPI_SPEC = {
  openapi: "3.1.0",
  info: {
    title: "llmlb API",
    version: "0.1.0",
    description:
      "OpenAI-compatible endpoints with optional cloud routing by model prefix (openai:/google:/anthropic:).",
  },
  servers: [{ url: "http://localhost:32768" }],
  paths: {
    "/v1/chat/completions": {
      post: {
        summary: "Chat completion (local or cloud depending on model prefix)",
        requestBody: {
          required: true,
          content: {
            "application/json": {
              schema: { $ref: "#/components/schemas/ChatRequest" },
            },
          },
        },
        responses: {
          "200": {
            description: "Chat completion response",
            content: {
              "application/json": {
                schema: { $ref: "#/components/schemas/ChatResponse" },
              },
            },
          },
        },
      },
    },
    "/v1/models": {
      get: {
        summary: "List available models",
        responses: {
          "200": { description: "List of models" },
        },
      },
    },
    "/v1/embeddings": {
      post: {
        summary: "Generate embeddings",
        requestBody: {
          required: true,
          content: {
            "application/json": {
              schema: { $ref: "#/components/schemas/EmbeddingRequest" },
            },
          },
        },
      },
    },
    "/api/auth/login": {
      post: { summary: "Login (sets HttpOnly cookie for the dashboard)" },
    },
    "/api/auth/me": {
      get: { summary: "Get current user session" },
    },
    "/api/endpoints": {
      get: { summary: "List endpoints" },
      post: { summary: "Create endpoint" },
    },
    "/api/endpoints/{id}": {
      get: { summary: "Get endpoint detail" },
      put: { summary: "Update endpoint" },
      delete: { summary: "Delete endpoint" },
    },
    "/api/dashboard/overview": {
      get: { summary: "Get dashboard overview" },
    },
    "/api/dashboard/stats": {
      get: { summary: "Get dashboard statistics" },
    },
    "/api/models/register": {
      post: { summary: "Register a model (admin only)" },
    },
  },
  components: {
    schemas: {
      ChatRequest: {
        type: "object",
        properties: {
          model: { type: "string", example: "openai:gpt-4o" },
          messages: {
            type: "array",
            items: { $ref: "#/components/schemas/ChatMessage" },
          },
          stream: { type: "boolean" },
        },
        required: ["model", "messages"],
      },
      ChatMessage: {
        type: "object",
        properties: {
          role: { type: "string", enum: ["system", "user", "assistant"] },
          content: { type: "string" },
        },
        required: ["role", "content"],
      },
      ChatResponse: {
        type: "object",
        properties: {
          id: { type: "string" },
          model: { type: "string" },
          choices: {
            type: "array",
            items: { $ref: "#/components/schemas/ChatChoice" },
          },
        },
      },
      ChatChoice: {
        type: "object",
        properties: {
          index: { type: "integer" },
          message: { $ref: "#/components/schemas/ChatMessage" },
          finish_reason: { type: "string" },
        },
      },
      EmbeddingRequest: {
        type: "object",
        properties: {
          model: { type: "string" },
          input: { oneOf: [{ type: "string" }, { type: "array" }] },
        },
        required: ["model", "input"],
      },
    },
  },
};
