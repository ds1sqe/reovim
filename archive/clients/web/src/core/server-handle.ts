/**
 * gRPC Server Handle Adapter
 *
 * Wraps the existing `ReovimClient` into the CLM `ServerHandle` interface.
 *
 * @module core/server-handle
 */

import type { ServerHandle } from "./contracts.js";
import type { OptionValue, OptionMetadata } from "./types.js";
import type { ReovimClient } from "../client.js";

/**
 * Server handle implementation backed by gRPC-Web.
 *
 * Delegates to the existing `ReovimClient` services.
 */
export class GrpcServerHandle implements ServerHandle {
  private client: ReovimClient;

  constructor(client: ReovimClient) {
    this.client = client;
  }

  async getOptions(names: string[]): Promise<Map<string, OptionValue>> {
    const result = new Map<string, OptionValue>();
    try {
      const response = await this.client.state.getOptions({ names });
      for (const opt of response.options) {
        const val = opt.value;
        if (val.case === "boolValue") {
          result.set(opt.name, { kind: "bool", value: val.value });
        } else if (val.case === "intValue") {
          result.set(opt.name, { kind: "integer", value: Number(val.value) });
        } else if (val.case === "stringValue") {
          result.set(opt.name, { kind: "string", value: val.value });
        }
      }
    } catch {
      // Return empty map on error (server might not support getOptions)
    }
    return result;
  }

  async executeCommand(command: string): Promise<void> {
    // Execute by sending the command as keys in ex-command form
    await this.client.input.sendKeys({ keys: `:${command}\r` });
  }

  async listCommands(): Promise<string[]> {
    // No server API for listing commands yet.
    return [];
  }

  async getOptionMetadata(_name: string): Promise<OptionMetadata | null> {
    // No server API for option metadata yet.
    return null;
  }
}
