/**
 * Shared Pi SSE client — the browser half of the `@assistant-ui/react-pi`
 * HTTP/SSE transport. Points at `agent/sse-server.ts` (run: `pnpm agent:sse`).
 */

import { createPiHttpClient } from "@assistant-ui/react-pi";

export const SSE_BASE_URL = "http://localhost:8913";

export const piClient = createPiHttpClient({ baseUrl: SSE_BASE_URL });

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
