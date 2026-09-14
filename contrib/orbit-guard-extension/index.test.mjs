#!/usr/bin/env node
/**
 * Behaviour tests for the Orbit guard extension entry.
 *
 * Run: node --test contrib/orbit-guard-extension/
 *
 * Drives the extension the way pi does — `activate(pi)`, then `tool_call`
 * events — with a fake `pi` and a fake `ctx.ui`, asserting exactly when a
 * tool call is confirmed, allowed, or blocked.
 */

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test, { beforeEach } from "node:test";

import activate from "./index.js";

let home;
let confirmCalls;
let confirmResult;

beforeEach(() => {
  home = fs.mkdtempSync(path.join(os.tmpdir(), "orbit-guard-entry-"));
  fs.mkdirSync(path.join(home, ".orbit-pi"), { recursive: true });
  confirmCalls = 0;
  confirmResult = true;
  process.env.HOME = home;
});

function setMode(mode) {
  fs.writeFileSync(
    path.join(home, ".orbit-pi", "access.json"),
    JSON.stringify({ mode }),
  );
}

function fakePi() {
  const handlers = new Map();
  return {
    on(event, handler) {
      handlers.set(event, handler);
    },
    fire(event, payload, ctx) {
      const handler = handlers.get(event);
      assert.ok(handler, `no handler for ${event}`);
      return handler(payload, ctx);
    },
  };
}

function fakeCtx({ hasUI = true } = {}) {
  return {
    hasUI,
    ui: {
      confirm: async () => {
        confirmCalls += 1;
        return confirmResult;
      },
    },
  };
}

test("full-access never prompts", async () => {
  setMode("full-access");
  const pi = fakePi();
  activate(pi);
  const result = await pi.fire(
    "tool_call",
    { toolName: "bash", input: { command: "rm -rf build" } },
    fakeCtx(),
  );
  assert.equal(result, undefined);
  assert.equal(confirmCalls, 0);
});

test("supervised allows reads and confirms edits", async () => {
  setMode("supervised");
  const pi = fakePi();
  activate(pi);

  const read = await pi.fire("tool_call", { toolName: "read", input: { path: "a" } }, fakeCtx());
  assert.equal(read, undefined, "reads pass through");
  assert.equal(confirmCalls, 0);

  const edit = await pi.fire("tool_call", { toolName: "edit", input: { path: "a" } }, fakeCtx());
  assert.equal(edit, undefined, "confirmed edit runs");
  assert.equal(confirmCalls, 1);
});

test("a denied confirmation blocks the tool with a reason", async () => {
  setMode("supervised");
  confirmResult = false;
  const pi = fakePi();
  activate(pi);
  const result = await pi.fire(
    "tool_call",
    { toolName: "bash", input: { command: "sudo rm -rf /" } },
    fakeCtx(),
  );
  assert.equal(result.block, true);
  assert.match(result.reason, /Denied/);
});

test("auto-accept-edits skips the prompt for edits but asks for exec", async () => {
  setMode("auto-accept-edits");
  const pi = fakePi();
  activate(pi);

  const edit = await pi.fire("tool_call", { toolName: "write", input: { path: "a" } }, fakeCtx());
  assert.equal(edit, undefined);
  assert.equal(confirmCalls, 0, "edits are not confirmed");

  await pi.fire("tool_call", { toolName: "bash", input: { command: "ls" } }, fakeCtx());
  assert.equal(confirmCalls, 1, "exec is confirmed");
});

test("without a UI a confirmation-required call is blocked, not allowed", async () => {
  setMode("supervised");
  const pi = fakePi();
  activate(pi);
  const result = await pi.fire(
    "tool_call",
    { toolName: "edit", input: { path: "a" } },
    fakeCtx({ hasUI: false }),
  );
  assert.equal(result.block, true);
  assert.equal(confirmCalls, 0);
});

test("a throwing confirm fails closed", async () => {
  setMode("supervised");
  const pi = fakePi();
  activate(pi);
  const ctx = {
    hasUI: true,
    ui: {
      confirm: async () => {
        throw new Error("dialog cancelled");
      },
    },
  };
  const result = await pi.fire("tool_call", { toolName: "bash", input: { command: "ls" } }, ctx);
  assert.equal(result.block, true);
});

test("a missing mode file behaves as full-access", async () => {
  const pi = fakePi();
  activate(pi);
  const result = await pi.fire("tool_call", { toolName: "bash", input: { command: "ls" } }, fakeCtx());
  assert.equal(result, undefined);
  assert.equal(confirmCalls, 0);
});

test("parallel confirms are serialized, never stacked", async () => {
  setMode("supervised");
  let active = 0;
  let maxActive = 0;
  const ctx = {
    hasUI: true,
    ui: {
      confirm: async () => {
        active += 1;
        maxActive = Math.max(maxActive, active);
        await new Promise((resolve) => setTimeout(resolve, 5));
        active -= 1;
        return true;
      },
    },
  };
  const pi = fakePi();
  activate(pi);
  await Promise.all([
    pi.fire("tool_call", { toolName: "bash", input: { command: "a" } }, ctx),
    pi.fire("tool_call", { toolName: "bash", input: { command: "b" } }, ctx),
  ]);
  assert.equal(maxActive, 1, "only one confirm in flight at a time");
});
