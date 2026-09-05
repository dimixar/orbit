import { describe, expect, it } from "vitest";
import {
  fileLabel,
  filterCommands,
  filterFiles,
  findActiveMention,
  mentionInsert,
  replaceMention,
  scoreMatch,
} from "../composer-mention";

describe("findActiveMention", () => {
  it("finds a slash token at the start of the draft", () => {
    expect(findActiveMention("/rev", 4)).toEqual({
      kind: "slash",
      trigger: "/",
      query: "rev",
      start: 0,
      end: 4,
    });
  });

  it("finds an @ token after whitespace", () => {
    expect(findActiveMention("see @src/lib", 12)).toEqual({
      kind: "file",
      trigger: "@",
      query: "src/lib",
      start: 4,
      end: 12,
    });
  });

  it("ignores a slash inside a path", () => {
    expect(findActiveMention("src/lib", 7)).toBeNull();
  });

  it("ignores an @ inside an email", () => {
    expect(findActiveMention("ada@orbit.dev", 13)).toBeNull();
  });

  it("tracks the caret inside the token, not the end of the draft", () => {
    expect(findActiveMention("/review extra", 3)).toEqual({
      kind: "slash",
      trigger: "/",
      query: "re",
      start: 0,
      end: 3,
    });
  });

  it("opens on a bare trigger", () => {
    expect(findActiveMention("/", 1)?.kind).toBe("slash");
    expect(findActiveMention("@", 1)?.kind).toBe("file");
  });
});

describe("replaceMention", () => {
  it("replaces the active token and leaves a trailing space from the insert", () => {
    const mention = findActiveMention("use /sk", 7);
    expect(mention).not.toBeNull();
    expect(replaceMention("use /sk", mention!, mentionInsert("/", "skill:intentui"))).toEqual({
      text: "use /skill:intentui ",
      caret: 20,
    });
  });

  it("keeps text after the caret", () => {
    const mention = findActiveMention("/rev please", 4);
    expect(mention).not.toBeNull();
    expect(replaceMention("/rev please", mention!, mentionInsert("/", "review"))).toEqual({
      text: "/review  please",
      caret: 8,
    });
  });
});

describe("filterCommands", () => {
  const commands = [
    { kind: "prompt" as const, name: "review", description: "Review staged git changes" },
    { kind: "skill" as const, name: "skill:intentui", description: "Intent UI conventions" },
    { kind: "skill" as const, name: "skill:shadcn", description: "shadcn components" },
  ];

  it("lists skills before prompts when the query is empty", () => {
    expect(filterCommands(commands, "").map((c) => c.name)).toEqual([
      "skill:intentui",
      "skill:shadcn",
      "review",
    ]);
  });

  it("matches name or description and keeps skills first", () => {
    expect(filterCommands(commands, "intent").map((c) => c.name)).toEqual(["skill:intentui"]);
    expect(filterCommands(commands, "staged").map((c) => c.name)).toEqual(["review"]);
  });
});

describe("filterFiles", () => {
  const files = [
    "src/lib/pi-client.ts",
    "src/components/chat-panel.tsx",
    "agent/sse-server.ts",
    "README.md",
  ];

  it("prefers basename matches over path matches", () => {
    expect(filterFiles(files, "sse")[0]).toBe("agent/sse-server.ts");
  });

  it("matches a path fragment", () => {
    expect(filterFiles(files, "chat")).toEqual(["src/components/chat-panel.tsx"]);
  });
});

describe("scoreMatch and fileLabel", () => {
  it("scores prefix higher than a mid-string hit", () => {
    expect(scoreMatch("review", "re")).toBeGreaterThan(scoreMatch("preview", "re"));
  });

  it("splits a nested path into name and directory", () => {
    expect(fileLabel("src/lib/pi-client.ts")).toEqual({
      name: "pi-client.ts",
      dir: "src/lib",
    });
    expect(fileLabel("README.md")).toEqual({ name: "README.md", dir: "" });
  });
});
