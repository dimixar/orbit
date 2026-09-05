/**
 * Classify files dropped or picked on the composer.
 *
 * Images the model can see become preview chips. Any other file with a
 * path becomes an `@` mention — relative when it sits in the workspace,
 * absolute when it comes from anywhere else.
 */

export const MAX_COMPOSER_IMAGES = 8;
export const MAX_IMAGE_BYTES = 10 * 1024 * 1024;

const VISION_MIME = new Set([
  "image/png",
  "image/jpeg",
  "image/jpg",
  "image/gif",
  "image/webp",
]);

const VISION_EXT = new Set(["png", "jpg", "jpeg", "gif", "webp"]);

export type ComposerImage = {
  id: string;
  name: string;
  mimeType: string;
  size: number;
  dataUrl: string;
};

export type AttachNotice =
  | { tone: "danger" | "muted"; message: string }
  | null;

export type ClassifiedFile =
  | { kind: "image"; name: string; mimeType: string }
  | { kind: "mention"; path: string }
  | { kind: "reject"; reason: string };

export function normalizeFsPath(path: string): string {
  return path.replace(/\\/g, "/");
}

export function fileExtension(name: string): string {
  const base = name.slice(name.lastIndexOf("/") + 1);
  const dot = base.lastIndexOf(".");
  return dot > 0 ? base.slice(dot + 1).toLowerCase() : "";
}

export function isAttachableImage(name: string, mimeType?: string): boolean {
  if (mimeType && VISION_MIME.has(mimeType.toLowerCase())) return true;
  return VISION_EXT.has(fileExtension(name));
}

export function imageMimeType(name: string, mimeType?: string): string {
  if (mimeType && VISION_MIME.has(mimeType.toLowerCase())) {
    return mimeType.toLowerCase() === "image/jpg" ? "image/jpeg" : mimeType.toLowerCase();
  }
  const ext = fileExtension(name);
  if (ext === "jpg" || ext === "jpeg") return "image/jpeg";
  if (ext === "gif") return "image/gif";
  if (ext === "webp") return "image/webp";
  return "image/png";
}

/** Absolute path on the File, when the webview (Tauri) exposes it. */
export function browserFilePath(file: File): string | undefined {
  const path = (file as File & { path?: unknown }).path;
  return typeof path === "string" && path.length > 0 ? path : undefined;
}

export function toWorkspaceRelative(
  absPath: string,
  workspacePath: string,
): string | null {
  const file = normalizeFsPath(absPath);
  const root = normalizeFsPath(workspacePath).replace(/\/+$/, "");
  if (!file || !root) return null;
  const fileCmp = file.toLowerCase();
  const rootCmp = root.toLowerCase();
  if (fileCmp === rootCmp) return null;
  if (!fileCmp.startsWith(`${rootCmp}/`)) return null;
  return file.slice(root.length + 1);
}

/** Workspace-relative when possible; otherwise the absolute path. */
export function mentionPath(absPath: string, workspacePath?: string): string {
  if (workspacePath) {
    const relative = toWorkspaceRelative(absPath, workspacePath);
    if (relative) return relative;
  }
  return normalizeFsPath(absPath);
}

export function classifyComposerFile({
  name,
  mimeType,
  path,
  workspacePath,
}: {
  name: string;
  mimeType?: string;
  path?: string;
  workspacePath?: string;
}): ClassifiedFile {
  if (isAttachableImage(name, mimeType)) {
    return { kind: "image", name, mimeType: imageMimeType(name, mimeType) };
  }

  if (path) {
    return { kind: "mention", path: mentionPath(path, workspacePath) };
  }

  return {
    kind: "reject",
    reason: "Couldn't find that file's location. Use the paperclip or drop it from Finder.",
  };
}

export function appendFileMentions(
  draft: string,
  paths: string[],
): { text: string; caret: number } {
  if (paths.length === 0) return { text: draft, caret: draft.length };
  const tokens = paths.map((path) => `@${path} `).join("");
  const prefix = draft.length > 0 && !/\s$/.test(draft) ? " " : "";
  const text = `${draft}${prefix}${tokens}`;
  return { text, caret: text.length };
}

export function formatAttachSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error ?? new Error("Couldn't read that file."));
    reader.onload = () => {
      if (typeof reader.result === "string") resolve(reader.result);
      else reject(new Error("Couldn't read that file."));
    };
    reader.readAsDataURL(file);
  });
}

export function bytesToDataUrl(
  bytes: Uint8Array,
  mimeType: string,
): Promise<string> {
  const copy = new Uint8Array(bytes);
  const blob = new Blob([copy], { type: mimeType });
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error ?? new Error("Couldn't read that file."));
    reader.onload = () => {
      if (typeof reader.result === "string") resolve(reader.result);
      else reject(new Error("Couldn't read that file."));
    };
    reader.readAsDataURL(blob);
  });
}

export function imageDataUrl(data: string, mimeType?: string): string {
  if (/^data:/i.test(data)) return data;
  const mime = mimeType && mimeType.length > 0 ? mimeType : "image/png";
  return `data:${mime};base64,${data}`;
}
