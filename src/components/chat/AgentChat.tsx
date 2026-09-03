"use client";

/**
 * Agent chat container.
 *
 *   Agent store → ChatMessages → AgentMessage → AgentMessageParts
 *
 * Responsibilities:
 *   - subscribe to the active run
 *   - submit user prompts and start agent runs through the adapter
 *   - connect the event stream and feed events into the store
 *   - queue new prompts while the agent is processing, then run them in
 *     order as each run completes
 *   - display messages and handle approval callbacks
 *
 * The store is the single source of truth: every run in the store renders as
 * a user turn (from the run's prompt) followed by the assistant message the
 * run built. Queued prompts render as pending user messages with a "queued"
 * marker until their run starts.
 *
 * Backend communication lives in the adapter, never in individual UI
 * components. If no adapter is provided, a deterministic mock is used so the
 * UI works standalone.
 */

import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { cn } from "cn";
import type { AgentAdapter } from "@/lib/agent/adapters";
import { createMockAdapter } from "@/lib/agent/adapters/mockAdapter";
import { agentStore, useAgentStore } from "@/lib/agent/store";
import type { AgentMessage as AgentMessageType } from "@/lib/agent/types";
import { ChatMessages } from "./ChatMessages";
import { ChatInput } from "./ChatInput";
import "./chat.css";

export type AgentChatProps = {
  adapter?: AgentAdapter;
  onFileClick?: (path: string) => void;
  onApprove?: (id: string) => void;
  onReject?: (id: string) => void;
  className?: string;
};

type QueuedPrompt = {
  prompt: string;
  createdAt: string;
};

export const AgentChat = memo(function AgentChat({
  adapter,
  onFileClick,
  onApprove,
  onReject,
  className,
}: AgentChatProps) {
  const adapterRef = useRef<AgentAdapter | null>(null);
  if (!adapterRef.current) {
    adapterRef.current = adapter ?? createMockAdapter();
  }

  const [error, setError] = useState<string | null>(null);
  const [queue, setQueue] = useState<QueuedPrompt[]>([]);
  const [queueTick, setQueueTick] = useState(0);
  const unsubscribersRef = useRef<(() => void)[]>([]);
  // Guards the async start so the queue pump never launches two runs at once.
  const startingRef = useRef(false);

  const runs = useAgentStore((state) => state.runs);
  const activeRunId = useAgentStore((state) => state.activeRunId);

  const activeRun = activeRunId ? runs[activeRunId] : undefined;
  const isStreaming = activeRun?.status === "streaming";
  // Any run still streaming means the agent is busy (queue new prompts).
  const isBusy = useAgentStore((state) =>
    Object.values(state.runs).some((run) => run.status === "streaming"),
  );

  // Derive the conversation from every run in the store, oldest first, then
  // append queued prompts as pending user messages.
  const messages = useMemo<AgentMessageType[]>(() => {
    const sorted = Object.values(runs).sort((a, b) =>
      a.startedAt.localeCompare(b.startedAt),
    );
    const list: AgentMessageType[] = [];
    for (const run of sorted) {
      if (run.prompt) {
        list.push({
          id: `user-${run.id}`,
          role: "user",
          createdAt: run.startedAt,
          parts: [{ type: "text", content: run.prompt }],
        });
      }
      list.push(run.message);
    }
    for (const [i, item] of queue.entries()) {
      list.push({
        id: `queued-${i}`,
        role: "user",
        createdAt: item.createdAt,
        queued: true,
        parts: [{ type: "text", content: item.prompt }],
      });
    }
    return list;
  }, [runs, queue]);

  // Clean up subscriptions on unmount.
  useEffect(() => {
    const unsubs = unsubscribersRef.current;
    return () => {
      for (const off of unsubs) off();
    };
  }, []);

  const startRun = useCallback(async (prompt: string) => {
    startingRef.current = true;
    try {
      const runId = await adapterRef.current!.start(prompt);
      // Create the run immediately so the UI shows "Thinking…" without
      // waiting for the first event to arrive.
      agentStore.startRun({ runId, prompt });

      const unsubscribe = adapterRef.current!.subscribe(runId, (event) => {
        agentStore.processEvent(event);
      });
      unsubscribersRef.current.push(unsubscribe);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      // Re-pump the queue so the next queued prompt still gets a chance.
      setQueueTick((t) => t + 1);
    } finally {
      startingRef.current = false;
    }
  }, []);

  // Pump the queue: whenever the agent is idle and prompts are queued, start
  // the next one. `startingRef` prevents double-launching while `start()`
  // is still resolving.
  useEffect(() => {
    if (isBusy || startingRef.current) return;
    if (queue.length === 0) return;
    const [next, ...rest] = queue;
    setQueue(rest);
    void startRun(next.prompt);
  }, [isBusy, queue, startRun, queueTick]);

  const handleSend = useCallback(
    (prompt: string) => {
      setError(null);
      if (isBusy) {
        // Agent is working — queue the message; it runs when the current
        // run completes.
        setQueue((q) => [...q, { prompt, createdAt: new Date().toISOString() }]);
        return;
      }
      void startRun(prompt);
    },
    [isBusy, startRun],
  );

  const handleStop = useCallback(() => {
    if (!activeRunId) return;
    adapterRef.current?.abort?.(activeRunId);
    agentStore.appendText(activeRunId, "\n\n_Stopped by user._");
    agentStore.completeRun(activeRunId);
    // Queued messages stay queued — they run next.
  }, [activeRunId]);

  return (
    <div className={cn("flex min-h-0 flex-1 flex-col", className)}>
      <ChatMessages
        messages={messages}
        isStreaming={isStreaming}
        error={error}
        onFileClick={onFileClick}
        onApprove={onApprove}
        onReject={onReject}
      />
      <ChatInput
        onSend={handleSend}
        onStop={handleStop}
        isStreaming={isStreaming}
        queuedCount={queue.length}
      />
    </div>
  );
});
