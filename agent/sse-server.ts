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
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { createPiNodeClient } from "@assistant-ui/react-pi/node";
import {
  SessionManager,
  type SessionInfo,
} from "@earendil-works/pi-coding-agent";
import { getUsage, listPlugins, listSkills } from "./workbench.js";

const execFileAsync = promisify(execFile);

const PORT = Number(process.env.PI_SSE_PORT ?? 8913);
const WORKSPACE_PATH = process.env.PI_WORKSPACE_PATH ?? process.cwd();

const client = createPiNodeClient({ workspacePath: WORKSPACE_PATH });

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const CORS_HEADERS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, POST, PATCH, DELETE, OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type, Authorization",
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
  console.error("[pi-sse] error:", message);
  sendJson(res, 500, { error: message });
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

// ---------------------------------------------------------------------------
// SSE stream
// ---------------------------------------------------------------------------

function streamEvents(
  req: IncomingMessage,
  res: ServerResponse,
  threadId: string,
): void {
  const includeSnapshot =
    new URL(req.url ?? "/", "http://localhost").searchParams.get("snapshot") !==
    "false";

  res.writeHead(200, {
    "Content-Type": "text/event-stream",
    "Cache-Control": "no-cache, no-transform",
    Connection: "keep-alive",
    "X-Accel-Buffering": "no",
    ...CORS_HEADERS,
  });
  res.write(": connected\n\n");

  const unsubscribe = client.subscribe(
    threadId,
    (event) => {
      res.write(`data: ${JSON.stringify(event)}\n\n`);
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
      .map(synthesizeThread)
      .filter(
        (thread) =>
          includeArchived || !archivedSessionFiles.has(thread.sessionFile),
      );

    const merged = [...threads, ...extras].sort((a, b) =>
      (b.updatedAt ?? "").localeCompare(a.updatedAt ?? ""),
    );

    return merged.map((thread) => {
      const info = byId.get(thread.id);
      if (!info) return thread;
      return {
        ...thread,
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

  // GET /workbench/plugins — packages + local extensions from settings.json
  if (method === "GET" && path === "/workbench/plugins") {
    return route(async (_req, res) => {
      sendJson(res, 200, await listPlugins());
    })(req, res, body);
  }

  // GET /models
  if (method === "GET" && path === "/models") {
    return route(async (_req, res) => {
      const models = await client.getAvailableModels({
        workspacePath: url.searchParams.get("workspacePath") ?? undefined,
      });
      sendJson(res, 200, models);
    })(req, res, body);
  }

  // GET /scoped-models — pi's `enabledModels` setting resolved against the
  // available catalog (`ids` is null when unscoped = every model usable).
  if (method === "GET" && path === "/scoped-models") {
    return route(async (_req, res) => {
      sendJson(res, 200, await client.getScopedModels());
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
      sendJson(res, 200, await client.setScopedModels(patterns ?? null));
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
        return streamEvents(req, res, threadId);
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
