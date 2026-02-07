import { describe, expect, it } from "vitest";
import type { ServerConfig } from "../src/config.js";
import { injectAuthHeadersForUrl } from "../src/tools/execute-curl.js";

function config(overrides: Partial<ServerConfig> = {}): ServerConfig {
  return {
    routerUrl: "http://localhost:32768",
    apiKey: undefined,
    adminApiKey: undefined,
    jwtToken: undefined,
    openapiPath: undefined,
    defaultTimeout: 30,
    ...overrides,
  };
}

describe("injectAuthHeadersForUrl", () => {
  it("injects api key for /v1/* inference endpoints", () => {
    const c = config({ apiKey: "sk_api" });
    const cmd = "curl http://localhost:32768/v1/models";
    const out = injectAuthHeadersForUrl(cmd, "http://localhost:32768/v1/models", c);
    expect(out).toBe('curl -H "X-API-Key: sk_api" http://localhost:32768/v1/models');
  });

  it("falls back to admin api key for /v1/* when api key is missing", () => {
    const c = config({ adminApiKey: "sk_admin" });
    const cmd = "curl http://localhost:32768/v1/models";
    const out = injectAuthHeadersForUrl(cmd, "http://localhost:32768/v1/models", c);
    expect(out).toBe('curl -H "X-API-Key: sk_admin" http://localhost:32768/v1/models');
  });

  it("injects admin api key for /api/* management endpoints", () => {
    const c = config({ adminApiKey: "sk_admin" });
    const cmd = "curl http://localhost:32768/api/dashboard/overview";
    const out = injectAuthHeadersForUrl(
      cmd,
      "http://localhost:32768/api/dashboard/overview",
      c
    );
    expect(out).toBe(
      'curl -H "X-API-Key: sk_admin" http://localhost:32768/api/dashboard/overview'
    );
  });

  it("falls back to jwt token for /api/* when admin api key is missing", () => {
    const c = config({ jwtToken: "jwt_legacy" });
    const cmd = "curl http://localhost:32768/api/dashboard/overview";
    const out = injectAuthHeadersForUrl(
      cmd,
      "http://localhost:32768/api/dashboard/overview",
      c
    );
    expect(out).toBe(
      'curl -H "Authorization: Bearer jwt_legacy" http://localhost:32768/api/dashboard/overview'
    );
  });

  it("prefers jwt token for /api/auth/* endpoints", () => {
    const c = config({ adminApiKey: "sk_admin", jwtToken: "jwt_legacy" });
    const cmd = "curl http://localhost:32768/api/auth/me";
    const out = injectAuthHeadersForUrl(cmd, "http://localhost:32768/api/auth/me", c);
    expect(out).toBe(
      'curl -H "Authorization: Bearer jwt_legacy" http://localhost:32768/api/auth/me'
    );
  });

  it("does not inject if the command already contains auth headers", () => {
    const c = config({ apiKey: "sk_api" });
    const cmd =
      'curl -H "X-API-Key: already" http://localhost:32768/v1/models';
    const out = injectAuthHeadersForUrl(cmd, "http://localhost:32768/v1/models", c);
    expect(out).toBe(cmd);
  });

  it("does not inject for non-api pages", () => {
    const c = config({ apiKey: "sk_api", adminApiKey: "sk_admin" });
    const cmd = "curl http://localhost:32768/dashboard";
    const out = injectAuthHeadersForUrl(cmd, "http://localhost:32768/dashboard", c);
    expect(out).toBe(cmd);
  });
});

