/**
 * Integration Test Utilities
 *
 * Provides fluent builders for headless web client integration tests.
 */

import { HeadlessWebClient } from "../../src/headless/index.js";
import { WebTestServerHarness } from "./server-harness.js";

/**
 * Integration test builder.
 *
 * Manages server lifecycle and client connections for integration tests.
 *
 * @example
 * ```typescript
 * const test = await WebIntegrationTest.create();
 * const client = await test.connect();
 *
 * await client.sendKeys("ihello<Esc>");
 * expect(client.getBuffer()).toContain("hello");
 *
 * await test.cleanup();
 * ```
 */
export class WebIntegrationTest {
  private harness: WebTestServerHarness;
  private clients: HeadlessWebClient[] = [];

  private constructor(harness: WebTestServerHarness) {
    this.harness = harness;
  }

  /**
   * Create a new integration test with an auto-spawned server.
   */
  static async create(): Promise<WebIntegrationTest> {
    const harness = new WebTestServerHarness();
    await harness.spawn();
    return new WebIntegrationTest(harness);
  }

  /**
   * Get the server port.
   */
  get port(): number {
    return this.harness.port;
  }

  /**
   * Get the server address.
   */
  get address(): string {
    return this.harness.address;
  }

  /**
   * Connect a headless client to the test server.
   *
   * @param width - Viewport width (default: 80)
   * @param height - Viewport height (default: 24)
   */
  async connect(width: number = 80, height: number = 24): Promise<HeadlessWebClient> {
    const client = await HeadlessWebClient.connect({
      address: this.harness.address,
      width,
      height,
    });
    this.clients.push(client);
    return client;
  }

  /**
   * Connect multiple headless clients to the test server.
   *
   * @param count - Number of clients to connect
   * @param width - Viewport width (default: 80)
   * @param height - Viewport height (default: 24)
   */
  async connectMultiple(
    count: number,
    width: number = 80,
    height: number = 24,
  ): Promise<HeadlessWebClient[]> {
    const clients: HeadlessWebClient[] = [];
    for (let i = 0; i < count; i++) {
      const client = await this.connect(width, height);
      clients.push(client);
    }
    return clients;
  }

  /**
   * Cleanup all clients and stop the server.
   */
  async cleanup(): Promise<void> {
    // Close all clients
    for (const client of this.clients) {
      client.close();
    }
    this.clients = [];

    // Stop server
    this.harness.stop();
  }
}

/**
 * Frame assertion helpers for integration tests.
 */
export const frameAssertions = {
  /**
   * Check if frame contains expected text.
   */
  contains(frame: string, expected: string): boolean {
    return frame.includes(expected);
  },

  /**
   * Check if frame does not contain unexpected text.
   */
  notContains(frame: string, unexpected: string): boolean {
    return !frame.includes(unexpected);
  },

  /**
   * Check if a specific line contains expected text.
   */
  lineContains(frame: string, lineNum: number, expected: string): boolean {
    const pattern = new RegExp(`\\[${lineNum}\\]\\s*(.*)$`, "m");
    const match = frame.match(pattern);
    if (!match || !match[1]) return false;
    return match[1].includes(expected);
  },

  /**
   * Get the mode from a frame capture.
   */
  getMode(frame: string): string | null {
    const match = frame.match(/mode:\s*(\S+)/);
    if (!match || !match[1]) return null;
    return match[1];
  },

  /**
   * Get cursor position from a frame capture.
   */
  getCursor(frame: string): { line: number; col: number } | null {
    const match = frame.match(/cursor:\s*line=(\d+),\s*col=(\d+)/);
    if (!match || !match[1] || !match[2]) return null;
    return {
      line: parseInt(match[1], 10),
      col: parseInt(match[2], 10),
    };
  },
};
