/**
 * Shared Pi SSE client — the browser half of the `@assistant-ui/react-pi`
 * HTTP/SSE transport. Points at `agent/sse-server.ts` (run: `pnpm agent:sse`).
 */

import { createPiHttpClient } from "@assistant-ui/react-pi";
import type {
  PiComposerCommand,
  PiPluginInfo,
  PiProvidersReport,
  PiSkillInfo,
  PiUsageReport,
} from "./pi-agent";
export const SSE_BASE_URL = "http://localhost:8913";
export const SSE_SERVER_ID = "orbit-pi-sse";

const noStoreFetch: typeof fetch = (input, init) =>
  fetch(input, { cache: "no-store", ...init });

export const piClient = createPiHttpClient({
  baseUrl: SSE_BASE_URL,
  fetchImpl: noStoreFetch,
  onStreamError: (error) => {
    console.error("[orbit] pi event stream:", error);
  },
});

export function isOrbitSseHealth(value: unknown): boolean {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    (value as { id?: unknown }).id === SSE_SERVER_ID &&
    (value as { ok?: unknown }).ok === true
  );
}

function notifyAgentReloaded(): void {
  window.dispatchEvent(new Event("orbit:models-updated"));
  window.dispatchEvent(new Event("orbit:threads-updated"));
  window.dispatchEvent(new Event("orbit:agent-reloaded"));
}

async function waitForSseHealth(timeoutMs = 15_000): Promise<void> {
  const started = Date.now();
  let lastError: unknown;
  while (Date.now() - started < timeoutMs) {
    try {
      const res = await fetch(`${SSE_BASE_URL}/health`, { cache: "no-store" });
      if (res.ok && isOrbitSseHealth(await res.json())) return;
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(
    lastError instanceof Error
      ? lastError.message
      : "The pi agent did not come back after reload",
  );
}

async function reloadPiRuntimeHttp(): Promise<void> {
  const res = await fetch(`${SSE_BASE_URL}/reload`, { method: "POST" });
  if (!res.ok) {
    throw new Error(
      `Failed to reload the pi agent (${res.status}): ${(await res.text()) || res.statusText}`,
    );
  }
}

/**
 * Restart the pi agent so live sessions re-read models.json, settings, and
 * auth. Prefers a process restart from Tauri; falls back to POST /reload
 * when the frontend is in the browser or the sidecar was started externally.
 */
function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function reloadPiAgent(): Promise<void> {
  if (isTauriRuntime()) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("restart_sse_server");
      await waitForSseHealth();
      notifyAgentReloaded();
      return;
    } catch (error) {
      console.warn("[orbit] process restart failed, reloading in-process:", error);
    }
  }
  await reloadPiRuntimeHttp();
  notifyAgentReloaded();
}

/**
 * Workbench data (usage report, skills, plugins) from the SSE server's
 * read-only `/workbench/*` endpoints — the same aggregators the chat runs on.
 * Resolves `null` when the server is unreachable so callers can show an
 * error/empty state instead of hanging.
 */
async function fetchWorkbench<T>(path: string): Promise<T | null> {
  try {
    const res = await fetch(`${SSE_BASE_URL}${path}`);
    if (!res.ok) return null;
    return (await res.json()) as T;
  } catch {
    return null;
  }
}

export function fetchUsageReport(): Promise<PiUsageReport | null> {
  return fetchWorkbench<PiUsageReport>("/workbench/usage");
}

export function fetchSkills(): Promise<PiSkillInfo[] | null> {
  return fetchWorkbench<PiSkillInfo[]>("/workbench/skills");
}

/** Skills (`/skill:name`) and prompt templates (`/name`) for the composer. */
export function fetchComposerCommands(
  workspacePath?: string,
): Promise<PiComposerCommand[] | null> {
  const query = workspacePath
    ? `?workspacePath=${encodeURIComponent(workspacePath)}`
    : "";
  return fetchWorkbench<PiComposerCommand[]>(`/workbench/commands${query}`);
}

/** Relative project files for `@` mentions, or null if the server is down. */
export async function fetchWorkspaceFiles(
  workspacePath: string,
): Promise<string[] | null> {
  try {
    const res = await fetch(
      `${SSE_BASE_URL}/workspace/files?workspacePath=${encodeURIComponent(workspacePath)}`,
    );
    if (!res.ok) return null;
    const data = (await res.json()) as { files?: string[] };
    return data.files ?? [];
  } catch {
    return null;
  }
}

export function fetchPlugins(): Promise<PiPluginInfo | null> {
  return fetchWorkbench<PiPluginInfo>("/workbench/plugins");
}

/**
 * Current git branch of a workspace folder, via the SSE server's best-effort
 * `GET /workspace/branch`. Resolves `null` when the folder isn't a repo, git
 * is missing, or the server is unreachable — callers just hide the indicator.
 */
export async function fetchWorkspaceBranch(
  workspacePath: string,
): Promise<string | null> {
  try {
    const res = await fetch(
      `${SSE_BASE_URL}/workspace/branch?workspacePath=${encodeURIComponent(workspacePath)}`,
    );
    if (!res.ok) return null;
    const data = (await res.json()) as { branch?: string | null };
    return data.branch ?? null;
  } catch {
    return null;
  }
}

/** All local branches of a workspace plus the checked-out one (null = detached HEAD). */
export async function fetchWorkspaceBranches(
  workspacePath: string,
): Promise<{ current: string | null; branches: string[] } | null> {
  try {
    const res = await fetch(
      `${SSE_BASE_URL}/workspace/branches?workspacePath=${encodeURIComponent(workspacePath)}`,
    );
    if (!res.ok) return null;
    return (await res.json()) as { current: string | null; branches: string[] };
  } catch {
    return null;
  }
}

/**
 * Pi's scoped models — the `enabledModels` setting (pi CLI /scoped-models)
 * resolved against the available catalog. `ids` is null when unscoped, meaning
 * every available model is usable. Resolves null when the server is
 * unreachable so callers can hide the control instead of hanging.
 */
type ScopedModelState = {
  patterns: string[] | null;
  ids: string[] | null;
};

export async function fetchScopedModels(): Promise<ScopedModelState | null> {
  return fetchWorkbench("/scoped-models");
}

/**
 * Persist the scoped-model set (pi CLI /scoped-models semantics: an empty or
 * null list clears the scope) and apply it to live sessions. Throws on server
 * failure so callers can surface the error.
 */
export async function saveScopedModels(
  patterns: string[] | null,
): Promise<ScopedModelState> {
  const res = await fetch(`${SSE_BASE_URL}/scoped-models`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ patterns }),
  });
  if (!res.ok) {
    throw new Error(
      `Failed to save scoped models (${res.status}): ${(await res.text()) || res.statusText}`,
    );
  }
  const state = (await res.json()) as ScopedModelState;
  notifyAgentReloaded();
  return state;
}

/**
 * Providers report — custom providers from pi's models.json plus a summary
 * of the live catalog. Resolves null when the server is unreachable.
 */
export function fetchProviders(): Promise<PiProvidersReport | null> {
  return fetchWorkbench<PiProvidersReport>("/providers");
}

/**
 * Create or replace a custom provider in models.json. `apiKey: ""` removes a
 * stored key; leaving it undefined keeps the existing one. Throws with the
 * server's message on failure; resolves with the refreshed providers report.
 */
export async function saveProvider(
  id: string,
  input: {
    name?: string;
    baseUrl: string;
    api: string;
    apiKey?: string;
    models: { id: string; name?: string; contextWindow?: number }[];
  },
): Promise<PiProvidersReport> {
  const res = await fetch(
    `${SSE_BASE_URL}/providers/${encodeURIComponent(id)}`,
    {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(input),
    },
  );
  if (!res.ok) {
    const data = (await res.json().catch(() => ({}))) as { error?: string };
    throw new Error(data.error ?? `Saving the provider failed (${res.status})`);
  }
  const report = (await res.json()) as PiProvidersReport;
  notifyAgentReloaded();
  return report;
}

/**
 * Remove a custom provider from models.json and refresh the live catalog.
 * Throws with the server's message on failure.
 */
export async function deleteProvider(id: string): Promise<PiProvidersReport> {
  const res = await fetch(
    `${SSE_BASE_URL}/providers/${encodeURIComponent(id)}`,
    { method: "DELETE" },
  );
  if (!res.ok) {
    const data = (await res.json().catch(() => ({}))) as { error?: string };
    throw new Error(data.error ?? `Removing the provider failed (${res.status})`);
  }
  const report = (await res.json()) as PiProvidersReport;
  notifyAgentReloaded();
  return report;
}

/** Checkout an existing branch. Throws with git's message on failure. */
export async function checkoutWorkspaceBranch(
  workspacePath: string,
  branch: string,
): Promise<void> {
  const res = await fetch(`${SSE_BASE_URL}/workspace/branch/checkout`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ workspacePath, branch }),
  });
  if (!res.ok) {
    const data = (await res.json().catch(() => ({}))) as { error?: string };
    throw new Error(data.error ?? `Checkout failed (${res.status})`);
  }
}

/** Create a branch from HEAD and check it out. Throws with git's message. */
export async function createWorkspaceBranch(
  workspacePath: string,
  name: string,
): Promise<void> {
  const res = await fetch(`${SSE_BASE_URL}/workspace/branch/create`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ workspacePath, name }),
  });
  if (!res.ok) {
    const data = (await res.json().catch(() => ({}))) as { error?: string };
    throw new Error(data.error ?? `Branch creation failed (${res.status})`);
  }
}

export interface OpenInApp {
  id: string;
  name: string;
  kind: "editor" | "terminal" | "files";
}

/** Installed IDEs/terminals that can open a project folder. */
export async function fetchWorkspaceApps(): Promise<OpenInApp[] | null> {
  try {
    const res = await fetch(`${SSE_BASE_URL}/workspace/apps`);
    if (!res.ok) return null;
    const data = (await res.json()) as { apps: OpenInApp[] };
    return data.apps;
  } catch {
    return null;
  }
}

/** Open the folder in an installed app. Throws with the server's message. */
export async function openWorkspaceInApp(
  workspacePath: string,
  appId: string,
): Promise<void> {
  const res = await fetch(`${SSE_BASE_URL}/workspace/open`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ workspacePath, appId }),
  });
  if (!res.ok) {
    const data = (await res.json().catch(() => ({}))) as { error?: string };
    throw new Error(data.error ?? `Could not open the folder (${res.status})`);
  }
}

export interface WorkspaceFileChange {
  path: string;
  additions: number;
  deletions: number;
  status: "modified" | "added" | "deleted";
}

/**
 * Git +/− stats for the files the agent touched in the current turn (numstat
 * against HEAD; untracked files counted as additions). Returns only paths
 * that actually changed, or null when the server is unreachable.
 */
export async function fetchWorkspaceChanges(
  workspacePath: string,
  paths: string[],
): Promise<WorkspaceFileChange[] | null> {
  try {
    const res = await fetch(`${SSE_BASE_URL}/workspace/changes`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ workspacePath, paths }),
    });
    if (!res.ok) return null;
    const data = (await res.json()) as { files: WorkspaceFileChange[] };
    return data.files;
  } catch {
    return null;
  }
}

/** All working-tree changes — the diff panel's file tree. */
export async function fetchWorkspaceStatus(
  workspacePath: string,
): Promise<WorkspaceFileChange[] | null> {
  try {
    const res = await fetch(
      `${SSE_BASE_URL}/workspace/status?workspacePath=${encodeURIComponent(workspacePath)}`,
    );
    if (!res.ok) return null;
    const data = (await res.json()) as { files: WorkspaceFileChange[] };
    return data.files;
  } catch {
    return null;
  }
}

/** Unified diff (uncommitted changes) for one file. Throws on failure. */
export async function fetchWorkspaceFileDiff(
  workspacePath: string,
  path: string,
): Promise<string> {
  const res = await fetch(
    `${SSE_BASE_URL}/workspace/file-diff?workspacePath=${encodeURIComponent(workspacePath)}&path=${encodeURIComponent(path)}`,
  );
  if (!res.ok) {
    const data = (await res.json().catch(() => ({}))) as { error?: string };
    throw new Error(data.error ?? `Could not load the diff (${res.status})`);
  }
  const data = (await res.json()) as { diff: string };
  return data.diff;
}
