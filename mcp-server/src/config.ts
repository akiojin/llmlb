import { z } from "zod";

const ConfigSchema = z.object({
  routerUrl: z.string().url().default("http://localhost:32768"),
  apiKey: z.string().optional(),
  adminApiKey: z.string().optional(),
  // NOTE: Deprecated. llmlb dashboard now uses HttpOnly cookies for JWT sessions.
  // Keep this for backward compatibility with older llmlb versions and custom deployments.
  jwtToken: z.string().optional(),
  openapiPath: z.string().optional(),
  defaultTimeout: z.number().min(1).max(300).default(30),
});

export type ServerConfig = z.infer<typeof ConfigSchema>;

export function loadConfig(): ServerConfig {
  const raw = {
    routerUrl: process.env.LLMLB_URL || "http://localhost:32768",
    apiKey: process.env.LLMLB_API_KEY,
    adminApiKey: process.env.LLMLB_ADMIN_API_KEY,
    jwtToken: process.env.LLMLB_JWT_TOKEN,
    openapiPath: process.env.LLMLB_OPENAPI_PATH,
    defaultTimeout: process.env.LLMLB_TIMEOUT
      ? parseInt(process.env.LLMLB_TIMEOUT, 10)
      : 30,
  };

  return ConfigSchema.parse(raw);
}
