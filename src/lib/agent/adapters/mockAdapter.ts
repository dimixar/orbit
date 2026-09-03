/**
 * Mock adapter — lets the entire UI run standalone against a deterministic
 * event stream. Swap in a real adapter (SSE/WebSocket) without touching the
 * chat components.
 */

import type { AgentAdapter } from "./index";
import { startMockRun, type MockRunController } from "../mockAgent";

export function createMockAdapter(options?: {
  interval?: number;
  textChunkSize?: number;
}): AgentAdapter {
  const controllers = new Map<string, MockRunController>();

  return {
    async start(_prompt, opts) {
      return opts?.runId ?? `run_${Date.now().toString(36)}`;
    },

    subscribe(runId, onEvent) {
      const controller = startMockRun({
        runId,
        interval: options?.interval,
        textChunkSize: options?.textChunkSize,
        onEvent,
      });
      controllers.set(runId, controller);
      return () => {
        controller.stop();
        controllers.delete(runId);
      };
    },

    abort(runId) {
      if (runId) {
        controllers.get(runId)?.stop();
        controllers.delete(runId);
      } else {
        for (const controller of controllers.values()) controller.stop();
        controllers.clear();
      }
    },
  };
}
