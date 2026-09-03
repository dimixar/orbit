/**
 * Pi agent daemon — hosts the pi-coding-agent SDK in Node and exposes it
 * to the Tauri webview over a WebSocket (JSON messages).
 *
 * Run in dev:     pnpm agent
 * Bundle for prod: pnpm agent:build  (outputs a single-file sidecar)
 */
import path from "node:path";
import {
  createAgentSession,
  ModelRuntime,
  SessionManager,
  type AgentSession,
  type ModelRuntime as ModelRuntimeType,
} from "@earendil-works/pi-coding-agent";
import { WebSocketServer, type WebSocket } from "ws";

const PORT = Number(process.env.PI_AGENT_PORT ?? 8912);

// ---------- JSON message protocol ----------

/** Messages received from the webview client */
type ClientMessage =
  | { type: "prompt"; text: string }
  | { type: "abort" }
  | { type: "get_state" }
  | { type: "list_sessions" }
  | { type: "open_session"; path: string };

/** Messages sent to the webview client */
type DaemonMessage =
  | { type: "ready"; sessionId: string }
  | { type: "delta"; text: string }
  | { type: "thinking"; text: string }
  | { type: "tool_start"; toolName: string }
  | { type: "tool_end"; toolName: string; isError: boolean }
  | { type: "assistant_end"; text: string }
  | { type: "agent_end" }
  | { type: "state"; model: string | undefined; isStreaming: boolean }
  | {
      type: "sessions";
      groups: {
        project: string;
        cwd: string;
        sessions: {
          path: string;
          id: string;
          name?: string;
          firstMessage: string;
          messageCount: number;
          modified: string;
        }[];
      }[];
    }
  | { type: "session_opened"; path: string }
  | { type: "error"; message: string };

// ---------- Agent session ----------

let session: AgentSession | undefined;
let modelRuntime: ModelRuntimeType | undefined;
let unsubscribeEvents: (() => void) | undefined;

async function getModelRuntime() {
  modelRuntime ??= await ModelRuntime.create();
  return modelRuntime;
}

async function ensureSession(): Promise<AgentSession> {
  if (session) return session;

  const { session: s } = await createAgentSession({
    // Persistent session: written to ~/.pi/agent/sessions/<encoded-cwd>/.
    // Swap for SessionManager.inMemory() if you don't want persistence.
    // See https://pi.dev/docs/latest/sdk#session-management
    sessionManager: SessionManager.create(process.cwd()),
    modelRuntime: await getModelRuntime(),
  });

  session = s;
  wireSessionEvents(s);
  return s;
}

/** Replace the active session with a previously saved one. */
async function openSession(filePath: string): Promise<void> {
  const { session: s } = await createAgentSession({
    sessionManager: SessionManager.open(filePath),
    modelRuntime: await getModelRuntime(),
  });

  unsubscribeEvents?.();
  session = s;
  wireSessionEvents(s);
  broadcast({ type: "session_opened", path: filePath });
  broadcast({ type: "state", model: s.model?.id, isStreaming: s.isStreaming });
}

// ---------- Session listing (grouped by project) ----------

interface SessionSummary {
  path: string;
  id: string;
  name?: string;
  firstMessage: string;
  messageCount: number;
  modified: string;
}

async function listSessionGroups() {
  const infos = await SessionManager.listAll();
  const groups = new Map<
    string,
    { project: string; cwd: string; sessions: SessionSummary[] }
  >();

  for (const info of infos) {
    const project = info.cwd ? path.basename(info.cwd) : "unknown";
    const group = groups.get(info.cwd) ?? { project, cwd: info.cwd, sessions: [] };
    group.sessions.push({
      path: info.path,
      id: info.id,
      name: info.name,
      firstMessage: info.firstMessage,
      messageCount: info.messageCount,
      modified: info.modified.toISOString(),
    });
    groups.set(info.cwd, group);
  }

  // Most recent session first, within each group and across groups
  const list = [...groups.values()].map((g) => ({
    ...g,
    sessions: g.sessions.sort((a, b) => b.modified.localeCompare(a.modified)),
  }));
  list.sort((a, b) =>
    (b.sessions[0]?.modified ?? "").localeCompare(a.sessions[0]?.modified ?? ""),
  );
  return list;
}

// ---------- WebSocket server ----------

const wss = new WebSocketServer({ port: PORT });
const clients = new Set<WebSocket>();

function send(ws: WebSocket, msg: DaemonMessage) {
  if (ws.readyState === ws.OPEN) ws.send(JSON.stringify(msg));
}
function broadcast(msg: DaemonMessage) {
  for (const ws of clients) send(ws, msg);
}

wss.on("connection", async (ws) => {
  clients.add(ws);
  console.log(`[pi-agent] client connected (${clients.size} total)`);

  try {
    const s = await ensureSession();
    send(ws, { type: "ready", sessionId: s.sessionId });
    send(ws, { type: "state", model: s.model?.id, isStreaming: s.isStreaming });
    send(ws, { type: "sessions", groups: await listSessionGroups() });
  } catch (err) {
    send(ws, { type: "error", message: `Failed to create agent session: ${err}` });
  }

  ws.on("message", async (raw) => {
    let msg: ClientMessage;
    try {
      msg = JSON.parse(raw.toString()) as ClientMessage;
    } catch {
      send(ws, { type: "error", message: "Invalid JSON message" });
      return;
    }

    try {
      switch (msg.type) {
        case "prompt": {
          const s = await ensureSession();
          // Queue as follow-up if the agent is mid-stream
          await s.prompt(msg.text, s.isStreaming ? { streamingBehavior: "followUp" } : undefined);
          break;
        }
        case "abort": {
          await session?.abort();
          break;
        }
        case "get_state": {
          const s = await ensureSession();
          send(ws, { type: "state", model: s.model?.id, isStreaming: s.isStreaming });
          break;
        }
        case "list_sessions": {
          send(ws, { type: "sessions", groups: await listSessionGroups() });
          break;
        }
        case "open_session": {
          await openSession(msg.path);
          break;
        }
        default:
          send(ws, { type: "error", message: `Unknown message type` });
      }
    } catch (err) {
      send(ws, { type: "error", message: String(err) });
    }
  });

  ws.on("close", () => {
    clients.delete(ws);
    console.log(`[pi-agent] client disconnected (${clients.size} total)`);
  });
});

// Rebroadcast agent events for the active session to all clients.
// Re-invoked whenever the active session is replaced.
function wireSessionEvents(s: AgentSession) {
  unsubscribeEvents = s.subscribe((event) => {
    switch (event.type) {
      case "message_update": {
        const e = event.assistantMessageEvent;
        if (e.type === "text_delta") broadcast({ type: "delta", text: e.delta });
        if (e.type === "thinking_delta") broadcast({ type: "thinking", text: e.delta });
        break;
      }
      case "tool_execution_start":
        broadcast({ type: "tool_start", toolName: event.toolName });
        break;
      case "tool_execution_end":
        broadcast({ type: "tool_end", toolName: event.toolName, isError: event.isError });
        break;
      case "agent_end":
        broadcast({ type: "agent_end" });
        break;
    }
  });
}

ensureSession()
  .then(() => {
    console.log(`[pi-agent] listening on ws://localhost:${PORT}`);
    console.log(`[pi-agent] cwd: ${process.cwd()}`);
  })
  .catch((err) => {
    console.error("[pi-agent] failed to initialise agent session:", err);
    process.exit(1);
  });