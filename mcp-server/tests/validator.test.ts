import { describe, expect, it } from "vitest";
import { HostValidator } from "../src/security/validator.js";

describe("HostValidator", () => {
  const validator = new HostValidator("http://localhost:8080");

  describe("validate", () => {
    it("should accept configured router URL", () => {
      const result = validator.validate("http://localhost:8080/v1/models");
      expect(result.valid).toBe(true);
    });

    it("should accept localhost variants", () => {
      expect(validator.validate("http://localhost/api").valid).toBe(true);
      expect(validator.validate("http://127.0.0.1:8080/api").valid).toBe(true);
      expect(validator.validate("http://127.0.0.1/api").valid).toBe(true);
    });

    it("should accept https protocol", () => {
      const result = validator.validate("https://localhost:8080/api");
      expect(result.valid).toBe(true);
    });

    it("should reject external hosts", () => {
      const result = validator.validate("http://example.com/api");
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("Host not allowed");
    });

    it("should reject file protocol", () => {
      const result = validator.validate("file:///etc/passwd");
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("Invalid protocol");
    });

    it("should reject ftp protocol", () => {
      const result = validator.validate("ftp://localhost/file");
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("Invalid protocol");
    });

    it("should reject invalid URLs", () => {
      const result = validator.validate("not-a-url");
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("Invalid URL");
    });

    it("should reject external hosts even with same port", () => {
      const result = validator.validate("http://malicious.com:8080/api");
      expect(result.valid).toBe(false);
      expect(result.reason).toContain("Host not allowed");
    });
  });

  describe("with custom router URL", () => {
    const customValidator = new HostValidator("https://api.example.com:9000");

    it("should accept configured custom host", () => {
      const result = customValidator.validate(
        "https://api.example.com:9000/v1/models"
      );
      expect(result.valid).toBe(true);
    });

    it("should still accept localhost", () => {
      const result = customValidator.validate("http://localhost/api");
      expect(result.valid).toBe(true);
    });

    it("should reject other external hosts", () => {
      const result = customValidator.validate("https://other.example.com/api");
      expect(result.valid).toBe(false);
    });
  });
});
