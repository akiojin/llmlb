/**
 * Host validation for curl commands.
 * Only allows requests to the configured router URL and localhost variants.
 */
export class HostValidator {
  private allowedHosts: Set<string>;

  constructor(routerUrl: string) {
    const url = new URL(routerUrl);
    this.allowedHosts = new Set([
      url.host,
      url.hostname,
      "localhost",
      "127.0.0.1",
      "::1",
    ]);
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

      // Host check
      if (
        !this.allowedHosts.has(url.hostname) &&
        !this.allowedHosts.has(url.host)
      ) {
        return {
          valid: false,
          reason: `Host not allowed: ${url.host}. Allowed: ${[...this.allowedHosts].join(", ")}`,
        };
      }

      return { valid: true };
    } catch {
      return { valid: false, reason: `Invalid URL: ${targetUrl}` };
    }
  }
}
