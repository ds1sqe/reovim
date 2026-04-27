/**
 * Manual interactive testing of snippet feature (#136).
 *
 * Simulates a human user step-by-step, capturing state after each action.
 * Run with: npx vitest run tests/integration/manual-snippet.test.ts
 */

import { describe, it, beforeAll, afterAll } from "vitest";
import { WebIntegrationTest } from "../helpers/integration.js";
import type { HeadlessWebClient } from "../../src/headless/index.js";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function printState(client: HeadlessWebClient, label: string) {
  console.log(`\n╔══════════════════════════════════════════╗`);
  console.log(`║  ${label.padEnd(40)}║`);
  console.log(`╚══════════════════════════════════════════╝`);
  console.log(`  Mode:   ${client.getModeDisplay()}`);
  console.log(`  Cursor: line=${client.getCursor().line}, col=${client.getCursor().col}`);
  console.log(`  Buffer:`);
  client.getLines().forEach((l, i) => console.log(`    [${i}] ${JSON.stringify(l)}`));
}

describe("Manual Snippet Session", () => {
  let test: WebIntegrationTest;
  let c: HeadlessWebClient;

  beforeAll(async () => {
    test = await WebIntegrationTest.create();
    c = await test.connect();
    await sleep(200);
  }, 15000);

  afterAll(async () => {
    await test.cleanup();
  });

  it("interactive walkthrough", async () => {
    // ── Step 0: Initial state ──
    printState(c, "STEP 0: Initial empty buffer");

    // ── Step 1: Enter insert mode, type "fn" ──
    await c.sendKeys("i");
    await sleep(100);
    await c.sendKeys("fn");
    await sleep(200);
    printState(c, "STEP 1: Typed 'ifn'");

    // ── Step 2: <C-s> to expand ──
    await c.sendKeys("<C-s>");
    await sleep(300);
    printState(c, "STEP 2: <C-s> → snippet expanded");

    // ── Step 3: Tab to $2 (params) ──
    await c.sendKeys("<Tab>");
    await sleep(200);
    printState(c, "STEP 3: <Tab> → cursor at $2 (params)");

    // ── Step 4: Type in params placeholder ──
    await c.sendKeys("x: i32");
    await sleep(200);
    printState(c, "STEP 4: Typed 'x: i32' in params");

    // ── Step 5: S-Tab back to $1 (name) ──
    await c.sendKeys("<S-Tab>");
    await sleep(200);
    printState(c, "STEP 5: <S-Tab> → back to $1 (name)");

    // ── Step 6: Type function name ──
    await c.sendKeys("add");
    await sleep(200);
    printState(c, "STEP 6: Typed 'add' as function name");

    // ── Step 7: Tab twice to $0 (body) ──
    await c.sendKeys("<Tab>");
    await sleep(100);
    await c.sendKeys("<Tab>");
    await sleep(200);
    printState(c, "STEP 7: <Tab><Tab> → cursor at $0 (body)");

    // ── Step 8: Type body code ──
    await c.sendKeys("x + 1");
    await sleep(200);
    printState(c, "STEP 8: Typed 'x + 1' in body");

    // ── Step 9: Escape exits snippet mode ──
    await c.sendKeys("<Esc>");
    await sleep(200);
    printState(c, "STEP 9: <Esc> → back to INSERT");

    // ── Step 10: Open new line, type "for", expand ──
    await c.sendKeys("<Esc>");
    await sleep(100);
    await c.sendKeys("o");
    await sleep(100);
    await c.sendKeys("for<C-s>");
    await sleep(300);
    printState(c, "STEP 10: New line → 'for' → <C-s>");

    // ── Step 11: Fill for-loop placeholders ──
    await c.sendKeys("item");
    await sleep(100);
    await c.sendKeys("<Tab>");
    await sleep(100);
    await c.sendKeys("items");
    await sleep(100);
    await c.sendKeys("<Tab>");
    await sleep(100);
    await c.sendKeys("process(item);");
    await sleep(200);
    printState(c, "STEP 11: Filled for-loop placeholders");

    // ── Step 12: Escape, new line, unknown prefix ──
    await c.sendKeys("<Esc><Esc>");
    await sleep(100);
    await c.sendKeys("o");
    await sleep(100);
    await c.sendKeys("zzz<C-s>");
    await sleep(300);
    printState(c, "STEP 12: 'zzz' + <C-s> → no match");

    // ── Final frame capture ──
    console.log("\n╔══════════════════════════════════════════╗");
    console.log("║  FINAL: Full frame capture                ║");
    console.log("╚══════════════════════════════════════════╝");
    const capture = c.capture("plain_text");
    console.log(capture.content);
  }, 30000);
});
