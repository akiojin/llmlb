#!/usr/bin/env node
import { loadConfig } from "./config.js";
import { runServer } from "./server.js";

async function main(): Promise<void> {
  try {
    const config = loadConfig();
    await runServer(config);
  } catch (error) {
    console.error("Failed to start MCP server:", error);
    process.exit(1);
  }
}

main();
