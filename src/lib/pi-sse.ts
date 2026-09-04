/**
 * SSE adapter — bridges the Pi × assistant-ui runtime (over SSE) to the
 * legacy chat-panel UI.
 *
 * The chat UI keeps its original look (header, composer, agent-elements
 * message list) but the data source is now the Pi runtime over SSE
 * (agent/sse-server.ts) instead of the WebSocket daemon.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  usePiRuntimeExtras,
  usePiThreadState,
  type PiModelInfo,
} from "@assistant-ui/react-pi";
import {
  fetchScopedModels,
  piClient,
  saveScopedModels,
} from "./pi-client";
import type { PiAgentMessage } from "@assistant-ui/react-pi";
import type { ChatMessage, ChatPart } from "@/lib/pi-agent";

/** Converts a Pi message into the legacy ChatMessage shape. */
function toChatMessage(message: PiAgentMessage): ChatMessage {
  const parts: ChatPart[] = [];
  const content =
    message.role === "user" || message.role === "assistant"
      ? (
          message as {
            content?: {
              type: string;
              text?: string;
              thinking?: string;
              name?: string;
              id?: string;
              arguments?: unknown;
            }[];
          }
        ).content
      : undefined;
  for (const item of content ?? []) {
    if (item.type === "text" && typeof item.text === "string") {
      parts.push({ type: "text", text: item.text });
    } else if (item.type === "thinking" && typeof item.thinking === "string") {
      parts.push({ type: "thinking", text: item.thinking, done: true });
    } else if (item.type === "toolCall" && typeof item.name === "string") {
      parts.push({
        type: "tool",
        toolName: item.name,
        toolCallId: item.id ?? "",
        input: item.arguments,
      });
    }
  }
  return {
    role: message.role === "user" ? "user" : "assistant",
    parts,
    // Epoch-ms creation time from pi — powers the per-message date/time in
    // the message list toolbar. Guarded: unknown roles may omit it.
    createdAt:
      typeof message.timestamp === "number" && message.timestamp > 0
        ? new Date(message.timestamp)
        : undefined,
  };
}

/** Converts a Pi toolResult message into a legacy tool part. */
function toToolResultPart(
  message: Extract<PiAgentMessage, { role: "toolResult" }>,
): Extract<ChatPart, { type: "tool" }> {
  const text = message.content
    .filter((c): c is { type: "text"; text: string } => c.type === "text")
    .map((c) => c.text)
    .join("\n");
  return {
    type: "tool",
    toolName: message.toolName,
    toolCallId: message.toolCallId,
    input: {},
    output: text,
    isError: message.isError,
  };
}

/**
 * Hook: fetches the available models from the SSE server.
 */
export function usePiModels(): PiModelInfo[] {
  const [models, setModels] = useState<PiModelInfo[]>([]);
  useEffect(() => {
    let cancelled = false;
    piClient
      .getAvailableModels()
      .then((list) => {
        if (!cancelled) setModels(list);
      })
      .catch(() => {
        // SSE server not running — leave the list empty.
      });
    return () => {
      cancelled = true;
    };
  }, []);
  return models;
}

/** Scoped-models state: the resolved ids (null = unscoped, every model
 * usable — pi treats that as “all enabled”), a toggle that persists through
 * the SSE server, and the last save outcome for status surfaces. `saving`
 * guards double-clicks on the checkboxes. */
export function useScopedModels(): {
  /** Scoped model ids as "provider/modelId", or null when unscoped. */
  scopedIds: string[] | null;
  /** Raw `enabledModels` patterns from settings.json — may be globs like
   * "anthropic/*:high" that match more than `scopedIds` resolves to. */
  patterns: string[] | null;
  saving: boolean;
  /** "saved" after a successful persist, "error" after a failed one (the
   * optimistic state has been reverted), null before the first save. */
  saveState: "saved" | "error" | null;
  /** Adds/removes one model from the scope and persists the new set. Passing
   * the full catalog lets an unscoped state (null) toggle off to “everything
   * except this one”, and re-enabling every model clears the scope entirely —
   * both matching pi CLI /scoped-models persist semantics. */
  toggleScoped: (id: string, allIds: string[]) => void;
  /** Clears the scope entirely (removes `enabledModels` from settings) —
   * every available model goes back to being enabled. */
  resetScope: () => void;
} {
  const [scopedIds, setScopedIds] = useState<string[] | null>(null);
  const [patterns, setPatterns] = useState<string[] | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveState, setSaveState] = useState<"saved" | "error" | null>(null);
  const scopedIdsRef = useRef<string[] | null>(null);
  useEffect(() => {
    scopedIdsRef.current = scopedIds;
  }, [scopedIds]);

  useEffect(() => {
    let cancelled = false;
    fetchScopedModels().then((state) => {
      if (!cancelled && state) {
        setScopedIds(state.ids);
        setPatterns(state.patterns);
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const toggleScoped = useCallback((id: string, allIds: string[]) => {
    // Unscoped means every model is enabled — toggling one off scopes to the
    // rest, matching the pi CLI selector.
    const current = scopedIdsRef.current ?? allIds;
    const next = current.includes(id)
      ? current.filter((x) => x !== id)
      : [...current, id];
    // Re-enabling every available model clears the scope (pi CLI: persisting
    // an all-enabled selection writes no patterns).
    const everythingEnabled =
      allIds.length > 0 && allIds.every((x) => next.includes(x));
    const persisted = everythingEnabled ? null : next;
    // Optimistic flip; the server reply is authoritative on resolve, and a
    // failure reverts to the previous set.
    scopedIdsRef.current = everythingEnabled ? null : next;
    setScopedIds(everythingEnabled ? null : next);
    setSaving(true);
    saveScopedModels(persisted)
      .then((state) => {
        scopedIdsRef.current = state.ids;
        setScopedIds(state.ids);
        setPatterns(state.patterns);
        setSaveState("saved");
      })
      .catch(() => {
        scopedIdsRef.current = current;
        setScopedIds(current);
        setSaveState("error");
      })
      .finally(() => setSaving(false));
  }, []);

  const resetScope = useCallback(() => {
    setSaving(true);
    saveScopedModels(null)
      .then((state) => {
        scopedIdsRef.current = state.ids;
        setScopedIds(state.ids);
        setPatterns(state.patterns);
        setSaveState("saved");
      })
      .catch(() => setSaveState("error"))
      .finally(() => setSaving(false));
  }, []);
  return { scopedIds, patterns, saving, saveState, toggleScoped, resetScope };
}

/**
 * Convert raw Pi messages into the legacy ChatMessage[] shape.
 *
 * Tool results are folded onto the trailing assistant message and merged into
 * the matching tool-call part (by toolCallId) so each tool call id appears
 * exactly once — pushing a second part with the same toolCallId would collide
 * as a duplicate React key in the message list.
 */
export function toChatMessages(
  messages: readonly PiAgentMessage[],
): ChatMessage[] {
  const list: ChatMessage[] = [];
  for (const message of messages) {
    if (message.role === "user" || message.role === "assistant") {
      list.push(toChatMessage(message));
    } else if (
      message.role === "toolResult" &&
      "toolCallId" in message &&
      "toolName" in message &&
      "content" in message &&
      "isError" in message
    ) {
      const last = list[list.length - 1];
      if (last && last.role === "assistant") {
        const result = toToolResultPart(
          message as Extract<PiAgentMessage, { role: "toolResult" }>,
        );
        const existing = last.parts.find(
          (p): p is Extract<ChatPart, { type: "tool" }> =>
            p.type === "tool" && p.toolCallId === result.toolCallId,
        );
        if (existing) {
          existing.output = result.output;
          existing.isError = result.isError;
        } else {
          last.parts.push(result);
        }
      }
    }
  }
  return list;
}

/**
 * Hook: exposes the Pi SSE runtime state in the legacy chat-panel shape.
 */
export function usePiSseChat() {
  const messages = usePiThreadState((s) => s.messages);
  const streamingIndex = usePiThreadState((s) => s.streamingMessageIndex);
  const { status, readiness, cancel, setModel, setThinkingLevel } =
    usePiRuntimeExtras();

  const chatMessages = useMemo<ChatMessage[]>(
    () => toChatMessages(messages),
    [messages],
  );

  const isStreaming = streamingIndex !== undefined || status === "running";
  // The SSE stream is up whenever the runtime reports a thread status
  // (idle/running) — readiness only reflects model selection.
  const connected = status === "idle" || status === "running";
  const model =
    readiness?.state === "ready" ? readiness.selection.modelId : undefined;
  const provider =
    readiness?.state === "ready" ? readiness.selection.provider : undefined;

  return {
    messages: chatMessages,
    isStreaming,
    connected,
    model,
    provider,
    cancel,
    setModel,
    setThinkingLevel,
  };
}
