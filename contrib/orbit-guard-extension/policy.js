/**
 * Orbit guard policy — pure, offline-testable access-mode decisions.
 *
 * Orbit runs with one of three access modes. For every tool call pi emits on
 * its `tool_call` hook, this module answers whether the call is allowed
 * outright or must be confirmed by the user:
 *
 *   supervised        read → allow · edit/exec/other → ask
 *   auto-accept-edits read/edit → allow · exec/other → ask
 *   full-access       allow everything (no prompts)
 *
 * `full-access` is the default, matching the app's historical behavior; the
 * mode is written by Orbit to `~/.orbit-pi/access.json` and read fresh on
 * every tool call, so changing it in the app re-arms live sessions without a
 * restart.
 */
import fs from "node:fs";

/** Fallback when the file is missing, unreadable, or names an unknown mode. */
export const DEFAULT_MODE = "full-access";

/** Every mode Orbit can write. Kept in sync with `crate::access::AccessMode`. */
export const MODES = ["supervised", "auto-accept-edits", "full-access"];

/** Tools that only observe: they never mutate the workspace. */
const READ_TOOLS = new Set(["read", "grep", "find", "ls", "glob", "list", "webfetch"]);

/** Tools that write files. */
const EDIT_TOOLS = new Set(["edit", "write", "multiedit", "apply_patch", "str_replace_editor"]);

/** Tools that run a process / shell command. */
const EXEC_TOOLS = new Set(["bash", "powershell", "shell", "exec", "terminal"]);

/**
 * Coerce an arbitrary value to a known mode, falling back to
 * [`DEFAULT_MODE`]. Unknown values never allow more than intended.
 */
export function normalizeMode(raw) {
  return typeof raw === "string" && MODES.includes(raw) ? raw : DEFAULT_MODE;
}

/**
 * Classify a pi tool name into a risk kind: `"read"`, `"edit"`, `"exec"`, or
 * `"other"` (extension / custom tools, which are treated conservatively).
 */
export function classify(toolName) {
  const name = String(toolName ?? "").toLowerCase();
  if (READ_TOOLS.has(name)) return "read";
  if (EDIT_TOOLS.has(name)) return "edit";
  if (EXEC_TOOLS.has(name)) return "exec";
  return "other";
}

/**
 * The decision for one tool call under `mode`: `"allow"` or `"ask"`. There is
 * intentionally no `"block"` — the guard prompts rather than silently denying,
 * and a failed prompt is treated as a denial by the caller (fail-safe).
 */
export function decide(mode, toolName) {
  const kind = classify(toolName);
  const resolved = normalizeMode(mode);
  if (resolved === "full-access") return "allow";
  // Reads are non-mutating, so every confined mode allows them.
  if (kind === "read") return "allow";
  if (resolved === "auto-accept-edits" && kind === "edit") return "allow";
  return "ask";
}

/** The path Orbit writes the active mode to. */
export function modeFilePath() {
  const home = process.env.HOME || process.env.USERPROFILE || ".";
  return `${home}/.orbit-pi/access.json`;
}

/**
 * Read the active mode from disk. Read fresh on every call so a change in the
 * app applies to a running session; any failure degrades to
 * [`DEFAULT_MODE`].
 */
export function readMode(filePath = modeFilePath()) {
  try {
    const parsed = JSON.parse(fs.readFileSync(filePath, "utf8"));
    return normalizeMode(parsed?.mode);
  } catch {
    return DEFAULT_MODE;
  }
}

/** A short, single-line summary of a tool call for the confirmation body. */
export function summarize(toolName, input) {
  const name = String(toolName ?? "tool");
  const value = input ?? {};
  const first = (...keys) => {
    for (const key of keys) {
      if (typeof value[key] === "string" && value[key].trim() !== "") return value[key];
    }
    return null;
  };
  const detail =
    first("command", "path", "file_path", "filename", "pattern", "url", "query") ??
    JSON.stringify(value);
  const text = String(detail ?? "").replace(/\s+/g, " ").trim();
  return text.length > 200 ? `${name}: ${text.slice(0, 200)}…` : `${name}: ${text}`;
}
