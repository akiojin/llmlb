import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListResourcesRequestSchema,
  ListToolsRequestSchema,
  ReadResourceRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import type { ServerConfig } from "./config.js";
import { getApiGuideContent, getApiGuideResources } from "./resources/api-guide.js";
import { getOpenApiContent, getOpenApiResource } from "./resources/openapi.js";
import { ExecuteCurlHandler } from "./tools/execute-curl.js";

export function createServer(config: ServerConfig): Server {
  const server = new Server(
    {
      name: "llm-router-mcp",
      version: "1.0.0",
    },
    {
      capabilities: {
        tools: {},
        resources: {},
      },
    }
  );

  // Initialize handlers
  const executeCurlHandler = new ExecuteCurlHandler(config);

  // Register tools/list handler
  server.setRequestHandler(ListToolsRequestSchema, async () => {
    return {
      tools: [
        {
          name: executeCurlHandler.name,
          description: executeCurlHandler.description,
          inputSchema: executeCurlHandler.inputSchema,
        },
      ],
    };
  });

  // Register tools/call handler
  server.setRequestHandler(CallToolRequestSchema, async (request) => {
    const { name, arguments: args } = request.params;

    if (name === executeCurlHandler.name) {
      try {
        const validated = executeCurlHandler.validate(args);
        const result = await executeCurlHandler.execute(validated);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify(result, null, 2),
            },
          ],
        };
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        return {
          content: [
            {
              type: "text",
              text: JSON.stringify({ error: message }, null, 2),
            },
          ],
          isError: true,
        };
      }
    }

    return {
      content: [
        {
          type: "text",
          text: `Unknown tool: ${name}`,
        },
      ],
      isError: true,
    };
  });

  // Register resources/list handler
  server.setRequestHandler(ListResourcesRequestSchema, async () => {
    const resources = [
      getOpenApiResource(),
      ...getApiGuideResources(),
    ];

    return { resources };
  });

  // Register resources/read handler
  server.setRequestHandler(ReadResourceRequestSchema, async (request) => {
    const { uri } = request.params;

    // OpenAPI resource
    if (uri === "llm-router://api/openapi") {
      const content = await getOpenApiContent(config.openapiPath);
      return {
        contents: [
          {
            uri,
            mimeType: "application/json",
            text: content,
          },
        ],
      };
    }

    // API Guide resources
    if (uri.startsWith("llm-router://guide/")) {
      const category = uri.replace("llm-router://guide/", "");
      const content = getApiGuideContent(category, config.routerUrl);
      if (content) {
        return {
          contents: [
            {
              uri,
              mimeType: "text/markdown",
              text: content,
            },
          ],
        };
      }
    }

    throw new Error(`Resource not found: ${uri}`);
  });

  return server;
}

export async function runServer(config: ServerConfig): Promise<void> {
  const server = createServer(config);
  const transport = new StdioServerTransport();
  await server.connect(transport);
}
