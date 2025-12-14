import { spawn } from "node:child_process";
import { z } from "zod";
import type { ServerConfig } from "../config.js";
import { CurlSanitizer } from "../security/sanitizer.js";
import { HostValidator } from "../security/validator.js";

/**
 * Input schema for execute_curl tool
 */
export const ExecuteCurlInputSchema = z.object({
  command: z
    .string()
    .min(1)
    .describe(
      'curl command to execute (e.g., "curl http://localhost:8080/v1/models")'
    ),
  auto_auth: z
    .boolean()
    .default(true)
    .optional()
    .describe("Automatically inject authentication headers from environment"),
  timeout: z
    .number()
    .min(1)
    .max(300)
    .default(30)
    .optional()
    .describe("Request timeout in seconds"),
});

export type ExecuteCurlInput = z.infer<typeof ExecuteCurlInputSchema>;

export interface ExecuteCurlResult {
  success: boolean;
  status_code?: number;
  headers?: Record<string, string>;
  body?: unknown;
  error?: string;
  duration_ms: number;
  executed_command: string;
}

/**
 * Handler for execute_curl tool.
 * Executes curl commands against the LLM Router API with security constraints.
 */
export class ExecuteCurlHandler {
  readonly name = "execute_curl";
  readonly description = `Execute a curl command against the LLM Router API.

SECURITY: Only requests to the configured router host are allowed.
Authentication headers are automatically injected from environment variables.

Examples:
- List models: curl http://localhost:8080/v1/models
- Chat completion: curl -X POST http://localhost:8080/v1/chat/completions -H "Content-Type: application/json" -d '{"model":"...", "messages":[...]}'
- List nodes: curl http://localhost:8080/v0/nodes
- Dashboard stats: curl http://localhost:8080/v0/dashboard/stats

Refer to the 'llm-router-openapi' resource for full API documentation.`;

  readonly inputSchema = {
    type: "object" as const,
    properties: {
      command: {
        type: "string",
        description:
          'curl command to execute (e.g., "curl http://localhost:8080/v1/models")',
      },
      auto_auth: {
        type: "boolean",
        default: true,
        description:
          "Automatically inject authentication headers from environment",
      },
      timeout: {
        type: "number",
        default: 30,
        description: "Request timeout in seconds (1-300)",
      },
    },
    required: ["command"],
  };

  private config: ServerConfig;
  private sanitizer: CurlSanitizer;
  private validator: HostValidator;

  constructor(config: ServerConfig) {
    this.config = config;
    this.sanitizer = new CurlSanitizer();
    this.validator = new HostValidator(config.routerUrl);
  }

  /**
   * Validate input using Zod schema
   */
  validate(input: unknown): ExecuteCurlInput {
    return ExecuteCurlInputSchema.parse(input);
  }

  /**
   * Execute the curl command
   */
  async execute(params: ExecuteCurlInput): Promise<ExecuteCurlResult> {
    const startTime = Date.now();
    const timeout = params.timeout ?? this.config.defaultTimeout;
    const autoAuth = params.auto_auth ?? true;

    // 1. Sanitize command
    const sanitizeResult = this.sanitizer.sanitize(params.command);
    if (!sanitizeResult.valid) {
      return {
        success: false,
        error: `Security violation: ${sanitizeResult.reason}`,
        duration_ms: Date.now() - startTime,
        executed_command: this.maskSensitive(params.command),
      };
    }

    // 2. Extract and validate URL
    const url = this.sanitizer.extractUrl(params.command);
    if (!url) {
      return {
        success: false,
        error: "Could not extract URL from curl command",
        duration_ms: Date.now() - startTime,
        executed_command: this.maskSensitive(params.command),
      };
    }

    const hostResult = this.validator.validate(url);
    if (!hostResult.valid) {
      return {
        success: false,
        error: hostResult.reason,
        duration_ms: Date.now() - startTime,
        executed_command: this.maskSensitive(params.command),
      };
    }

    // 3. Inject auth headers if needed
    let command = params.command;
    if (autoAuth) {
      command = this.injectAuthHeaders(command, url);
    }

    // 4. Execute curl
    try {
      const result = await this.executeCommand(command, timeout);
      return {
        ...result,
        duration_ms: Date.now() - startTime,
        executed_command: this.maskSensitive(command),
      };
    } catch (error) {
      return {
        success: false,
        error: error instanceof Error ? error.message : String(error),
        duration_ms: Date.now() - startTime,
        executed_command: this.maskSensitive(command),
      };
    }
  }

  /**
   * Inject authentication headers based on endpoint type
   */
  private injectAuthHeaders(command: string, url: string): string {
    // Skip if already has auth headers
    if (
      command.includes("Authorization:") ||
      command.includes("X-API-Key:") ||
      command.includes("X-Node-Token:")
    ) {
      return command;
    }

    const isManagementEndpoint =
      url.includes("/v0/users") ||
      url.includes("/v0/api-keys") ||
      url.includes("/v0/auth/me");

    let authHeader = "";

    if (isManagementEndpoint && this.config.jwtToken) {
      // Management API uses JWT
      authHeader = `-H "Authorization: Bearer ${this.config.jwtToken}"`;
    } else if (this.config.apiKey) {
      // Inference API uses API key
      authHeader = `-H "X-API-Key: ${this.config.apiKey}"`;
    }

    if (authHeader) {
      // Insert after "curl "
      return command.replace(/^curl\s+/, `curl ${authHeader} `);
    }

    return command;
  }

  /**
   * Execute curl command using spawn (shell: false for security)
   */
  private executeCommand(
    command: string,
    timeout: number
  ): Promise<Omit<ExecuteCurlResult, "duration_ms" | "executed_command">> {
    return new Promise((resolve) => {
      // Parse command into args
      const args = this.parseCommandArgs(command);

      // Add curl options for better output
      // -s: silent, -S: show errors, -w: write status code
      args.push("-s", "-S", "-w", "\n__STATUS_CODE__:%{http_code}");

      const proc = spawn("curl", args, {
        timeout: timeout * 1000,
        shell: false, // Security: no shell
      });

      let stdout = "";
      let stderr = "";

      proc.stdout.on("data", (data) => {
        stdout += data.toString();
      });

      proc.stderr.on("data", (data) => {
        stderr += data.toString();
      });

      proc.on("error", (error) => {
        resolve({
          success: false,
          error: `Failed to execute curl: ${error.message}`,
        });
      });

      proc.on("close", (code) => {
        if (code !== 0 && !stdout) {
          resolve({
            success: false,
            error: stderr || `curl exited with code ${code}`,
          });
          return;
        }

        // Parse status code from output
        const statusMatch = stdout.match(/__STATUS_CODE__:(\d+)$/);
        const statusCode = statusMatch ? parseInt(statusMatch[1], 10) : 0;
        const body = stdout.replace(/__STATUS_CODE__:\d+$/, "").trim();

        // Try to parse as JSON
        let parsedBody: unknown;
        try {
          parsedBody = JSON.parse(body);
        } catch {
          parsedBody = body;
        }

        resolve({
          success: statusCode >= 200 && statusCode < 300,
          status_code: statusCode,
          body: parsedBody,
        });
      });
    });
  }

  /**
   * Parse curl command string into args array
   */
  private parseCommandArgs(command: string): string[] {
    const args: string[] = [];
    let current = "";
    let inQuote = false;
    let quoteChar = "";

    // Remove "curl " prefix
    const cmdPart = command.replace(/^curl\s+/, "");

    for (let i = 0; i < cmdPart.length; i++) {
      const char = cmdPart[i];

      if ((char === '"' || char === "'") && !inQuote) {
        inQuote = true;
        quoteChar = char;
      } else if (char === quoteChar && inQuote) {
        inQuote = false;
        quoteChar = "";
      } else if (char === " " && !inQuote) {
        if (current) {
          args.push(current);
          current = "";
        }
      } else {
        current += char;
      }
    }

    if (current) {
      args.push(current);
    }

    return args;
  }

  /**
   * Mask sensitive information in command for logging
   */
  private maskSensitive(command: string): string {
    return command
      .replace(/Bearer\s+[^\s"']+/gi, "Bearer ***")
      .replace(/X-API-Key:\s*[^\s"']+/gi, "X-API-Key: ***")
      .replace(/X-Node-Token:\s*[^\s"']+/gi, "X-Node-Token: ***")
      .replace(/sk_[a-zA-Z0-9]+/g, "sk_***")
      .replace(/nt_[a-zA-Z0-9-]+/g, "nt_***");
  }
}
