/**
 * Browser-side client for the Pi agent daemon (agent/index.ts).
 *
 * The pi-coding-agent SDK is Node-only, so it runs in a sidecar process
 * and streams events to the webview over WebSocket. This module wraps
 * that connection behind a tiny event-emitter style API.
 *
 * Start the daemon in dev with:  pnpm agent
 */

export type PiAgentState = {
  connected: boolean;
  sessionId?: string;
  model?: string;
  isStreaming: boolean;
};

export type PiSessionSummary = {
  path: string;
  id: string;
  name?: string;
  firstMessage: string;
  messageCount: number;
  modified: string; // ISO date
};

export type PiSessionGroup = {
  project: string;
  cwd: string;
  sessions: PiSessionSummary[];
};

export type PiAgentEvents = {
  ready: (msg: { sessionId: string }) => void;
  delta: (msg: { text: string }) => void;
  thinking: (msg: { text: string }) => void;
  tool_start: (msg: { toolName: string }) => void;
  tool_end: (msg: { toolName: string; isError: boolean }) => void;
  agent_end: (msg: Record<string, never>) => void;
  state: (msg: { model?: string; isStreaming: boolean }) => void;
  sessions: (msg: { groups: PiSessionGroup[] }) => void;
  session_opened: (msg: { path: string }) => void;
  error: (msg: { message: string }) => void;
  status: (connected: boolean) => void;
};

type Handler<K extends keyof PiAgentEvents> = (payload: Parameters<PiAgentEvents[K]>[0]) => void;

const WS_URL = `ws://localhost:${import.meta.env.PI_AGENT_PORT ?? 8912}`;

export class PiAgentClient {
  private ws: WebSocket | null = null;
  private handlers = new Map<string, Set<Handler<never>>>();
  private reconnectTimer: number | null = null;
  private closed = false;
  public state: PiAgentState = { connected: false, isStreaming: false };

  on<K extends keyof PiAgentEvents>(event: K, handler: Handler<K>): () => void {
    const set = this.handlers.get(event) ?? new Set();
    set.add(handler as Handler<never>);
    this.handlers.set(event, set);
    return () => set.delete(handler as Handler<never>);
  }

  private emit<K extends keyof PiAgentEvents>(event: K, payload: Parameters<PiAgentEvents[K]>[0]) {
    for (const h of this.handlers.get(event) ?? []) (h as Handler<K>)(payload);
  }

  connect() {
    // A socket is already connecting/open — nothing to do.
    if (this.ws && (this.ws.readyState === WebSocket.OPEN || this.ws.readyState === WebSocket.CONNECTING)) return;
    // `disconnect()` latches `closed` to stop the reconnect loop after an
    // intentional close (e.g. StrictMode's first cleanup). A fresh `connect()`
    // — such as the re-mount — clears the latch and dials out again.
    this.closed = false;
    const ws = new WebSocket(WS_URL);
    this.ws = ws;

    ws.onopen = () => {
      this.state = { ...this.state, connected: true };
      this.emit("status", true);
    };

    ws.onmessage = (e) => {
      let msg: { type: string } & Record<string, unknown>;
      try {
        msg = JSON.parse(e.data) as { type: string } & Record<string, unknown>;
      } catch {
        return;
      }
      if (msg.type === "ready") {
        this.state = { ...this.state, sessionId: msg.sessionId as string };
        this.emit("ready", { sessionId: msg.sessionId as string });
      } else if (msg.type === "state") {
        this.state = {
          ...this.state,
          model: msg.model as string | undefined,
          isStreaming: msg.isStreaming as boolean,
        };
        this.emit("state", { model: msg.model as string | undefined, isStreaming: Boolean(msg.isStreaming) });
      } else if (
        msg.type in {
          delta: 1,
          thinking: 1,
          tool_start: 1,
          tool_end: 1,
          agent_end: 1,
          sessions: 1,
          session_opened: 1,
          error: 1,
        }
      ) {
        if (msg.type === "delta" || msg.type === "thinking") {
          this.state = { ...this.state, isStreaming: true };
        }
        if (msg.type === "agent_end") {
          this.state = { ...this.state, isStreaming: false };
        }
        this.emit(msg.type as keyof PiAgentEvents, msg as unknown as never);
      }
    };

    ws.onclose = () => {
      this.ws = null;
      this.state = { ...this.state, connected: false, isStreaming: false };
      this.emit("status", false);
      // Retry every 2s (daemon not started yet, or restarted)
      if (!this.closed) {
        this.reconnectTimer = window.setTimeout(() => this.connect(), 2000);
      }
    };

    ws.onerror = () => ws.close();
  }

  disconnect() {
    this.closed = true;
    if (this.reconnectTimer) window.clearTimeout(this.reconnectTimer);
    this.ws?.close();
    this.ws = null;
  }

  private send(msg: Record<string, unknown>) {
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(msg));
  }

  /** Send a prompt to the agent. Results arrive via delta/agent_end events. */
  prompt(text: string) {
    this.send({ type: "prompt", text });
  }

  /** Abort the current agent run. */
  abort() {
    this.send({ type: "abort" });
  }

  /** Ask the daemon for current model/streaming state (arrives via `state`). */
  requestState() {
    this.send({ type: "get_state" });
  }

  /** Ask the daemon for all saved sessions, grouped by project (arrives via `sessions`). */
  requestSessions() {
    this.send({ type: "list_sessions" });
  }

  /** Switch the daemon's active session to a previously saved one. */
  openSession(path: string) {
    this.send({ type: "open_session", path });
  }
}

// Convenience singleton
export const piAgent = new PiAgentClient();