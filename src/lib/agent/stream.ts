/**
 * Streaming transport for agent events.
 *
 * Transport-specific logic lives here — never inside React components.
 * SSE is implemented first; the `AgentEventTransport` interface is designed
 * so a WebSocket transport can be added later without touching the store or
 * the UI.
 */

import { parseAgentEvent, type AgentEvent } from "./events";

export interface AgentEventTransport {
  connect(): void;
  disconnect(): void;
  /** Registers an event listener; returns an unsubscribe function. */
  onEvent(callback: (event: AgentEvent) => void): () => void;
}

export type StreamOptions = {
  /** SSE endpoint URL. */
  url: string;
  runId: string;
  /** Optional headers (e.g. Authorization). */
  headers?: Record<string, string>;
  onOpen?: () => void;
  onError?: (error: Error) => void;
  onClose?: () => void;
};

/* ------------------------------------------------------------------ */
/* SSE parsing (pure, testable)                                        */
/* ------------------------------------------------------------------ */

/**
 * Parses a raw SSE chunk into well-formed events.
 *
 * Handles the common `data: {...}\n\n` framing, multi-line `data:` fields,
 * and ignores `event:`/`id:`/comments. Malformed or unknown events are
 * dropped so a bad line can't corrupt the store.
 */
export function parseSSEChunk(chunk: string): AgentEvent[] {
  const events: AgentEvent[] = [];
  const blocks = chunk.split(/\r?\n\r?\n/);

  for (const block of blocks) {
    const dataLines: string[] = [];
    for (const line of block.split(/\r?\n/)) {
      if (line.startsWith("data:")) {
        dataLines.push(line.slice(5).trimStart());
      } else if (line.startsWith(":")) {
        // SSE comment — ignore.
      }
    }
    if (dataLines.length === 0) continue;
    const payload = dataLines.join("\n");
    if (payload === "[DONE]") continue;
    try {
      const parsed = JSON.parse(payload);
      const event = parseAgentEvent(parsed);
      if (event) events.push(event);
    } catch {
      // Not JSON — skip this block.
    }
  }
  return events;
}

/* ------------------------------------------------------------------ */
/* SSE transport                                                       */
/* ------------------------------------------------------------------ */

export function connectAgentStream(options: StreamOptions): AgentEventTransport {
  const listeners = new Set<(event: AgentEvent) => void>();
  let controller: AbortController | null = null;

  const transport: AgentEventTransport = {
    onEvent(callback) {
      listeners.add(callback);
      return () => {
        listeners.delete(callback);
      };
    },

    connect() {
      if (controller) return;
      controller = new AbortController();
      const { signal } = controller;

      const url = new URL(options.url, window.location.origin);
      url.searchParams.set("runId", options.runId);

      const run = async () => {
        try {
          const response = await fetch(url, {
            headers: options.headers,
            signal,
          });
          if (!response.ok || !response.body) {
            throw new Error(`SSE request failed: ${response.status}`);
          }
          options.onOpen?.();

          const reader = response.body.getReader();
          const decoder = new TextDecoder();
          let buffer = "";

          while (true) {
            const { done, value } = await reader.read();
            if (done) break;
            buffer += decoder.decode(value, { stream: true });
            // Only parse complete blocks (terminated by a blank line).
            let boundary = buffer.indexOf("\n\n");
            while (boundary !== -1) {
              const block = buffer.slice(0, boundary);
              buffer = buffer.slice(boundary + 2);
              for (const event of parseSSEChunk(block)) {
                for (const listener of listeners) listener(event);
              }
              boundary = buffer.indexOf("\n\n");
            }
          }
          // Flush any trailing block.
          if (buffer.trim()) {
            for (const event of parseSSEChunk(buffer)) {
              for (const listener of listeners) listener(event);
            }
          }
          options.onClose?.();
        } catch (error) {
          if (signal.aborted) return;
          options.onError?.(error instanceof Error ? error : new Error(String(error)));
        }
      };

      void run();
    },

    disconnect() {
      controller?.abort();
      controller = null;
    },
  };

  return transport;
}

/* ------------------------------------------------------------------ */
/* WebSocket transport (ready for later backends)                      */
/* ------------------------------------------------------------------ */

export type WebSocketStreamOptions = StreamOptions & {
  url: string;
  protocols?: string | string[];
};

/**
 * WebSocket transport. The backend is expected to send one JSON-encoded
 * `AgentEvent` per message. Kept alongside SSE so switching transports is a
 * one-line change in the adapter.
 */
export function connectAgentWebSocket(
  options: WebSocketStreamOptions,
): AgentEventTransport {
  const listeners = new Set<(event: AgentEvent) => void>();
  let socket: WebSocket | null = null;

  const transport: AgentEventTransport = {
    onEvent(callback) {
      listeners.add(callback);
      return () => {
        listeners.delete(callback);
      };
    },

    connect() {
      if (socket) return;
      const ws = new WebSocket(options.url, options.protocols);
      socket = ws;

      ws.onopen = () => options.onOpen?.();
      ws.onmessage = (message) => {
        try {
          const event = parseAgentEvent(JSON.parse(String(message.data)));
          if (event) for (const listener of listeners) listener(event);
        } catch {
          // Ignore malformed frames.
        }
      };
      ws.onerror = () => options.onError?.(new Error("WebSocket error"));
      ws.onclose = () => {
        socket = null;
        options.onClose?.();
      };
    },

    disconnect() {
      socket?.close();
      socket = null;
    },
  };

  return transport;
}
