#!/usr/bin/env node

/**
 * reovim-web capture - Playwright-based web client screenshot tool.
 *
 * Launches headless Chromium, navigates to the web client, waits for
 * connection and render, then captures PNG or HTML output.
 *
 * Usage:
 *   npx reovim-web capture --grpc 127.0.0.1:13000 --web-url http://localhost:5173
 *   npx reovim-web capture --format html --grpc 127.0.0.1:13000 -o capture.html
 */

import { parseArgs } from "node:util";
import { writeFileSync } from "node:fs";

async function main() {
  const { values } = parseArgs({
    options: {
      grpc: { type: "string", default: "127.0.0.1:12521" },
      "web-url": { type: "string", default: "http://localhost:5173" },
      width: { type: "string", default: "1920" },
      height: { type: "string", default: "1080" },
      format: { type: "string", default: "png" },
      output: { type: "string", short: "o" },
      dpr: { type: "string", default: "1" },
      timeout: { type: "string", default: "10000" },
    },
    strict: true,
  });

  const format = values.format!;
  if (format !== "png" && format !== "html") {
    console.error(`Error: unsupported format '${format}'. Use 'png' or 'html'.`);
    console.error("For text capture, use: reovim cli capture --format plain_text");
    process.exit(1);
  }

  // Dynamic import - Playwright is a peer dependency
  let chromium: typeof import("playwright").chromium;
  try {
    ({ chromium } = await import("playwright"));
  } catch {
    console.error("Error: Playwright is required for web capture.");
    console.error("Install with: npm install playwright && npx playwright install chromium");
    process.exit(1);
  }

  const width = Number(values.width);
  const height = Number(values.height);

  const browser = await chromium.launch();
  const page = await browser.newPage({
    viewport: { width, height },
    deviceScaleFactor: Number(values.dpr),
  });

  try {
    // Inject server address before page loads (same mechanism as E2E tests)
    const serverUrl = `http://${values.grpc}`;
    await page.addInitScript(`window.__REOVIM_SERVER = "${serverUrl}"`);
    await page.goto(values["web-url"]!, { waitUntil: "domcontentloaded" });

    const timeoutMs = Number(values.timeout);

    // Wait for web client to connect and finish first render.
    // 1. Connection complete: .connecting class removed from #app
    // 2. Render complete: mode indicator shows text (always set after state fetch)
    await page.waitForSelector("#app:not(.connecting)", { timeout: timeoutMs });
    await page.waitForFunction(
      () => (document.getElementById("mode")?.textContent?.length ?? 0) > 0,
      { timeout: timeoutMs },
    );

    if (format === "png") {
      const screenshot = await page.screenshot({ type: "png", fullPage: false });
      if (values.output) {
        writeFileSync(values.output, screenshot);
      } else {
        process.stdout.write(screenshot);
      }
    } else {
      const html = await page.content();
      if (values.output) {
        writeFileSync(values.output, html);
      } else {
        process.stdout.write(html);
      }
    }
  } finally {
    await browser.close();
  }
}

main().catch((e: Error) => { console.error(e.message); process.exit(1); });
