import { describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useComposerAttach } from "../use-composer-attach";

vi.mock("@/lib/tauri-files", () => ({
  isTauri: () => false,
  pathBasename: (path: string) => path.split(/[\\/]/).pop() || path,
  pickComposerFiles: vi.fn(),
  readLocalFileBytes: vi.fn(async () => new Uint8Array([1, 2, 3, 4])),
  listenTauriFileDrop: vi.fn(async () => () => {}),
}));

function file(name: string, type: string, path?: string): File {
  const next = new File(["pixels"], name, { type });
  if (path) Object.defineProperty(next, "path", { value: path });
  return next;
}

describe("useComposerAttach", () => {
  it("stages images and mentions project files", async () => {
    const insertFileMentions = vi.fn();
    const { result } = renderHook(() =>
      useComposerAttach({
        workspacePath: "/Users/ada/orbit",
        insertFileMentions,
      }),
    );

    await act(async () => {
      await result.current.ingest([
        file("shot.png", "image/png"),
        file("pi-client.ts", "text/plain", "/Users/ada/orbit/src/lib/pi-client.ts"),
      ]);
    });

    expect(result.current.images).toHaveLength(1);
    expect(result.current.images[0]!.name).toBe("shot.png");
    expect(insertFileMentions).toHaveBeenCalledWith(["src/lib/pi-client.ts"]);
    expect(result.current.notice).toBeNull();
  });

  it("rejects a document with no project path", async () => {
    const insertFileMentions = vi.fn();
    const { result } = renderHook(() =>
      useComposerAttach({
        workspacePath: "/Users/ada/orbit",
        insertFileMentions,
      }),
    );

    await act(async () => {
      await result.current.ingest([file("notes.ts", "text/plain")]);
    });

    expect(result.current.images).toHaveLength(0);
    expect(insertFileMentions).not.toHaveBeenCalled();
    expect(result.current.notice?.tone).toBe("danger");
  });

  it("stages images and mentions from absolute paths", async () => {
    const insertFileMentions = vi.fn();
    const { result } = renderHook(() =>
      useComposerAttach({
        workspacePath: "/Users/ada/orbit",
        insertFileMentions,
      }),
    );

    await act(async () => {
      await result.current.ingestPaths([
        "/Users/ada/orbit/docs/shot.png",
        "/Users/ada/orbit/src/lib/pi-client.ts",
      ]);
    });

    expect(result.current.images).toHaveLength(1);
    expect(result.current.images[0]!.name).toBe("shot.png");
    expect(insertFileMentions).toHaveBeenCalledWith(["src/lib/pi-client.ts"]);
  });

  it("mentions files dropped from outside the workspace", async () => {
    const insertFileMentions = vi.fn();
    const { result } = renderHook(() =>
      useComposerAttach({
        workspacePath: "/Users/ada/orbit",
        insertFileMentions,
      }),
    );

    await act(async () => {
      await result.current.ingestPaths(["/Users/ada/Downloads/brief.md"]);
    });

    expect(insertFileMentions).toHaveBeenCalledWith([
      "/Users/ada/Downloads/brief.md",
    ]);
    expect(result.current.notice).toBeNull();
  });
});
