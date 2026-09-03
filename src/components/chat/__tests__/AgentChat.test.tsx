import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { AgentChat } from "../AgentChat";
import { createMockAdapter } from "@/lib/agent/adapters/mockAdapter";
import { agentStore } from "@/lib/agent/store";

vi.mock("@/lib/markdown/highlighter", () => ({
  highlightCode: (code: string) =>
    Promise.resolve(`<pre class="shiki"><code>${code}</code></pre>`),
  normalizeLanguage: (l?: string) => l ?? "text",
}));

vi.mock("../markdown/MermaidViewer", () => ({
  MermaidViewer: ({ code }: { code: string }) => (
    <div data-testid="mermaid">{code}</div>
  ),
}));

describe("AgentChat streaming pipeline", () => {
  it("streams the full mock run into structured parts incrementally", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    const adapter = createMockAdapter({ interval: 5 });
    agentStore.clearAll();

    render(<AgentChat adapter={adapter} />);

    const input = screen.getByRole("textbox", { name: "Prompt" });
    await userEvent.type(input, "Fix the authentication bug");
    await userEvent.click(screen.getByRole("button", { name: "Send" }));

    // User message appears immediately.
    expect(screen.getByText("Fix the authentication bug")).toBeInTheDocument();

    // Plan streams in and progresses as tools complete.
    await waitFor(() => {
      expect(screen.getByText("Plan")).toBeInTheDocument();
    });
    await waitFor(() => {
      expect(screen.getByText("Inspect auth middleware")).toBeInTheDocument();
    });
    await waitFor(() => {
      // Step 1 completes once read_file finishes.
      expect(screen.getByText("Complete")).toBeInTheDocument();
    });

    // Tool calls stream in with their results.
    await waitFor(() => {
      expect(screen.getByText("read_file")).toBeInTheDocument();
    });
    await waitFor(() => {
      expect(screen.getByText("edit_file")).toBeInTheDocument();
    });
    await waitFor(() => {
      expect(screen.getByText("terminal")).toBeInTheDocument();
    });

    // Diff appears (the file name shows in the tool input and the diff header).
    await waitFor(() => {
      expect(screen.getAllByText("src/auth.ts").length).toBeGreaterThan(0);
    });

    // Final markdown text streams in.
    await waitFor(() => {
      expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("Fixed");
    });
    expect(screen.getAllByText(/42 passed/).length).toBeGreaterThan(0);

    // The run completes.
    await waitFor(() => {
      expect(agentStore.getState().runs).toMatchObject({
        [Object.keys(agentStore.getState().runs)[0]!]: {
          status: "complete",
        },
      });
    });
  }, 20000);

  it("supports stopping a running run", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    const adapter = createMockAdapter({ interval: 50 });
    agentStore.clearAll();

    render(<AgentChat adapter={adapter} />);

    const input = screen.getByRole("textbox", { name: "Prompt" });
    await userEvent.type(input, "Do something");
    await userEvent.click(screen.getByRole("button", { name: "Send" }));

    // Stop button appears while streaming (input is empty after send).
    const stop = await screen.findByRole("button", { name: "Stop generation" });
    await userEvent.click(stop);

    await waitFor(() => {
      const run = Object.values(agentStore.getState().runs)[0];
      expect(run?.status).toBe("complete");
    });
  }, 20000);

  it("turns the send button into stop while processing and back to send when typing", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    const adapter = createMockAdapter({ interval: 50 });
    agentStore.clearAll();

    render(<AgentChat adapter={adapter} />);

    const input = screen.getByRole("textbox", { name: "Prompt" });
    await userEvent.type(input, "First prompt");
    await userEvent.click(screen.getByRole("button", { name: "Send" }));

    // While streaming with an empty input, the button is Stop.
    const stop = await screen.findByRole("button", { name: "Stop generation" });
    expect(stop).toBeInTheDocument();

    // Typing a new message turns it back into a Send button (queues).
    await userEvent.type(input, "Second prompt");
    expect(screen.getByRole("button", { name: "Send" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Stop generation" })).toBeNull();
  }, 20000);

  it("queues new messages while the agent is processing and runs them in order", async () => {
    const { default: userEvent } = await import("@testing-library/user-event");
    const adapter = createMockAdapter({ interval: 5 });
    agentStore.clearAll();

    render(<AgentChat adapter={adapter} />);

    const input = screen.getByRole("textbox", { name: "Prompt" });

    // Send the first prompt and wait for the run to start streaming.
    await userEvent.type(input, "First prompt");
    await userEvent.click(screen.getByRole("button", { name: "Send" }));
    await screen.findByText("First prompt");

    // While streaming, send a second prompt — it should be queued.
    await userEvent.type(input, "Second prompt");
    await userEvent.click(screen.getByRole("button", { name: "Send" }));

    // The queued message shows with a Queued badge.
    const queued = await screen.findByText("Queued");
    expect(queued).toBeInTheDocument();
    expect(screen.getByText("Second prompt")).toBeInTheDocument();
    expect(screen.getByText("1 queued")).toBeInTheDocument();

    // Once the first run completes, the second run starts: the queued badge
    // disappears and a second agent message appears.
    await waitFor(
      () => {
        expect(screen.queryByText("Queued")).toBeNull();
        const runs = Object.values(agentStore.getState().runs);
        expect(runs.length).toBe(2);
        expect(runs[1]?.status).toBe("streaming");
      },
      { timeout: 10000 },
    );

    // Both runs eventually complete.
    await waitFor(
      () => {
        const runs = Object.values(agentStore.getState().runs);
        expect(runs.every((r) => r.status === "complete")).toBe(true);
      },
      { timeout: 20000 },
    );
  }, 30000);
});
