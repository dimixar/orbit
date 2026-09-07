import { describe, expect, it } from "vitest";
import { fileTypeKey } from "../file-type-icon";

describe("fileTypeKey", () => {
  it("uses the last extension", () => {
    expect(fileTypeKey("src/lib/pi-client.ts")).toBe("ts");
    expect(fileTypeKey("ComposerMentionMenu.tsx")).toBe("tsx");
    expect(fileTypeKey("README.md")).toBe("md");
  });

  it("recognizes dotfiles and basenames without an extension", () => {
    expect(fileTypeKey(".gitignore")).toBe("gitignore");
    expect(fileTypeKey("Dockerfile")).toBe("dockerfile");
    expect(fileTypeKey("Makefile")).toBe("makefile");
  });
});
