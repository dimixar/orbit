/**
 * Pi agent daemon — hosts the pi-coding-agent SDK in Node and exposes it
 * to the Tauri webview over a WebSocket (JSON messages).
 *
 * Run in dev:     pnpm agent
 * Bundle for prod: pnpm agent:build  (outputs a single-file sidecar)
 */
import path from "node:path";
import fs from "node:fs";
import {
  createAgentSession,
  ModelRuntime,
  SessionManager,
  type AgentSession,
  type ModelRuntime as ModelRuntimeType,
} from "@earendil-works/pi-coding-agent";
import { WebSocketServer, type WebSocket } from "ws";
import {
  getUsage,
  listPlugins,
  listSkills,
  PI_DIR,
  type PluginInfo,
  type SkillInfo,
  type UsageReport,
} from "./workbench.js";

const PORT = Number(process.env.PI_AGENT_PORT ?? 8912);

/** Bump when the wire protocol changes (new event fields, message types). */
const PROTOCOL = 3;

// ---------- JSON message protocol ----------

/** Messages received from the webview client */
type ThinkingLevel = Parameters<AgentSession["setThinkingLevel"]>[0];

type ClientMessage =
  | { type: "prompt"; text: string }
  | { type: "abort" }
  | { type: "get_state" }
  | { type: "list_sessions" }
  | { type: "open_session"; path: string }
  | { type: "set_model"; provider: string; modelId: string }
  | { type: "set_project"; cwd: string }
  | { type: "set_thinking_level"; level: ThinkingLevel }
  | { type: "get_usage" }
  | { type: "list_skills" }
  | { type: "list_plugins" }
  | { type: "get_settings_info" };

/** A model the user can pick, from the runtime's auth-checked catalog */
type ModelInfo = { provider: string; id: string; name: string; reasoning: boolean };

/** Messages sent to the webview client */
type DaemonMessage =
  | { type: "ready"; sessionId: string; cwd: string; protocol: number }
  | { type: "project"; cwd: string }
  | { type: "delta"; text: string }
  | { type: "thinking"; text: string }
  | { type: "tool_start"; toolCallId: string; toolName: string; args: unknown }
  | { type: "tool_end"; toolCallId: string; toolName: string; result: unknown; isError: boolean }
  | { type: "assistant_end"; text: string }
  | { type: "agent_end" }
  | {
      type: "state";
      model: string | undefined;
      thinkingLevel: string;
      thinkingLevels: string[];
      isStreaming: boolean;
    }
  | { type: "models"; models: ModelInfo[] }
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
  | { type: "session_opened"; path: string; messages: ChatTurn[] }
  | { type: "usage"; usage: UsageReport }
  | { type: "skills"; skills: SkillInfo[] }
  | { type: "plugins"; plugins: PluginInfo; }
  | { type: "settings_info"; settings: SettingsInfo }
  | { type: "error"; message: string };

// ---------- Workbench data types ----------

type SettingsInfo = {
  defaultModel: string | null;
  defaultProvider: string | null;
  defaultThinkingLevel: string | null;
  hasAuth: boolean;
  authProviders: string[];
  settingsPath: string;
};

/** A rendered chat turn: user text plus assistant text/thinking/tool parts. */
type ChatTurnPart =
  | { type: "text"; text: string }
  | { type: "thinking"; text: string; done?: boolean }
  | {
      type: "tool";
      toolName: string;
      toolCallId: string;
      input?: unknown;
      output?: unknown;
      isError?: boolean;
    };
type ChatTurn = { role: "user" | "assistant"; parts: ChatTurnPart[] };

// ---------- Agent session ----------

let session: AgentSession | undefined;
let modelRuntime: ModelRuntimeType | undefined;
let unsubscribeEvents: (() => void) | undefined;
let currentCwd = process.cwd();

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
    sessionManager: SessionManager.create(currentCwd),
    modelRuntime: await getModelRuntime(),
  });

  session = s;
  wireSessionEvents(s);
  return s;
}

/**
 * Read a session file back into plain chat turns. Sessions are append-only
 * JSONL: user/assistant "message" entries carry content arrays. Thinking and
 * tool calls are kept as first-class parts so history renders like the live
 * stream; tool results are folded back onto their tool call by id.
 * Malformed lines are skipped.
 *
 * The file is a tree: every entry links to its parent via `parentId`, and
 * edits, retries, and aborted runs leave abandoned branches behind. Reading
 * the file linearly replays every branch, so abandoned turns show up
 * duplicated and interleaved (text looks doubled and cut). Pi itself only
 * follows one chain, so we do the same: start from the last entry (the
 * active leaf) and walk `parentId` links back to the root, then replay that
 * chain in chronological order.
 */
async function readSessionMessages(filePath: string): Promise<ChatTurn[]> {
  const turns: ChatTurn[] = [];
  const pendingTools = new Map<
    string,
    Extract<ChatTurnPart, { type: "tool" }>
  >();
  let text = "";
  try {
    text = await fs.promises.readFile(filePath, "utf8");
  } catch {
    return turns;
  }

  // Parse every entry, preserving file order (the active leaf is the last
  // entry appended to the file).
  const byId = new Map<string, any>();
  const ordered: any[] = [];
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed.startsWith("{")) continue;
    let entry: any;
    try {
      entry = JSON.parse(trimmed);
    } catch {
      continue;
    }
    if (typeof entry?.id !== "string") continue;
    byId.set(entry.id, entry);
    ordered.push(entry);
  }

  // Walk the active chain leaf -> root, then reverse into chronological
  // order. Missing parents or cycles stop the walk rather than failing.
  const chain: any[] = [];
  const seen = new Set<string>();
  let cursor: any = ordered[ordered.length - 1];
  while (cursor && typeof cursor.id === "string" && !seen.has(cursor.id)) {
    seen.add(cursor.id);
    chain.unshift(cursor);
    const parentId: unknown = cursor.parentId;
    cursor = typeof parentId === "string" ? byId.get(parentId) : undefined;
  }

  for (const entry of chain) {
    if (entry.type !== "message") continue;
    const msg = entry.message;
    const role = msg?.role;
    const content = msg?.content;
    if (!Array.isArray(content)) continue;

    if (role === "user") {
      const body = content
        .filter((b: any) => b?.type === "text" && typeof b.text === "string")
        .map((b: any) => b.text)
        .join("\n")
        .trim();
      if (body) turns.push({ role, parts: [{ type: "text", text: body }] });
      continue;
    }

    if (role === "assistant") {
      const parts: ChatTurnPart[] = [];
      for (const block of content) {
        if (block?.type === "thinking" && typeof block.thinking === "string") {
          parts.push({ type: "thinking", text: block.thinking, done: true });
        } else if (
          block?.type === "text" &&
          typeof block.text === "string" &&
          block.text.trim()
        ) {
          parts.push({ type: "text", text: block.text });
        } else if (block?.type === "toolCall" && typeof block.id === "string") {
          const part: ChatTurnPart = {
            type: "tool",
            toolName: String(block.name ?? ""),
            toolCallId: block.id,
            input: block.arguments,
          };
          parts.push(part);
          pendingTools.set(block.id, part);
        }
      }
      if (parts.length > 0) turns.push({ role, parts });
      continue;
    }

    if (role === "toolResult" && typeof msg.toolCallId === "string") {
      const part = pendingTools.get(msg.toolCallId);
      if (!part) continue;
      const body = content
        .filter((b: any) => b?.type === "text" && typeof b.text === "string")
        .map((b: any) => b.text)
        .join("\n");
      part.output = body || undefined;
      part.isError = msg.isError === true;
      pendingTools.delete(msg.toolCallId);
    }
  }
  return turns;
}

/** Replace the active session with a previously saved one. */
async function openSession(filePath: string): Promise<void> {
  const messages = await readSessionMessages(filePath);
  const { session: s } = await createAgentSession({
    sessionManager: SessionManager.open(filePath),
    modelRuntime: await getModelRuntime(),
  });

  unsubscribeEvents?.();
  session = s;
  wireSessionEvents(s);
  broadcast({ type: "session_opened", path: filePath, messages });
  broadcast(stateMessage(s));
}

/** Replace the active session with a fresh one anchored at a new project folder. */
async function newProjectSession(cwd: string): Promise<void> {
  await session?.abort();
  const { session: s } = await createAgentSession({
    sessionManager: SessionManager.create(cwd),
    modelRuntime: await getModelRuntime(),
  });

  currentCwd = cwd;
  unsubscribeEvents?.();
  session = s;
  wireSessionEvents(s);
  broadcast({ type: "project", cwd });
  broadcast(stateMessage(s));
  broadcast({ type: "sessions", groups: await listSessionGroups() });
}

/** Snapshot of everything the composer chips display. */
function stateMessage(s: AgentSession): DaemonMessage {
  return {
    type: "state",
    model: s.model?.id,
    thinkingLevel: s.thinkingLevel,
    thinkingLevels: s.model ? (s.getAvailableThinkingLevels() as string[]) : [],
    isStreaming: s.isStreaming,
  };
}

/** Auth-checked list of pickable models, grouped by provider for the menu. */
async function listAvailableModels(): Promise<ModelInfo[]> {
  try {
    const rt = await getModelRuntime();
    const available = await rt.getAvailable();
    return available
      .map((m) => ({
        provider: String(m.provider),
        id: m.id,
        name: m.name,
        reasoning: m.reasoning,
      }))
      .sort(
        (a, b) =>
          a.provider.localeCompare(b.provider) || a.name.localeCompare(b.name),
      );
  } catch (err) {
    console.error("[pi-agent] failed to list available models:", err);
    return [];
  }
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

// ---------- Workbench aggregators (read-only) ----------

async function readJson<T>(file: string): Promise<T | null> {
  try {
    return JSON.parse(await fs.promises.readFile(file, "utf8")) as T;
  } catch {
    return null;
  }
}

async function getSettingsInfo(): Promise<SettingsInfo> {
  const settingsPath = path.join(PI_DIR, "settings.json");
  const settings =
    (await readJson<{
      defaultModel?: string;
      defaultProvider?: string;
      defaultThinkingLevel?: string;
    }>(settingsPath)) ?? {};
  // auth.json existence only — never read its contents.
  let hasAuth = false;
  let authProviders: string[] = [];
  try {
    const auth = await readJson<Record<string, unknown>>(path.join(PI_DIR, "auth.json"));
    if (auth && typeof auth === "object") {
      authProviders = Object.keys(auth).filter((k) => typeof auth[k] === "object");
      hasAuth = authProviders.length > 0;
    }
  } catch {
    // no auth file
  }
  return {
    defaultModel: settings.defaultModel ?? null,
    defaultProvider: settings.defaultProvider ?? null,
    defaultThinkingLevel: settings.defaultThinkingLevel ?? null,
    hasAuth,
    authProviders,
    settingsPath,
  };
}

/**
 * Usage aggregation lives in agent/workbench.ts (shared with the SSE server).
 */

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

  // Register the message handler synchronously — anything a client sends
  // while session warm-up is still running must not be silently dropped.
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
          send(ws, stateMessage(s));
          break;
        }
        case "set_model": {
          const s = await ensureSession();
          const rt = await getModelRuntime();
          const m = rt.getModel(msg.provider, msg.modelId);
          if (!m) throw new Error(`Unknown model: ${msg.provider}/${msg.modelId}`);
          await s.setModel(m);
          broadcast(stateMessage(s));
          break;
        }
        case "set_project": {
          await newProjectSession(msg.cwd);
          break;
        }
        case "set_thinking_level": {
          const s = await ensureSession();
          s.setThinkingLevel(msg.level);
          broadcast(stateMessage(s));
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
        case "get_usage": {
          send(ws, { type: "usage", usage: await getUsage() });
          break;
        }
        case "list_skills": {
          send(ws, { type: "skills", skills: await listSkills() });
          break;
        }
        case "list_plugins": {
          send(ws, { type: "plugins", plugins: await listPlugins() });
          break;
        }
        case "get_settings_info": {
          send(ws, { type: "settings_info", settings: await getSettingsInfo() });
          break;
        }
        default:
          send(ws, { type: "error", message: `Unknown message type` });
      }
    } catch (err) {
      send(ws, { type: "error", message: String(err) });
    }
  });

  try {
    const s = await ensureSession();
    send(ws, { type: "ready", sessionId: s.sessionId, cwd: currentCwd, protocol: PROTOCOL });
    send(ws, stateMessage(s));
    send(ws, { type: "sessions", groups: await listSessionGroups() });
    send(ws, { type: "models", models: await listAvailableModels() });
  } catch (err) {
    send(ws, { type: "error", message: `Failed to create agent session: ${err}` });
  }

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
        broadcast({
          type: "tool_start",
          toolCallId: event.toolCallId,
          toolName: event.toolName,
          args: event.args,
        });
        break;
      case "tool_execution_end":
        broadcast({
          type: "tool_end",
          toolCallId: event.toolCallId,
          toolName: event.toolName,
          result: event.result,
          isError: event.isError,
        });
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