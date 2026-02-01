/**
 * Vim Commands E2E Tests
 *
 * These tests mirror the vim_commands.rs integration tests.
 * They test vim-like editing operations through the web client.
 *
 * NOTE: These tests are currently skipped because they require:
 * 1. A running reovim server with gRPC-Web support
 * 2. The web client configured to connect to the test server port
 *
 * Enable these tests once the server-client connection is properly configured.
 */

import { test, expect, waitForConnection, getMode, getBufferContent, sendKeys, navigateWithPort } from "./fixtures";

test.describe("Vim Commands", () => {
  // These tests require full server integration
  test.describe.configure({ mode: "serial" });

  // Skip all tests if server port not available
  test.skip(({ serverPort }) => !serverPort, "Server not available");

  test.beforeEach(async ({ page, serverPort }) => {
    // Navigate with the test server port
    await navigateWithPort(page, serverPort);

    // Wait for connection before each test
    await waitForConnection(page, 15000);
  });

  // Motion commands
  test.describe("Motion Commands", () => {
    test.skip("h - move cursor left", async ({ page }) => {
      await waitForConnection(page);
      // Start with some text
      await sendKeys(page, "ihello<Esc>");
      // Move left
      await sendKeys(page, "h");
      // Verify cursor moved (would need cursor position API)
    });

    test.skip("l - move cursor right", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "ihello<Esc>0l");
      // Verify cursor at column 1
    });

    test.skip("j - move cursor down", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "ihello<CR>world<Esc>ggj");
      // Verify cursor on line 2
    });

    test.skip("k - move cursor up", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "ihello<CR>world<Esc>k");
      // Verify cursor on line 1
    });

    test.skip("w - move word forward", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "ihello world<Esc>0w");
      // Verify cursor at start of "world"
    });

    test.skip("b - move word backward", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "ihello world<Esc>b");
      // Verify cursor at start of "world"
    });

    test.skip("0 - move to line start", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "ihello<Esc>0");
      // Verify cursor at column 0
    });

    test.skip("$ - move to line end", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "ihello<Esc>0$");
      // Verify cursor at end of line
    });

    test.skip("gg - move to first line", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "iline1<CR>line2<CR>line3<Esc>gg");
      // Verify cursor on line 1
    });

    test.skip("G - move to last line", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "iline1<CR>line2<CR>line3<Esc>ggG");
      // Verify cursor on line 3
    });
  });

  // Delete commands
  test.describe("Delete Commands", () => {
    test.skip("x - delete character", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "ihello<Esc>0x");
      const content = await getBufferContent(page);
      expect(content).toBe("ello");
    });

    test.skip("dw - delete word", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "ihello world<Esc>0dw");
      const content = await getBufferContent(page);
      expect(content).toBe("world");
    });

    test.skip("dd - delete line", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "iline1<CR>line2<Esc>ggdd");
      const content = await getBufferContent(page);
      expect(content).toBe("line2");
    });

    test.skip("d$ - delete to end of line", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "ihello world<Esc>0wwd$");
      const content = await getBufferContent(page);
      expect(content).toBe("hello ");
    });
  });

  // Change commands
  test.describe("Change Commands", () => {
    test.skip("cw - change word", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "ihello<Esc>0cwworld<Esc>");
      const content = await getBufferContent(page);
      expect(content).toBe("world");
    });

    test.skip("cc - change line", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "ihello world<Esc>0ccnew line<Esc>");
      const content = await getBufferContent(page);
      expect(content).toBe("new line");
    });
  });

  // Yank and put commands
  test.describe("Yank and Put", () => {
    test.skip("yw - yank word", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "ihello<Esc>0ywAp");
      // Verify "hello" was yanked and pasted
    });

    test.skip("yy - yank line", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "ihello<Esc>yyp");
      const content = await getBufferContent(page);
      expect(content).toContain("hello\nhello");
    });

    test.skip("p - put after", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "ihello<Esc>yywp");
      // Verify word was pasted after cursor
    });

    test.skip("P - put before", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "ihello<Esc>yywP");
      // Verify word was pasted before cursor
    });
  });

  // Mode transitions
  test.describe("Mode Transitions", () => {
    test.skip("i - enter insert mode", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "i");
      const mode = await getMode(page);
      expect(mode?.toUpperCase()).toContain("INSERT");
    });

    test.skip("a - append mode", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "a");
      const mode = await getMode(page);
      expect(mode?.toUpperCase()).toContain("INSERT");
    });

    test.skip("<Esc> - return to normal", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "i<Esc>");
      const mode = await getMode(page);
      expect(mode?.toUpperCase()).toContain("NORMAL");
    });

    test.skip(": - enter command mode", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, ":");
      const mode = await getMode(page);
      expect(mode?.toUpperCase()).toContain("COMMAND");
    });

    test.skip("/ - enter search forward mode", async ({ page }) => {
      await waitForConnection(page);
      await sendKeys(page, "/");
      const mode = await getMode(page);
      expect(mode?.toUpperCase()).toContain("COMMAND");
    });
  });
});
