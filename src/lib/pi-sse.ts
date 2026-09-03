/**
 * SSE adapter — bridges the Pi × assistant-ui runtime (over SSE) to the
 * legacy chat-panel UI.
 *
 * The chat UI keeps its original look (header, composer, agent-elements
 * message list) but the data source is now the Pi runtime over SSE
 * (agent/sse-server.ts) instead of the WebSocket daemon.
 */

import { useEffect, useMemo, useState } from "react";
import { usePiRuntimeExtras, usePiThreadState, type PiModelInfo } from "@assistant-ui/react-pi";
import { piClient } from "./pi-client";
import type { PiAgentMessage } from "@assistant-ui/react-pi";
import type { ChatMessage, ChatPart } from "@/lib/pi-agent";

/** Converts a Pi message into the legacy ChatMessage shape. */
function toChatMessage(message: PiAgentMessage): ChatMessage {
  const parts: ChatPart[] = [];
  const content =
    message.role === "user" || message.role === "assistant"
      ? (message as { content?: { type: string; text?: string; thinking?: string; name?: string; id?: string; arguments?: unknown }[] }).content
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
  return { role: message.role === "user" ? "user" : "assistant", parts };
}

/** Converts a Pi toolResult message into a legacy tool part. */
function toToolResultPart(message: Extract<PiAgentMessage, { role: "toolResult" }>): ChatPart {
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

/**
 * Hook: exposes the Pi SSE runtime state in the legacy chat-panel shape.
 */
export function usePiSseChat() {
  const messages = usePiThreadState((s) => s.messages);
  const streamingIndex = usePiThreadState((s) => s.streamingMessageIndex);
  const { status, readiness, cancel, setModel, setThinkingLevel } =
    usePiRuntimeExtras();

  const chatMessages = useMemo<ChatMessage[]>(() => {
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
        // Fold tool results onto the trailing assistant message.
        const last = list[list.length - 1];
        if (last && last.role === "assistant") {
          last.parts.push(toToolResultPart(message as Extract<PiAgentMessage, { role: "toolResult" }>));
        }
      }
    }
    return list;
  }, [messages]);

  const isStreaming = streamingIndex !== undefined || status === "running";
  // The SSE stream is up whenever the runtime reports a thread status
  // (idle/running) — readiness only reflects model selection.
  const connected = status === "idle" || status === "running";
  const model = readiness?.state === "ready"
    ? readiness.selection.modelId
    : undefined;
  const provider = readiness?.state === "ready"
    ? readiness.selection.provider
    : undefined;

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
