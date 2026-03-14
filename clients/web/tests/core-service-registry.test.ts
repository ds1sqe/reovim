/**
 * Web Client Service Registry Tests (#650)
 */

import { describe, it, expect } from "vitest";
import { WebClientServiceRegistry } from "../src/core/service-registry.js";

describe("WebClientServiceRegistry", () => {
  it("starts empty", () => {
    const reg = new WebClientServiceRegistry();
    expect(reg.size).toBe(0);
  });

  it("register and retrieve a service", () => {
    const reg = new WebClientServiceRegistry();
    const svc = { doThing: () => "done" };
    reg.register("my-svc", svc);
    expect(reg.get<typeof svc>("my-svc")).toBe(svc);
  });

  it("contains returns true for registered key", () => {
    const reg = new WebClientServiceRegistry();
    reg.register("a", 1);
    expect(reg.contains("a")).toBe(true);
    expect(reg.contains("b")).toBe(false);
  });

  it("size increments on new keys", () => {
    const reg = new WebClientServiceRegistry();
    reg.register("a", 1);
    reg.register("b", 2);
    reg.register("c", 3);
    expect(reg.size).toBe(3);
  });

  it("overwrite keeps size the same", () => {
    const reg = new WebClientServiceRegistry();
    reg.register("key", "v1");
    reg.register("key", "v2");
    expect(reg.size).toBe(1);
    expect(reg.get("key")).toBe("v2");
  });

  it("get returns undefined for missing key", () => {
    const reg = new WebClientServiceRegistry();
    expect(reg.get("nope")).toBeUndefined();
  });

  it("stores typed services", () => {
    interface Counter {
      count: number;
      increment(): void;
    }

    const reg = new WebClientServiceRegistry();
    const counter: Counter = {
      count: 0,
      increment() {
        this.count++;
      },
    };
    reg.register("counter", counter);

    const retrieved = reg.get<Counter>("counter");
    expect(retrieved).toBeDefined();
    retrieved!.increment();
    expect(retrieved!.count).toBe(1);
  });
});
