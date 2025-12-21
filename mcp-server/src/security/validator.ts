/**
 * Host validation for curl commands.
 * Only allows requests to the configured router URL and localhost variants.
 * For security, external hosts require exact port match, while localhost allows any port.
 */

const LOCALHOST_HOSTNAMES = ["localhost", "127.0.0.1", "::1"];

export class HostValidator {
  private allowedHostsWithPort: Set<string>;
  private allowedHostnames: Set<string>;

  constructor(routerUrl: string) {
    const url = new URL(routerUrl);
    // Store host with port for external hosts (strict match)
    this.allowedHostsWithPort = new Set([url.host]);
    // Store hostnames for localhost (port-agnostic)
    this.allowedHostnames = new Set(LOCALHOST_HOSTNAMES);
  }

  validate(targetUrl: string): { valid: boolean; reason?: string } {
    try {
      const url = new URL(targetUrl);

      // Protocol check
      if (!["http:", "https:"].includes(url.protocol)) {
        return {
          valid: false,
          reason: `Invalid protocol: ${url.protocol}. Only http/https allowed.`,
        };
      }

      // Localhost check (any port allowed)
      if (LOCALHOST_HOSTNAMES.includes(url.hostname)) {
        return { valid: true };
      }

      // External host check (exact host:port match required)
      if (!this.allowedHostsWithPort.has(url.host)) {
        return {
          valid: false,
          reason: `Host not allowed: ${url.host}. Allowed: ${[...this.allowedHostsWithPort].join(", ")}, ${LOCALHOST_HOSTNAMES.join(", ")}`,
        };
      }

      return { valid: true };
    } catch {
      return { valid: false, reason: `Invalid URL: ${targetUrl}` };
    }
  }
}
