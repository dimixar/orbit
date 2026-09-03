/**
 * Event → message reducer.
 *
 * Converts a stream of `AgentEvent`s into normalized `AgentMessage.parts[]`.
 * Pure and immutable: every transition returns a new `AgentState` and never
 * mutates the previous one.
 *
 * Correlation rules:
 *   - `tool.call`   → creates a `ToolCallPart`
 *   - `tool.result` → updates the matching `ToolCallPart` (by `id`) and
 *                     appends a `ToolResultPart` (by `toolCallId`)
 *   - `assistant.text` → appends to the trailing `TextPart` while streaming
 *   - `file.diff`   → creates/updates a `DiffPart` (by file)
 *   - `agent.plan`  → replaces the `PlanPart` steps
 *   - `agent.error` → appends an `ErrorPart` and fails the run
 */

import type {
  AgentMessage,
  AgentPart,
  AgentRun,
  AgentState,
  DiffPart,
  PlanPart,
  ReasoningPart,
  TextPart,
  ToolCallPart,
  ToolResultPart,
} from "./types";
import type { AgentEvent } from "./events";

export function createInitialState(): AgentState {
  return { runs: {}, activeRunId: undefined };
}

/* ------------------------------------------------------------------ */
/* Small immutable helpers                                             */
/* ------------------------------------------------------------------ */

function updateRun(
  state: AgentState,
  runId: string,
  updater: (run: AgentRun) => AgentRun,
): AgentState {
  const run = state.runs[runId];
  if (!run) return state;
  return {
    ...state,
    runs: { ...state.runs, [runId]: updater(run) },
  };
}

function updateMessage(
  run: AgentRun,
  updater: (message: AgentMessage) => AgentMessage,
): AgentRun {
  return { ...run, message: updater(run.message) };
}

function upsertPart(
  message: AgentMessage,
  part: AgentPart,
  match: (p: AgentPart) => boolean,
): AgentMessage {
  const index = message.parts.findIndex(match);
  if (index === -1) {
    return { ...message, parts: [...message.parts, part] };
  }
  const parts = [...message.parts];
  parts[index] = part;
  return { ...message, parts };
}

function lastPart(message: AgentMessage): AgentPart | undefined {
  return message.parts[message.parts.length - 1];
}

/* ------------------------------------------------------------------ */
/* Reducer                                                             */
/* ------------------------------------------------------------------ */

export function agentReducer(state: AgentState, event: AgentEvent): AgentState {
  switch (event.type) {
    case "agent.start":
      return reduceStart(state, event);
    case "agent.status":
      return reduceStatus(state, event);
    case "agent.plan":
      return reducePlan(state, event);
    case "agent.reasoning":
      return reduceReasoning(state, event);
    case "tool.call":
      return reduceToolCall(state, event);
    case "tool.result":
      return reduceToolResult(state, event);
    case "file.change":
      return reduceFileChange(state, event);
    case "file.diff":
      return reduceFileDiff(state, event);
    case "assistant.text":
      return reduceAssistantText(state, event);
    case "agent.error":
      return reduceError(state, event);
    case "agent.complete":
      return reduceComplete(state, event);
    case "tool.update":
      return reduceToolUpdate(state, event);
    case "tool.result.update":
      return reduceToolResultUpdate(state, event);
    default:
      return state;
  }
}

function reduceStart(
  state: AgentState,
  event: Extract<AgentEvent, { type: "agent.start" }>,
): AgentState {
  const existing = state.runs[event.runId];
  if (existing) {
    // A redelivered start event must not reset an in-progress run.
    return { ...state, activeRunId: event.runId };
  }
  const messageId = event.data.messageId ?? `msg_${event.runId}`;
  const run: AgentRun = {
    id: event.runId,
    status: "streaming",
    statusLabel: "planning",
    prompt: event.data.prompt,
    startedAt: event.timestamp,
    message: {
      id: messageId,
      role: "assistant",
      runId: event.runId,
      createdAt: event.timestamp,
      status: "streaming",
      parts: [],
    },
  };
  return {
    runs: { ...state.runs, [event.runId]: run },
    activeRunId: event.runId,
  };
}

function reduceStatus(
  state: AgentState,
  event: Extract<AgentEvent, { type: "agent.status" }>,
): AgentState {
  return updateRun(state, event.runId, (run) => ({
    ...run,
    statusLabel: event.data.status,
  }));
}

function reducePlan(
  state: AgentState,
  event: Extract<AgentEvent, { type: "agent.plan" }>,
): AgentState {
  return updateRun(state, event.runId, (run) =>
    updateMessage(run, (message) => {
      const plan: PlanPart = { type: "plan", steps: event.data.steps };
      return upsertPart(message, plan, (p) => p.type === "plan");
    }),
  );
}

function reduceReasoning(
  state: AgentState,
  event: Extract<AgentEvent, { type: "agent.reasoning" }>,
): AgentState {
  return updateRun(state, event.runId, (run) =>
    updateMessage(run, (message) => {
      const { content, status = "streaming", delta = false } = event.data;
      const existing = message.parts.find(
        (p): p is ReasoningPart => p.type === "reasoning",
      );
      const nextContent = existing && delta ? existing.content + content : content;
      const reasoning: ReasoningPart = {
        type: "reasoning",
        content: nextContent,
        status,
      };
      return upsertPart(message, reasoning, (p) => p.type === "reasoning");
    }),
  );
}

function reduceToolCall(
  state: AgentState,
  event: Extract<AgentEvent, { type: "tool.call" }>,
): AgentState {
  return updateRun(state, event.runId, (run) =>
    updateMessage(run, (message) => {
      const { id, tool, input, status = "running" } = event.data;
      const call: ToolCallPart = {
        type: "tool_call",
        id,
        tool,
        status,
        input: input ?? {},
        startedAt: event.timestamp,
      };
      // Never duplicate a tool call when additional streaming events arrive.
      return upsertPart(message, call, (p) => p.type === "tool_call" && p.id === id);
    }),
  );
}

function reduceToolResult(
  state: AgentState,
  event: Extract<AgentEvent, { type: "tool.result" }>,
): AgentState {
  return updateRun(state, event.runId, (run) =>
    updateMessage(run, (message) => {
      const { toolCallId, status, output, durationMs, error } = event.data;

      // 1. Update the matching ToolCallPart in place (merge, never replace —
      //    the original tool name and input must survive).
      const callIndex = message.parts.findIndex(
        (p) => p.type === "tool_call" && p.id === toolCallId,
      );
      let withCall = message;
      if (callIndex !== -1) {
        const parts = [...message.parts];
        const call = parts[callIndex] as ToolCallPart;
        parts[callIndex] = { ...call, status, output, durationMs };
        withCall = { ...message, parts };
      }

      // 2. Append a ToolResultPart (or update an existing one for the same id).
      const result: ToolResultPart = {
        type: "tool_result",
        toolCallId,
        status,
        output,
        durationMs,
        error,
      };
      return upsertPart(
        withCall,
        result,
        (p) => p.type === "tool_result" && p.toolCallId === toolCallId,
      );
    }),
  );
}

function reduceFileChange(
  state: AgentState,
  event: Extract<AgentEvent, { type: "file.change" }>,
): AgentState {
  return updateRun(state, event.runId, (run) =>
    updateMessage(run, (message) => ({
      ...message,
      parts: [
        ...message.parts,
        { type: "file", path: event.data.path, action: event.data.action },
      ],
    })),
  );
}

function reduceFileDiff(
  state: AgentState,
  event: Extract<AgentEvent, { type: "file.diff" }>,
): AgentState {
  return updateRun(state, event.runId, (run) =>
    updateMessage(run, (message) => {
      const diff: DiffPart = {
        type: "diff",
        file: event.data.file,
        patch: event.data.patch,
        additions: event.data.additions,
        deletions: event.data.deletions,
      };
      return upsertPart(message, diff, (p) => p.type === "diff" && p.file === diff.file);
    }),
  );
}

function reduceAssistantText(
  state: AgentState,
  event: Extract<AgentEvent, { type: "assistant.text" }>,
): AgentState {
  return updateRun(state, event.runId, (run) =>
    updateMessage(run, (message) => {
      const { content, delta = false } = event.data;
      const last = lastPart(message);
      if (last && last.type === "text") {
        const text: TextPart = {
          type: "text",
          content: delta ? last.content + content : content,
        };
        return upsertPart(message, text, (p) => p.type === "text" && p === last);
      }
      return { ...message, parts: [...message.parts, { type: "text", content }] };
    }),
  );
}

function reduceError(
  state: AgentState,
  event: Extract<AgentEvent, { type: "agent.error" }>,
): AgentState {
  return updateRun(state, event.runId, (run) => {
    const withError = updateMessage(run, (message) => ({
      ...message,
      parts: [
        ...message.parts,
        { type: "error", message: event.data.message, details: event.data.details },
      ],
    }));
    return {
      ...withError,
      status: "error",
      error: event.data.message,
      completedAt: event.timestamp,
      message: { ...withError.message, status: "error" },
    };
  });
}

function reduceComplete(
  state: AgentState,
  event: Extract<AgentEvent, { type: "agent.complete" }>,
): AgentState {
  return updateRun(state, event.runId, (run) => ({
    ...run,
    status: "complete",
    completedAt: event.timestamp,
    message: { ...run.message, status: "complete" },
  }));
}

function reduceToolUpdate(
  state: AgentState,
  event: Extract<AgentEvent, { type: "tool.update" }>,
): AgentState {
  return updateRun(state, event.runId, (run) =>
    updateMessage(run, (message) => {
      const { id, patch } = event.data;
      const index = message.parts.findIndex(
        (p) => p.type === "tool_call" && p.id === id,
      );
      if (index === -1) return message;
      const parts = [...message.parts];
      const call = parts[index] as ToolCallPart;
      parts[index] = { ...call, ...patch };
      return { ...message, parts };
    }),
  );
}

function reduceToolResultUpdate(
  state: AgentState,
  event: Extract<AgentEvent, { type: "tool.result.update" }>,
): AgentState {
  return updateRun(state, event.runId, (run) =>
    updateMessage(run, (message) => {
      const { toolCallId, patch } = event.data;
      const index = message.parts.findIndex(
        (p) => p.type === "tool_result" && p.toolCallId === toolCallId,
      );
      if (index === -1) return message;
      const parts = [...message.parts];
      const result = parts[index] as ToolResultPart;
      parts[index] = { ...result, ...patch };
      return { ...message, parts };
    }),
  );
}
