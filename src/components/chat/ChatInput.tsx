"use client";

/**
 * Prompt composer: auto-resizing textarea, Enter to send, Shift+Enter for a
 * newline.
 *
 * While the agent is processing:
 *   - the send button becomes a stop button (empty input)
 *   - typing a new message turns it back into a send button that QUEUES the
 *     message behind the running agent (Enter works too)
 *   - a small badge shows how many messages are queued
 */

import { memo, useEffect, useRef, useState } from "react";
import { cn } from "cn";

export type ChatInputProps = {
  onSend: (text: string) => void;
  onStop?: () => void;
  isStreaming?: boolean;
  /** Number of messages queued behind the running agent. */
  queuedCount?: number;
  disabled?: boolean;
  placeholder?: string;
  className?: string;
};

export const ChatInput = memo(function ChatInput({
  onSend,
  onStop,
  isStreaming = false,
  queuedCount = 0,
  disabled = false,
  placeholder = "Ask the agent to do something…",
  className,
}: ChatInputProps) {
  const [draft, setDraft] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  }, [draft]);

  const hasText = draft.trim().length > 0;
  const canSend = !disabled && hasText;
  // While streaming with an empty input, the button stops the run; once the
  // user types, it becomes a send button that queues the message.
  const showStop = isStreaming && !hasText;

  const submit = () => {
    if (!canSend) return;
    onSend(draft.trim());
    setDraft("");
    const el = textareaRef.current;
    if (el) el.style.height = "auto";
  };

  return (
    <div className={cn("shrink-0 px-4 pb-4", className)}>
      <form
        className="mx-auto w-full max-w-3xl"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <div className="flex items-end gap-2 rounded-xl border border-border bg-card px-3 py-2 shadow-[0_8px_24px_rgba(0,0,0,0.12)] focus-within:border-ring/60 focus-within:ring-2 focus-within:ring-ring/20">
          <textarea
            ref={textareaRef}
            rows={1}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                e.preventDefault();
                submit();
              }
            }}
            placeholder={placeholder}
            aria-label="Prompt"
            spellCheck={false}
            disabled={disabled}
            className="max-h-40 min-h-6 flex-1 resize-none bg-transparent py-1 text-sm leading-6 text-fg outline-none placeholder:text-muted-fg disabled:opacity-50"
          />

          {showStop ? (
            <button
              type="button"
              aria-label="Stop generation"
              title="Stop"
              onClick={onStop}
              className="flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-full bg-secondary text-fg transition-colors hover:bg-muted"
            >
              <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-2.5">
                <rect x="3.5" y="3.5" width="9" height="9" rx="1.5" fill="currentColor" />
              </svg>
            </button>
          ) : (
            <button
              type="submit"
              aria-label="Send"
              disabled={!canSend}
              className={cn(
                "flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-full transition-colors",
                canSend
                  ? "bg-fg text-bg hover:bg-fg/80"
                  : "cursor-default bg-secondary text-muted-fg/50",
              )}
            >
              <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
                <path d="M8 3v10M4 9l4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
          )}
        </div>

        <div className="mt-1.5 flex items-center gap-2 px-1 text-[11px] text-muted-fg/70">
          <span>
            {isStreaming
              ? "Enter to queue · Shift+Enter for a new line"
              : "Enter to send · Shift+Enter for a new line"}
          </span>
          {queuedCount > 0 && (
            <span className="ml-auto flex items-center gap-1 rounded-sm bg-primary-subtle px-1.5 py-px font-medium text-primary-subtle-fg">
              <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3">
                <path d="M3 4h10M3 8h7M3 12h4" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
              </svg>
              {queuedCount} queued
            </span>
          )}
        </div>
      </form>
    </div>
  );
});
