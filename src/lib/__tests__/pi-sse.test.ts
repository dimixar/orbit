import { describe, expect, it } from "vitest";
import { mergeExactScopePatterns, toChatMessages } from "@/lib/pi-sse";
import type { PiAgentMessage } from "@assistant-ui/react-pi";

const usage = {
  input: 0,
  output: 0,
  cacheRead: 0,
  cacheWrite: 0,
  totalTokens: 0,
  cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
};

function assistantMessage(
  toolCalls: { id: string; name: string; arguments: Record<string, unknown> }[],
): PiAgentMessage {
  return {
    role: "assistant",
    content: [
      { type: "text", text: "Running tools…" },
      ...toolCalls.map((tc) => ({ type: "toolCall" as const, ...tc })),
    ],
    api: "openai-completions",
    provider: "synthetic",
    model: "test-model",
    usage,
    stopReason: "toolUse",
    timestamp: 1,
  };
}

function toolResult(
  toolCallId: string,
  text: string,
  isError = false,
): PiAgentMessage {
  return {
    role: "toolResult",
    toolCallId,
    toolName: "bash",
    content: [{ type: "text", text }],
    isError,
    timestamp: 2,
  };
}

describe("toChatMessages", () => {
  it("merges tool results into their tool-call part so each toolCallId appears once", () => {
    const messages: PiAgentMessage[] = [
      { role: "user", content: "list files", timestamp: 0 },
      assistantMessage([
        { id: "bash:0", name: "bash", arguments: { command: "ls" } },
        { id: "bash:1", name: "bash", arguments: { command: "pwd" } },
      ]),
      toolResult("bash:0", "src\npackage.json"),
      toolResult("bash:1", "/Users/hyphun-raj/Developer/orbit"),
    ];

    const chat = toChatMessages(messages);
    const assistant = chat[1]!;

    // One part per tool call id — no duplicate keys.
    const toolParts = assistant.parts.filter((p) => p.type === "tool");
    expect(toolParts).toHaveLength(2);
    const ids = toolParts.map((p) => p.toolCallId);
    expect(new Set(ids).size).toBe(ids.length);

    // Outputs are folded onto the matching call.
    const bash0 = toolParts.find((p) => p.toolCallId === "bash:0")!;
    expect(bash0.output).toBe("src\npackage.json");
    expect(bash0.isError).toBe(false);
    const bash1 = toolParts.find((p) => p.toolCallId === "bash:1")!;
    expect(bash1.output).toBe("/Users/hyphun-raj/Developer/orbit");
  });

  it("keeps per-run tool call ids separate across multiple runs in one turn", () => {
    // Pi resets its tool call counter per run, so a second run in the same
    // turn reuses `bash:0`. Each run's result must fold onto its own message.
    const messages: PiAgentMessage[] = [
      { role: "user", content: "do the thing", timestamp: 0 },
      assistantMessage([
        { id: "bash:0", name: "bash", arguments: { command: "first" } },
      ]),
      toolResult("bash:0", "first output"),
      assistantMessage([
        { id: "bash:0", name: "bash", arguments: { command: "second" } },
      ]),
      toolResult("bash:0", "second output"),
    ];

    const chat = toChatMessages(messages);
    const [first, second] = chat.filter((m) => m.role === "assistant");

    const firstTools = first!.parts.filter((p) => p.type === "tool");
    const secondTools = second!.parts.filter((p) => p.type === "tool");
    expect(firstTools).toHaveLength(1);
    expect(secondTools).toHaveLength(1);
    expect(firstTools[0]!.output).toBe("first output");
    expect(secondTools[0]!.output).toBe("second output");
  });

  it("keeps string user prompts and image attachments", () => {
    const chat = toChatMessages([
      { role: "user", content: "list files", timestamp: 0 },
      {
        role: "user",
        content: [
          { type: "text", text: "look" },
          { type: "image", data: "AAAA", mimeType: "image/png" },
        ],
        timestamp: 1,
      },
    ]);

    expect(chat[0]!.parts).toEqual([{ type: "text", text: "list files" }]);
    expect(chat[1]!.parts).toEqual([
      { type: "text", text: "look" },
      { type: "image", url: "data:image/png;base64,AAAA" },
    ]);
  });

  it("marks error results on the merged part", () => {
    const messages: PiAgentMessage[] = [
      assistantMessage([
        { id: "bash:0", name: "bash", arguments: { command: "boom" } },
      ]),
      toolResult("bash:0", "command not found", true),
    ];

    const chat = toChatMessages(messages);
    const tool = chat[0]!.parts.find((p) => p.type === "tool")!;
    expect(tool.isError).toBe(true);
    expect(tool.output).toBe("command not found");
  });

  it("keeps thinking open on the streaming assistant message", () => {
    const messages: PiAgentMessage[] = [
      { role: "user", content: "think", timestamp: 0 },
      {
        role: "assistant",
        content: [{ type: "thinking", thinking: "hmm" }],
        api: "openai-completions",
        provider: "synthetic",
        model: "test-model",
        usage,
        stopReason: "stop",
        timestamp: 1,
      },
    ];

    const streaming = toChatMessages(messages, { streamingMessageIndex: 1 });
    const settled = toChatMessages(messages);

    const streamingThinking = streaming[1]!.parts.find(
      (part) => part.type === "thinking",
    );
    const settledThinking = settled[1]!.parts.find(
      (part) => part.type === "thinking",
    );
    expect(streamingThinking).toMatchObject({ text: "hmm", done: false });
    expect(settledThinking).toMatchObject({ text: "hmm", done: true });
  });
});

describe("mergeExactScopePatterns", () => {
  it("keeps exact provider/model patterns that the resolver dropped", () => {
    const ids = mergeExactScopePatterns(
      ["ollama/kimi-k3:cloud"],
      [
        "ollama/kimi-k3:cloud",
        "synthetic/hf:moonshotai/Kimi-K3",
        "anthropic/*:high",
      ],
    );
    expect(ids).toEqual([
      "ollama/kimi-k3:cloud",
      "synthetic/hf:moonshotai/Kimi-K3",
    ]);
  });

  it("leaves an unscoped catalog alone", () => {
    expect(mergeExactScopePatterns(null, null)).toBeNull();
  });
});
