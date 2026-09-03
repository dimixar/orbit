/**
 * Agent state store.
 *
 * A tiny vanilla store (no external dependency) exposed to React through
 * `useSyncExternalStore`. It supports multiple simultaneous runs without
 * mixing their events: every event carries a `runId` and the reducer routes
 * it to the matching run.
 *
 * The imperative API (`startRun`, `appendText`, `completeToolCall`, …) is a
 * convenience layer that constructs events and dispatches them through the
 * same reducer used by the streaming transport — one code path, fully
 * testable, and safe to call from React event handlers.
 */

import { useSyncExternalStore } from "react";
import { createEvent, type AgentEvent } from "./events";
import { agentReducer, createInitialState } from "./reducer";
import type {
  AgentState,
  AgentRunStatus,
  DiffPart,
  PlanStep,
  ToolStatus,
} from "./types";

type Listener = () => void;

let nextRunId = 0;

export function generateRunId(prefix = "run"): string {
  nextRunId += 1;
  return `${prefix}_${Date.now().toString(36)}_${nextRunId}`;
}

class AgentStore {
  private state: AgentState = createInitialState();
  private listeners = new Set<Listener>();

  getState = (): AgentState => this.state;

  subscribe = (listener: Listener): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  private setState(updater: (state: AgentState) => AgentState): void {
    const next = updater(this.state);
    if (next === this.state) return;
    this.state = next;
    for (const listener of this.listeners) listener();
  }

  private dispatch(event: AgentEvent): void {
    this.setState((state) => agentReducer(state, event));
  }

  /* ------------------------------------------------------------------ */
  /* Run lifecycle                                                       */
  /* ------------------------------------------------------------------ */

  /** Creates a new run and returns its id. */
  startRun(options?: { runId?: string; prompt?: string; messageId?: string }): string {
    const runId = options?.runId ?? generateRunId();
    this.dispatch(
      createEvent("agent.start", runId, {
        prompt: options?.prompt,
        messageId: options?.messageId,
      }),
    );
    return runId;
  }

  /** Feeds a single event from the transport into the store. */
  processEvent(event: AgentEvent): void {
    this.dispatch(event);
  }

  /** Feeds a batch of events (e.g. replayed history) in order. */
  processEvents(events: AgentEvent[]): void {
    for (const event of events) this.dispatch(event);
  }

  completeRun(runId: string): void {
    this.dispatch(createEvent("agent.complete", runId, {}));
  }

  failRun(runId: string, message: string, details?: unknown): void {
    this.dispatch(createEvent("agent.error", runId, { message, details }));
  }

  clearRun(runId: string): void {
    this.setState((state) => {
      const runs = { ...state.runs };
      delete runs[runId];
      return {
        runs,
        activeRunId: state.activeRunId === runId ? undefined : state.activeRunId,
      };
    });
  }

  clearAll(): void {
    this.setState(() => createInitialState());
  }

  /* ------------------------------------------------------------------ */
  /* Part-level imperative updates                                       */
  /* ------------------------------------------------------------------ */

  /** Appends (or replaces) assistant text on the run's message. */
  appendText(runId: string, content: string, options?: { delta?: boolean }): void {
    this.dispatch(
      createEvent("assistant.text", runId, {
        content,
        delta: options?.delta ?? true,
      }),
    );
  }

  /** Patches an existing tool call part in place. */
  updateToolCall(
    runId: string,
    id: string,
    patch: Partial<{
      status: ToolStatus;
      input: unknown;
      output: unknown;
      durationMs: number;
    }>,
  ): void {
    this.dispatch(createEvent("tool.update", runId, { id, patch }));
  }

  /** Marks a tool call complete and appends/updates its result part. */
  completeToolCall(
    runId: string,
    toolCallId: string,
    result: {
      status?: "success" | "error";
      output?: unknown;
      durationMs?: number;
      error?: string;
    },
  ): void {
    this.dispatch(
      createEvent("tool.result", runId, {
        toolCallId,
        status: result.status ?? "success",
        output: result.output,
        durationMs: result.durationMs,
        error: result.error,
      }),
    );
  }

  /** Patches an existing tool result part in place. */
  updateToolResult(
    runId: string,
    toolCallId: string,
    patch: Partial<{
      status: "success" | "error";
      output: unknown;
      durationMs: number;
      error: string;
    }>,
  ): void {
    this.dispatch(createEvent("tool.result.update", runId, { toolCallId, patch }));
  }

  /** Replaces the run's plan steps. */
  updatePlan(runId: string, steps: PlanStep[]): void {
    this.dispatch(createEvent("agent.plan", runId, { steps }));
  }

  /** Creates or updates a diff part for a file. */
  addDiff(runId: string, diff: Omit<DiffPart, "type">): void {
    this.dispatch(createEvent("file.diff", runId, diff));
  }

  /** Sets the run's status label (e.g. "planning", "working"). */
  setStatus(runId: string, status: string): void {
    this.dispatch(createEvent("agent.status", runId, { status }));
  }

  /** Convenience: mark a run's message status without a full event. */
  setRunStatus(runId: string, status: AgentRunStatus): void {
    this.setState((state) => {
      const run = state.runs[runId];
      if (!run) return state;
      return {
        ...state,
        runs: {
          ...state.runs,
          [runId]: {
            ...run,
            status,
            message: { ...run.message, status },
          },
        },
      };
    });
  }
}

/** Singleton store used by the chat UI. */
export const agentStore = new AgentStore();

/** React binding: subscribe to a slice of agent state. */
export function useAgentStore<T>(selector: (state: AgentState) => T): T {
  return useSyncExternalStore(
    agentStore.subscribe,
    () => selector(agentStore.getState()),
    () => selector(agentStore.getState()),
  );
}
