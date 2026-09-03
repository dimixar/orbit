import { describe, expect, it } from "vitest";
import { createEvent } from "../events";
import { agentReducer, createInitialState } from "../reducer";
import type { AgentState } from "../types";

function run(state: AgentState, runId: string) {
  return state.runs[runId]!;
}

function parts(state: AgentState, runId: string) {
  return run(state, runId).message.parts;
}

describe("agentReducer", () => {
  it("creates a run on agent.start", () => {
    let state = createInitialState();
    state = agentReducer(
      state,
      createEvent("agent.start", "run_1", { prompt: "hi" }),
    );
    expect(state.activeRunId).toBe("run_1");
    expect(run(state, "run_1").status).toBe("streaming");
    expect(run(state, "run_1").message.role).toBe("assistant");
    expect(run(state, "run_1").message.parts).toEqual([]);
  });

  it("does not reset an existing run on a redelivered agent.start", () => {
    let state = createInitialState();
    state = agentReducer(state, createEvent("agent.start", "run_1", {}));
    state = agentReducer(
      state,
      createEvent("assistant.text", "run_1", { content: "hello", delta: true }),
    );
    state = agentReducer(state, createEvent("agent.start", "run_1", {}));
    expect(parts(state, "run_1")).toHaveLength(1);
  });

  it("creates a ToolCallPart on tool.call and never duplicates it", () => {
    let state = createInitialState();
    state = agentReducer(state, createEvent("agent.start", "run_1", {}));
    state = agentReducer(
      state,
      createEvent("tool.call", "run_1", {
        id: "call_1",
        tool: "read_file",
        input: { path: "src/auth.ts" },
      }),
    );
    state = agentReducer(
      state,
      createEvent("tool.call", "run_1", {
        id: "call_1",
        tool: "read_file",
        input: { path: "src/auth.ts" },
      }),
    );
    const calls = parts(state, "run_1").filter((p) => p.type === "tool_call");
    expect(calls).toHaveLength(1);
    expect(calls[0]).toMatchObject({
      type: "tool_call",
      id: "call_1",
      tool: "read_file",
      status: "running",
    });
  });

  it("correlates tool.result with the matching tool call and appends a result part", () => {
    let state = createInitialState();
    state = agentReducer(state, createEvent("agent.start", "run_1", {}));
    state = agentReducer(
      state,
      createEvent("tool.call", "run_1", {
        id: "call_1",
        tool: "read_file",
        input: { path: "src/auth.ts" },
      }),
    );
    state = agentReducer(
      state,
      createEvent("tool.result", "run_1", {
        toolCallId: "call_1",
        status: "success",
        output: "export function validateToken…",
        durationMs: 214,
      }),
    );

    const call = parts(state, "run_1").find((p) => p.type === "tool_call")!;
    expect(call).toMatchObject({
      type: "tool_call",
      id: "call_1",
      tool: "read_file", // original tool name preserved
      status: "success",
      durationMs: 214,
    });

    const result = parts(state, "run_1").find(
      (p) => p.type === "tool_result",
    )!;
    expect(result).toMatchObject({
      type: "tool_result",
      toolCallId: "call_1",
      status: "success",
      output: "export function validateToken…",
    });
  });

  it("appends streaming text to the trailing TextPart", () => {
    let state = createInitialState();
    state = agentReducer(state, createEvent("agent.start", "run_1", {}));
    state = agentReducer(
      state,
      createEvent("assistant.text", "run_1", { content: "## Fixed", delta: true }),
    );
    state = agentReducer(
      state,
      createEvent("assistant.text", "run_1", { content: "\n\nDone.", delta: true }),
    );
    const text = parts(state, "run_1").find((p) => p.type === "text")!;
    expect(text).toMatchObject({ type: "text", content: "## Fixed\n\nDone." });
    expect(parts(state, "run_1").filter((p) => p.type === "text")).toHaveLength(1);
  });

  it("replaces text when delta is false", () => {
    let state = createInitialState();
    state = agentReducer(state, createEvent("agent.start", "run_1", {}));
    state = agentReducer(
      state,
      createEvent("assistant.text", "run_1", { content: "old", delta: true }),
    );
    state = agentReducer(
      state,
      createEvent("assistant.text", "run_1", { content: "new", delta: false }),
    );
    const text = parts(state, "run_1").find((p) => p.type === "text")!;
    expect(text).toMatchObject({ type: "text", content: "new" });
  });

  it("creates and updates a DiffPart per file", () => {
    let state = createInitialState();
    state = agentReducer(state, createEvent("agent.start", "run_1", {}));
    state = agentReducer(
      state,
      createEvent("file.diff", "run_1", {
        file: "src/auth.ts",
        patch: "--- a/src/auth.ts\n+++ b/src/auth.ts",
        additions: 12,
        deletions: 4,
      }),
    );
    state = agentReducer(
      state,
      createEvent("file.diff", "run_1", {
        file: "src/auth.ts",
        patch: "--- a/src/auth.ts\n+++ b/src/auth.ts\n@@ -1 +1 @@",
        additions: 13,
        deletions: 4,
      }),
    );
    const diffs = parts(state, "run_1").filter((p) => p.type === "diff");
    expect(diffs).toHaveLength(1);
    expect(diffs[0]).toMatchObject({ type: "diff", file: "src/auth.ts", additions: 13 });
  });

  it("replaces plan steps on agent.plan", () => {
    let state = createInitialState();
    state = agentReducer(state, createEvent("agent.start", "run_1", {}));
    state = agentReducer(
      state,
      createEvent("agent.plan", "run_1", {
        steps: [{ id: "1", title: "Inspect", status: "pending" }],
      }),
    );
    state = agentReducer(
      state,
      createEvent("agent.plan", "run_1", {
        steps: [
          { id: "1", title: "Inspect", status: "complete" },
          { id: "2", title: "Fix", status: "running" },
        ],
      }),
    );
    const plan = parts(state, "run_1").find((p) => p.type === "plan")!;
    expect(plan).toMatchObject({
      type: "plan",
      steps: [
        { id: "1", title: "Inspect", status: "complete" },
        { id: "2", title: "Fix", status: "running" },
      ],
    });
  });

  it("appends an ErrorPart and fails the run on agent.error", () => {
    let state = createInitialState();
    state = agentReducer(state, createEvent("agent.start", "run_1", {}));
    state = agentReducer(
      state,
      createEvent("agent.error", "run_1", { message: "Command failed", details: { exitCode: 1 } }),
    );
    expect(run(state, "run_1").status).toBe("error");
    expect(run(state, "run_1").error).toBe("Command failed");
    expect(run(state, "run_1").message.status).toBe("error");
    const error = parts(state, "run_1").find((p) => p.type === "error")!;
    expect(error).toMatchObject({ type: "error", message: "Command failed" });
  });

  it("marks the run complete on agent.complete", () => {
    let state = createInitialState();
    state = agentReducer(state, createEvent("agent.start", "run_1", {}));
    state = agentReducer(state, createEvent("agent.complete", "run_1", {}));
    expect(run(state, "run_1").status).toBe("complete");
    expect(run(state, "run_1").message.status).toBe("complete");
    expect(run(state, "run_1").completedAt).toBeDefined();
  });

  it("keeps concurrent runs isolated", () => {
    let state = createInitialState();
    state = agentReducer(state, createEvent("agent.start", "run_a", {}));
    state = agentReducer(state, createEvent("agent.start", "run_b", {}));

    state = agentReducer(
      state,
      createEvent("tool.call", "run_a", { id: "a1", tool: "read_file", input: {} }),
    );
    state = agentReducer(
      state,
      createEvent("assistant.text", "run_b", { content: "from B", delta: true }),
    );

    expect(parts(state, "run_a").map((p) => p.type)).toEqual(["tool_call"]);
    expect(parts(state, "run_b").map((p) => p.type)).toEqual(["text"]);
    expect(state.activeRunId).toBe("run_b");
  });

  it("ignores events for unknown runs", () => {
    let state = createInitialState();
    const next = agentReducer(
      state,
      createEvent("assistant.text", "ghost", { content: "x", delta: true }),
    );
    expect(next).toBe(state);
  });

  it("updates a tool call in place via the internal tool.update event", () => {
    let state = createInitialState();
    state = agentReducer(state, createEvent("agent.start", "run_1", {}));
    state = agentReducer(
      state,
      createEvent("tool.call", "run_1", { id: "c1", tool: "bash", input: { command: "ls" } }),
    );
    state = agentReducer(
      state,
      createEvent("tool.update", "run_1", {
        id: "c1",
        patch: { status: "success", output: "src/" },
      }),
    );
    const call = parts(state, "run_1").find((p) => p.type === "tool_call")!;
    expect(call).toMatchObject({ status: "success", output: "src/", tool: "bash" });
  });
});
