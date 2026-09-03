"use client";

/**
 * Minimal assistant-ui Thread composed from `@assistant-ui/react` primitives.
 *
 * The full `@assistant-ui/react-ui` package is incompatible with
 * `@assistant-ui/react@0.15` (it imports APIs removed in 0.15), so this
 * builds the thread UI directly from the primitives that 0.15 ships:
 * `ThreadPrimitive`, `ComposerPrimitive`, `MessagePrimitive`,
 * `MessagePartPrimitive` — styled with Orbit's design system.
 *
 * Renders: user/assistant messages, markdown text, reasoning, tool calls,
 * Pi's queue (mid-run follow-up/steer), and a composer with send/cancel.
 */

import { memo } from "react";
import {
  ComposerPrimitive,
  MessagePrimitive,
  QueueItemPrimitive,
  ThreadPrimitive,
  useAuiState,
  useMessagePartReasoning,
  useMessagePartText,
  type ToolCallMessagePartComponent,
} from "@assistant-ui/react";
import { usePiRuntimeExtras, usePiThreadState } from "@assistant-ui/react-pi";
import { cn } from "cn";
import { MarkdownRenderer } from "../markdown/MarkdownRenderer";

/* ------------------------------------------------------------------ */
/* Text part                                                           */
/* ------------------------------------------------------------------ */

function PiTextPart() {
  const part = useMessagePartText();
  const text = part?.type === "text" ? part.text : "";
  if (!text) return null;
  return (
    <div className="text-sm">
      <MarkdownRenderer content={text} />
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Reasoning part                                                      */
/* ------------------------------------------------------------------ */

function PiReasoningPart() {
  const part = useMessagePartReasoning();
  if (!part) return null;
  const running = part.status?.type === "running";
  return (
    <div className="flex items-start gap-2 rounded-md border border-border/70 bg-muted/30 px-3 py-2 text-xs text-muted-fg">
      <span className="relative mt-1 flex size-2 shrink-0">
        {running && (
          <span className="absolute inline-flex size-full animate-ping rounded-full bg-primary/60" />
        )}
        <span
          className={cn(
            "relative inline-flex size-2 rounded-full",
            running ? "bg-primary" : "bg-muted-fg/40",
          )}
        />
      </span>
      <div className="min-w-0 flex-1 whitespace-pre-wrap leading-relaxed">
        {part.text}
        {running && (
          <span className="ml-0.5 inline-block h-3 w-0.5 animate-pulse bg-primary align-middle" />
        )}
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Tool call                                                           */
/* ------------------------------------------------------------------ */

const PiToolCall: ToolCallMessagePartComponent = ({ toolName, args, result, status }) => {
  const isRunning = status?.type === "running";
  const isIncomplete = status?.type === "incomplete";
  const argsText = args ? JSON.stringify(args, null, 2) : "";
  const resultText =
    typeof result === "string" ? result : result ? JSON.stringify(result, null, 2) : "";

  return (
    <div
      className={cn(
        "overflow-hidden rounded-md border text-xs",
        isIncomplete
          ? "border-danger-subtle/50 bg-danger-subtle/20"
          : "border-border bg-card",
      )}
    >
      <div className="flex items-center gap-2 px-3 py-2">
        <span
          className={cn(
            "flex size-3.5 shrink-0 items-center justify-center",
            isIncomplete ? "text-danger-subtle-fg" : "text-muted-fg",
          )}
        >
          {isRunning ? (
            <span className="size-3 animate-spin rounded-full border-[1.5px] border-primary/25 border-t-primary" />
          ) : isIncomplete ? (
            <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
              <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
            </svg>
          ) : (
            <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
              <path d="M3 8.5 6.5 12 13 4.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          )}
        </span>
        <span className="shrink-0 font-medium text-fg">{toolName}</span>
        {argsText && (
          <span className="min-w-0 truncate font-mono text-[11px] text-muted-fg/80">
            {argsText}
          </span>
        )}
        <span className="ml-auto shrink-0 text-muted-fg/70">
          {isRunning ? "running…" : isIncomplete ? "failed" : "done"}
        </span>
      </div>
      {resultText && (
        <pre className="max-h-48 overflow-auto border-t border-border/60 bg-muted/30 px-3 py-2 font-mono text-[11px] leading-relaxed text-fg/85">
          {resultText}
        </pre>
      )}
    </div>
  );
};

/* ------------------------------------------------------------------ */
/* Messages                                                            */
/* ------------------------------------------------------------------ */

function PiUserMessage() {
  return (
    <MessagePrimitive.Root className="flex justify-end">
      <div className="max-w-[85%] rounded-2xl rounded-br-md bg-primary px-3.5 py-2 text-sm leading-relaxed text-primary-fg">
        <MessagePrimitive.Content components={{ Text: PiTextPart }} />
      </div>
    </MessagePrimitive.Root>
  );
}

function PiAssistantMessage() {
  return (
    <MessagePrimitive.Root className="flex gap-3">
      <div className="mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-md bg-fg text-bg">
        <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
          <path d="M4 12.5V3.5" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
          <path d="M4 3.5h3a2.5 2.5 0 0 1 0 5H4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
          <rect x="9.5" y="9" width="3" height="3" rx="0.6" fill="currentColor" />
        </svg>
      </div>
      <div className="min-w-0 flex-1 space-y-2.5">
        <MessagePrimitive.Content
          components={{
            Text: PiTextPart,
            Reasoning: PiReasoningPart,
            tools: { Fallback: PiToolCall },
          }}
        />
      </div>
    </MessagePrimitive.Root>
  );
}

/* ------------------------------------------------------------------ */
/* Composer                                                            */
/* ------------------------------------------------------------------ */

function PiComposer() {
  // While a run is in flight the send button becomes a stop button; the
  // composer still accepts input (Enter queues a follow-up via Pi's queue).
  const { status } = usePiRuntimeExtras();
  const runStatus = usePiThreadState((s) => s.runStatus);
  const threadMsgs = useAuiState((s) => s.thread.messages.length);
  const isRunning = status === "running" || runStatus === "running";

  return (
    <ComposerPrimitive.Root className="shrink-0 px-4 pb-4">
      <div className="mx-auto mb-2 w-full max-w-3xl space-y-1">
        <ComposerPrimitive.Queue>
          {() => (
            <div className="flex items-center gap-2 rounded-md border border-border/70 bg-muted/30 px-3 py-1.5 text-xs text-muted-fg">
              <span className="size-1.5 shrink-0 rounded-full bg-primary" />
              <QueueItemPrimitive.Text className="min-w-0 flex-1 truncate" />
              <span className="shrink-0 text-[10px] uppercase tracking-wide text-muted-fg/60">
                queued
              </span>
            </div>
          )}
        </ComposerPrimitive.Queue>
      </div>
      <div className="mx-auto flex w-full max-w-3xl items-end gap-2 rounded-xl border border-border bg-card px-3 py-2 shadow-[0_8px_24px_rgba(0,0,0,0.12)] focus-within:border-ring/60 focus-within:ring-2 focus-within:ring-ring/20">
        <ComposerPrimitive.Input
          rows={1}
          placeholder="Ask the agent to do something…"
          aria-label="Prompt"
          className="max-h-40 min-h-6 flex-1 resize-none bg-transparent py-1 text-sm leading-6 text-fg outline-none placeholder:text-muted-fg"
        />
        {isRunning ? (
          <ComposerPrimitive.Cancel
            aria-label="Stop generation"
            className="flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-full bg-secondary text-fg transition-colors hover:bg-muted"
          >
            <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-2.5">
              <rect x="3.5" y="3.5" width="9" height="9" rx="1.5" fill="currentColor" />
            </svg>
          </ComposerPrimitive.Cancel>
        ) : (
          <ComposerPrimitive.Send
            aria-label="Send"
            className="flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-full bg-fg text-bg transition-colors hover:bg-fg/80 disabled:cursor-default disabled:bg-secondary disabled:text-muted-fg/50"
          >
            <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
              <path d="M8 3v10M4 9l4 4 4-4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </ComposerPrimitive.Send>
        )}
      </div>
      <p className="mx-auto mt-1.5 w-full max-w-3xl px-1 text-[11px] text-muted-fg/70">
        Enter to send · Shift+Enter for a new line · while running, Enter queues a follow-up
      </p>
      <p data-testid="composer-debug" className="mx-auto w-full max-w-3xl px-1 text-[10px] text-muted-fg">
        status={status} runStatus={runStatus} threadMsgs={threadMsgs}
      </p>
    </ComposerPrimitive.Root>
  );
}

/* ------------------------------------------------------------------ */
/* Thread                                                              */
/* ------------------------------------------------------------------ */

export const PiThread = memo(function PiThread() {
  return (
    <ThreadPrimitive.Root className="flex h-full flex-col">
      <ThreadPrimitive.Viewport className="min-h-0 flex-1 overflow-y-auto">
        <div className="mx-auto w-full max-w-3xl space-y-6 px-4 py-6">
          <ThreadPrimitive.Messages
            components={{
              UserMessage: PiUserMessage,
              AssistantMessage: PiAssistantMessage,
            }}
          />
        </div>
      </ThreadPrimitive.Viewport>
      <PiComposer />
    </ThreadPrimitive.Root>
  );
});
