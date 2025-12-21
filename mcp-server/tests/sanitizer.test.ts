import { describe, expect, it } from "vitest";
import { CurlSanitizer } from "../src/security/sanitizer.js";

describe("CurlSanitizer", () => {
  const sanitizer = new CurlSanitizer();

  describe("sanitize", () => {
    it("should accept valid curl commands", () => {
      const result = sanitizer.sanitize("curl http://localhost:8080/v1/models");
      expect(result.valid).toBe(true);
    });

    it("should accept curl with headers", () => {
      const result = sanitizer.sanitize(
        'curl -X POST http://localhost:8080/v1/chat -H "Content-Type: application/json"'
      );
      expect(result.valid).toBe(true);
    });

    it("should accept curl with data", () => {
      const result = sanitizer.sanitize(
        'curl -X POST http://localhost:8080/api -d \'{"key":"value"}\''
      );
      expect(result.valid).toBe(true);
    });

    it("should reject non-curl commands", () => {
      const result = sanitizer.sanitize("wget http://localhost:8080");
      expect(result.valid).toBe(false);
      expect(result.reason).toContain('must start with "curl "');
    });

    it("should reject output file options", () => {
      const result = sanitizer.sanitize(
        "curl -o /tmp/file http://localhost:8080"
      );
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("Forbidden option");
    });

    it("should reject --output option", () => {
      const result = sanitizer.sanitize(
        "curl --output=/tmp/file http://localhost:8080"
      );
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("Forbidden option");
    });

    it("should reject config file options", () => {
      const result = sanitizer.sanitize("curl -K /etc/curlrc http://localhost");
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("Forbidden option");
    });

    it("should reject user credentials", () => {
      const result = sanitizer.sanitize(
        "curl -u admin:password http://localhost"
      );
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("Forbidden option");
    });

    it("should reject shell injection with semicolon", () => {
      const result = sanitizer.sanitize(
        "curl http://localhost; rm -rf /"
      );
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("shell injection");
    });

    it("should reject shell injection with pipe", () => {
      const result = sanitizer.sanitize(
        "curl http://localhost | bash"
      );
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("shell injection");
    });

    it("should reject command substitution", () => {
      const result = sanitizer.sanitize(
        "curl http://localhost/$(whoami)"
      );
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("shell injection");
    });

    it("should reject backtick command substitution", () => {
      const result = sanitizer.sanitize(
        "curl http://localhost/`whoami`"
      );
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("shell injection");
    });

    it("should reject short option with value concatenated (-o/path)", () => {
      const result = sanitizer.sanitize(
        "curl -o/tmp/file http://localhost:8080"
      );
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("Forbidden option");
    });

    it("should reject -O with URL concatenated", () => {
      const result = sanitizer.sanitize(
        "curl -Ohttp://localhost:8080/file"
      );
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("Forbidden option");
    });

    it("should reject -u with credentials concatenated", () => {
      const result = sanitizer.sanitize(
        "curl -uadmin:password http://localhost:8080"
      );
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("Forbidden option");
    });

    it("should reject -K with config path concatenated", () => {
      const result = sanitizer.sanitize(
        "curl -K/etc/curlrc http://localhost:8080"
      );
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("Forbidden option");
    });
  });

  describe("extractUrl", () => {
    it("should extract URL from simple command", () => {
      const url = sanitizer.extractUrl("curl http://localhost:8080/v1/models");
      expect(url).toBe("http://localhost:8080/v1/models");
    });

    it("should extract URL with options before", () => {
      const url = sanitizer.extractUrl(
        "curl -X POST http://localhost:8080/api"
      );
      expect(url).toBe("http://localhost:8080/api");
    });

    it("should extract URL with headers", () => {
      const url = sanitizer.extractUrl(
        'curl -H "Content-Type: application/json" https://example.com/api'
      );
      expect(url).toBe("https://example.com/api");
    });

    it("should return null for command without URL", () => {
      const url = sanitizer.extractUrl("curl --help");
      expect(url).toBeNull();
    });
  });
});
