import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { createRef } from "react";
import { ComposerMentionMenu } from "../ComposerMentionMenu";

describe("ComposerMentionMenu", () => {
  it("groups skills and prompts and selects on press", () => {
    const onSelect = vi.fn();
    const triggerRef = createRef<HTMLDivElement>();
    render(
      <div>
        <div ref={triggerRef} />
        <ComposerMentionMenu
          open
          triggerRef={triggerRef}
          kind="slash"
          items={[
            {
              id: "skill:intentui",
              kind: "skill",
              name: "skill:intentui",
              description: "Intent UI conventions",
              insert: "/skill:intentui ",
            },
            {
              id: "prompt:review",
              kind: "prompt",
              name: "review",
              description: "Review staged changes",
              insert: "/review ",
            },
          ]}
          highlightedIndex={0}
          loading={false}
          error={false}
          emptyMessage="No skills or prompts installed."
          listboxId="composer-mention-list"
          onHighlight={vi.fn()}
          onSelect={onSelect}
          onOpenChange={vi.fn()}
        />
      </div>,
    );

    expect(screen.getByText("Skills")).toBeInTheDocument();
    expect(screen.getByText("Prompts")).toBeInTheDocument();
    expect(screen.getByText("/skill:intentui")).toBeInTheDocument();
    expect(screen.getByText("/review")).toBeInTheDocument();

    screen.getByText("/review").closest("[role=option]")?.dispatchEvent(
      new MouseEvent("mousedown", { bubbles: true }),
    );
    expect(onSelect).toHaveBeenCalledWith(
      expect.objectContaining({ id: "prompt:review" }),
    );
  });

  it("shows file names with their directory", () => {
    const triggerRef = createRef<HTMLDivElement>();
    render(
      <div>
        <div ref={triggerRef} />
        <ComposerMentionMenu
          open
          triggerRef={triggerRef}
          kind="file"
          items={[
            {
              id: "src/lib/pi-client.ts",
              kind: "file",
              path: "src/lib/pi-client.ts",
              insert: "@src/lib/pi-client.ts ",
            },
          ]}
          highlightedIndex={0}
          loading={false}
          error={false}
          emptyMessage="No project files found."
          listboxId="composer-mention-list"
          onHighlight={vi.fn()}
          onSelect={vi.fn()}
          onOpenChange={vi.fn()}
        />
      </div>,
    );

    expect(screen.getByText("pi-client.ts")).toBeInTheDocument();
    expect(screen.getByText("src/lib")).toBeInTheDocument();
    expect(document.querySelector("[data-file-type=ts]")).toBeInTheDocument();
    expect(screen.getByText("pi-client.ts").parentElement).toHaveClass("gap-3");
  });
});
