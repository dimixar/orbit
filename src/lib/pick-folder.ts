/**
 * Native folder picker for the desktop app.
 *
 * Uses the Tauri dialog plugin (`dialog:default` capability is granted in
 * src-tauri/capabilities/default.json). Outside Tauri (plain-browser
 * `pnpm dev`) the native picker is unavailable, so callers check `isTauri()`
 * and fall back to a read-only label instead of a control that does nothing.
 */
import { open } from "@tauri-apps/plugin-dialog";

/** True when the frontend runs inside a Tauri webview. */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Basename of an absolute path (handles POSIX and Windows separators). */
export function pathBasename(path: string): string {
  const parts = path.replace(/[\\/]+$/, "").split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

/**
 * Opens the OS folder picker and resolves to the absolute path of the chosen
 * folder, or `null` when the user cancels or the picker is unavailable.
 */
export async function pickWorkspaceFolder(): Promise<string | null> {
  if (!isTauri()) return null;
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Choose a project folder",
  });
  return typeof selected === "string" ? selected : null;
}