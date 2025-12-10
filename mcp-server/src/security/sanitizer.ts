/**
 * Curl command sanitizer to prevent command injection and dangerous operations.
 */

// Options that could write to filesystem or expose sensitive data
const FORBIDDEN_OPTIONS = [
  // File output
  "-o",
  "--output",
  "-O",
  "--remote-name",
  // Config files
  "-K",
  "--config",
  "-q",
  "--disable",
  // Credential exposure
  "-u",
  "--user",
  "--netrc",
  "--netrc-file",
  "--netrc-optional",
  "--delegation",
  // External commands
  "--libcurl",
  // Trace/debug to files
  "--trace",
  "--trace-ascii",
  "--trace-time",
  // Protocol control (could bypass restrictions)
  "--proto",
  "--proto-default",
  "--proto-redir",
];

// Patterns that indicate shell injection attempts
const FORBIDDEN_PATTERNS = [
  /[;&|`]/,        // Shell command separators
  /\$\(/,          // Command substitution
  /\$\{/,          // Variable expansion
  />\s*[/~]/,      // Redirect to absolute path
  />>/,            // Append redirect
  /\|\s*\w+/,      // Pipe to command
  /<\s*[/~]/,      // Input redirect from absolute path
  /\\n/,           // Newline injection
];

export interface SanitizeResult {
  valid: boolean;
  reason?: string;
}

export class CurlSanitizer {
  sanitize(command: string): SanitizeResult {
    const trimmed = command.trim();

    // Must start with curl
    if (!trimmed.startsWith("curl ") && trimmed !== "curl") {
      return { valid: false, reason: 'Command must start with "curl "' };
    }

    // Check for forbidden patterns (shell injection)
    for (const pattern of FORBIDDEN_PATTERNS) {
      if (pattern.test(trimmed)) {
        return {
          valid: false,
          reason: `Forbidden pattern detected: potential shell injection`,
        };
      }
    }

    // Tokenize and check for forbidden options
    const tokens = this.tokenize(trimmed);
    for (const token of tokens) {
      // Check exact match for short options
      if (FORBIDDEN_OPTIONS.includes(token)) {
        return { valid: false, reason: `Forbidden option: ${token}` };
      }
      // Check for long options with = (e.g., --output=file)
      for (const opt of FORBIDDEN_OPTIONS) {
        if (opt.startsWith("--") && token.startsWith(`${opt}=`)) {
          return { valid: false, reason: `Forbidden option: ${opt}` };
        }
      }
    }

    return { valid: true };
  }

  /**
   * Extract URL from curl command.
   */
  extractUrl(command: string): string | null {
    const tokens = this.tokenize(command);

    // Skip options and find URL
    let skipNext = false;
    for (const token of tokens) {
      if (skipNext) {
        skipNext = false;
        continue;
      }

      // Options that take a value
      if (
        token.startsWith("-") &&
        !token.startsWith("--") &&
        token.length === 2
      ) {
        skipNext = true;
        continue;
      }

      // Long options with value
      if (
        token.startsWith("--") &&
        !token.includes("=") &&
        ![
          "--compressed",
          "--silent",
          "-s",
          "-S",
          "-i",
          "--include",
          "-v",
          "--verbose",
          "-L",
          "--location",
        ].includes(token)
      ) {
        skipNext = true;
        continue;
      }

      // Found a non-option token that looks like URL
      if (!token.startsWith("-") && token !== "curl") {
        if (token.startsWith("http://") || token.startsWith("https://")) {
          return token;
        }
      }
    }

    return null;
  }

  /**
   * Tokenize command respecting quotes.
   */
  private tokenize(command: string): string[] {
    const tokens: string[] = [];
    let current = "";
    let inQuote = false;
    let quoteChar = "";

    for (let i = 0; i < command.length; i++) {
      const char = command[i];

      if ((char === '"' || char === "'") && !inQuote) {
        inQuote = true;
        quoteChar = char;
      } else if (char === quoteChar && inQuote) {
        inQuote = false;
        quoteChar = "";
      } else if (char === " " && !inQuote) {
        if (current) {
          tokens.push(current);
          current = "";
        }
      } else {
        current += char;
      }
    }

    if (current) {
      tokens.push(current);
    }

    return tokens;
  }
}
