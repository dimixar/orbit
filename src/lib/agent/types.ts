/**
 * Core data model for the coding-agent chat system.
 *
 * Every meaningful agent action is represented as a typed `AgentPart` instead
 * of being encoded into a single Markdown string. The renderer switches on
 * `part.type` and delegates to a dedicated component per part.
 *
 * Adding a new part type later (e.g. `search_result`, `browser_preview`,
 * `sql_query`, `sub_agent`, `command`) requires:
 *   1. one new type here,
 *   2. one reducer mapping in `reducer.ts`,
 *   3. one renderer in `components/chat/parts/` + a case in `AgentMessageParts`.
 */

export type AgentRunStatus = "streaming" | "complete" | "error";

export type ToolStatus = "running" | "success" | "error";

export type PlanStepStatus = "pending" | "running" | "complete" | "error";

export type ApprovalStatus = "pending" | "approved" | "rejected";

export type FileAction = "created" | "modified" | "deleted" | "read";

export type AgentRole = "user" | "assistant";

/** A single message in the conversation (user or assistant). */
export type AgentMessage = {
  id: string;
  role: AgentRole;
  runId?: string;
  createdAt: string;
  status?: AgentRunStatus;
  /** True for a user message that is queued behind a running agent. */
  queued?: boolean;
  parts: AgentPart[];
};

/** Discriminated union of every structured message part. */
export type AgentPart =
  | TextPart
  | ReasoningPart
  | PlanPart
  | ToolCallPart
  | ToolResultPart
  | CodePart
  | DiffPart
  | FilePart
  | ErrorPart
  | ApprovalPart;

/** Normal assistant prose, rendered as Markdown. */
export type TextPart = {
  type: "text";
  content: string;
};

/** Agent status/progress summary shown as secondary, collapsible content. */
export type ReasoningPart = {
  type: "reasoning";
  content: string;
  status: "streaming" | "complete";
};

/** The agent's execution plan. Steps update in place as events arrive. */
export type PlanPart = {
  type: "plan";
  steps: PlanStep[];
};

export type PlanStep = {
  id: string;
  title: string;
  status: PlanStepStatus;
};

/** A tool invocation. Correlated with `ToolResultPart` via `id`/`toolCallId`. */
export type ToolCallPart = {
  type: "tool_call";
  id: string;
  tool: string;
  status: ToolStatus;
  input: unknown;
  output?: unknown;
  startedAt?: string;
  durationMs?: number;
};

/** The result of a tool call, correlated via `toolCallId`. */
export type ToolResultPart = {
  type: "tool_result";
  toolCallId: string;
  status: "success" | "error";
  output: unknown;
  durationMs?: number;
  error?: string;
};

/** Standalone code returned as structured agent data. */
export type CodePart = {
  type: "code";
  language: string;
  code: string;
  filename?: string;
};

/** A unified diff for a single file. */
export type DiffPart = {
  type: "diff";
  file: string;
  patch: string;
  additions?: number;
  deletions?: number;
};

/** A filesystem operation (created / modified / deleted / read). */
export type FilePart = {
  type: "file";
  path: string;
  action: FileAction;
};

/** An agent or tool error. Never breaks the message renderer. */
export type ErrorPart = {
  type: "error";
  message: string;
  details?: unknown;
};

/** A human-approval gate. UI updates immediately on approve/reject. */
export type ApprovalPart = {
  type: "approval";
  id: string;
  title: string;
  description?: string;
  status: ApprovalStatus;
};

/** A run is the unit of agent execution; it builds one assistant message. */
export type AgentRun = {
  id: string;
  status: AgentRunStatus;
  /** Free-form phase label surfaced by the UI, e.g. "planning", "working". */
  statusLabel?: string;
  /** The user prompt that started this run (used to render the user turn). */
  prompt?: string;
  /** The assistant message being built by this run. */
  message: AgentMessage;
  startedAt: string;
  completedAt?: string;
  error?: string;
};

/** Global agent state: all runs keyed by id, plus the active one. */
export type AgentState = {
  runs: Record<string, AgentRun>;
  activeRunId?: string;
};

export function isAgentPart(part: unknown): part is AgentPart {
  if (typeof part !== "object" || part === null) return false;
  const type = (part as { type?: unknown }).type;
  return (
    typeof type === "string" &&
    [
      "text",
      "reasoning",
      "plan",
      "tool_call",
      "tool_result",
      "code",
      "diff",
      "file",
      "error",
      "approval",
    ].includes(type)
  );
}
