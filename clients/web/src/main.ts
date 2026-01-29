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
    // Use the same hostname as the page, with the gRPC port
    const serverUrl = `http://${window.location.hostname}:12521`;
    console.log("Connecting to:", serverUrl);
    const client = createClient(serverUrl);

    // Initialize editor state
    const editor = new Editor(client);

    // Setup keyboard input
    setupKeyboardHandler(client, editor);

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
