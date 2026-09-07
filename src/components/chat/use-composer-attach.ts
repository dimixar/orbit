"use client";

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ClipboardEvent,
  type DragEvent,
} from "react";
import {
  browserFilePath,
  bytesToDataUrl,
  classifyComposerFile,
  imageMimeType,
  isAttachableImage,
  MAX_COMPOSER_IMAGES,
  MAX_IMAGE_BYTES,
  readFileAsDataUrl,
  type AttachNotice,
  type ComposerImage,
} from "@/lib/composer-attach";
import {
  isTauri,
  listenTauriFileDrop,
  pathBasename,
  pickComposerFiles,
  readLocalFileBytes,
} from "@/lib/tauri-files";

function hasFilePayload(event: DragEvent): boolean {
  return Array.from(event.dataTransfer.types).includes("Files");
}

export function useComposerAttach({
  workspacePath,
  insertFileMentions,
  onDragStart,
}: {
  workspacePath?: string;
  insertFileMentions: (paths: string[]) => void;
  onDragStart?: () => void;
}) {
  const [images, setImages] = useState<ComposerImage[]>([]);
  const [notice, setNotice] = useState<AttachNotice>(null);
  const [dragActive, setDragActive] = useState(false);
  const dragDepth = useRef(0);
  const imageCountRef = useRef(0);
  imageCountRef.current = images.length;

  const finishIngest = useCallback(
    (mentions: string[], next: ComposerImage[], rejects: string[]) => {
      if (mentions.length > 0) insertFileMentions(mentions);
      if (next.length > 0) setImages((current) => [...current, ...next]);
      setNotice(
        rejects.length > 0
          ? { tone: "danger", message: [...new Set(rejects)].join(" ") }
          : null,
      );
    },
    [insertFileMentions],
  );

  const ingest = useCallback(
    async (files: File[]) => {
      if (files.length === 0) return;
      const mentions: string[] = [];
      const rejects: string[] = [];
      const next: ComposerImage[] = [];
      let imageCount = imageCountRef.current;

      for (const file of files) {
        const classified = classifyComposerFile({
          name: file.name,
          mimeType: file.type,
          path: browserFilePath(file),
          workspacePath,
        });

        if (classified.kind === "mention") {
          mentions.push(classified.path);
          continue;
        }

        if (classified.kind === "reject") {
          rejects.push(classified.reason);
          continue;
        }

        if (file.size > MAX_IMAGE_BYTES) {
          rejects.push(`${file.name} is larger than 10 MB.`);
          continue;
        }

        if (imageCount >= MAX_COMPOSER_IMAGES) {
          rejects.push(`You can attach up to ${MAX_COMPOSER_IMAGES} images.`);
          break;
        }

        try {
          const dataUrl = await readFileAsDataUrl(file);
          next.push({
            id: crypto.randomUUID(),
            name: file.name,
            mimeType: classified.mimeType,
            size: file.size,
            dataUrl,
          });
          imageCount += 1;
        } catch {
          rejects.push(`Couldn't read ${file.name}.`);
        }
      }

      finishIngest(mentions, next, rejects);
    },
    [finishIngest, workspacePath],
  );

  const ingestPaths = useCallback(
    async (paths: string[]) => {
      if (paths.length === 0) return;
      const mentions: string[] = [];
      const rejects: string[] = [];
      const next: ComposerImage[] = [];
      let imageCount = imageCountRef.current;

      for (const path of paths) {
        const name = pathBasename(path);
        const classified = classifyComposerFile({
          name,
          mimeType: isAttachableImage(name) ? imageMimeType(name) : undefined,
          path,
          workspacePath,
        });

        if (classified.kind === "mention") {
          mentions.push(classified.path);
          continue;
        }

        if (classified.kind === "reject") {
          rejects.push(classified.reason);
          continue;
        }

        if (imageCount >= MAX_COMPOSER_IMAGES) {
          rejects.push(`You can attach up to ${MAX_COMPOSER_IMAGES} images.`);
          break;
        }

        try {
          const bytes = await readLocalFileBytes(path);
          if (bytes.byteLength > MAX_IMAGE_BYTES) {
            rejects.push(`${name} is larger than 10 MB.`);
            continue;
          }
          const dataUrl = await bytesToDataUrl(bytes, classified.mimeType);
          next.push({
            id: crypto.randomUUID(),
            name,
            mimeType: classified.mimeType,
            size: bytes.byteLength,
            dataUrl,
          });
          imageCount += 1;
        } catch {
          rejects.push(`Couldn't read ${name}.`);
        }
      }

      finishIngest(mentions, next, rejects);
    },
    [finishIngest, workspacePath],
  );

  const removeImage = useCallback((id: string) => {
    setImages((current) => current.filter((image) => image.id !== id));
  }, []);

  const clearImages = useCallback(() => {
    setImages([]);
    setNotice(null);
  }, []);

  const pickFiles = useCallback(async () => {
    if (isTauri()) {
      try {
        const paths = await pickComposerFiles();
        if (paths?.length) await ingestPaths(paths);
      } catch (error) {
        console.error("[orbit] attach picker failed:", error);
        setNotice({
          tone: "danger",
          message: "Couldn't open the file picker.",
        });
      }
      return;
    }
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = true;
    input.addEventListener("change", () => {
      void ingest(Array.from(input.files ?? []));
    });
    input.click();
  }, [ingest, ingestPaths]);

  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listenTauriFileDrop((event) => {
      if (event.type === "enter" || event.type === "over") {
        setDragActive(true);
        onDragStart?.();
        return;
      }
      if (event.type === "leave") {
        setDragActive(false);
        return;
      }
      setDragActive(false);
      if (event.paths?.length) void ingestPaths(event.paths);
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [ingestPaths, onDragStart]);

  const onDragEnter = useCallback(
    (event: DragEvent<HTMLElement>) => {
      if (!hasFilePayload(event)) return;
      event.preventDefault();
      event.stopPropagation();
      dragDepth.current += 1;
      if (dragDepth.current === 1) {
        setDragActive(true);
        onDragStart?.();
      }
    },
    [onDragStart],
  );

  const onDragOver = useCallback((event: DragEvent<HTMLElement>) => {
    if (!hasFilePayload(event)) return;
    event.preventDefault();
    event.stopPropagation();
    event.dataTransfer.dropEffect = "copy";
  }, []);

  const onDragLeave = useCallback((event: DragEvent<HTMLElement>) => {
    if (!hasFilePayload(event)) return;
    event.preventDefault();
    event.stopPropagation();
    dragDepth.current -= 1;
    if (dragDepth.current <= 0) {
      dragDepth.current = 0;
      setDragActive(false);
    }
  }, []);

  const onDrop = useCallback(
    (event: DragEvent<HTMLElement>) => {
      event.preventDefault();
      event.stopPropagation();
      dragDepth.current = 0;
      setDragActive(false);
      const files = Array.from(event.dataTransfer.files);
      if (files.length > 0) {
        void ingest(files);
        return;
      }
      const uriList = event.dataTransfer.getData("text/uri-list");
      if (uriList) {
        const paths = uriList
          .split("\n")
          .map((line) => line.trim())
          .filter((line) => line && !line.startsWith("#"))
          .map((uri) => {
            try {
              return decodeURIComponent(new URL(uri).pathname);
            } catch {
              return "";
            }
          })
          .filter(Boolean);
        if (paths.length > 0) void ingestPaths(paths);
      }
    },
    [ingest, ingestPaths],
  );

  const onPaste = useCallback(
    (event: ClipboardEvent<HTMLTextAreaElement>) => {
      const files = Array.from(event.clipboardData.items)
        .filter((item) => item.kind === "file")
        .map((item) => item.getAsFile())
        .filter((file): file is File => file !== null);
      if (files.length === 0) return;
      event.preventDefault();
      void ingest(files);
    },
    [ingest],
  );

  const onSelect = useCallback(
    (list: FileList | null) => {
      if (!list || list.length === 0) return;
      void ingest(Array.from(list));
    },
    [ingest],
  );

  return {
    images,
    notice,
    dragActive,
    ingest,
    ingestPaths,
    removeImage,
    clearImages,
    pickFiles,
    onDragEnter,
    onDragOver,
    onDragLeave,
    onDrop,
    onPaste,
    onSelect,
  };
}
