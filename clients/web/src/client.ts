/**
 * gRPC-Web Client Setup
 *
 * Creates typed clients for all reovim services using Connect-Web.
 */

import { createGrpcWebTransport } from "@connectrpc/connect-web";
import { createClient as createConnectClient } from "@connectrpc/connect";

// Service definitions from generated code
import { InputService } from "./gen/reovim/v2/input_connect.js";
import { StateService } from "./gen/reovim/v2/state_connect.js";
import { BufferService } from "./gen/reovim/v2/buffer_connect.js";
import { NotificationService } from "./gen/reovim/v2/notification_connect.js";
import { ServerService } from "./gen/reovim/v2/server_connect.js";
import { PresenceService } from "./gen/reovim/v2/presence_connect.js";

export interface ReovimClient {
  input: ReturnType<typeof createConnectClient<typeof InputService>>;
  state: ReturnType<typeof createConnectClient<typeof StateService>>;
  buffer: ReturnType<typeof createConnectClient<typeof BufferService>>;
  notification: ReturnType<typeof createConnectClient<typeof NotificationService>>;
  server: ReturnType<typeof createConnectClient<typeof ServerService>>;
  presence: ReturnType<typeof createConnectClient<typeof PresenceService>>;
}

/**
 * Create a reovim client connected to the specified server.
 *
 * @param baseUrl - Server URL (e.g., "http://localhost:12521")
 * @returns Object with typed clients for all services
 */
export function createClient(baseUrl: string): ReovimClient {
  const transport = createGrpcWebTransport({
    baseUrl,
  });

  return {
    input: createConnectClient(InputService, transport),
    state: createConnectClient(StateService, transport),
    buffer: createConnectClient(BufferService, transport),
    notification: createConnectClient(NotificationService, transport),
    server: createConnectClient(ServerService, transport),
    presence: createConnectClient(PresenceService, transport),
  };
}
