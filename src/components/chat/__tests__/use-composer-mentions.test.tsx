import { describe, expect, it, vi, beforeEach } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { createRef } from "react";
import { useComposerMentions } from "../use-composer-mentions";

const fetchComposerCommands = vi.fn();
const fetchWorkspaceFiles = vi.fn();

vi.mock("@/lib/pi-client", () => ({
  fetchComposerCommands: (...args: unknown[]) => fetchComposerCommands(...args),
  fetchWorkspaceFiles: (...args: unknown[]) => fetchWorkspaceFiles(...args),
}));

describe("useComposerMentions", () => {
  beforeEach(() => {
    fetchComposerCommands.mockReset();
    fetchWorkspaceFiles.mockReset();
    fetchComposerCommands.mockResolvedValue([
      {
        id: "skill:intentui",
        kind: "skill",
        name: "skill:intentui",
        description: "Intent UI conventions",
      },
      {
        id: "prompt:review",
        kind: "prompt",
        name: "review",
        description: "Review staged changes",
      },
    ]);
    fetchWorkspaceFiles.mockResolvedValue(["src/lib/pi-client.ts"]);
  });

  it("opens on / and inserts the selected skill command", async () => {
    const textarea = document.createElement("textarea");
    document.body.appendChild(textarea);
    const textareaRef = createRef<HTMLTextAreaElement>();
    Object.defineProperty(textareaRef, "current", { value: textarea, writable: true });

    const { result, rerender } = renderHook(
      ({ draft }: { draft: string }) => {
        const setDraft = (next: string) => {
          textarea.value = next;
          rerender({ draft: next });
        };
        return useComposerMentions({
          draft,
          setDraft,
          textareaRef,
          workspacePath: "/tmp/orbit",
        });
      },
      { initialProps: { draft: "" } },
    );

    act(() => {
      result.current.onDraftChange("/", 1);
    });
    rerender({ draft: "/" });

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.open).toBe(true);
    expect(result.current.items.map((item) => item.id)).toEqual([
      "skill:intentui",
      "prompt:review",
    ]);

    act(() => {
      result.current.select(result.current.items[0]!);
    });

    expect(textarea.value).toBe("/skill:intentui ");
  });

  it("appends dropped project files as @mentions", () => {
    const textarea = document.createElement("textarea");
    document.body.appendChild(textarea);
    const textareaRef = createRef<HTMLTextAreaElement>();
    Object.defineProperty(textareaRef, "current", { value: textarea, writable: true });

    const { result, rerender } = renderHook(
      ({ draft }: { draft: string }) => {
        const setDraft = (next: string) => {
          textarea.value = next;
          rerender({ draft: next });
        };
        return useComposerMentions({
          draft,
          setDraft,
          textareaRef,
          workspacePath: "/tmp/orbit",
        });
      },
      { initialProps: { draft: "see" } },
    );

    act(() => {
      result.current.insertFileMentions(["src/lib/pi-client.ts"]);
    });

    expect(textarea.value).toBe("see @src/lib/pi-client.ts ");
  });
});
