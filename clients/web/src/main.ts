/**
 * reovim Web Client - Entry Point
 *
 * Minimal web client for reovim using gRPC-Web via Connect-Web.
 */

import { createClient } from "./client.js";
import { setupKeyboardHandler } from "./input.js";
import { Editor } from "./editor.js";

async function main() {
  const app = document.getElementById("app");
  if (!app) {
    throw new Error("App element not found");
  }

  // Mark as connecting
  app.classList.add("connecting");
  app.classList.remove("disconnected");

  try {
    // Create gRPC-Web client
    // Allow port override via URL query parameter for E2E testing
    // e.g., http://localhost:5173/?port=12522
    const urlParams = new URLSearchParams(window.location.search);
    const port = urlParams.get("port") || "12521";
    const serverUrl = `http://${window.location.hostname}:${port}`;
    console.log("Connecting to:", serverUrl);
    const client = createClient(serverUrl);

    // Join presence session to get unique client ID (Phase 11.2)
    // CRITICAL: Without this, all clients share state as ClientId(0)
    const joinResponse = await client.presence.join({
      clientType: "web",
      displayName: "Web Client",
    });
    const myClientId = joinResponse.clientId;
    console.log("Joined presence session, client ID:", myClientId.toString());

    // Initialize editor state
    const editor = new Editor(client, myClientId);

    // Initialize WASM module for multi-window support
    await editor.init();

    // Setup keyboard input with client ID
    setupKeyboardHandler(client, editor, myClientId);

    // Initial state fetch
    await editor.refresh();

    // Subscribe to notifications for real-time updates
    editor.subscribeToNotifications();

    // Mark as connected
    app.classList.remove("connecting");

    console.log("reovim web client connected");
  } catch (error) {
    console.error("Failed to connect:", error);
    app.classList.remove("connecting");
    app.classList.add("disconnected");
  }
}

// Start the application
main().catch(console.error);
