import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { AgentMessage } from "../AgentMessage";
import type { AgentMessage as AgentMessageType } from "@/lib/agent/types";

// Shiki highlighting is async; the CodeBlock falls back to plain text until
// it resolves, which is enough to assert content presence.
vi.mock("@/lib/markdown/highlighter", () => ({
  highlightCode: () => Promise.resolve("<pre class='shiki'><code>code</code></pre>"),
  normalizeLanguage: (l?: string) => l ?? "text",
}));

function makeMessage(parts: AgentMessageType["parts"]): AgentMessageType {
  return {
    id: "msg_1",
    role: "assistant",
    runId: "run_1",
    createdAt: new Date().toISOString(),
    status: "complete",
    parts,
  };
}

describe("AgentMessage", () => {
  it("renders a text part as markdown", async () => {
    render(
      <AgentMessage
        message={makeMessage([{ type: "text", content: "## Done\n\nAll good." }])}
      />,
    );
    expect(await screen.findByRole("heading", { level: 2 })).toHaveTextContent("Done");
    expect(screen.getByText("All good.")).toBeInTheDocument();
  });

  it("renders a reasoning part", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    render(
      <AgentMessage
        message={makeMessage([
          { type: "reasoning", content: "Inspecting auth…", status: "complete" },
        ])}
      />,
    );
    expect(screen.getByText("Working notes")).toBeInTheDocument();
    // The panel is collapsed by default — expand it to reveal the content.
    await userEvent.click(screen.getByRole("button", { name: "Working notes" }));
    expect(screen.getByText("Inspecting auth…")).toBeInTheDocument();
  });

  it("renders a plan part with step statuses", () => {
    render(
      <AgentMessage
        message={makeMessage([
          {
            type: "plan",
            steps: [
              { id: "1", title: "Inspect", status: "complete" },
              { id: "2", title: "Fix", status: "running" },
              { id: "3", title: "Test", status: "pending" },
            ],
          },
        ])}
      />,
    );
    expect(screen.getByText("Plan")).toBeInTheDocument();
    expect(screen.getByText("Inspect")).toBeInTheDocument();
    expect(screen.getByText("Fix")).toBeInTheDocument();
    expect(screen.getByText("Test")).toBeInTheDocument();
  });

  it("renders a tool call with input and status", () => {
    render(
      <AgentMessage
        message={makeMessage([
          {
            type: "tool_call",
            id: "call_1",
            tool: "read_file",
            status: "success",
            input: { path: "src/auth.ts" },
            durationMs: 214,
          },
        ])}
      />,
    );
    expect(screen.getByText("read_file")).toBeInTheDocument();
    expect(screen.getByText("src/auth.ts")).toBeInTheDocument();
    expect(screen.getByText("214ms")).toBeInTheDocument();
  });

  it("renders a tool result with output", () => {
    render(
      <AgentMessage
        message={makeMessage([
          {
            type: "tool_result",
            toolCallId: "call_1",
            status: "success",
            output: "42 tests passed",
            durationMs: 3421,
          },
        ])}
      />,
    );
    expect(screen.getByText("Tool output")).toBeInTheDocument();
    expect(screen.getByText("42 tests passed")).toBeInTheDocument();
  });

  it("renders a diff with additions and deletions", () => {
    render(
      <AgentMessage
        message={makeMessage([
          {
            type: "diff",
            file: "src/auth.ts",
            patch: "--- a/src/auth.ts\n+++ b/src/auth.ts\n@@ -1 +1 @@\n-old\n+new",
            additions: 1,
            deletions: 1,
          },
        ])}
      />,
    );
    expect(screen.getByText("src/auth.ts")).toBeInTheDocument();
    expect(screen.getByText("+1")).toBeInTheDocument();
    expect(screen.getByText("-1")).toBeInTheDocument();
    expect(screen.getByText("+new")).toBeInTheDocument();
  });

  it("renders a code part", () => {
    render(
      <AgentMessage
        message={makeMessage([
          { type: "code", language: "ts", code: "const x = 1", filename: "x.ts" },
        ])}
      />,
    );
    expect(screen.getByText("x.ts")).toBeInTheDocument();
    expect(screen.getByText("const x = 1")).toBeInTheDocument();
  });

  it("renders a file change with action badge", () => {
    render(
      <AgentMessage
        message={makeMessage([
          { type: "file", path: "src/auth.ts", action: "modified" },
        ])}
      />,
    );
    expect(screen.getByText("Modified")).toBeInTheDocument();
    expect(screen.getByText("src/auth.ts")).toBeInTheDocument();
  });

  it("renders an error part without breaking the message", () => {
    render(
      <AgentMessage
        message={makeMessage([
          { type: "text", content: "Before" },
          { type: "error", message: "Command failed", details: { exitCode: 1 } },
          { type: "text", content: "After" },
        ])}
      />,
    );
    expect(screen.getByText("Command failed")).toBeInTheDocument();
    expect(screen.getByText("Before")).toBeInTheDocument();
    expect(screen.getByText("After")).toBeInTheDocument();
  });

  it("renders an approval part and calls callbacks", async () => {
    const onApprove = vi.fn();
    const onReject = vi.fn();
    const { default: userEvent } = await import("@testing-library/user-event");
    render(
      <AgentMessage
        message={makeMessage([
          {
            type: "approval",
            id: "appr_1",
            title: "Run: npm install",
            status: "pending",
          },
        ])}
        onApprove={onApprove}
        onReject={onReject}
      />,
    );
    expect(screen.getByText("Run: npm install")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Approve" }));
    expect(onApprove).toHaveBeenCalledWith("appr_1");
    expect(screen.getByText("Approved")).toBeInTheDocument();
  });

  it("shows a streaming badge while the message is streaming", () => {
    render(
      <AgentMessage
        message={{
          ...makeMessage([]),
          status: "streaming",
        }}
      />,
    );
    expect(screen.getByText("Streaming")).toBeInTheDocument();
  });

  it("shows a thinking placeholder for an empty streaming message", () => {
    render(
      <AgentMessage
        message={{
          ...makeMessage([]),
          status: "streaming",
        }}
      />,
    );
    expect(screen.getByText("Thinking…")).toBeInTheDocument();
  });
});
