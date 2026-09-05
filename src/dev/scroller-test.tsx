/**
 * Dev-only scratch page for exercising the chat message scroller with
 * headless Chrome (see scripts/test-message-scroller.mjs). Not part of the
 * app build — load via http://localhost:1420/scroller-test.html in dev.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import type { ChatStatus, UIMessage } from "ai";
import "@/index.css";
import { MessageList } from "@/components/agent-elements/message-list";

const PARAGRAPH =
  "The scroller follows streamed output while the reader rests at the live edge. ";

function makeTurn(index: number): UIMessage[] {
  return [
    {
      id: `u-${index}`,
      role: "user",
      parts: [{ type: "text", text: `Question number ${index}?` }],
    } as UIMessage,
    {
      id: `a-${index}`,
      role: "assistant",
      parts: [
        {
          type: "text",
          text: Array.from({ length: 12 }, () => PARAGRAPH).join(""),
        },
      ],
    } as UIMessage,
  ];
}

function initialMessages(): UIMessage[] {
  return Array.from({ length: 40 }, (_, i) => makeTurn(i)).flat();
}

function Harness() {
  const [messages, setMessages] = useState<UIMessage[]>(initialMessages);
  const [status, setStatus] = useState<ChatStatus>("ready");
  const streamTimer = useRef<number | null>(null);

  const stopStream = useCallback(() => {
    if (streamTimer.current !== null) {
      window.clearInterval(streamTimer.current);
      streamTimer.current = null;
    }
    setStatus("ready");
  }, []);

  /** Simulate a streaming reply: grow the last assistant message over time. */
  const startStream = useCallback(() => {
    if (streamTimer.current !== null) return;
    setStatus("streaming");
    streamTimer.current = window.setInterval(() => {
      setMessages((prev) => {
        const last = prev[prev.length - 1];
        if (!last || last.role !== "assistant") return prev;
        const grown: UIMessage = {
          ...last,
          parts: [
            {
              type: "text",
              text:
                (
                  last.parts.find((p) => p.type === "text") as
                    { text: string } | undefined
                )?.text +
                PARAGRAPH +
                PARAGRAPH,
            },
          ],
        };
        return [...prev.slice(0, -1), grown];
      });
    }, 120);
    window.setTimeout(stopStream, 4000);
  }, [stopStream]);

  /** Simulate the user sending a new message (status flip + append). */
  const sendMessage = useCallback(() => {
    setStatus("submitted");
    setMessages((prev) => [
      ...prev,
      {
        id: `u-${Date.now()}`,
        role: "user",
        parts: [{ type: "text", text: "A brand new question?" }],
      } as UIMessage,
      {
        id: `a-${Date.now()}`,
        role: "assistant",
        parts: [{ type: "text", text: "Working on it…" }],
      } as UIMessage,
    ]);
  }, []);

  // Expose controls for the CDP driver.
  useEffect(() => {
    Object.assign(window, { startStream, sendMessage, stopStream });
  }, [startStream, sendMessage, stopStream]);

  return (
    <div
      style={{
        height: "100vh",
        display: "flex",
        flexDirection: "column",
        background: "var(--bg)",
        color: "var(--fg)",
      }}
    >
      <div
        style={{
          display: "flex",
          gap: 8,
          borderBottom: "1px solid var(--border)",
          padding: 8,
        }}
      >
        <button type="button" onClick={startStream} id="ctl-stream">
          Stream
        </button>
        <button type="button" onClick={sendMessage} id="ctl-send">
          Send
        </button>
        <button type="button" onClick={stopStream} id="ctl-stop">
          Stop
        </button>
      </div>
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          flex: 1,
          minHeight: 0,
        }}
      >
        <MessageList
          messages={messages}
          status={status}
          onQuote={() => {}}
          workspacePath="/tmp"
        />
      </div>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<Harness />);
