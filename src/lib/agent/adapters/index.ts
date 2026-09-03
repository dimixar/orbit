/**
 * Adapter boundary between the actual coding agent and the frontend
 * protocol.
 *
 * The chat UI only knows about `AgentAdapter`. If the backend changes from
 * one coding agent framework to another (or from SSE to WebSocket), only the
 * adapter implementation needs to change — the store, reducer, and React
 * components stay untouched.
 */

import type { AgentEvent } from "../events";
import type { AgentEventTransport } from "../stream";

export interface AgentAdapter {
  /** Starts a run for the given prompt; resolves with the run id. */
  start(prompt: string, options?: { runId?: string }): Promise<string>;

  /**
   * Subscribes to events for a run. Returns an unsubscribe function.
   * Implementations typically wrap an `AgentEventTransport`.
   */
  subscribe(
    runId: string,
    onEvent: (event: AgentEvent) => void,
  ): () => void;

  /** Aborts the active run, if any. */
  abort?(runId?: string): void;
}

/** Wraps an `AgentEventTransport` (SSE/WebSocket) as an `AgentAdapter`. */
export function createTransportAdapter(
  transportFactory: (runId: string) => AgentEventTransport,
): AgentAdapter {
  const transports = new Map<string, AgentEventTransport>();

  return {
    async start(prompt, options) {
      const runId = options?.runId ?? `run_${Date.now().toString(36)}`;
      // The transport is created lazily on first subscribe so the run id is
      // available to the URL/query string.
      void prompt;
      return runId;
    },

    subscribe(runId, onEvent) {
      let transport = transports.get(runId);
      if (!transport) {
        transport = transportFactory(runId);
        transports.set(runId, transport);
        transport.connect();
      }
      const unsubscribe = transport.onEvent(onEvent);
      return () => {
        unsubscribe();
        const t = transports.get(runId);
        if (t) {
          t.disconnect();
          transports.delete(runId);
        }
      };
    },

    abort(runId) {
      if (runId) {
        transports.get(runId)?.disconnect();
        transports.delete(runId);
      } else {
        for (const transport of transports.values()) transport.disconnect();
        transports.clear();
      }
    },
  };
}
