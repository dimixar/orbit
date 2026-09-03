"use client";

/**
 * Standalone demo of the structured coding-agent chat.
 *
 * Uses the deterministic mock adapter so the full event → parts → render
 * pipeline can be exercised without a real agent backend. Also offers a
 * "Load fixture" button that streams the complete example run (including a
 * failed tool) with delays, like a live run.
 */

import { useCallback, useState } from "react";
import { createMockAdapter } from "@/lib/agent/adapters/mockAdapter";
import { agentStore } from "@/lib/agent/store";
import { startMockRun } from "@/lib/agent/mockAgent";
import { codingAgentRunEvents } from "@/lib/agent/fixtures/codingAgentRun";
import { AgentChat } from "./AgentChat";

const mockAdapter = createMockAdapter({ interval: 400, textChunkSize: 30 });

export function AgentChatDemo() {
  const [fixtureLoaded, setFixtureLoaded] = useState(false);

  const loadFixture = useCallback(() => {
    agentStore.clearAll();
    // Stream the fixture with delays so it behaves like a live run.
    startMockRun({
      runId: codingAgentRunEvents[0]!.runId,
      events: codingAgentRunEvents,
      interval: 250,
      onEvent: (event) => agentStore.processEvent(event),
    });
    setFixtureLoaded(true);
  }, []);

  return (
    <div className="flex min-h-0 flex-1 flex-col bg-bg">
      <header className="flex h-12 shrink-0 items-center justify-between border-b border-border px-4">
        <div className="flex items-center gap-2">
          <h1 className="text-[13px] font-medium tracking-[-0.01em] text-fg">
            Structured Agent Chat
          </h1>
          <span className="rounded-sm bg-primary-subtle px-1.5 py-px text-[10px] font-medium text-primary-subtle-fg">
            demo
          </span>
        </div>
        <button
          type="button"
          onClick={loadFixture}
          className="cursor-pointer rounded-md border border-border px-2.5 py-1 text-xs text-muted-fg transition-colors hover:bg-muted hover:text-fg"
        >
          {fixtureLoaded ? "Fixture loaded" : "Load fixture"}
        </button>
      </header>

      <AgentChat adapter={mockAdapter} className="flex min-h-0 flex-1 flex-col" />
    </div>
  );
}
