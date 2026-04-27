/**
 * Headless Web Client Module
 *
 * Provides a Node.js-only web client for testing.
 * No DOM rendering, pure gRPC communication.
 */

export {
  HeadlessWebClient,
  type HeadlessClientOptions,
} from "./client.js";

// Re-export capture types for convenience
export {
  type CaptureFormat,
  type FrameCapture,
  type FrameMetadata,
} from "../capture/index.js";
