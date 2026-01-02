import { z } from "zod";

const ConfigSchema = z.object({
  routerUrl: z.string().url().default("http://localhost:51280"),
  apiKey: z.string().optional(),
  jwtToken: z.string().optional(),
  openapiPath: z.string().optional(),
  defaultTimeout: z.number().min(1).max(300).default(30),
});

export type ServerConfig = z.infer<typeof ConfigSchema>;

export function loadConfig(): ServerConfig {
  const raw = {
    routerUrl: process.env.LLM_ROUTER_URL || "http://localhost:8080",
    apiKey: process.env.LLM_ROUTER_API_KEY,
    jwtToken: process.env.LLM_ROUTER_JWT_TOKEN,
    openapiPath: process.env.LLM_ROUTER_OPENAPI_PATH,
    defaultTimeout: process.env.LLM_ROUTER_TIMEOUT
      ? parseInt(process.env.LLM_ROUTER_TIMEOUT, 10)
      : 30,
  };

  return ConfigSchema.parse(raw);
}
