/**
 * gRPC Server Handle Tests (#650)
 *
 * Tests the GrpcServerHandle adapter with a mock ReovimClient.
 */

import { describe, it, expect, vi } from "vitest";
import { GrpcServerHandle } from "../src/core/server-handle.js";
import type { ReovimClient } from "../src/client.js";

function mockClient(): ReovimClient {
  return {
    input: {
      sendKeys: vi.fn().mockResolvedValue({ ok: true }),
    },
    state: {
      getOptions: vi.fn().mockResolvedValue({ options: [] }),
    },
    buffer: {} as ReovimClient["buffer"],
    notification: {} as ReovimClient["notification"],
    server: {} as ReovimClient["server"],
    presence: {} as ReovimClient["presence"],
    setSessionToken: vi.fn(),
  } as unknown as ReovimClient;
}

describe("GrpcServerHandle", () => {
  it("executeCommand sends as ex-command keys", async () => {
    const client = mockClient();
    const handle = new GrpcServerHandle(client);

    await handle.executeCommand("write");

    expect(client.input.sendKeys).toHaveBeenCalledWith({
      keys: ":write\r",
    });
  });

  it("getOptions returns empty map on empty response", async () => {
    const client = mockClient();
    const handle = new GrpcServerHandle(client);

    const result = await handle.getOptions(["tabstop"]);
    expect(result.size).toBe(0);
  });

  it("getOptions parses bool option", async () => {
    const client = mockClient();
    (client.state.getOptions as ReturnType<typeof vi.fn>).mockResolvedValue({
      options: [
        { name: "number", value: { case: "boolValue", value: true } },
      ],
    });
    const handle = new GrpcServerHandle(client);

    const result = await handle.getOptions(["number"]);
    expect(result.get("number")).toEqual({ kind: "bool", value: true });
  });

  it("getOptions parses int option", async () => {
    const client = mockClient();
    (client.state.getOptions as ReturnType<typeof vi.fn>).mockResolvedValue({
      options: [
        { name: "tabstop", value: { case: "intValue", value: 4n } },
      ],
    });
    const handle = new GrpcServerHandle(client);

    const result = await handle.getOptions(["tabstop"]);
    expect(result.get("tabstop")).toEqual({ kind: "integer", value: 4 });
  });

  it("getOptions parses string option", async () => {
    const client = mockClient();
    (client.state.getOptions as ReturnType<typeof vi.fn>).mockResolvedValue({
      options: [
        {
          name: "filetype",
          value: { case: "stringValue", value: "rust" },
        },
      ],
    });
    const handle = new GrpcServerHandle(client);

    const result = await handle.getOptions(["filetype"]);
    expect(result.get("filetype")).toEqual({ kind: "string", value: "rust" });
  });

  it("getOptions returns empty map on error", async () => {
    const client = mockClient();
    (client.state.getOptions as ReturnType<typeof vi.fn>).mockRejectedValue(
      new Error("network"),
    );
    const handle = new GrpcServerHandle(client);

    const result = await handle.getOptions(["any"]);
    expect(result.size).toBe(0);
  });

  it("listCommands returns empty array", async () => {
    const client = mockClient();
    const handle = new GrpcServerHandle(client);

    const result = await handle.listCommands();
    expect(result).toEqual([]);
  });

  it("getOptionMetadata returns null", async () => {
    const client = mockClient();
    const handle = new GrpcServerHandle(client);

    const result = await handle.getOptionMetadata("tabstop");
    expect(result).toBeNull();
  });
});
