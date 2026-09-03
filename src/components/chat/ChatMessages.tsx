"use client";

/**
 * Renders all messages: user messages, assistant messages, streaming
 * messages, plus loading/error states. Handles auto-scroll and a
 * jump-to-latest affordance. Does NOT interpret agent events — it only
 * renders normalized `AgentMessage`s.
 *
 * Scroll strategy (single, robust mechanism):
 *   - A ResizeObserver watches the content wrapper. Whenever the content
 *     height changes (streaming growth, completion shrink, new messages):
 *       - if the user is pinned to the bottom → stay pinned to the bottom
 *       - if the user scrolled up → keep the viewport stable by offsetting
 *         the scrollTop by the height delta
 *   - No competing smooth-scroll animations, so the viewport can never be
 *     left mid-animation or yanked to the top when a run completes.
 */

import {
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { cn } from "cn";
import type { AgentMessage as AgentMessageType } from "@/lib/agent/types";
import { AgentMessage as AgentMessageView } from "./AgentMessage";

export type ChatMessagesProps = {
  messages: AgentMessageType[];
  isStreaming?: boolean;
  error?: string | null;
  onFileClick?: (path: string) => void;
  onApprove?: (id: string) => void;
  onReject?: (id: string) => void;
  className?: string;
};

const SCROLL_THRESHOLD = 80;

export const ChatMessages = memo(function ChatMessages({
  messages,
  error,
  onFileClick,
  onApprove,
  onReject,
  className,
}: ChatMessagesProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const shouldAutoScrollRef = useRef(true);
  const [atBottom, setAtBottom] = useState(true);

  const isAtBottom = useCallback(() => {
    const container = containerRef.current;
    if (!container) return true;
    return (
      container.scrollHeight - container.scrollTop - container.clientHeight <
      SCROLL_THRESHOLD
    );
  }, []);

  const handleScroll = useCallback(() => {
    shouldAutoScrollRef.current = isAtBottom();
    setAtBottom(shouldAutoScrollRef.current);
  }, [isAtBottom]);

  // Single scroll mechanism: keep the viewport correct whenever the content
  // height changes. Runs once on mount (initial pin) and then via the
  // ResizeObserver for every subsequent content change.
  useLayoutEffect(() => {
    const container = containerRef.current;
    const content = contentRef.current;
    if (!container || !content) return;

    // Pin to the bottom on mount.
    container.scrollTop = container.scrollHeight;
    shouldAutoScrollRef.current = true;
    setAtBottom(true);

    let lastContentHeight = content.getBoundingClientRect().height;
    let prevScrollHeight = container.scrollHeight;

    const observer = new ResizeObserver(() => {
      const newContentHeight = content.getBoundingClientRect().height;
      if (newContentHeight === lastContentHeight) return;
      lastContentHeight = newContentHeight;

      const newScrollHeight = container.scrollHeight;
      if (shouldAutoScrollRef.current) {
        // Pinned to the bottom — follow the stream (or the completion
        // shrink) so the latest message stays in view.
        container.scrollTop = newScrollHeight;
      } else if (newScrollHeight !== prevScrollHeight && prevScrollHeight > 0) {
        // User scrolled up — keep the viewport stable by offsetting the
        // scrollTop by the height delta.
        container.scrollTop += newScrollHeight - prevScrollHeight;
      }
      prevScrollHeight = newScrollHeight;
    });

    observer.observe(content);
    return () => observer.disconnect();
  }, []);

  // When a brand-new user message is sent, re-pin to the bottom even if the
  // user had scrolled up (they just asked a new question). Tracked by the
  // last *user* message id because the assistant's empty message is appended
  // to the list at the same time the run starts.
  const lastUserMessageId = useMemo(() => {
    for (let i = messages.length - 1; i >= 0; i -= 1) {
      if (messages[i]!.role === "user") return messages[i]!.id;
    }
    return null;
  }, [messages]);
  const lastUserMessageIdRef = useRef(lastUserMessageId);
  useEffect(() => {
    if (lastUserMessageId && lastUserMessageId !== lastUserMessageIdRef.current) {
      shouldAutoScrollRef.current = true;
      setAtBottom(true);
      const container = containerRef.current;
      if (container) container.scrollTop = container.scrollHeight;
    }
    lastUserMessageIdRef.current = lastUserMessageId;
  }, [lastUserMessageId]);

  const jumpToLatest = () => {
    shouldAutoScrollRef.current = true;
    setAtBottom(true);
    const container = containerRef.current;
    if (container) container.scrollTop = container.scrollHeight;
  };

  return (
    <div className={cn("relative flex min-h-0 flex-1", className)}>
      <div
        ref={containerRef}
        onScroll={handleScroll}
        className="min-h-0 flex-1 overflow-y-auto"
      >
        <div ref={contentRef} className="mx-auto w-full max-w-3xl space-y-6 px-4 py-6">
          {messages.map((message) =>
            message.role === "user" ? (
              <UserMessageView key={message.id} message={message} />
            ) : (
              <AgentMessageView
                key={message.id}
                message={message}
                onFileClick={onFileClick}
                onApprove={onApprove}
                onReject={onReject}
              />
            ),
          )}

          {error && (
            <div className="mx-auto max-w-3xl">
              <div className="flex items-start gap-2.5 rounded-lg border border-danger-subtle/50 bg-danger-subtle/20 px-3 py-2.5">
                <span className="mt-px flex size-4 shrink-0 items-center justify-center text-danger-subtle-fg">
                  <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-4">
                    <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
                  </svg>
                </span>
                <p className="text-xs leading-relaxed text-danger-subtle-fg">{error}</p>
              </div>
            </div>
          )}
        </div>
      </div>

      {!atBottom && (
        <button
          type="button"
          aria-label="Go to latest message"
          onClick={jumpToLatest}
          className="absolute bottom-3 left-1/2 z-10 flex size-7 -translate-x-1/2 cursor-pointer items-center justify-center rounded-full border border-border bg-card text-muted-fg shadow-[0_8px_24px_rgba(0,0,0,0.18)] transition-colors hover:text-fg"
        >
          <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
            <path d="M8 3v10M4 9l4 4 4-4" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </button>
      )}
    </div>
  );
});

function UserMessageView({ message }: { message: AgentMessageType }) {
  const text = message.parts
    .filter((p): p is { type: "text"; content: string } => p.type === "text")
    .map((p) => p.content)
    .join("\n\n");

  return (
    <div className="flex flex-col items-end gap-1">
      <div className="max-w-[85%] rounded-2xl rounded-br-md bg-primary px-3.5 py-2 text-sm leading-relaxed text-primary-fg">
        {text || "…"}
      </div>
      {message.queued && (
        <span className="flex items-center gap-1 rounded-sm bg-primary-subtle px-1.5 py-px text-[10px] font-medium text-primary-subtle-fg">
          <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-2.5">
            <path d="M3 4h10M3 8h7M3 12h4" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
          </svg>
          Queued
        </span>
      )}
    </div>
  );
}
