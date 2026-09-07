/**
 * Desktop file I/O for the composer: native picker, path reads, and OS drops.
 *
 * Tauri intercepts OS file drags (HTML5 drop never fires) and WKWebView
 * blocks programmatic `<input type="file">` clicks. The dialog plugin and
 * `onDragDropEvent` are the paths that actually work in the app.
 */

import { isTauri, pathBasename } from "@/lib/pick-folder";

export { isTauri, pathBasename };

export async function pickComposerFiles(): Promise<string[] | null> {
  if (!isTauri()) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const { homeDir } = await import("@tauri-apps/api/path");
  let defaultPath: string | undefined;
  try {
    defaultPath = await homeDir();
  } catch {
    defaultPath = undefined;
  }
  const selected = await open({
    multiple: true,
    directory: false,
    title: "Attach files",
    defaultPath,
  });
  if (selected == null) return null;
  return Array.isArray(selected) ? selected : [selected];
}

export async function readLocalFileBytes(path: string): Promise<Uint8Array> {
  const { invoke } = await import("@tauri-apps/api/core");
  const data = await invoke<number[]>("read_local_file", { path });
  return new Uint8Array(data);
}

export async function listenTauriFileDrop(
  handler: (event: {
    type: "enter" | "over" | "drop" | "leave";
    paths?: string[];
  }) => void,
): Promise<() => void> {
  if (!isTauri()) return () => {};
  const { getCurrentWebview } = await import("@tauri-apps/api/webview");
  return getCurrentWebview().onDragDropEvent((event) => {
    const payload = event.payload;
    if (payload.type === "enter") {
      handler({ type: "enter", paths: payload.paths });
    } else if (payload.type === "over") {
      handler({ type: "over" });
    } else if (payload.type === "drop") {
      handler({ type: "drop", paths: payload.paths });
    } else {
      handler({ type: "leave" });
    }
  });
}
