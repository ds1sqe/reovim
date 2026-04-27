/**
 * Test Server Harness
 *
 * Spawns a reovim server for integration tests.
 * Similar to Phase 15's Playwright fixtures but for Vitest.
 */

/// <reference types="node" />

import { spawn, type ChildProcess } from "node:child_process";
import { setTimeout as delay } from "node:timers/promises";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

// ESM equivalent of __dirname
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

/**
 * Find the reovim binary for testing.
 */
function findReovimBinary(): string {
  // Allow override via environment variable
  const envBinary = process.env["REOVIM_BINARY"];
  if (envBinary) {
    return envBinary;
  }

  // Find workspace root from test file location
  // Path: clients/web/tests/helpers/server-harness.ts
  const workspaceRoot = join(__dirname, "..", "..", "..", "..");
  const releasePath = join(workspaceRoot, "target", "release", "reovim");
  const debugPath = join(workspaceRoot, "target", "debug", "reovim");

  // Prefer debug build for local development
  const isCI = process.env["CI"] === "true";
  return isCI ? releasePath : debugPath;
}

/**
 * Extract port from server output.
 */
function extractPort(output: string): number | null {
  // Match patterns like "Listening on 127.0.0.1:12521" or "tcp://127.0.0.1:12522"
  const match = output.match(/(?:Listening on|tcp:\/\/).*?:(\d+)/);
  if (!match || !match[1]) return null;
  return parseInt(match[1], 10);
}

/**
 * Server harness for Vitest integration tests.
 *
 * Spawns a reovim server on an OS-assigned port and provides
 * connection information for test clients.
 */
export class WebTestServerHarness {
  private serverProcess: ChildProcess | null = null;
  private _port: number | null = null;
  private stderrOutput: string = "";

  /** Get the server port (throws if not started) */
  get port(): number {
    if (!this._port) {
      throw new Error("Server not started - call spawn() first");
    }
    return this._port;
  }

  /** Get the server address as host:port */
  get address(): string {
    return `127.0.0.1:${this.port}`;
  }

  /** Get the base URL for gRPC-Web connections */
  get baseUrl(): string {
    return `http://${this.address}`;
  }

  /**
   * Spawn a test server.
   *
   * @param timeoutMs - Maximum time to wait for server startup (default: 10000)
   */
  async spawn(timeoutMs: number = 10000): Promise<void> {
    const binary = findReovimBinary();

    // Start server with gRPC transport, port 0 for dynamic assignment
    // NOTE: Requires binary built with: cargo build -p reovim-app --features grpc
    this.serverProcess = spawn(binary, ["server", "--grpc", "0"], {
      stdio: ["ignore", "pipe", "pipe"],
      // Set cwd to workspace root for proper module loading
      cwd: join(__dirname, "..", "..", "..", ".."),
    });

    // Collect stderr output to find the port
    this.stderrOutput = "";

    this.serverProcess.stderr?.on("data", (data: Buffer) => {
      const text = data.toString();
      this.stderrOutput += text;

      // Try to extract port from output
      if (!this._port) {
        this._port = extractPort(this.stderrOutput);
      }
    });

    this.serverProcess.stdout?.on("data", (data: Buffer) => {
      // Log server output for debugging
      if (process.env["DEBUG"]) {
        process.stdout.write(`[reovim] ${data.toString()}`);
      }
    });

    // Wait for server to start (with timeout)
    const startTime = Date.now();

    while (!this._port && Date.now() - startTime < timeoutMs) {
      await delay(100);

      // Check if process died
      if (this.serverProcess.exitCode !== null) {
        throw new Error(
          `Server exited with code ${this.serverProcess.exitCode}:\n${this.stderrOutput}`,
        );
      }
    }

    if (!this._port) {
      this.stop();
      throw new Error(
        `Failed to start server within ${timeoutMs}ms:\n${this.stderrOutput}`,
      );
    }

    console.log(`[harness] Server started on port ${this._port}`);
  }

  /**
   * Stop the server.
   */
  stop(): void {
    if (!this.serverProcess) return;

    console.log("[harness] Stopping server...");
    this.serverProcess.kill("SIGTERM");

    // Force kill after a short delay if still running
    const proc = this.serverProcess;
    setTimeout(() => {
      if (proc.exitCode === null) {
        proc.kill("SIGKILL");
      }
    }, 500);

    this.serverProcess = null;
    this._port = null;
  }
}
