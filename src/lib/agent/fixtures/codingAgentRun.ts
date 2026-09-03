/**
 * Realistic example run fixture for Storybook / dev testing.
 *
 * Contains a plan, file reads, a file edit with a diff, a terminal command
 * that FAILS first (exit 1) and then succeeds, and a final Markdown
 * response — exercising every part type except approval.
 */

import { createEvent, type AgentEvent } from "../events";
import type { AgentMessage, PlanStep } from "../types";

export const codingAgentRunId = "run_fixture";

const now = Date.now();
let seq = 0;
const ev = (type: AgentEvent["type"], data: unknown): AgentEvent =>
  createEvent(type, codingAgentRunId, data as never, `fix-${++seq}`);

const planSteps: PlanStep[] = [
  { id: "1", title: "Inspect auth middleware", status: "complete" },
  { id: "2", title: "Find the token validation bug", status: "complete" },
  { id: "3", title: "Apply the fix", status: "complete" },
  { id: "4", title: "Run tests", status: "complete" },
];
const step = (id: string, status: PlanStep["status"]): PlanStep => ({
  id,
  title: planSteps.find((s) => s.id === id)!.title,
  status,
});

const diffPatch = [
  "--- a/src/auth.ts",
  "+++ b/src/auth.ts",
  "@@ -12,7 +12,11 @@ export function validateToken(request) {",
  "   const token = request.headers.token",
  "-  if (!token) throw new Error('Missing token')",
  "+  const header = request.headers.authorization",
  "+  const token = header?.startsWith('Bearer ') ? header.slice(7) : undefined",
  "+  if (!token) throw new Error('Missing token')",
  " ",
  "   try {",
  "     const payload = verify(token, SECRET)",
  "@@ -24,6 +28,8 @@ export function validateToken(request) {",
  "   } catch (err) {",
  "     throw new Error('Invalid token')",
  "   }",
  "+",
  "+  return payload",
  " }",
].join("\n");

export const codingAgentRunEvents: AgentEvent[] = [
  ev("agent.start", { prompt: "Fix the authentication bug", messageId: "msg_fixture" }),
  ev("agent.plan", {
    steps: [step("1", "pending"), step("2", "pending"), step("3", "pending"), step("4", "pending")],
  }),
  ev("agent.reasoning", {
    content: "Inspecting the authentication implementation.",
    status: "streaming",
  }),
  ev("agent.reasoning", {
    content: " The token is read from a non-standard header.",
    status: "streaming",
    delta: true,
  }),
  ev("agent.reasoning", {
    content: " I'll switch it to the standard Authorization header.",
    status: "complete",
    delta: true,
  }),

  ev("tool.call", { id: "call_1", tool: "read_file", input: { path: "src/auth.ts" } }),
  ev("tool.result", {
    toolCallId: "call_1",
    status: "success",
    output: "export function validateToken(request) {\n  const token = request.headers.token\n  ...",
    durationMs: 214,
  }),
  ev("agent.plan", {
    steps: [step("1", "complete"), step("2", "running"), step("3", "pending"), step("4", "pending")],
  }),

  ev("tool.call", {
    id: "call_2",
    tool: "edit_file",
    input: { path: "src/auth.ts", description: "Use Authorization header" },
  }),
  ev("file.diff", { file: "src/auth.ts", patch: diffPatch, additions: 12, deletions: 4 }),
  ev("tool.result", {
    toolCallId: "call_2",
    status: "success",
    output: "Applied 1 edit to src/auth.ts",
    durationMs: 512,
  }),
  ev("agent.plan", {
    steps: [step("1", "complete"), step("2", "complete"), step("3", "running"), step("4", "pending")],
  }),

  // First test run FAILS — exercises the error path.
  ev("tool.call", { id: "call_3", tool: "terminal", input: { command: "npm test" } }),
  ev("tool.result", {
    toolCallId: "call_3",
    status: "error",
    output: "✗ auth › validates a Bearer token\n  AssertionError: expected undefined to be 'Bearer abc'",
    durationMs: 1204,
    error: "Exit code 1 — 1 test failed",
  }),
  ev("agent.plan", {
    steps: [step("1", "complete"), step("2", "complete"), step("3", "complete"), step("4", "running")],
  }),

  // Agent fixes the remaining test and re-runs.
  ev("tool.call", {
    id: "call_4",
    tool: "edit_file",
    input: { path: "src/auth.test.ts", description: "Update test to use Authorization header" },
  }),
  ev("file.diff", {
    file: "src/auth.test.ts",
    patch: [
      "--- a/src/auth.test.ts",
      "+++ b/src/auth.test.ts",
      "@@ -3,7 +3,7 @@ import { validateToken } from './auth'",
      "-  const req = { headers: { token: 'abc' } }",
      "+  const req = { headers: { authorization: 'Bearer abc' } }",
      "   expect(validateToken(req)).toBe('abc')",
    ].join("\n"),
    additions: 1,
    deletions: 1,
  }),
  ev("tool.result", {
    toolCallId: "call_4",
    status: "success",
    output: "Applied 1 edit to src/auth.test.ts",
    durationMs: 388,
  }),

  ev("tool.call", { id: "call_5", tool: "terminal", input: { command: "npm test" } }),
  ev("tool.result", {
    toolCallId: "call_5",
    status: "success",
    output: "Test Files  1 passed (1)\n     Tests  42 passed (42)",
    durationMs: 3421,
  }),
  ev("agent.plan", {
    steps: [step("1", "complete"), step("2", "complete"), step("3", "complete"), step("4", "complete")],
  }),

  ev("assistant.text", {
    content:
      "## Fixed\n\nThe authentication middleware was updated to read the token from the standard `Authorization` header.\n\n### Changes\n\n- Fixed token validation\n- Added missing error handling\n- Updated the test to use the new header\n\n**Tests:** 42 passed",
  }),
  ev("agent.complete", { summary: "Fixed the authentication bug" }),
];

/** The final normalized message the fixture produces — handy for static renders. */
export const codingAgentRunMessage: AgentMessage = {
  id: "msg_fixture",
  role: "assistant",
  runId: codingAgentRunId,
  createdAt: new Date(now).toISOString(),
  status: "complete",
  parts: [
    {
      type: "reasoning",
      content:
        "Inspecting the authentication implementation. The token is read from a non-standard header. I'll switch it to the standard Authorization header.",
      status: "complete",
    },
    { type: "plan", steps: planSteps },
    {
      type: "tool_call",
      id: "call_1",
      tool: "read_file",
      status: "success",
      input: { path: "src/auth.ts" },
      output: "export function validateToken(request) {\n  const token = request.headers.token\n  ...",
      durationMs: 214,
    },
    {
      type: "tool_call",
      id: "call_2",
      tool: "edit_file",
      status: "success",
      input: { path: "src/auth.ts", description: "Use Authorization header" },
      output: "Applied 1 edit to src/auth.ts",
      durationMs: 512,
    },
    { type: "diff", file: "src/auth.ts", patch: diffPatch, additions: 12, deletions: 4 },
    {
      type: "tool_call",
      id: "call_3",
      tool: "terminal",
      status: "error",
      input: { command: "npm test" },
      output: "✗ auth › validates a Bearer token\n  AssertionError: expected undefined to be 'Bearer abc'",
      durationMs: 1204,
    },
    {
      type: "tool_result",
      toolCallId: "call_3",
      status: "error",
      output: "✗ auth › validates a Bearer token\n  AssertionError: expected undefined to be 'Bearer abc'",
      durationMs: 1204,
      error: "Exit code 1 — 1 test failed",
    },
    {
      type: "tool_call",
      id: "call_4",
      tool: "edit_file",
      status: "success",
      input: { path: "src/auth.test.ts", description: "Update test to use Authorization header" },
      output: "Applied 1 edit to src/auth.test.ts",
      durationMs: 388,
    },
    {
      type: "diff",
      file: "src/auth.test.ts",
      patch: [
        "--- a/src/auth.test.ts",
        "+++ b/src/auth.test.ts",
        "@@ -3,7 +3,7 @@ import { validateToken } from './auth'",
        "-  const req = { headers: { token: 'abc' } }",
        "+  const req = { headers: { authorization: 'Bearer abc' } }",
        "   expect(validateToken(req)).toBe('abc')",
      ].join("\n"),
      additions: 1,
      deletions: 1,
    },
    {
      type: "tool_call",
      id: "call_5",
      tool: "terminal",
      status: "success",
      input: { command: "npm test" },
      output: "Test Files  1 passed (1)\n     Tests  42 passed (42)",
      durationMs: 3421,
    },
    {
      type: "tool_result",
      toolCallId: "call_5",
      status: "success",
      output: "Test Files  1 passed (1)\n     Tests  42 passed (42)",
      durationMs: 3421,
    },
    {
      type: "text",
      content:
        "## Fixed\n\nThe authentication middleware was updated to read the token from the standard `Authorization` header.\n\n### Changes\n\n- Fixed token validation\n- Added missing error handling\n- Updated the test to use the new header\n\n**Tests:** 42 passed",
    },
  ],
};
