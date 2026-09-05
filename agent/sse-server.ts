/**
 * Pi SSE server — exposes the `@assistant-ui/react-pi` PiClient contract over
 * HTTP + SSE, backed by `createPiNodeClient` (which drives the Pi SDK
 * in-process via a process-singleton `PiThreadSupervisor`).
 *
 * This is the server half of the assistant-ui integration:
 *
 *   browser (webview)                    this server (Node)
 *   ─────────────────                    ──────────────────
 *   createPiHttpClient  ──HTTP/SSE──▶    createPiNodeClient
 *   usePiRuntime                          └ PiThreadSupervisor → Pi SDK
 *
 * Run in dev:  pnpm agent:sse
 *
 * NOTE: this process drives the Pi SDK on `PI_WORKSPACE_PATH` (default: the
 * project dir). Don't run it at the same time as the WebSocket daemon
 * (`pnpm agent`) on the same workspace — two processes must not manage the
 * same session files. Use a different `PI_WORKSPACE_PATH` if you need both.
 */
import {
  createServer,
  type IncomingMessage,
  type ServerResponse,
} from "node:http";
import { execFile, execFileSync } from "node:child_process";
import { promisify } from "node:util";
import { existsSync, promises as fs } from "node:fs";
import { homedir } from "node:os";
import { isAbsolute, join, relative, sep } from "node:path";
import {
  createPiNodeClient,
  getPiThreadSupervisor,
} from "@assistant-ui/react-pi/node";
import type { PiModelInfo, PiThinkingLevel } from "@assistant-ui/react-pi";
import {
  DefaultResourceLoader,
  ModelRuntime,
  SessionManager,
  SettingsManager,
  getAgentDir,
  resolveModelScopeWithDiagnostics,
  type SessionInfo,
} from "@earendil-works/pi-coding-agent";
import { getUsage, listPlugins, listSkills } from "./workbench.js";
import {
  includeSnapshotFromUrl,
  isRunSettledEvent,
  liveStatusFromRecords,
  shouldIncludeSnapshot,
  writeSseEvent,
} from "./sse-stream.js";

const execFileAsync = promisify(execFile);

const PORT = Number(process.env.PI_SSE_PORT ?? 8913);
const WORKSPACE_PATH = process.env.PI_WORKSPACE_PATH ?? process.cwd();
const SERVER_ID = "orbit-pi-sse";
const STARTED_AT = new Date().toISOString();
/** react-pi pins the supervisor on `globalThis` under this key. */
const SUPERVISOR_KEY = "__assistantUiPiThreadSupervisor";

let reloadedAt: string | null = null;
let client = createPiNodeClient({ workspacePath: WORKSPACE_PATH });
let supervisor = getPiThreadSupervisor({
  workspacePath: WORKSPACE_PATH,
}) as PiSupervisorInternals;

type RuntimeModel = {
  provider: string;
  id: string;
  name?: string;
  reasoning?: unknown;
  thinkingLevelMap?: Partial<Record<PiThinkingLevel, unknown>>;
};

type LivePiRecord = {
  threadId?: string;
  seq?: number;
  session?: {
    sessionId?: string;
    isStreaming?: boolean;
    isCompacting?: boolean;
    isRetrying?: boolean;
    setScopedModels?: (models: unknown[]) => void;
  };
};

type PiSupervisorInternals = {
  getModelRuntime?: () => Promise<ModelRuntime>;
  records?: Map<string, LivePiRecord>;
  dispose?: () => Promise<void>;
};

type ScopedModelState = {
  patterns: string[] | null;
  ids: string[] | null;
};

const THINKING_LEVELS: PiThinkingLevel[] = [
  "off",
  "minimal",
  "low",
  "medium",
  "high",
  "xhigh",
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const CORS_HEADERS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, POST, PUT, PATCH, DELETE, OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type, Authorization",
};

const NO_STORE_HEADERS = {
  "Cache-Control": "no-store, max-age=0",
  Pragma: "no-cache",
};

function readBody(req: IncomingMessage): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on("data", (chunk: Buffer) => chunks.push(chunk));
    req.on("end", () => {
      if (chunks.length === 0) return resolve({});
      try {
        resolve(JSON.parse(Buffer.concat(chunks).toString("utf8")));
      } catch (error) {
        reject(error);
      }
    });
    req.on("error", reject);
  });
}

function sendJson(res: ServerResponse, status: number, body: unknown): void {
  res.writeHead(status, {
    "Content-Type": "application/json",
    ...NO_STORE_HEADERS,
    ...CORS_HEADERS,
  });
  res.end(JSON.stringify(body));
}

function sendNoContent(res: ServerResponse): void {
  res.writeHead(204, CORS_HEADERS);
  res.end();
}

function sendError(res: ServerResponse, error: unknown): void {
  const message = error instanceof Error ? error.message : String(error);
  const status =
    error instanceof RequestError && Number.isFinite(error.status)
      ? error.status
      : 500;
  console.error("[pi-sse] error:", message);
  sendJson(res, status, { error: message });
}

/** An error carrying the HTTP status the client should see (4xx = their side). */
class RequestError extends Error {
  status: number;
  constructor(message: string, status = 400) {
    super(message);
    this.status = status;
  }
}

/** Wraps a route handler, catching errors into a 500. */
function route(
  handler: (
    req: IncomingMessage,
    res: ServerResponse,
    body: unknown,
  ) => Promise<void> | void,
) {
  return async (req: IncomingMessage, res: ServerResponse, body: unknown) => {
    try {
      await handler(req, res, body);
    } catch (error) {
      sendError(res, error);
    }
  };
}

function mapRuntimeModel(model: RuntimeModel): PiModelInfo {
  const availableThinkingLevels = model.thinkingLevelMap
    ? THINKING_LEVELS.filter((level) => model.thinkingLevelMap?.[level] !== null)
    : undefined;
  return {
    provider: String(model.provider),
    modelId: model.id,
    ...(model.name ? { name: model.name } : {}),
    supportsThinking: Boolean(model.reasoning),
    ...(availableThinkingLevels ? { availableThinkingLevels } : {}),
  };
}

async function refreshExtensionProviders(
  runtime: ModelRuntime,
  cwd = WORKSPACE_PATH,
): Promise<void> {
  try {
    const agentDir = getAgentDir();
    const settingsManager = SettingsManager.create(cwd, agentDir);
    const loader = new DefaultResourceLoader({
      cwd,
      agentDir,
      settingsManager,
    });
    await loader.reload();
    const registrations = loader.getExtensions().runtime;
    const nextProviderIds = new Set<string>();

    for (const { name, config } of registrations.pendingProviderRegistrations) {
      nextProviderIds.add(name);
      try {
        runtime.registerProvider(name, config);
      } catch {
        // Keep the previous catalog usable if one extension is broken.
      }
    }
    registrations.pendingProviderRegistrations.length = 0;

    for (const { provider } of registrations.pendingNativeProviderRegistrations) {
      nextProviderIds.add(provider.id);
      try {
        runtime.registerNativeProvider(provider);
      } catch {
        // Keep the previous catalog usable if one extension is broken.
      }
    }
    registrations.pendingNativeProviderRegistrations.length = 0;

    for (const providerId of runtime.getRegisteredProviderIds()) {
      if (nextProviderIds.has(providerId)) continue;
      try {
        runtime.unregisterProvider(providerId);
      } catch {
        // Best-effort cleanup; a later refresh will self-heal.
      }
    }
  } catch {
    // Model listing should still work with the installed runtime's base catalog.
  }
}

async function getFreshModelRuntime(cwd = WORKSPACE_PATH): Promise<ModelRuntime> {
  if (!supervisor.getModelRuntime) {
    throw new Error("Pi supervisor does not expose a model runtime");
  }
  const runtime = await supervisor.getModelRuntime();
  await refreshExtensionProviders(runtime, cwd);
  return runtime;
}

async function listAvailableModels(cwd = WORKSPACE_PATH): Promise<PiModelInfo[]> {
  const runtime = await getFreshModelRuntime(cwd);
  try {
    await runtime.refresh();
  } catch {
    // Fall back to the last known snapshot/static registry below.
  }
  const available = runtime.getAvailableSnapshot();
  const models = available.length > 0 ? available : runtime.getModels();
  return models.map((model) => mapRuntimeModel(model as RuntimeModel));
}

async function resolveScopedModels(
  patterns: string[] | null,
  cwd = WORKSPACE_PATH,
): Promise<ScopedModelState & { scopedModels: unknown[] }> {
  if (!patterns || patterns.length === 0) {
    return { patterns: null, ids: null, scopedModels: [] };
  }
  const runtime = await getFreshModelRuntime(cwd);
  try {
    await runtime.refresh();
  } catch {
    // Same fallback as listAvailableModels — keep the last snapshot.
  }
  const scoped = await resolveModelScopeWithDiagnostics(patterns, runtime);

  // Pi's pattern parser treats the last ':' as a thinking-level suffix
  // (`provider/model:high`). Extension ids like `hf:moonshotai/Kimi-K3`
  // therefore fail to resolve even when they are in the catalog. Re-add
  // any pattern that is an exact `provider/modelId` from the same list
  // the Scoped models page renders.
  const catalog = new Map<string, RuntimeModel>();
  const snapshot = runtime.getAvailableSnapshot();
  const registered = runtime.getModels();
  for (const model of [...registered, ...snapshot] as RuntimeModel[]) {
    catalog.set(`${model.provider}/${model.id}`.toLowerCase(), model);
  }

  const seen = new Set<string>();
  const ids: string[] = [];
  const scopedModels: unknown[] = [];
  const add = (
    model: { provider: string; id: string },
    entry?: unknown,
  ) => {
    const key = `${model.provider}/${model.id}`;
    const norm = key.toLowerCase();
    if (seen.has(norm)) return;
    seen.add(norm);
    ids.push(key);
    scopedModels.push(entry ?? { model, thinkingLevel: undefined });
  };

  for (const item of scoped.scopedModels) {
    add(item.model, item);
  }
  for (const pattern of patterns) {
    const model = catalog.get(pattern.toLowerCase());
    if (model) add(model);
  }

  return { patterns, ids, scopedModels };
}

function applyScopedModelsToLiveSessions(scopedModels: unknown[]): void {
  for (const record of supervisor.records?.values() ?? []) {
    try {
      record.session?.setScopedModels?.(scopedModels);
    } catch {
      // Best effort: cold sessions pick up settings when the SDK opens them.
    }
  }
}

async function getScopedModels(): Promise<ScopedModelState> {
  const settings = SettingsManager.create(WORKSPACE_PATH, getAgentDir());
  const patterns = settings.getEnabledModels();
  const { scopedModels, ...state } = await resolveScopedModels(patterns ?? null);
  applyScopedModelsToLiveSessions(scopedModels);
  return state;
}

async function setScopedModels(
  patterns: string[] | null,
): Promise<ScopedModelState> {
  const effective = patterns && patterns.length > 0 ? [...patterns] : null;
  const settings = SettingsManager.create(WORKSPACE_PATH, getAgentDir());
  settings.setEnabledModels(effective ?? undefined);
  const { scopedModels, ...state } = await resolveScopedModels(effective);
  applyScopedModelsToLiveSessions(scopedModels);
  return state;
}

// ---------------------------------------------------------------------------
// Providers — pi's models.json: the custom-provider layer shared with the CLI
// ---------------------------------------------------------------------------

const PROVIDER_ID_PATTERN = /^[a-z0-9][a-z0-9._-]*$/i;

/** API families pi's runtime knows how to speak (the models.json `api` field). */
const PROVIDER_APIS = [
  "openai-completions",
  "openai-responses",
  "anthropic-messages",
  "google-generative-ai",
  "amazon-bedrock",
  "azure-openai-responses",
] as const;

type ProviderModelConfig = {
  id?: unknown;
  name?: unknown;
  contextWindow?: unknown;
  maxTokens?: unknown;
};

type ProviderFileEntry = Record<string, unknown>;

type ProviderModelOut = {
  id: string;
  name?: string;
  contextWindow?: number;
};

type ProviderOut = {
  id: string;
  name?: string;
  baseUrl: string;
  api: string;
  hasApiKey: boolean;
  models: ProviderModelOut[];
  /** True when a pi built-in with the same id serves the models. */
  matchesBuiltin?: boolean;
};

type SupportedProviderOut = {
  id: string;
  name: string;
  baseUrl?: string;
  api?: string;
  modelCount: number;
  /** First N catalog models, for prefilling the add form. */
  models: ProviderModelOut[];
};

type ProvidersResponse = {
  modelsJsonPath: string;
  /** models.json exists but is broken — surfaced instead of silently hidden. */
  parseError?: string;
  custom: ProviderOut[];
  catalog: { provider: string; modelCount: number; isCustom: boolean }[];
  catalogError?: string;
  /** Every provider pi supports — the only source the add form offers. */
  supported: SupportedProviderOut[];
};

function modelsJsonPath(): string {
  try {
    // pi keeps models.json in the agent dir (~/.pi/agent/models.json).
    return join(getAgentDir(), "models.json");
  } catch {
    return join(homedir(), ".pi", "agent", "models.json");
  }
}

/** Read models.json, tolerating a missing file. Throws on malformed JSON —
 *  a broken file must never be silently overwritten by a provider save. */
async function readModelsJson(): Promise<Record<string, unknown>> {
  try {
    const raw = await fs.readFile(modelsJsonPath(), "utf8");
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new RequestError(
        "models.json must contain a JSON object — fix or remove the file first",
      );
    }
    return parsed as Record<string, unknown>;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return {};
    throw error;
  }
}

async function writeModelsJson(config: Record<string, unknown>): Promise<void> {
  await fs.writeFile(
    modelsJsonPath(),
    `${JSON.stringify(config, null, 2)}\n`,
    "utf8",
  );
}

function normalizeProviderEntry(id: string, entry: ProviderFileEntry): ProviderOut {
  const models = Array.isArray(entry.models)
    ? (entry.models as ProviderModelConfig[])
        .filter((m) => m && typeof m.id === "string" && m.id.trim())
        .map((m) => ({
          id: String(m.id),
          ...(typeof m.name === "string" ? { name: m.name } : {}),
          ...(typeof m.contextWindow === "number"
            ? { contextWindow: m.contextWindow }
            : {}),
        }))
    : [];
  return {
    id,
    ...(typeof entry.name === "string" ? { name: entry.name } : {}),
    baseUrl: typeof entry.baseUrl === "string" ? entry.baseUrl : "",
    api: typeof entry.api === "string" ? entry.api : "",
    hasApiKey: typeof entry.apiKey === "string" && entry.apiKey.length > 0,
    models,
  };
}

/** Live catalog summary: which providers are serving models right now. */
function summarizeCatalog(
  runtime: ModelRuntime,
  customIds: ReadonlySet<string>,
): { provider: string; modelCount: number; isCustom: boolean }[] {
  const snapshot = runtime.getAvailableSnapshot();
  const models = (
    snapshot.length > 0 ? snapshot : runtime.getModels()
  ) as readonly RuntimeModel[];
  const counts = new Map<string, number>();
  for (const model of models) {
    counts.set(model.provider, (counts.get(model.provider) ?? 0) + 1);
  }
  return [...counts.entries()]
    .map(([provider, modelCount]) => ({
      provider,
      modelCount,
      isCustom: customIds.has(provider),
    }))
    .sort((a, b) => a.provider.localeCompare(b.provider));
}

/**
 * The providers pi itself supports — the only list the add form offers.
 * Providers without models (the "radius" OAuth hub) are not addable entries.
 */
function getSupportedProviders(
  runtime: ModelRuntime,
): SupportedProviderOut[] {
  const out: SupportedProviderOut[] = [];
  for (const provider of runtime.getProviders()) {
    const models = provider.getModels() as unknown as {
      id: string;
      name?: string;
      api?: string;
      contextWindow?: number;
    }[];
    if (models.length === 0) continue;
    out.push({
      id: provider.id,
      name: provider.name || provider.id,
      ...(provider.baseUrl ? { baseUrl: provider.baseUrl } : {}),
      ...(models[0]?.api ? { api: String(models[0].api) } : {}),
      modelCount: models.length,
      models: models.slice(0, 30).map((m) => ({
        id: m.id,
        ...(m.name ? { name: m.name } : {}),
        ...(typeof m.contextWindow === "number"
          ? { contextWindow: m.contextWindow }
          : {}),
      })),
    });
  }
  return out.sort((a, b) => a.name.localeCompare(b.name));
}

async function listProviders(): Promise<ProvidersResponse> {
  let config: Record<string, unknown>;
  let parseError: string | undefined;
  try {
    config = await readModelsJson();
  } catch (error) {
    parseError = error instanceof Error ? error.message : String(error);
    config = {};
  }
  const providers = (config.providers ?? {}) as Record<string, ProviderFileEntry>;
  const custom = Object.entries(providers)
    .map(([id, entry]) => normalizeProviderEntry(id, entry ?? {}))
    .sort((a, b) => a.id.localeCompare(b.id));

  let catalog: ProvidersResponse["catalog"] = [];
  let catalogError: string | undefined;
  let supported: SupportedProviderOut[] = [];
  try {
    const runtime = await getFreshModelRuntime();
    try {
      await runtime.refresh();
    } catch {
      // Same fallback as listAvailableModels — last snapshot is still truthy.
    }
    supported = getSupportedProviders(runtime);
    const supportedIds = new Set(supported.map((p) => p.id));
    for (const entry of custom) {
      if (supportedIds.has(entry.id)) entry.matchesBuiltin = true;
    }
    catalog = summarizeCatalog(
      runtime,
      new Set(custom.map((p) => p.id)),
    );
  } catch (error) {
    catalogError = error instanceof Error ? error.message : String(error);
  }

  return {
    modelsJsonPath: modelsJsonPath(),
    ...(parseError ? { parseError } : {}),
    custom,
    catalog,
    ...(catalogError ? { catalogError } : {}),
    supported,
  };
}

type ProviderSaveInput = {
  name?: string;
  baseUrl: string;
  api: string;
  /** undefined = keep the stored key; empty string = remove it. */
  apiKey?: string;
  models: { id: string; name?: string; contextWindow?: number }[];
};

function validateProviderInput(
  id: string,
  body: unknown,
  allowedApis: ReadonlySet<string>,
  knownProviderIds: ReadonlySet<string>,
): ProviderSaveInput {
  const input = (body ?? {}) as Record<string, unknown>;
  const baseUrl = typeof input.baseUrl === "string" ? input.baseUrl.trim() : "";
  if (!/^https?:\/\//.test(baseUrl)) {
    throw new RequestError("Base URL must start with http:// or https://");
  }
  const api = typeof input.api === "string" ? input.api : "";
  if (!allowedApis.has(api)) {
    throw new RequestError(
      `Unknown API type "${api}" — pi supports: ${[...allowedApis].sort().join(", ")}`,
    );
  }
  const rawModels = Array.isArray(input.models) ? input.models : [];
  const models = rawModels
    .map((m) => (m ?? {}) as Record<string, unknown>)
    .filter((m) => typeof m.id === "string" && m.id.trim())
    .map((m) => ({
      id: (m.id as string).trim(),
      ...(typeof m.name === "string" && m.name.trim()
        ? { name: m.name.trim() }
        : {}),
      ...(typeof m.contextWindow === "number"
          && Number.isFinite(m.contextWindow)
          && m.contextWindow > 0
        ? { contextWindow: Math.floor(m.contextWindow) }
        : {}),
    }));
  // A provider pi already supports may carry just a key/base URL — its
  // built-in catalog serves the models. Unknown providers need explicit models.
  if (models.length === 0 && !knownProviderIds.has(id)) {
    throw new RequestError(
      `At least one model id is required — "${id}" is not a provider pi already supports`,
    );
  }
  const seen = new Set<string>();
  for (const model of models) {
    const key = model.id.toLowerCase();
    if (seen.has(key)) {
      throw new RequestError(`Duplicate model id "${model.id}"`);
    }
    seen.add(key);
  }
  return {
    ...(typeof input.name === "string" && input.name.trim()
      ? { name: input.name.trim() }
      : {}),
    baseUrl,
    api,
    ...(typeof input.apiKey === "string" ? { apiKey: input.apiKey } : {}),
    models,
  };
}

/** New providers only reach sessions after a catalog refresh; best-effort so
 *  the save still lands even if the runtime is mid-something. */
async function refreshCatalog(): Promise<void> {
  try {
    const runtime = await getFreshModelRuntime();
    await runtime.refresh();
  } catch {
    // A later refresh picks the change up; listProviders still reports truth.
  }
}

/**
 * Recreate the process-singleton supervisor so live sessions and ModelRuntime
 * re-read models.json / settings.json / auth. The HTTP listener stays up.
 */
async function reloadPiRuntime(): Promise<void> {
  try {
    await supervisor.dispose?.();
  } catch {
    // A failed dispose must not keep the stale singleton around.
  }
  delete (globalThis as Record<string, unknown>)[SUPERVISOR_KEY];
  client = createPiNodeClient({ workspacePath: WORKSPACE_PATH });
  supervisor = getPiThreadSupervisor({
    workspacePath: WORKSPACE_PATH,
  }) as PiSupervisorInternals;
  reloadedAt = new Date().toISOString();
  await refreshCatalog();
}

// ---------------------------------------------------------------------------
// SSE stream
// ---------------------------------------------------------------------------

async function writeSettledSnapshot(
  res: ServerResponse,
  threadId: string,
): Promise<void> {
  if (res.writableEnded) return;
  try {
    const snapshot = await client.getThread(threadId);
    if (res.writableEnded) return;
    // Stamp after the supervisor's last live seq so the HTTP client does not
    // treat this as a reconnect reset (seq 0 < liveSnapshotSeq).
    const liveSeq = supervisor.records?.get(threadId)?.seq;
    writeSseEvent(res, {
      type: "snapshot",
      snapshot,
      threadId,
      seq: (typeof liveSeq === "number" ? liveSeq : 0) + 1,
    });
  } catch {
    // The live event stream already delivered what it could.
  }
}

function streamEvents(
  req: IncomingMessage,
  res: ServerResponse,
  threadId: string,
  url: URL,
): void {
  // Official SDK: subscribe first, then prompt. react-pi often opens with
  // `?snapshot=false` because it already called getThread / sendMessage.
  // Honor that while a run is live so a reconnect snapshot does not wipe
  // the streaming pointer. Still snapshot idle/unknown threads so a late
  // subscriber receives session.messages (the documented source of truth).
  const includeSnapshot = shouldIncludeSnapshot(
    includeSnapshotFromUrl(url),
    liveStatusFromRecords(threadId, supervisor.records),
  );

  res.writeHead(200, {
    "Content-Type": "text/event-stream",
    "Cache-Control": "no-cache, no-transform",
    Connection: "keep-alive",
    "X-Accel-Buffering": "no",
    ...CORS_HEADERS,
  });
  res.socket?.setNoDelay(true);
  res.flushHeaders?.();
  res.write(": connected\n\n");

  let settled = false;
  const unsubscribe = client.subscribe(
    threadId,
    (event) => {
      writeSseEvent(res, event);
      // react-pi drops SDK `agent_end.messages`. Re-emit session.messages
      // as a snapshot so the UI settles instead of staying on "working…".
      if (!settled && isRunSettledEvent(event)) {
        settled = true;
        void writeSettledSnapshot(res, threadId);
      }
    },
    { includeSnapshot },
  );

  // Heartbeat so proxies don't drop the idle stream.
  const heartbeat = setInterval(() => res.write(": ping\n\n"), 15000);

  req.on("close", () => {
    clearInterval(heartbeat);
    unsubscribe();
  });
}

// ---------------------------------------------------------------------------
// Thread-list enrichment
// ---------------------------------------------------------------------------

/** Collapse a first message to a single-line snippet. */
function oneLine(text: string, max = 140): string {
  const clean = text.replace(/\s+/g, " ").trim();
  return clean.length > max ? `${clean.slice(0, max).trimEnd()}…` : clean;
}

/**
 * Archive tracking for cross-workspace sessions. The supervisor filters
 * archived sessions only for its *own* workspace catalog; sessions synthesized
 * from `SessionManager.listAll()` bypass it, so the SSE server keeps its own
 * in-memory set (same lifetime as the supervisor's — resets on restart).
 */
const archivedSessionFiles = new Set<string>();

/**
 * Synthesize PiThreadMetadata-shaped metadata for a session found on disk but
 * outside the supervisor's workspace catalog. These threads are "cold": the
 * supervisor opens them on demand via findSessionInfo → SessionManager.listAll()
 * when the client switches to them, so id/sessionFile are all we need.
 */
function synthesizeThread(info: SessionInfo) {
  const title =
    info.name?.trim() ||
    info.firstMessage.slice(0, 60).trim() ||
    info.path.split("/").pop() ||
    "";
  return {
    id: info.id,
    status: "idle" as const,
    sessionFile: info.path,
    messageCount: info.messageCount,
    createdAt: info.created.toISOString(),
    updatedAt: info.modified.toISOString(),
    ...(title ? { title } : {}),
    ...(info.cwd ? { workspacePath: info.cwd } : {}),
    ...(info.parentSessionPath
      ? { parentSessionPath: info.parentSessionPath }
      : {}),
  };
}

/**
 * Full thread listing for the sidebar.
 *
 * With a `workspacePath` query param: sessions for that workspace only.
 * Without one: sessions across ALL projects — the supervisor catalog for the
 * current workspace (with live run statuses) merged with cold sessions from
 * `SessionManager.listAll()`, most-recent first.
 *
 * Enrichment adds `firstMessage` and `sessionName` from SessionInfo: the Pi
 * metadata folds both away (title = name || first-60-chars), but sidebar rows
 * need them separately — *title* on one line, a first-message snippet below.
 * Best-effort: falls back to the raw supervisor list if the disk scan fails.
 */
async function listThreadsResponse(url: URL) {
  const workspaceParam = url.searchParams.get("workspacePath") ?? undefined;
  const includeArchived = url.searchParams.get("includeArchived") === "true";
  const threads = await client.listThreads({
    workspacePath: workspaceParam,
    includeArchived,
  });

  try {
    const infos = workspaceParam
      ? await SessionManager.list(workspaceParam)
      : await SessionManager.listAll();
    const byId = new Map(infos.map((info) => [info.id, info]));

    // Add cross-workspace sessions the supervisor catalog doesn't cover.
    const known = new Set(threads.map((thread) => thread.id));
    const extras = infos
      .filter((info) => !known.has(info.id))
      .map((info) => {
        const thread = synthesizeThread(info);
        const live = liveStatusFromRecords(info.id, supervisor.records);
        return live ? { ...thread, status: live } : thread;
      })
      .filter(
        (thread) =>
          includeArchived || !archivedSessionFiles.has(thread.sessionFile),
      );

    const merged = [...threads, ...extras].sort((a, b) =>
      (b.updatedAt ?? "").localeCompare(a.updatedAt ?? ""),
    );

    return merged.map((thread) => {
      const live = liveStatusFromRecords(thread.id, supervisor.records);
      const withLive =
        live && thread.status !== live ? { ...thread, status: live } : thread;
      const info = byId.get(thread.id);
      if (!info) return withLive;
      return {
        ...withLive,
        ...(info.firstMessage
          ? { firstMessage: oneLine(info.firstMessage) }
          : {}),
        ...(info.name?.trim() ? { sessionName: info.name.trim() } : {}),
      };
    });
  } catch (error) {
    console.error("[pi-sse] thread enrichment failed:", error);
    return threads;
  }
}

/**
 * Run git in a workspace. Rethrows failures as only git's human-readable
 * `fatal:`/`error:` line — never the full command line or stderr dump — so the
 * UI can show the cause without shell noise.
 */
async function runGit(
  workspacePath: string,
  args: string[],
  timeout = 5_000,
): Promise<string> {
  try {
    const { stdout } = await execFileAsync(
      "git",
      ["-C", workspacePath, ...args],
      {
        timeout,
      },
    );
    return stdout;
  } catch (error) {
    const stderr = (error as { stderr?: string }).stderr ?? String(error);
    const line = stderr
      .split("\n")
      .map((l) => l.trim())
      .find((l) => l.startsWith("fatal:") || l.startsWith("error:"));
    throw new Error(line ?? "git command failed");
  }
}

/**
 * Current git branch of a workspace folder, best-effort.
 *
 * Resolves `HEAD` via `git -C <path> rev-parse --abbrev-ref HEAD` so the
 * composer's status row can show the real branch instead of a hardcoded one.
 * Anything that can fail — missing folder, not a repo, git not installed —
 * resolves to `null`; the UI just hides the indicator.
 */
async function gitBranch(workspacePath: string): Promise<string | null> {
  try {
    const branch = (
      await runGit(workspacePath, ["rev-parse", "--abbrev-ref", "HEAD"])
    ).trim();
    return branch || null;
  } catch {
    return null;
  }
}

/**
 * All local branches of a workspace plus which one is checked out, best-effort.
 * `current` is `null` for a detached HEAD (no branch name to highlight).
 */
async function gitBranches(
  workspacePath: string,
): Promise<{ current: string | null; branches: string[] }> {
  try {
    const stdout = await runGit(workspacePath, [
      "for-each-ref",
      "--format=%(refname:short)%09%(HEAD)",
      "refs/heads",
    ]);
    const branches: string[] = [];
    let current: string | null = null;
    for (const line of stdout.split("\n")) {
      const [name, head] = line.split("\t");
      if (!name) continue;
      branches.push(name);
      if (head?.trim() === "*") current = name;
    }
    return { current, branches };
  } catch {
    return { current: null, branches: [] };
  }
}

/** Guard for branch-name mutations: reject empty names and option-lookalikes. */
function assertBranchName(name: unknown): string {
  if (typeof name !== "string" || !name.trim()) {
    throw new Error("Branch name is required");
  }
  const trimmed = name.trim();
  if (trimmed.startsWith("-")) {
    throw new Error(`Invalid branch name: ${trimmed}`);
  }
  return trimmed;
}

/** Checkout an existing local branch; throws git's `fatal:` line on failure. */
async function gitCheckout(
  workspacePath: string,
  branch: string,
): Promise<void> {
  await runGit(workspacePath, ["checkout", branch], 15_000);
}

/** Create a new branch from HEAD and check it out in one step. */
async function gitCreateBranch(
  workspacePath: string,
  name: string,
): Promise<void> {
  await runGit(workspacePath, ["checkout", "-b", name], 15_000);
}

// ---------------------------------------------------------------------------
// Composer `/` commands and `@` files
// ---------------------------------------------------------------------------

const FILE_LIST_CAP = 5000;
const SKIP_FILE_DIRS = new Set([
  ".git",
  "node_modules",
  "dist",
  "build",
  ".next",
  "target",
  "coverage",
  "vendor",
  ".turbo",
  ".cache",
  "out",
  ".output",
  "Pods",
  "__pycache__",
]);

function assertAbsolutePath(value: unknown, label: string): string {
  if (typeof value !== "string" || !value.trim()) {
    throw new Error(`${label} is required`);
  }
  const trimmed = value.trim();
  if (!isAbsolute(trimmed)) {
    throw new Error(`${label} must be an absolute path`);
  }
  return trimmed;
}

/** Walk a folder when git isn't available; skips build/vendor trees. */
async function walkWorkspaceFiles(root: string, cap: number): Promise<string[]> {
  const out: string[] = [];
  const visit = async (dir: string) => {
    if (out.length >= cap) return;
    let entries: Awaited<ReturnType<typeof fs.readdir>>;
    try {
      entries = await fs.readdir(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      if (out.length >= cap) return;
      const hidden = entry.name.startsWith(".");
      const keepHiddenDir =
        entry.name === ".agents" ||
        entry.name === ".pi" ||
        entry.name === ".github";
      if (entry.isDirectory()) {
        if (SKIP_FILE_DIRS.has(entry.name) || (hidden && !keepHiddenDir)) continue;
        await visit(join(dir, entry.name));
        continue;
      }
      if (!entry.isFile()) continue;
      out.push(relative(root, join(dir, entry.name)).split(sep).join("/"));
    }
  };
  await visit(root);
  return out;
}

/**
 * Project files for `@` mentions. Prefers `git ls-files` (tracked + untracked,
 * gitignored excluded) so the list matches what the agent can see.
 */
async function listWorkspaceFiles(workspacePath: string): Promise<string[]> {
  if (!existsSync(workspacePath)) return [];
  try {
    const stdout = await runGit(
      workspacePath,
      ["ls-files", "--cached", "--others", "--exclude-standard", "-z"],
      10_000,
    );
    return stdout.split("\0").filter(Boolean).slice(0, FILE_LIST_CAP);
  } catch {
    return walkWorkspaceFiles(workspacePath, FILE_LIST_CAP);
  }
}

type ComposerCommand = {
  id: string;
  kind: "skill" | "prompt";
  name: string;
  description: string;
  argumentHint?: string;
  scope?: "user" | "project" | "temporary";
};

/**
 * Slash commands the composer can insert: prompt templates as `/name` and
 * skills as `/skill:name` — the same names pi expands when the prompt is sent.
 */
async function listComposerCommands(cwd: string): Promise<ComposerCommand[]> {
  const agentDir = getAgentDir();
  const settingsManager = SettingsManager.create(cwd, agentDir);
  const loader = new DefaultResourceLoader({
    cwd,
    agentDir,
    settingsManager,
  });
  await loader.reload();
  const commands: ComposerCommand[] = [];
  for (const prompt of loader.getPrompts().prompts) {
    commands.push({
      id: `prompt:${prompt.name}`,
      kind: "prompt",
      name: prompt.name,
      description: prompt.description,
      ...(prompt.argumentHint ? { argumentHint: prompt.argumentHint } : {}),
      scope: prompt.sourceInfo.scope,
    });
  }
  for (const skill of loader.getSkills().skills) {
    commands.push({
      id: `skill:${skill.name}`,
      kind: "skill",
      name: `skill:${skill.name}`,
      description: skill.description,
      scope: skill.sourceInfo.scope,
    });
  }
  return commands;
}

// ---------------------------------------------------------------------------
// Workspace “Open in” apps
// ---------------------------------------------------------------------------

interface OpenInApp {
  id: string;
  name: string;
  kind: "editor" | "terminal" | "files";
  /** macOS app name for `open -a`; probed under the standard Applications dirs. */
  darwinApp?: string;
  /** Linux CLI binaries probed via `which` (first match wins). */
  linuxBins?: string[];
}

/** Curated catalog of apps that can open a project folder, in display order. */
const OPEN_IN_CATALOG: OpenInApp[] = [
  {
    id: "vscode",
    name: "VS Code",
    kind: "editor",
    darwinApp: "Visual Studio Code",
    linuxBins: ["code"],
  },
  {
    id: "cursor",
    name: "Cursor",
    kind: "editor",
    darwinApp: "Cursor",
    linuxBins: ["cursor"],
  },
  {
    id: "zed",
    name: "Zed",
    kind: "editor",
    darwinApp: "Zed",
    linuxBins: ["zed"],
  },
  {
    id: "windsurf",
    name: "Windsurf",
    kind: "editor",
    darwinApp: "Windsurf",
    linuxBins: ["windsurf"],
  },
  {
    id: "sublime",
    name: "Sublime Text",
    kind: "editor",
    darwinApp: "Sublime Text",
    linuxBins: ["subl"],
  },
  { id: "xcode", name: "Xcode", kind: "editor", darwinApp: "Xcode" },
  {
    id: "android-studio",
    name: "Android Studio",
    kind: "editor",
    darwinApp: "Android Studio",
  },
  {
    id: "intellij",
    name: "IntelliJ IDEA",
    kind: "editor",
    darwinApp: "IntelliJ IDEA",
    linuxBins: ["idea"],
  },
  {
    id: "webstorm",
    name: "WebStorm",
    kind: "editor",
    darwinApp: "WebStorm",
    linuxBins: ["webstorm"],
  },
  { id: "terminal", name: "Terminal", kind: "terminal", darwinApp: "Terminal" },
  { id: "iterm", name: "iTerm2", kind: "terminal", darwinApp: "iTerm" },
  {
    id: "warp",
    name: "Warp",
    kind: "terminal",
    darwinApp: "Warp",
    linuxBins: ["warp-terminal"],
  },
  {
    id: "ghostty",
    name: "Ghostty",
    kind: "terminal",
    darwinApp: "Ghostty",
    linuxBins: ["ghostty"],
  },
  {
    id: "kitty",
    name: "Kitty",
    kind: "terminal",
    darwinApp: "kitty",
    linuxBins: ["kitty"],
  },
  {
    id: "alacritty",
    name: "Alacritty",
    kind: "terminal",
    darwinApp: "Alacritty",
    linuxBins: ["alacritty"],
  },
  { id: "finder", name: "Finder", kind: "files", darwinApp: "Finder" },
];

const DARWIN_APP_DIRS = [
  "/Applications",
  "/System/Applications",
  "/System/Applications/Utilities",
];

/** Probe whether a catalog app is installed on this machine. */
function isOpenInAppInstalled(app: OpenInApp): boolean {
  if (process.platform === "darwin") {
    if (!app.darwinApp) return false;
    if (app.kind === "files") return true; // Finder ships with macOS
    const dirs = [...DARWIN_APP_DIRS, join(homedir(), "Applications")];
    return dirs.some((dir) => existsSync(join(dir, `${app.darwinApp}.app`)));
  }
  if (process.platform === "linux") {
    return (app.linuxBins ?? []).some((bin) => {
      try {
        return (
          execFileSync("which", [bin], { timeout: 2_000 }).toString().trim()
            .length > 0
        );
      } catch {
        return false;
      }
    });
  }
  return false;
}

/** Open the folder in the given app; throws a human-readable error on failure. */
async function openWorkspaceInApp(
  workspacePath: string,
  appId: string,
): Promise<void> {
  const app = OPEN_IN_CATALOG.find((a) => a.id === appId);
  if (!app) throw new Error(`Unknown app: ${appId}`);
  if (!existsSync(workspacePath)) {
    throw new Error(`Folder not found: ${workspacePath}`);
  }
  if (!isOpenInAppInstalled(app)) {
    throw new Error(`${app.name} is not installed on this machine`);
  }
  if (process.platform === "darwin") {
    const args =
      app.kind === "files"
        ? [workspacePath]
        : ["-a", app.darwinApp!, workspacePath];
    await execFileAsync("open", args, { timeout: 10_000 });
    return;
  }
  if (process.platform === "linux") {
    const bin = (app.linuxBins ?? []).find((candidate) => {
      try {
        return (
          execFileSync("which", [candidate], { timeout: 2_000 })
            .toString()
            .trim().length > 0
        );
      } catch {
        return false;
      }
    });
    if (!bin) throw new Error(`${app.name} is not installed`);
    await execFileAsync(bin, [workspacePath], { timeout: 10_000 });
    return;
  }
  throw new Error("Opening apps is not supported on this platform");
}

/**
 * Git stats for the files the agent touched in the current turn. Numstat
 * against HEAD (or the empty tree for a fresh repo) gives tracked +/− counts;
 * untracked paths are counted with `wc -l`. Only paths that actually changed
 * are returned.
 */
const EMPTY_TREE_HASH = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

interface WorkspaceFileChange {
  path: string;
  additions: number;
  deletions: number;
  status: "modified" | "added" | "deleted";
}

async function gitChangedFiles(
  workspacePath: string,
  paths: string[],
): Promise<WorkspaceFileChange[]> {
  let hasHead = true;
  try {
    await runGit(workspacePath, ["rev-parse", "--verify", "HEAD"]);
  } catch {
    hasHead = false;
  }
  const base = hasHead ? "HEAD" : EMPTY_TREE_HASH;

  const files = new Map<string, { additions: number; deletions: number }>();
  let statusByPath = new Map<string, string>();
  try {
    const out = await runGit(workspacePath, [
      "diff",
      "--numstat",
      base,
      "--",
      ...paths,
    ]);
    for (const line of out.split("\n")) {
      if (!line.trim()) continue;
      const [addRaw, delRaw, ...rest] = line.split("\t");
      // Rename numstat: `src/{old => new}/x.ts` — keep the new path.
      let path = rest
        .join("\t")
        .replace(/\{[^{}]*=>\s*([^{}]*)\}/, "$1")
        .trim();
      if (!path) continue;
      files.set(path, {
        additions: addRaw === "-" ? 0 : Number(addRaw) || 0,
        deletions: delRaw === "-" ? 0 : Number(delRaw) || 0,
      });
    }
  } catch {
    // Diff failed (e.g. not a repo) — fall through to untracked counting.
  }

  // Status pass: marks deleted files and finds untracked ones (numstat skips
  // untracked paths — count their lines instead).
  try {
    const statusOut = await runGit(workspacePath, [
      "status",
      "--porcelain",
      "--",
      ...paths,
    ]);
    for (const line of statusOut.split("\n")) {
      if (line.length < 4) continue;
      const code = line.slice(0, 2);
      let path = line.slice(3).trim();
      path = path.replace(/\{[^{}]*=>\s*([^{}]*)\}/, "$1").trim();
      if (path) statusByPath.set(path, code);
    }
    for (const [path, code] of statusByPath) {
      if (code === "??") {
        // Untracked → every line is an addition.
        let additions = 0;
        try {
          const { stdout } = await execFileAsync(
            "wc",
            ["-l", join(workspacePath, path)],
            { timeout: 5_000 },
          );
          additions = Math.max(0, Number(stdout.trim().split(/\s+/)[0]) || 0);
        } catch {
          additions = 0;
        }
        files.set(path, { additions, deletions: 0 });
      } else if (code.includes("D") && !files.has(path)) {
        files.set(path, { additions: 0, deletions: 0 });
      }
    }
  } catch {
    // Not a repo — no changes to report.
  }

  const result: WorkspaceFileChange[] = [];
  for (const [path, stats] of files) {
    const code = statusByPath.get(path) ?? "";
    let status: WorkspaceFileChange["status"] = "modified";
    if (code.includes("D")) status = "deleted";
    else if (code === "??" || code.includes("A")) status = "added";
    if (
      stats.additions === 0 &&
      stats.deletions === 0 &&
      status === "modified"
    ) {
      continue; // Touched but content-identical — not a change.
    }
    result.push({ path, ...stats, status });
  }
  return result;
}

/**
 * Unified diff for one file, scoped to uncommitted changes (HEAD, or the
 * empty tree for a fresh repo). Untracked files diff against /dev/null —
 * `--no-index` exits 1 when differences exist, which execFileAsync treats as
 * a failure, so stdout is recovered from the error.
 */
async function gitFileDiff(
  workspacePath: string,
  filePath: string,
): Promise<string> {
  let hasHead = true;
  try {
    await runGit(workspacePath, ["rev-parse", "--verify", "HEAD"]);
  } catch {
    hasHead = false;
  }
  const base = hasHead ? "HEAD" : EMPTY_TREE_HASH;

  // Untracked files have no HEAD entry — `git diff HEAD -- f` exits 0 with
  // empty output, so they must diff against /dev/null instead.
  let untracked = false;
  try {
    const status = await runGit(workspacePath, [
      "status",
      "--porcelain",
      "--",
      filePath,
    ]);
    untracked = (status.split("\n")[0] ?? "").startsWith("??");
  } catch {
    untracked = false;
  }

  if (untracked || !hasHead) {
    const abs = join(workspacePath, filePath);
    if (!existsSync(abs)) throw new Error(`File not found: ${filePath}`);
    try {
      const { stdout } = await execFileAsync(
        "git",
        ["diff", "--no-index", "--", "/dev/null", abs],
        { timeout: 15_000 },
      );
      return stdout;
    } catch (error) {
      // Exit 1 = differences found — that IS the output.
      const stdout = (error as { stdout?: string }).stdout;
      if (stdout !== undefined) return stdout;
      throw new Error(`Could not diff ${filePath}`);
    }
  }

  return runGit(workspacePath, ["diff", base, "--", filePath], 15_000);
}

// Router

const server = createServer(async (req, res) => {
  if (req.method === "OPTIONS") {
    res.writeHead(204, CORS_HEADERS);
    res.end();
    return;
  }

  const url = new URL(req.url ?? "/", "http://localhost");
  const path = url.pathname;
  const method = req.method ?? "GET";
  const body = await readBody(req).catch(() => ({}));

  // GET /health — identity check used by the Tauri launcher and diagnostics.
  if (method === "GET" && path === "/health") {
    return sendJson(res, 200, {
      id: SERVER_ID,
      ok: true,
      pid: process.pid,
      workspacePath: WORKSPACE_PATH,
      startedAt: STARTED_AT,
      ...(reloadedAt ? { reloadedAt } : {}),
    });
  }

  // POST /reload — recreate the in-process supervisor so models/settings/auth
  // are re-read without taking the HTTP listener down.
  if (method === "POST" && path === "/reload") {
    return route(async (_req, res) => {
      await reloadPiRuntime();
      sendJson(res, 200, {
        id: SERVER_ID,
        ok: true,
        pid: process.pid,
        reloadedAt,
      });
    })(req, res, body);
  }

  // GET /threads — no workspacePath param = all workspaces
  if (method === "GET" && path === "/threads") {
    return route(async (_req, res) => {
      sendJson(res, 200, await listThreadsResponse(url));
    })(req, res, body);
  }

  // POST /threads
  if (method === "POST" && path === "/threads") {
    return route(async (_req, res, body) => {
      const snapshot = await client.createThread(
        (body ?? {}) as {
          workspacePath?: string;
          title?: string;
          initialMessage?: unknown;
        },
      );
      await getScopedModels();
      sendJson(res, 200, snapshot);
    })(req, res, body);
  }

  // GET /workspace/branch?workspacePath=… — current git branch, best-effort
  if (method === "GET" && path === "/workspace/branch") {
    return route(async (_req, res) => {
      const workspacePath = url.searchParams.get("workspacePath");
      const branch = workspacePath ? await gitBranch(workspacePath) : null;
      sendJson(res, 200, { branch });
    })(req, res, body);
  }

  // GET /workspace/branches?workspacePath=… — all local branches + current
  if (method === "GET" && path === "/workspace/branches") {
    return route(async (_req, res) => {
      const workspacePath = url.searchParams.get("workspacePath");
      if (!workspacePath) throw new Error("workspacePath is required");
      sendJson(res, 200, await gitBranches(workspacePath));
    })(req, res, body);
  }

  // POST /workspace/branch/checkout { workspacePath, branch }
  if (method === "POST" && path === "/workspace/branch/checkout") {
    return route(async (_req, res, body) => {
      const { workspacePath, branch } = (body ?? {}) as {
        workspacePath?: string;
        branch?: string;
      };
      if (!workspacePath) throw new Error("workspacePath is required");
      const name = assertBranchName(branch);
      await gitCheckout(workspacePath, name);
      sendJson(res, 200, { current: name });
    })(req, res, body);
  }

  // POST /workspace/branch/create { workspacePath, name } — create + checkout
  if (method === "POST" && path === "/workspace/branch/create") {
    return route(async (_req, res, body) => {
      const { workspacePath, name } = (body ?? {}) as {
        workspacePath?: string;
        name?: string;
      };
      if (!workspacePath) throw new Error("workspacePath is required");
      const branch = assertBranchName(name);
      await gitCreateBranch(workspacePath, branch);
      sendJson(res, 200, { current: branch });
    })(req, res, body);
  }

  // POST /workspace/open { workspacePath, appId } — open the folder in an app
  if (method === "POST" && path === "/workspace/open") {
    return route(async (_req, res, body) => {
      const { workspacePath, appId } = (body ?? {}) as {
        workspacePath?: string;
        appId?: string;
      };
      if (!workspacePath) throw new Error("workspacePath is required");
      if (!appId) throw new Error("appId is required");
      await openWorkspaceInApp(workspacePath, appId);
      sendJson(res, 200, { ok: true });
    })(req, res, body);
  }

  // GET /workspace/apps — installed IDEs/terminals for the “Open in” picker
  if (method === "GET" && path === "/workspace/apps") {
    return route(async (_req, res) => {
      const apps = OPEN_IN_CATALOG.filter(isOpenInAppInstalled).map(
        ({ id, name, kind }) => ({ id, name, kind }),
      );
      sendJson(res, 200, { apps });
    })(req, res, body);
  }

  // POST /workspace/changes { workspacePath, paths } — git +/− per touched file
  if (method === "POST" && path === "/workspace/changes") {
    return route(async (_req, res, body) => {
      const { workspacePath, paths } = (body ?? {}) as {
        workspacePath?: string;
        paths?: string[];
      };
      if (!workspacePath) throw new Error("workspacePath is required");
      const changed = await gitChangedFiles(
        workspacePath,
        Array.isArray(paths) ? paths.slice(0, 100) : [],
      );
      sendJson(res, 200, { files: changed });
    })(req, res, body);
  }

  // GET /workspace/status — all working-tree changes (for the diff panel tree)
  if (method === "GET" && path === "/workspace/status") {
    return route(async (_req, res) => {
      const workspacePath = url.searchParams.get("workspacePath");
      if (!workspacePath) throw new Error("workspacePath is required");
      const files = await gitChangedFiles(workspacePath, []);
      sendJson(res, 200, { files });
    })(req, res, body);
  }

  // GET /workspace/file-diff?workspacePath=&path= — unified diff for one file
  if (method === "GET" && path === "/workspace/file-diff") {
    return route(async (_req, res) => {
      const workspacePath = url.searchParams.get("workspacePath");
      const filePath = url.searchParams.get("path");
      if (!workspacePath || !filePath) {
        throw new Error("workspacePath and path are required");
      }
      sendJson(res, 200, {
        diff: await gitFileDiff(workspacePath, filePath),
      });
    })(req, res, body);
  }

  // GET /workbench/usage — token/cost report across every session on disk
  if (method === "GET" && path === "/workbench/usage") {
    return route(async (_req, res) => {
      sendJson(res, 200, await getUsage());
    })(req, res, body);
  }

  // GET /workbench/skills — skills installed in ~/.pi/agent/skills
  if (method === "GET" && path === "/workbench/skills") {
    return route(async (_req, res) => {
      sendJson(res, 200, await listSkills());
    })(req, res, body);
  }

  // GET /workbench/commands — slash-invocable skills + prompt templates
  if (method === "GET" && path === "/workbench/commands") {
    return route(async (_req, res) => {
      const raw = url.searchParams.get("workspacePath");
      const cwd = raw ? assertAbsolutePath(raw, "workspacePath") : WORKSPACE_PATH;
      sendJson(res, 200, await listComposerCommands(cwd));
    })(req, res, body);
  }

  // GET /workspace/files?workspacePath= — project files for @ mentions
  if (method === "GET" && path === "/workspace/files") {
    return route(async (_req, res) => {
      const workspacePath = assertAbsolutePath(
        url.searchParams.get("workspacePath"),
        "workspacePath",
      );
      sendJson(res, 200, { files: await listWorkspaceFiles(workspacePath) });
    })(req, res, body);
  }

  // GET /workbench/plugins — packages + local extensions from settings.json
  if (method === "GET" && path === "/workbench/plugins") {
    return route(async (_req, res) => {
      sendJson(res, 200, await listPlugins());
    })(req, res, body);
  }

  // GET /models
  if (method === "GET" && path === "/models") {
    return route(async (_req, res) => {
      const models = await listAvailableModels(
        url.searchParams.get("workspacePath") ?? WORKSPACE_PATH,
      );
      sendJson(res, 200, models);
    })(req, res, body);
  }

  // GET /scoped-models — pi's `enabledModels` setting resolved against the
  // available catalog (`ids` is null when unscoped = every model usable).
  if (method === "GET" && path === "/scoped-models") {
    return route(async (_req, res) => {
      sendJson(res, 200, await getScopedModels());
    })(req, res, body);
  }

  // PUT /scoped-models — persist the scoped set ({ patterns: string[] | null,
  // pi CLI /scoped-models semantics) and apply it to live sessions.
  if (method === "PUT" && path === "/scoped-models") {
    return route(async (_req, res, body) => {
      const { patterns } = (body ?? {}) as { patterns?: string[] | null };
      if (patterns !== null && patterns !== undefined && !Array.isArray(patterns)) {
        throw new Error("PUT /scoped-models requires { patterns: string[] | null }");
      }
      sendJson(res, 200, await setScopedModels(patterns ?? null));
    })(req, res, body);
  }

  // GET /providers — custom providers (models.json) + live catalog summary
  if (method === "GET" && path === "/providers") {
    return route(async (_req, res) => {
      sendJson(res, 200, await listProviders());
    })(req, res, body);
  }

  // PUT /providers/:id — create or replace a custom provider entry
  if (method === "PUT" && /^\/providers\/([^/]+)$/.test(path)) {
    return route(async (_req, res, body) => {
      const id = decodeURIComponent(path.slice("/providers/".length));
      if (!PROVIDER_ID_PATTERN.test(id)) {
        throw new RequestError(
          "Provider id may only contain letters, numbers, dots, dashes and underscores",
        );
      }
      // pi's own provider list decides what's addable and which api types
      // exist — a static list would drift out of the runtime's truth.
      let allowedApis = new Set<string>(PROVIDER_APIS);
      let knownProviderIds = new Set<string>();
      try {
        const runtime = await getFreshModelRuntime();
        allowedApis = new Set([
          ...allowedApis,
          ...runtime
            .getProviders()
            .flatMap((p) => {
              const model = p.getModels()[0] as { api?: string } | undefined;
              return model?.api ? [String(model.api)] : [];
            }),
        ]);
        knownProviderIds = new Set(
          runtime
            .getProviders()
            .filter((p) => p.getModels().length > 0)
            .map((p) => p.id),
        );
      } catch {
        // Offline runtime: the static families + empty known-set still guard
        // the write; a saved entry is re-validated by pi on load.
      }
      const input = validateProviderInput(
        id,
        body,
        allowedApis,
        knownProviderIds,
      );
      const config = await readModelsJson();
      const providers = (config.providers ?? {}) as Record<string, ProviderFileEntry>;
      const existing = providers[id];
      const entry: ProviderFileEntry = {
        // Preserve unknown keys (comments are lost — JSON.stringify cannot
        // keep them; pi strips them on read anyway).
        ...(existing ?? {}),
        ...(input.name ? { name: input.name } : {}),
        baseUrl: input.baseUrl,
        api: input.api,
        // undefined = keep the stored key; empty string = remove it.
        ...(input.apiKey === undefined
          ? {}
          : input.apiKey
            ? { apiKey: input.apiKey }
            : {}),
        ...(input.models.length > 0 ? { models: input.models } : {}),
      };
      if (!input.apiKey && input.apiKey !== undefined) {
        // Explicit empty apiKey clears a stored one.
        delete entry.apiKey;
      }
      if (input.models.length === 0) {
        // Built-in-catalog entry: no models key, pi's own models serve it.
        delete entry.models;
      }
      config.providers = { ...providers, [id]: entry };
      await writeModelsJson(config);
      await reloadPiRuntime();
      sendJson(res, 200, await listProviders());
    })(req, res, body);
  }

  // DELETE /providers/:id — remove a custom provider from models.json
  if (method === "DELETE" && /^\/providers\/([^/]+)$/.test(path)) {
    return route(async (_req, res) => {
      const id = decodeURIComponent(path.slice("/providers/".length));
      const config = await readModelsJson();
      const providers = (config.providers ?? {}) as Record<string, ProviderFileEntry>;
      if (!(id in providers)) {
        throw new RequestError(
          `Provider "${id}" is not in models.json — built-in providers are managed by pi, not this file`,
          404,
        );
      }
      const { [id]: _removed, ...rest } = providers;
      config.providers = rest;
      await writeModelsJson(config);
      await reloadPiRuntime();
      sendJson(res, 200, await listProviders());
    })(req, res, body);
  }

  // /threads/:id[/action[/subaction]]
  const match = path.match(/^\/threads\/([^/]+)(?:\/([^/]+))?(?:\/([^/]+))?$/);
  if (match) {
    const threadId = decodeURIComponent(match[1]!);
    const action = match[2];
    const subaction = match[3];

    // GET /threads/:id
    if (method === "GET" && !action) {
      return route(async (_req, res) => {
        sendJson(res, 200, await client.getThread(threadId));
      })(req, res, body);
    }

    // PATCH /threads/:id  (rename)
    if (method === "PATCH" && !action) {
      return route(async (_req, res, body) => {
        const title = (body as { title?: string } | undefined)?.title;
        if (typeof title !== "string")
          throw new Error("PATCH /threads/:id requires { title }");
        await client.renameThread(threadId, title);
        sendNoContent(res);
      })(req, res, body);
    }

    // DELETE /threads/:id
    if (method === "DELETE" && !action) {
      return route(async (_req, res) => {
        await client.deleteThread(threadId);
        sendNoContent(res);
      })(req, res, body);
    }

    if (action) {
      // GET /threads/:id/events  (SSE)
      if (method === "GET" && action === "events") {
        return streamEvents(req, res, threadId, url);
      }

      // POST /threads/:id/messages
      if (method === "POST" && action === "messages") {
        return route(async (_req, res, body) => {
          const input = (body as { input?: unknown } | undefined)?.input;
          if (input === undefined)
            throw new Error("POST /messages requires { input }");
          await client.sendMessage(threadId, input as never);
          sendNoContent(res);
        })(req, res, body);
      }

      // POST /threads/:id/cancel
      if (method === "POST" && action === "cancel") {
        return route(async (_req, res) => {
          await client.cancelRun(threadId);
          sendNoContent(res);
        })(req, res, body);
      }

      // POST /threads/:id/queue/clear
      if (method === "POST" && action === "queue" && subaction === "clear") {
        return route(async (_req, res) => {
          sendJson(res, 200, await client.clearQueue(threadId));
        })(req, res, body);
      }

      // POST /threads/:id/model
      if (method === "POST" && action === "model") {
        return route(async (_req, res, body) => {
          const { provider, modelId } = (body ?? {}) as {
            provider?: string;
            modelId?: string;
          };
          if (!provider || !modelId)
            throw new Error("POST /model requires { provider, modelId }");
          await getFreshModelRuntime();
          await client.setModel(threadId, { provider, modelId });
          sendNoContent(res);
        })(req, res, body);
      }

      // POST /threads/:id/thinking
      if (method === "POST" && action === "thinking") {
        return route(async (_req, res, body) => {
          const level = (body as { level?: string } | undefined)?.level;
          if (!level) throw new Error("POST /thinking requires { level }");
          await client.setThinkingLevel(threadId, level as never);
          sendNoContent(res);
        })(req, res, body);
      }

      // POST /threads/:id/archive | /unarchive
      if (
        method === "POST" &&
        (action === "archive" || action === "unarchive")
      ) {
        return route(async (_req, res) => {
          const sessionFile = (
            (await client.getThread(threadId).catch(() => undefined)) as
              { metadata?: { sessionFile?: string } } | undefined
          )?.metadata?.sessionFile;
          if (action === "archive") {
            if (sessionFile) archivedSessionFiles.add(sessionFile);
            await client.archiveThread(threadId);
          } else {
            if (sessionFile) archivedSessionFiles.delete(sessionFile);
            await client.unarchiveThread(threadId);
          }
          sendNoContent(res);
        })(req, res, body);
      }

      // POST /threads/:id/host-ui
      if (method === "POST" && action === "host-ui") {
        return route(async (_req, res, body) => {
          const response = (body as { response?: unknown } | undefined)
            ?.response;
          if (response === undefined)
            throw new Error("POST /host-ui requires { response }");
          await client.respondToHostUiRequest(threadId, response as never);
          sendNoContent(res);
        })(req, res, body);
      }
    }
  }

  sendJson(res, 404, { error: `Not found: ${method} ${path}` });
});

server.listen(PORT, () => {
  console.log(`[pi-sse] listening on http://localhost:${PORT}`);
  console.log(`[pi-sse] workspace: ${WORKSPACE_PATH}`);
  console.log(
    `[pi-sse] PiClient contract: GET/POST /threads, GET /threads/:id/events (SSE)`,
  );
});
