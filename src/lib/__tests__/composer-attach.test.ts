import { describe, expect, it } from "vitest";
import {
  appendFileMentions,
  classifyComposerFile,
  imageDataUrl,
  imageMimeType,
  isAttachableImage,
  mentionPath,
  toWorkspaceRelative,
} from "../composer-attach";

describe("isAttachableImage", () => {
  it("accepts vision mime types and extensions", () => {
    expect(isAttachableImage("shot.PNG", "image/png")).toBe(true);
    expect(isAttachableImage("shot.jpg")).toBe(true);
    expect(isAttachableImage("shot.webp", "image/webp")).toBe(true);
  });

  it("rejects svg and documents", () => {
    expect(isAttachableImage("mark.svg", "image/svg+xml")).toBe(false);
    expect(isAttachableImage("notes.ts")).toBe(false);
  });
});

describe("toWorkspaceRelative", () => {
  it("strips the workspace root with mixed separators", () => {
    expect(
      toWorkspaceRelative(
        "/Users/ada/orbit/src/lib/pi-client.ts",
        "/Users/ada/orbit",
      ),
    ).toBe("src/lib/pi-client.ts");
    expect(
      toWorkspaceRelative("C:\\Users\\ada\\orbit\\src\\app.tsx", "C:\\Users\\ada\\orbit"),
    ).toBe("src/app.tsx");
  });

  it("rejects the workspace itself and files outside it", () => {
    expect(toWorkspaceRelative("/Users/ada/orbit", "/Users/ada/orbit")).toBeNull();
    expect(
      toWorkspaceRelative("/Users/ada/other/file.ts", "/Users/ada/orbit"),
    ).toBeNull();
  });
});

describe("classifyComposerFile", () => {
  it("classifies a path-only png from a desktop drop", () => {
    expect(
      classifyComposerFile({
        name: "shot.png",
        path: "/Users/ada/Desktop/shot.png",
        workspacePath: "/Users/ada/orbit",
      }),
    ).toEqual({ kind: "image", name: "shot.png", mimeType: "image/png" });
  });

  it("classifies images before path mentions", () => {
    expect(
      classifyComposerFile({
        name: "shot.png",
        mimeType: "image/png",
        path: "/Users/ada/orbit/shot.png",
        workspacePath: "/Users/ada/orbit",
      }),
    ).toEqual({ kind: "image", name: "shot.png", mimeType: "image/png" });
  });

  it("mentions project files that are not images", () => {
    expect(
      classifyComposerFile({
        name: "pi-client.ts",
        mimeType: "text/plain",
        path: "/Users/ada/orbit/src/lib/pi-client.ts",
        workspacePath: "/Users/ada/orbit",
      }),
    ).toEqual({ kind: "mention", path: "src/lib/pi-client.ts" });
  });

  it("mentions files from any directory by absolute path", () => {
    expect(
      classifyComposerFile({
        name: "notes.md",
        mimeType: "text/plain",
        path: "/Users/ada/Downloads/notes.md",
        workspacePath: "/Users/ada/orbit",
      }),
    ).toEqual({ kind: "mention", path: "/Users/ada/Downloads/notes.md" });
  });

  it("mentions an absolute path when no workspace is open", () => {
    expect(
      classifyComposerFile({
        name: "notes.md",
        path: "/Users/ada/Desktop/notes.md",
      }),
    ).toEqual({ kind: "mention", path: "/Users/ada/Desktop/notes.md" });
  });

  it("rejects documents without a filesystem path", () => {
    const result = classifyComposerFile({
      name: "notes.ts",
      mimeType: "text/plain",
    });
    expect(result.kind).toBe("reject");
  });
});

describe("mentionPath", () => {
  it("prefers a workspace-relative path, otherwise keeps the absolute path", () => {
    expect(mentionPath("/Users/ada/orbit/src/app.tsx", "/Users/ada/orbit")).toBe(
      "src/app.tsx",
    );
    expect(mentionPath("/Users/ada/Downloads/shot.png", "/Users/ada/orbit")).toBe(
      "/Users/ada/Downloads/shot.png",
    );
  });
});

describe("appendFileMentions", () => {
  it("adds spaced @tokens at the end of the draft", () => {
    expect(appendFileMentions("look at", ["src/a.ts", "src/b.ts"])).toEqual({
      text: "look at @src/a.ts @src/b.ts ",
      caret: "look at @src/a.ts @src/b.ts ".length,
    });
  });

  it("does not add a leading space on an empty draft", () => {
    expect(appendFileMentions("", ["README.md"])).toEqual({
      text: "@README.md ",
      caret: "@README.md ".length,
    });
  });
});

describe("image helpers", () => {
  it("normalizes jpeg mime from a jpg name", () => {
    expect(imageMimeType("photo.jpg")).toBe("image/jpeg");
  });

  it("wraps raw base64 as a data URL", () => {
    expect(imageDataUrl("AAAA", "image/png")).toBe("data:image/png;base64,AAAA");
    expect(imageDataUrl("data:image/gif;base64,BB")).toBe("data:image/gif;base64,BB");
  });
});
