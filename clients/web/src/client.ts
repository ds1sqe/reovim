/**
 * gRPC-Web Client Setup
 *
 * Creates typed clients for all reovim services using Connect-Web.
 * Supports token-based authentication (#483) via `x-reovim-token` header.
 */

import { createGrpcWebTransport } from "@connectrpc/connect-web";
import {
  createClient as createConnectClient,
  type Interceptor,
} from "@connectrpc/connect";

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
  /** Store a session token received from Join() for subsequent requests. */
  setSessionToken(token: string): void;
}

/**
 * Create a reovim client connected to the specified server.
 *
 * After calling `presence.join()`, store the returned `session_token` via
 * `client.setSessionToken(token)`. All subsequent requests will include
 * the `x-reovim-token` header for server-side identity resolution (#483).
 *
 * @param baseUrl - Server URL (e.g., "http://localhost:12521")
 * @returns Object with typed clients for all services
 */
export function createClient(baseUrl: string): ReovimClient {
  // Mutable token state captured by the interceptor closure
  const tokenState = { value: "" };

  // Interceptor that injects x-reovim-token header on every request (#483)
  const authInterceptor: Interceptor = (next) => async (req) => {
    if (tokenState.value) {
      req.header.set("x-reovim-token", tokenState.value);
    }
    return next(req);
  };

  const transport = createGrpcWebTransport({
    baseUrl,
    interceptors: [authInterceptor],
  });

  return {
    input: createConnectClient(InputService, transport),
    state: createConnectClient(StateService, transport),
    buffer: createConnectClient(BufferService, transport),
    notification: createConnectClient(NotificationService, transport),
    server: createConnectClient(ServerService, transport),
    presence: createConnectClient(PresenceService, transport),
    setSessionToken(token: string) {
      tokenState.value = token;
    },
  };
}
