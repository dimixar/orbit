import { describe, expect, it } from "vitest";
import { normalizeOutgoingPiEvent } from "@/lib/pi-event-normalize";

describe("normalizeOutgoingPiEvent", () => {
  it("fills missing assistant usage so message_update stays valid", () => {
    const event = normalizeOutgoingPiEvent({
      type: "message_update",
      threadId: "t1",
      seq: 4,
      message: {
        role: "assistant",
        content: [{ type: "text", text: "Hello", textSignature: { v: 1, id: "a" } }],
        timestamp: 1,
      },
      assistantMessageEvent: {
        type: "text_delta",
        contentIndex: 0,
        delta: "o",
        partial: {
          role: "assistant",
          content: [{ type: "text", text: "Hello" }],
        },
      },
    }) as unknown as {
      message: {
        usage: { cost: { total: number } };
        stopReason: string;
        content: { textSignature?: unknown }[];
      };
      assistantMessageEvent: { partial: { usage: unknown; api: string } };
    };

    expect(event.message.stopReason).toBe("pending");
    expect(event.message.usage.cost.total).toBe(0);
    expect(event.message.content[0]).not.toHaveProperty("textSignature");
    expect(event.assistantMessageEvent.partial.api).toBe("unknown");
    expect(event.assistantMessageEvent.partial.usage).toMatchObject({
      input: 0,
      output: 0,
    });
  });

  it("parses streamed tool-call arguments from JSON strings", () => {
    const event = normalizeOutgoingPiEvent({
      type: "message_update",
      message: {
        role: "assistant",
        content: [
          {
            type: "toolCall",
            id: "bash:0",
            name: "bash",
            arguments: '{"command":"ls"}',
          },
        ],
      },
    }) as unknown as {
      message: { content: { arguments: unknown }[] };
    };

    expect(event.message.content[0]!.arguments).toEqual({ command: "ls" });
  });

  it("leaves non-message events untouched", () => {
    const event = { type: "agent_start", threadId: "t1", seq: 1 };
    expect(normalizeOutgoingPiEvent(event)).toBe(event);
  });
});
