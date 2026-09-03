/**
 * Normalized agent event protocol.
 *
 * These events are produced by the agent execution layer (streamed over SSE,
 * WebSocket, or emitted by a mock) and consumed by the reducer in
 * `reducer.ts`. The wire format is intentionally transport-agnostic:
 *
 *   { id, runId, timestamp, type, data }
 *
 * The event layer knows nothing about React — it only describes what the
 * agent did. The reducer converts events into `AgentPart`s; components decide
 * how those parts are displayed.
 */

import type {
  FileAction,
  PlanStep,
  ToolStatus,
} from "./types";

export const AGENT_EVENT_TYPES = [
  "agent.start",
  "agent.status",
  "agent.plan",
  "agent.reasoning",
  "tool.call",
  "tool.result",
  "file.change",
  "file.diff",
  "assistant.text",
  "agent.error",
  "agent.complete",
  // Internal events — used by the store's imperative API, never sent over
  // the wire. Kept in the same union so the reducer has a single code path.
  "tool.update",
  "tool.result.update",
] as const;

export type AgentEventType = (typeof AGENT_EVENT_TYPES)[number];

export const WIRE_EVENT_TYPES = AGENT_EVENT_TYPES.filter(
  (t) => t !== "tool.update" && t !== "tool.result.update",
) as Exclude<AgentEventType, "tool.update" | "tool.result.update">[];

type AgentEventBase = {
  id: string;
  runId: string;
  timestamp: string;
};

export type AgentStartEvent = AgentEventBase & {
  type: "agent.start";
  data: { prompt?: string; messageId?: string };
};

export type AgentStatusEvent = AgentEventBase & {
  type: "agent.status";
  data: { status: string };
};

export type AgentPlanEvent = AgentEventBase & {
  type: "agent.plan";
  data: { steps: PlanStep[] };
};

export type AgentReasoningEvent = AgentEventBase & {
  type: "agent.reasoning";
  data: {
    content: string;
    status?: "streaming" | "complete";
    /** When true, `content` is appended to the existing reasoning text. */
    delta?: boolean;
  };
};

export type ToolCallEvent = AgentEventBase & {
  type: "tool.call";
  data: {
    id: string;
    tool: string;
    input?: unknown;
    status?: ToolStatus;
  };
};

export type ToolResultEvent = AgentEventBase & {
  type: "tool.result";
  data: {
    toolCallId: string;
    status: "success" | "error";
    output?: unknown;
    durationMs?: number;
    error?: string;
  };
};

export type FileChangeEvent = AgentEventBase & {
  type: "file.change";
  data: { path: string; action: FileAction };
};

export type FileDiffEvent = AgentEventBase & {
  type: "file.diff";
  data: {
    file: string;
    patch: string;
    additions?: number;
    deletions?: number;
  };
};

export type AssistantTextEvent = AgentEventBase & {
  type: "assistant.text";
  data: {
    content: string;
    /** When true, `content` is appended to the trailing text part. */
    delta?: boolean;
  };
};

export type AgentErrorEvent = AgentEventBase & {
  type: "agent.error";
  data: { message: string; details?: unknown };
};

export type AgentCompleteEvent = AgentEventBase & {
  type: "agent.complete";
  data?: { summary?: string };
};

/* ------------------------------------------------------------------ */
/* Internal events (store imperative API)                              */
/* ------------------------------------------------------------------ */

export type ToolUpdateEvent = AgentEventBase & {
  type: "tool.update";
  data: {
    id: string;
    patch: Partial<{
      status: ToolStatus;
      input: unknown;
      output: unknown;
      durationMs: number;
    }>;
  };
};

export type ToolResultUpdateEvent = AgentEventBase & {
  type: "tool.result.update";
  data: {
    toolCallId: string;
    patch: Partial<{
      status: "success" | "error";
      output: unknown;
      durationMs: number;
      error: string;
    }>;
  };
};

export type AgentEvent =
  | AgentStartEvent
  | AgentStatusEvent
  | AgentPlanEvent
  | AgentReasoningEvent
  | ToolCallEvent
  | ToolResultEvent
  | FileChangeEvent
  | FileDiffEvent
  | AssistantTextEvent
  | AgentErrorEvent
  | AgentCompleteEvent
  | ToolUpdateEvent
  | ToolResultUpdateEvent;

/* ------------------------------------------------------------------ */
/* Validation                                                          */
/* ------------------------------------------------------------------ */

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isString(value: unknown): value is string {
  return typeof value === "string";
}

/**
 * Validates an unknown payload (e.g. a parsed SSE line) against the event
 * envelope. Returns `null` when the payload is not a well-formed event.
 * Unknown event types are rejected so a mismatched backend can't corrupt
 * the store.
 */
export function parseAgentEvent(raw: unknown): AgentEvent | null {
  if (!isRecord(raw)) return null;
  const { id, runId, timestamp, type, data } = raw;
  if (!isString(id) || !isString(runId) || !isString(timestamp)) return null;
  if (!isString(type) || !(AGENT_EVENT_TYPES as readonly string[]).includes(type)) {
    return null;
  }
  if (data !== undefined && !isRecord(data)) return null;

  return {
    id,
    runId,
    timestamp,
    type: type as AgentEventType,
    data: (data ?? {}) as Record<string, unknown>,
  } as AgentEvent;
}

/** Builds a well-formed event envelope. */
export function createEvent<T extends AgentEvent["type"]>(
  type: T,
  runId: string,
  data: Extract<AgentEvent, { type: T }>["data"],
  id?: string,
): Extract<AgentEvent, { type: T }> {
  return {
    id: id ?? `${type}-${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
    runId,
    timestamp: new Date().toISOString(),
    type,
    data,
  } as Extract<AgentEvent, { type: T }>;
}
