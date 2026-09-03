/**
 * Deterministic mock agent.
 *
 * Emits a realistic coding-agent event sequence with delays between events
 * so the entire UI can be developed and verified independently of the real
 * agent backend. The default sequence mirrors the "definition of done" flow:
 *
 *   agent.start → agent.plan → reasoning → read_file → edit_file + diff →
 *   terminal (npm test) → assistant.text (streamed) → agent.complete
 */

import { createEvent, type AgentEvent } from "./events";
import type { PlanStep } from "./types";

export type MockAgentOptions = {
  runId?: string;
  prompt?: string;
  /** Milliseconds between events (default 450). */
  interval?: number;
  /** Override the default event sequence. */
  events?: AgentEvent[];
  /** Approximate characters per `assistant.text` delta (default 40). */
  textChunkSize?: number;
  onEvent: (event: AgentEvent) => void;
  onComplete?: () => void;
  signal?: AbortSignal;
};

export type MockRunController = {
  stop: () => void;
  /** True once every event has been emitted. */
  finished: () => boolean;
};

/* ------------------------------------------------------------------ */
/* Default sequence                                                    */
/* ------------------------------------------------------------------ */

/** Splits text into chunks for streaming `assistant.text` delta events. */
export function chunkText(text: string, chunkSize = 40): string[] {
  if (chunkSize <= 0) return [text];
  const chunks: string[] = [];
  let rest = text;
  while (rest.length > chunkSize) {
    // Prefer breaking at a word boundary near the chunk size.
    let cut = rest.lastIndexOf(" ", chunkSize);
    if (cut < chunkSize * 0.5) cut = chunkSize;
    chunks.push(rest.slice(0, cut));
    rest = rest.slice(cut).trimStart();
  }
  if (rest) chunks.push(rest);
  return chunks;
}

export function buildCodingAgentRunEvents(
  prompt = "Fix the authentication bug",
  runId = "run_mock",
  textChunkSize = 40,
): AgentEvent[] {
  let seq = 0;
  const ev = (type: AgentEvent["type"], data: unknown): AgentEvent =>
    createEvent(type, runId, data as never, `mock-${runId}-${++seq}`);

  const planSteps: PlanStep[] = [
    { id: "1", title: "Inspect auth middleware", status: "pending" },
    { id: "2", title: "Update token validation", status: "pending" },
    { id: "3", title: "Run tests", status: "pending" },
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

  const finalText =
    "## Fixed\n\nThe authentication middleware was updated to read the token from the standard `Authorization` header.\n\n### Changes\n\n- Fixed token validation\n- Added missing error handling\n- Added tests\n\n**Tests:** 42 passed";

  return [
    ev("agent.start", { prompt, messageId: `msg_${runId}` }),
    ev("agent.plan", { steps: planSteps }),
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
    // Plan progresses: step 1 done, step 2 running.
    ev("agent.plan", {
      steps: [step("1", "complete"), step("2", "running"), step("3", "pending")],
    }),

    ev("tool.call", {
      id: "call_2",
      tool: "edit_file",
      input: { path: "src/auth.ts", description: "Use Authorization header" },
    }),
    ev("file.diff", {
      file: "src/auth.ts",
      patch: diffPatch,
      additions: 12,
      deletions: 4,
    }),
    ev("tool.result", {
      toolCallId: "call_2",
      status: "success",
      output: "Applied 1 edit to src/auth.ts",
      durationMs: 512,
    }),
    // Plan progresses: step 2 done, step 3 running.
    ev("agent.plan", {
      steps: [step("1", "complete"), step("2", "complete"), step("3", "running")],
    }),

    ev("tool.call", { id: "call_3", tool: "terminal", input: { command: "npm test" } }),
    ev("tool.result", {
      toolCallId: "call_3",
      status: "success",
      output: "Test Files  1 passed (1)\n     Tests  42 passed (42)",
      durationMs: 3421,
    }),
    // Plan completes.
    ev("agent.plan", {
      steps: [step("1", "complete"), step("2", "complete"), step("3", "complete")],
    }),

    // Final response streams in chunks, like the CLI.
    ...chunkText(finalText, textChunkSize).map((chunk) =>
      ev("assistant.text", { content: chunk, delta: true }),
    ),
    ev("agent.complete", { summary: "Fixed the authentication bug" }),
  ];
}

/* ------------------------------------------------------------------ */
/* Runner                                                              */
/* ------------------------------------------------------------------ */

export function startMockRun(options: MockAgentOptions): MockRunController {
  const {
    runId = "run_mock",
    prompt,
    interval = 450,
    textChunkSize = 40,
    onEvent,
    onComplete,
    signal,
  } = options;

  const events =
    options.events ?? buildCodingAgentRunEvents(prompt, runId, textChunkSize);
  let index = 0;
  let stopped = false;
  let timer: ReturnType<typeof setTimeout> | null = null;

  const emitNext = () => {
    if (stopped) return;
    if (index >= events.length) {
      onComplete?.();
      return;
    }
    onEvent(events[index]!);
    index += 1;
    timer = setTimeout(emitNext, interval);
  };

  const onAbort = () => {
    stopped = true;
    if (timer) clearTimeout(timer);
  };
  signal?.addEventListener("abort", onAbort, { once: true });

  // First event fires immediately so the UI reacts without a visible delay.
  timer = setTimeout(emitNext, 0);

  return {
    stop() {
      stopped = true;
      if (timer) clearTimeout(timer);
      signal?.removeEventListener("abort", onAbort);
    },
    finished: () => index >= events.length,
  };
}
