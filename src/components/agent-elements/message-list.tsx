import React, {
  memo,
  useRef,
  useEffect,
  useLayoutEffect,
  useCallback,
  useState,
  useMemo,
} from "react";
import type { UIMessage, ChatStatus } from "ai";
import { cn } from "./utils/cn";

import { UserMessage } from "./user-message";
import { Markdown } from "./markdown";
import { ErrorMessage } from "./error-message";
import type { CustomToolRendererProps } from "./types";
import { ToolRowBase } from "./tools/tool-row-base";
import {
  IconCopy,
  IconCheck,
  IconArrowDown,
  IconDots,
  IconQuote,
} from "@tabler/icons-react";
import { ToolRenderer as DefaultToolRenderer } from "./tools/tool-renderer";
import { normalizeAssistantToolParts } from "./utils/tool-part-normalizer";
import { SpiralLoader } from "./spiral-loader";
import {
  fetchWorkspaceChanges,
  openWorkspaceInApp,
  type WorkspaceFileChange,
} from "@/lib/pi-client";
import { DocumentTextIcon } from "@heroicons/react/24/outline";
import {
  Menu,
  MenuContent,
  MenuItem,
  MenuLabel,
  MenuTrigger,
} from "@/components/ui/menu";

export type MessageListProps = {
  messages: UIMessage[];
  status: ChatStatus;
  className?: string;
  showCopyToolbar?: boolean;
  suppressQuestionTool?: boolean;
  /**
   * Where to position the scroll container on initial mount.
   * - "bottom" (default): classic chat behavior, pinned to the latest message.
   * - "top": start from the top of the conversation — useful for static demos
   *   or read-only transcripts where the user should read top-to-bottom.
   */
  initialScrollBehavior?: "bottom" | "top";
  /**
   * Insert a message into the composer as a blockquote — powers the
   * “Quote in composer” action in each message's options menu.
   */
  onQuote?: (text: string) => void;
  /** Active workspace folder — scopes the git changes card. */
  workspacePath?: string;
  /** Opens the in-app git changes panel (the card's Review action). */
  onReviewChanges?: () => void;
  /**
   * When true (default) clicking an attached image in a user message opens
   * the fullscreen lightbox preview. Set to false to disable previews.
   */
  enableImagePreview?: boolean;
  slots?: {
    UserMessage?: React.ComponentType<{
      message: UIMessage;
      className?: string;
      enableImagePreview?: boolean;
    }>;
    ToolRenderer?: React.ComponentType<ToolRendererProps>;
  };
  classNames?: {
    userMessage?: string;
  };
  toolRenderers?: Record<string, React.ComponentType<CustomToolRendererProps>>;
};

const SCROLL_THRESHOLD = 80;
const timeFormatter = new Intl.DateTimeFormat("en-US", {
  hour: "numeric",
  minute: "2-digit",
  hour12: true,
});
const dateFormatter = new Intl.DateTimeFormat("en-US", {
  month: "short",
  day: "numeric",
});
/** Full precision for the tooltip: weekday, date, and seconds. */
const fullStampFormatter = new Intl.DateTimeFormat("en-US", {
  dateStyle: "full",
  timeStyle: "medium",
});
type ToolPartBase = {
  type: string;
  toolCallId?: string;
  state?: string;
  input?: unknown;
  output?: unknown;
  result?: unknown;
};

type ToolRendererProps = {
  part: ToolPartBase;
  nestedTools?: ToolPartBase[];
  chatStatus?: string;
  toolRenderers?: Record<string, React.ComponentType<CustomToolRendererProps>>;
};

function normalizeMessages(messages: UIMessage[]): UIMessage[] {
  let changed = false;
  const normalized = messages.map((message) => {
    if (Array.isArray(message.parts) && message.parts.length > 0)
      return message;
    const raw = message as { content?: string; text?: string };
    const content = raw.content ?? raw.text;
    if (typeof content !== "string" || !content) return message;
    changed = true;
    return {
      ...message,
      parts: [{ type: "text", text: content }],
    } as UIMessage;
  });
  return changed ? normalized : messages;
}

function getLastAssistantHasContent(messages: UIMessage[]) {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const msg = messages[i];
    if (msg?.role !== "assistant") continue;
    return (msg.parts ?? []).some((part) => {
      if (isTextPart(part)) return part.text.trim().length > 0;
      return isV5ToolPart(part);
    });
  }
  return false;
}

function getLastUserMessageId(messages: UIMessage[]) {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const msg = messages[i];
    if (msg?.role === "user") return msg.id;
  }
  return null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isTextPart(part: unknown): part is { type: "text"; text: string } {
  return (
    isRecord(part) && part.type === "text" && typeof part.text === "string"
  );
}

function isErrorPart(
  part: unknown,
): part is { type: "error"; title?: string; message: string } {
  return (
    isRecord(part) && part.type === "error" && typeof part.message === "string"
  );
}

function isV5ToolPart(part: unknown): part is ToolPartBase {
  if (!isRecord(part)) return false;
  const partType = part.type;
  return (
    partType === "dynamic-tool" ||
    (typeof partType === "string" && partType.startsWith("tool-"))
  );
}

function getTextFromParts(parts: unknown[], joiner: string): string {
  return parts
    .filter(isTextPart)
    .map((part) => part.text)
    .join(joiner);
}

/**
 * Files the agent edited in a turn — extracted from Edit/Write tool calls so
 * the changes card scopes to what the agent actually touched this turn.
 */
export function collectEditedPaths(messages: UIMessage[]): string[] {
  const paths = new Set<string>();
  for (const msg of messages) {
    for (const part of msg.parts ?? []) {
      const p = part as {
        type?: string;
        input?: { file_path?: string; path?: string };
      };
      if (
        p.input &&
        (p.type === "tool-Edit" ||
          p.type === "tool-Write" ||
          p.type === "tool-MultiEdit")
      ) {
        const fp = p.input.file_path ?? p.input.path;
        if (typeof fp === "string" && fp) paths.add(fp);
      }
    }
  }
  return [...paths];
}

/**
 * “Changed N files” card — git +/− for the files the agent touched this turn,
 * with a Review action that opens the workspace in the last-used app (where
 * the IDE's git tooling shows the diff). Hidden when nothing changed.
 */
function GitChangesCard({
  workspacePath,
  paths,
  onReview,
}: {
  workspacePath: string;
  paths: string[];
  /** Opens the in-app git changes panel. */
  onReview?: () => void;
}) {
  const [files, setFiles] = useState<WorkspaceFileChange[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    void fetchWorkspaceChanges(workspacePath, paths).then((next) => {
      if (!cancelled) setFiles(next);
    });
    return () => {
      cancelled = true;
    };
  }, [workspacePath, paths]);

  if (!files || files.length === 0) return null;
  const additions = files.reduce((sum, f) => sum + f.additions, 0);
  const deletions = files.reduce((sum, f) => sum + f.deletions, 0);

  const review = () => {
    if (onReview) {
      onReview();
      return;
    }
    // Standalone usage (no panel host): fall back to the system IDE.
    void openWorkspaceInApp(
      workspacePath,
      localStorage.getItem("orbit:open-in:last") ?? "vscode",
    ).catch(() => {});
  };

  return (
    <div className="overflow-hidden rounded-lg border border-border bg-card">
      <div className="flex items-center gap-2 px-3 py-2">
        <span className="text-xs font-medium text-fg">
          Changed {files.length === 1 ? "1 file" : `${files.length} files`}
        </span>
        <span className="text-xs tabular-nums text-success-subtle-fg">
          +{additions}
        </span>
        <span className="text-xs tabular-nums text-danger-subtle-fg">
          -{deletions}
        </span>
        <button
          type="button"
          onClick={review}
          className="ml-auto flex cursor-pointer items-center gap-1.5 rounded-md border border-border bg-secondary px-2 py-0.5 text-xs text-fg outline-none transition-colors duration-100 hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
        >
          <DocumentTextIcon className="size-3.5 shrink-0" strokeWidth={1.6} />
          Review
        </button>
      </div>
      <div className="divide-y divide-border/60 border-t border-border/60">
        {files.map((file) => {
          const relative = file.path.startsWith(`${workspacePath}/`)
            ? file.path.slice(workspacePath.length + 1)
            : file.path;
          return (
            <div
              key={file.path}
              className="flex items-center justify-between gap-3 px-3 py-1.5"
            >
              <span
                className="min-w-0 truncate font-mono text-[11px] text-muted-fg"
                title={file.path}
              >
                {relative}
              </span>
              <span className="flex shrink-0 items-center gap-2 text-[11px] tabular-nums">
                {file.additions > 0 && (
                  <span className="text-success-subtle-fg">
                    +{file.additions}
                  </span>
                )}
                {file.deletions > 0 && (
                  <span className="text-danger-subtle-fg">
                    -{file.deletions}
                  </span>
                )}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

/** “⋯ Working for Xs” — live elapsed timer while the agent streams. */
function WorkingIndicator() {
  const [seconds, setSeconds] = useState(0);
  useEffect(() => {
    const start = Date.now();
    const timer = setInterval(() => {
      setSeconds(Math.floor((Date.now() - start) / 1000));
    }, 1000);
    return () => clearInterval(timer);
  }, []);
  return (
    <div
      className="flex h-[28px] items-center gap-2 text-xs text-an-foreground-muted"
      aria-live="polite"
    >
      <span className="flex items-center gap-0.5" aria-hidden="true">
        {[0, 1, 2].map((i) => (
          <span
            key={i}
            className="size-1 rounded-full bg-current motion-safe:animate-pulse"
            style={{
              animationDelay: `${i * 200}ms`,
              animationDuration: "1.2s",
            }}
          />
        ))}
      </span>
      Working for {seconds}s
    </div>
  );
}

function formatTimestamp(date: Date): string {
  const now = new Date();
  const isSameDay =
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate();
  // Time only for today; date AND time for anything older.
  if (isSameDay) {
    return timeFormatter.format(date);
  }
  return `${dateFormatter.format(date)}, ${timeFormatter.format(date)}`;
}

function CopyButton({
  text,
  onCopied,
}: {
  text: string;
  onCopied?: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const copiedTimerRef = useRef<number | null>(null);

  const handleCopy = () => {
    navigator.clipboard.writeText(text);
    setCopied(true);
    if (copiedTimerRef.current) {
      window.clearTimeout(copiedTimerRef.current);
    }
    copiedTimerRef.current = window.setTimeout(() => {
      setCopied(false);
      copiedTimerRef.current = null;
    }, 2000);
    onCopied?.();
  };
  return (
    <button
      type="button"
      tabIndex={-1}
      onClick={handleCopy}
      onPointerDown={(event) => {
        event.stopPropagation();
      }}
      onMouseDown={(event) => event.stopPropagation()}
      className={cn(
        "size-6 flex items-center justify-center rounded-md active:scale-[0.97] transition-[background-color,opacity,transform] duration-150 ease-out",
        "opacity-50 bg-transparent hover:opacity-100 hover:bg-an-foreground/10",
      )}
    >
      <div className="relative w-3.5 h-3.5">
        <IconCopy
          className={cn(
            "absolute inset-0 w-3.5 h-3.5 text-an-foreground-muted transition-[opacity,transform] duration-150 ease-out",
            copied ? "opacity-0 scale-50" : "opacity-100 scale-100",
          )}
        />
        <IconCheck
          className={cn(
            "absolute inset-0 w-3.5 h-3.5 text-an-foreground-muted transition-[opacity,transform] duration-150 ease-out",
            copied ? "opacity-100 scale-100" : "opacity-0 scale-50",
          )}
        />
      </div>
    </button>
  );
}

function MessageToolbar({
  text,
  createdAt,
  showCopy = true,
  heightClass,
  hoverClass,
  isVisible,
  alignClass,
  onCopied,
  onQuote,
}: {
  text?: string;
  /** Message creation time — full date + time, precise tooltip. */
  createdAt?: Date | string | null;
  showCopy?: boolean;
  heightClass: string;
  hoverClass: string;
  isVisible: boolean;
  alignClass: string;
  onCopied?: () => void;
  /** Inserts the message as a blockquote in the composer. */
  onQuote?: (text: string) => void;
}) {
  const [menuOpen, setMenuOpen] = useState(false);
  const hasText = Boolean(text);
  // Pi timestamps are epoch ms; 0 means "unknown" — don't render 1970.
  const stampDate = createdAt ? new Date(createdAt) : undefined;
  const stamp = stampDate && stampDate.getTime() > 0 ? stampDate : undefined;
  return (
    <div
      className={cn(
        "flex items-center gap-1 pt-1 text-xs text-an-foreground-muted/70 opacity-0 transition-opacity duration-100 pointer-events-none",
        heightClass,
        alignClass,
        hoverClass,
        (isVisible || menuOpen) && "opacity-100 pointer-events-auto",
      )}
      onMouseDown={(event) => event.stopPropagation()}
      onPointerDown={(event) => event.stopPropagation()}
    >
      {stamp && (
        <time
          dateTime={stamp.toISOString()}
          title={fullStampFormatter.format(stamp)}
          className="flex h-6 items-center tabular-nums"
        >
          {formatTimestamp(stamp)}
        </time>
      )}
      {hasText && text && showCopy && (
        <CopyButton text={text} onCopied={onCopied} />
      )}
      {hasText && text && onQuote && (
        <Menu onOpenChange={setMenuOpen}>
          <MenuTrigger
            aria-label="Message options"
            className={cn(
              "size-6 flex items-center justify-center rounded-md outline-none active:scale-[0.97] transition-[background-color,opacity,transform] duration-150 ease-out",
              "opacity-50 bg-transparent hover:opacity-100 hover:bg-an-foreground/10 focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-ring",
              menuOpen && "opacity-100 bg-an-foreground/10",
            )}
          >
            <IconDots className="size-3.5 text-an-foreground-muted" />
          </MenuTrigger>
          <MenuContent placement="top">
            <MenuItem onAction={() => onQuote(text)}>
              <IconQuote />
              <MenuLabel>Quote in composer</MenuLabel>
            </MenuItem>
          </MenuContent>
        </Menu>
      )}
    </div>
  );
}

/** Group flat messages into turns (user message + following assistant messages) */
function groupMessagesIntoTurns(messages: UIMessage[]) {
  const turns: { userMsg?: UIMessage; assistantMsgs: UIMessage[] }[] = [];
  let current: { userMsg?: UIMessage; assistantMsgs: UIMessage[] } | null =
    null;

  for (const msg of messages) {
    if (msg.role === "user") {
      if (current) turns.push(current);
      current = { userMsg: msg, assistantMsgs: [] };
    } else if (msg.role === "assistant") {
      if (!current) current = { assistantMsgs: [] };
      current.assistantMsgs.push(msg);
    }
  }
  if (current) turns.push(current);
  return turns;
}

export const MessageList = memo(function MessageList({
  messages,
  status,
  className,
  showCopyToolbar = true,
  suppressQuestionTool = false,
  initialScrollBehavior = "bottom",
  enableImagePreview = true,
  slots,
  classNames,
  toolRenderers,
  onQuote,
  workspacePath,
  onReviewChanges,
}: MessageListProps) {
  const chatContainerRef = useRef<HTMLDivElement>(null);
  const contentWrapperRef = useRef<HTMLDivElement>(null);
  const chatContainerObserverRef = useRef<ResizeObserver | null>(null);
  const shouldAutoScrollRef = useRef(true);
  const prevScrollTopRef = useRef(0);
  const lastMessageIdRef = useRef<string | null>(
    messages[messages.length - 1]?.id ?? null,
  );
  const assistantSpaceActiveRef = useRef(false);
  const [activeCopyId, setActiveCopyId] = useState<string | null>(null);
  const [atBottom, setAtBottom] = useState(initialScrollBehavior !== "top");

  const CustomUserMessage = slots?.UserMessage || UserMessage;
  const CustomToolRenderer = slots?.ToolRenderer || DefaultToolRenderer;

  const markCopied = useCallback((id: string) => {
    setActiveCopyId(id);
  }, []);

  useEffect(() => {
    const handlePointerDown = () => {
      setActiveCopyId(null);
    };
    window.addEventListener("pointerdown", handlePointerDown);
    return () => window.removeEventListener("pointerdown", handlePointerDown);
  }, []);

  const isStreaming = status === "streaming" || status === "submitted";

  const containerRefCallback = useCallback((el: HTMLDivElement | null) => {
    (
      chatContainerRef as React.MutableRefObject<HTMLDivElement | null>
    ).current = el;

    if (chatContainerObserverRef.current) {
      chatContainerObserverRef.current.disconnect();
      chatContainerObserverRef.current = null;
    }
    if (el) {
      el.style.setProperty("--chat-container-height", `${el.clientHeight}px`);
      const observer = new ResizeObserver((entries) => {
        const height = entries[0]?.contentRect.height ?? 0;
        el.style.setProperty("--chat-container-height", `${height}px`);
      });
      observer.observe(el);
      chatContainerObserverRef.current = observer;
    }
  }, []);

  useEffect(() => {
    return () => {
      if (chatContainerObserverRef.current)
        chatContainerObserverRef.current.disconnect();
    };
  }, []);

  const scrollToBottomInstant = useCallback(() => {
    const container = chatContainerRef.current;
    if (!container) return;
    container.scrollTop = container.scrollHeight;
  }, []);

  const scrollToBottomSettled = useCallback(() => {
    let rafOne = 0;
    let rafTwo = 0;
    scrollToBottomInstant();
    rafOne = requestAnimationFrame(() => {
      scrollToBottomInstant();
      rafTwo = requestAnimationFrame(() => {
        scrollToBottomInstant();
      });
    });
    return () => {
      cancelAnimationFrame(rafOne);
      cancelAnimationFrame(rafTwo);
    };
  }, [scrollToBottomInstant]);

  const isAtBottom = useCallback(() => {
    const container = chatContainerRef.current;
    if (!container) return true;
    return (
      container.scrollHeight - container.scrollTop - container.clientHeight <
      SCROLL_THRESHOLD
    );
  }, []);

  const handleScroll = useCallback(() => {
    const container = chatContainerRef.current;
    if (!container) return;

    const currentScrollTop = container.scrollTop;
    const prevScrollTop = prevScrollTopRef.current;
    prevScrollTopRef.current = currentScrollTop;

    if (currentScrollTop < prevScrollTop) {
      shouldAutoScrollRef.current = false;
      if (!jumpLockRef.current) setAtBottom(false);
      return;
    }
    shouldAutoScrollRef.current = isAtBottom();
    if (!jumpLockRef.current) setAtBottom(shouldAutoScrollRef.current);
  }, [isAtBottom]);

  // While a jump-to-latest is animating, intermediate scroll positions are
  // not user intent — don't re-show the button (or pause follow) mid-flight.
  const jumpLockRef = useRef(false);
  const jumpToLatest = useCallback(() => {
    const container = chatContainerRef.current;
    if (!container) return;
    shouldAutoScrollRef.current = true;
    jumpLockRef.current = true;
    setAtBottom(true);
    const settle = () => {
      jumpLockRef.current = false;
      setAtBottom(isAtBottom());
    };
    container.addEventListener("scrollend", settle, { once: true });
    // Fallback for engines without scrollend (older WKWebView).
    window.setTimeout(() => {
      container.removeEventListener("scrollend", settle);
      settle();
    }, 1500);
    // Long histories: instant jump (smooth scrolling 100k+ px never settles).
    const distance = container.scrollHeight - container.scrollTop;
    container.scrollTo({
      top: container.scrollHeight,
      behavior: distance > 2000 ? "auto" : "smooth",
    });
  }, [isAtBottom]);

  useLayoutEffect(() => {
    const container = chatContainerRef.current;
    const contentWrapper = contentWrapperRef.current;
    if (!container || !contentWrapper) return;

    if (initialScrollBehavior === "top") {
      container.scrollTop = 0;
      shouldAutoScrollRef.current = false;
    } else {
      container.scrollTop = container.scrollHeight;
      shouldAutoScrollRef.current = true;
    }

    let lastContentHeight = contentWrapper.getBoundingClientRect().height;
    let prevScrollHeight = container.scrollHeight;

    const resizeObserver = new ResizeObserver(() => {
      const newContentHeight = contentWrapper.getBoundingClientRect().height;
      if (newContentHeight === lastContentHeight) return;
      lastContentHeight = newContentHeight;

      if (!shouldAutoScrollRef.current) {
        const newScrollHeight = container.scrollHeight;
        if (newScrollHeight !== prevScrollHeight && prevScrollHeight > 0) {
          const delta = newScrollHeight - prevScrollHeight;
          container.scrollTop = container.scrollTop + delta;
        }
      }
      prevScrollHeight = container.scrollHeight;
    });

    resizeObserver.observe(contentWrapper);
    return () => resizeObserver.disconnect();
  }, []);

  const normalizedMessages = useMemo(
    () => normalizeMessages(messages),
    [messages],
  );
  const lastMessage = normalizedMessages[normalizedMessages.length - 1];
  const lastMessageId = lastMessage?.id ?? null;
  const lastMessageRole = lastMessage?.role ?? null;
  const lastUserMessageId = useMemo(
    () => getLastUserMessageId(normalizedMessages),
    [normalizedMessages],
  );

  const lastUserMessageIdRef = useRef(lastUserMessageId);
  const pendingPlanningScrollUserIdRef = useRef<string | null>(null);
  useLayoutEffect(() => {
    if (
      lastUserMessageId &&
      lastUserMessageId !== lastUserMessageIdRef.current
    ) {
      shouldAutoScrollRef.current = true;
      pendingPlanningScrollUserIdRef.current = lastUserMessageId;
      const cancel = scrollToBottomSettled();
      lastUserMessageIdRef.current = lastUserMessageId;
      return cancel;
    }
  }, [lastUserMessageId, scrollToBottomSettled]);

  const planningLabel = "Processing...";
  const turns = useMemo(
    () => groupMessagesIntoTurns(normalizedMessages),
    [normalizedMessages],
  );
  const showPlanning = useMemo(() => {
    const lastMessage = normalizedMessages[normalizedMessages.length - 1];
    if (!lastMessage) return false;
    const lastTurn = turns[turns.length - 1];
    const hasAssistant = Boolean(lastTurn && lastTurn.assistantMsgs.length > 0);
    if (lastMessage.role === "user" && !hasAssistant) return true;
    return isStreaming && !getLastAssistantHasContent(normalizedMessages);
  }, [isStreaming, normalizedMessages, turns]);
  const isNewAssistantMessage =
    lastMessageRole === "assistant" &&
    Boolean(lastMessageId) &&
    lastMessageId !== lastMessageIdRef.current;
  const showAssistantBreathingSpace =
    showPlanning || assistantSpaceActiveRef.current || isNewAssistantMessage;

  useEffect(() => {
    if (lastMessageRole === "assistant") {
      if (lastMessageId && lastMessageId !== lastMessageIdRef.current) {
        assistantSpaceActiveRef.current = true;
      }
    }
    if (lastMessageRole === "user") {
      assistantSpaceActiveRef.current = false;
    }
    lastMessageIdRef.current = lastMessageId;
  }, [lastMessageId, lastMessageRole]);

  useLayoutEffect(() => {
    if (!showPlanning || !lastUserMessageId) return;
    if (pendingPlanningScrollUserIdRef.current !== lastUserMessageId) return;
    const cancel = scrollToBottomSettled();
    pendingPlanningScrollUserIdRef.current = null;
    return cancel;
  }, [lastUserMessageId, showPlanning, scrollToBottomSettled]);

  // Streaming follow — the library only pins to bottom on mount and on new
  // user messages; a growing assistant reply reflows the content and the
  // viewport drifts away from the live text. Re-pin while a reply streams in,
  // but only when the user is still auto-following (didn't scroll up).
  useLayoutEffect(() => {
    if (status !== "streaming" && status !== "submitted") return;
    const last = normalizedMessages[normalizedMessages.length - 1];
    if (!last || last.role !== "assistant") return;
    if (!shouldAutoScrollRef.current) return;
    const cancel = scrollToBottomSettled();
    return cancel;
  }, [normalizedMessages, status, scrollToBottomSettled]);

  return (
    <div className="relative flex min-h-0 flex-1">
      <div
        ref={containerRefCallback}
        onScroll={handleScroll}
        className={cn(
          "an-message-list flex-1 min-h-0 overflow-y-auto",
          className,
        )}
      >
        <div ref={contentWrapperRef} className="mx-auto px-4 py-6 max-w-an">
          <div className="space-y-2">
            {turns.map((turn, turnIndex) => {
              const isLastTurn = turnIndex === turns.length - 1;
              const turnKey = turn.userMsg?.id ?? `turn-${turnIndex}`;

              return (
                <div key={turnKey} className="relative space-y-2">
                  {turn.userMsg &&
                    (() => {
                      const text = getTextFromParts(
                        turn.userMsg!.parts ?? [],
                        "",
                      );
                      const hasParts = (turn.userMsg!.parts ?? []).length > 0;
                      if (!text && !hasParts) return null;
                      const userCreatedAt = (
                        turn.userMsg as { createdAt?: Date | string }
                      )?.createdAt;
                      const userCopyKey = `user-${turn.userMsg.id}`;
                      const userCopyVisible = activeCopyId === userCopyKey;
                      // Only render the toolbar when it has content — copy
                      // button (gated by showCopyToolbar), timestamp, or the
                      // options menu. Otherwise a 28px-tall empty row inflates
                      // the gap to the assistant reply.
                      const showUserToolbar =
                        Boolean(userCreatedAt) ||
                        (Boolean(text) &&
                          (showCopyToolbar || Boolean(onQuote)));
                      return (
                        <div className="group/user-message">
                          <CustomUserMessage
                            message={turn.userMsg}
                            className={classNames?.userMessage}
                            enableImagePreview={enableImagePreview}
                          />
                          {showUserToolbar && (
                            <MessageToolbar
                              text={text}
                              showCopy={showCopyToolbar}
                              createdAt={userCreatedAt}
                              heightClass="h-[28px]"
                              hoverClass="group-hover/user-message:opacity-100 group-hover/user-message:pointer-events-auto"
                              isVisible={userCopyVisible}
                              alignClass="justify-end"
                              onCopied={() => markCopied(userCopyKey)}
                              onQuote={onQuote}
                            />
                          )}
                        </div>
                      );
                    })()}

                  {turn.assistantMsgs.length > 0 &&
                    !(isLastTurn && showPlanning) &&
                    (() => {
                      const assistantText = getTextFromParts(
                        turn.assistantMsgs.flatMap((msg) => msg.parts ?? []),
                        "\n\n",
                      );
                      const isTurnStreaming = isStreaming && isLastTurn;
                      // Only reserve toolbar height when there's actually
                      // something to show in it. With showCopyToolbar=false the
                      // toolbar would otherwise render as a 48px-tall empty box,
                      // creating large gaps between assistant turns.
                      const lastAssistantMsg = turn.assistantMsgs[
                        turn.assistantMsgs.length - 1
                      ] as { createdAt?: Date | string } | undefined;
                      const assistantCreatedAt = lastAssistantMsg?.createdAt
                        ? new Date(lastAssistantMsg.createdAt)
                        : undefined;
                      const showToolbar =
                        Boolean(assistantText.trim()) &&
                        !isTurnStreaming &&
                        (showCopyToolbar || Boolean(onQuote));
                      const copyKey = `assistant-${turnKey}-all`;
                      const toolbarText = assistantText;
                      // Files the agent edited this turn → git changes card.
                      const editedPaths = collectEditedPaths(
                        turn.assistantMsgs,
                      );

                      return (
                        <div className="group/assistant-turn">
                          <div className="flex flex-col gap-3">
                            {turn.assistantMsgs.map((msg, i) => {
                              const isLastMsg =
                                isLastTurn &&
                                i === turn.assistantMsgs.length - 1;
                              return (
                                <AssistantParts
                                  key={msg.id}
                                  msg={msg}
                                  isLast={isLastMsg}
                                  isStreaming={isStreaming}
                                  suppressQuestionTool={suppressQuestionTool}
                                  ToolRendererComponent={CustomToolRenderer}
                                  toolRenderers={toolRenderers}
                                />
                              );
                            })}
                          </div>
                          {isTurnStreaming ? (
                            <WorkingIndicator />
                          ) : (
                            <>
                              {editedPaths.length > 0 && workspacePath && (
                                <GitChangesCard
                                  workspacePath={workspacePath}
                                  paths={editedPaths}
                                  onReview={onReviewChanges}
                                />
                              )}
                              {showToolbar || activeCopyId === copyKey ? (
                                <MessageToolbar
                                  text={toolbarText}
                                  createdAt={assistantCreatedAt}
                                  showCopy={showCopyToolbar}
                                  heightClass="h-[48px] flex items-start w-full"
                                  hoverClass="group-hover/assistant-turn:opacity-100 group-hover/assistant-turn:pointer-events-auto"
                                  isVisible={activeCopyId === copyKey}
                                  alignClass="justify-start"
                                  onCopied={() => markCopied(copyKey)}
                                  onQuote={onQuote}
                                />
                              ) : null}
                            </>
                          )}
                        </div>
                      );
                    })()}

                  {isLastTurn && showPlanning && (
                    <ToolRowBase
                      icon={<SpiralLoader size={12} />}
                      shimmerLabel={planningLabel}
                      completeLabel="Done"
                      isAnimating={true}
                    />
                  )}
                </div>
              );
            })}
          </div>
          {showAssistantBreathingSpace && (
            <div
              aria-hidden="true"
              className="min-h-[max(140px,24vh)] mx-auto max-w-an w-full"
            />
          )}
        </div>
      </div>
      {!atBottom && (
        <button
          type="button"
          aria-label="Go to latest message"
          title="Go to latest message"
          onClick={jumpToLatest}
          className="absolute bottom-3 left-1/2 z-10 flex size-7 -translate-x-1/2 cursor-pointer items-center justify-center rounded-full border border-border bg-card text-muted-fg shadow-[0_8px_24px_rgba(0,0,0,0.18),0_1px_2px_rgba(0,0,0,0.08)] transition-colors duration-150 hover:text-fg motion-safe:animate-rise"
        >
          <IconArrowDown className="size-4" />
        </button>
      )}
    </div>
  );
});

function AssistantParts({
  msg,
  isLast,
  isStreaming,
  suppressQuestionTool,
  ToolRendererComponent,
  toolRenderers,
}: {
  msg: UIMessage;
  isLast: boolean;
  isStreaming: boolean;
  suppressQuestionTool: boolean;
  ToolRendererComponent: React.ComponentType<ToolRendererProps>;
  toolRenderers?: Record<string, React.ComponentType<CustomToolRendererProps>>;
}) {
  const parts = useMemo(
    () => normalizeAssistantToolParts(msg.parts ?? []) as unknown[],
    [msg.parts],
  );

  const { elements } = useMemo(() => {
    const elems: React.ReactNode[] = [];
    const taskPartIds = new Set(
      parts
        .filter(
          (p): p is ToolPartBase =>
            isV5ToolPart(p) &&
            (p.type === "tool-Task" || p.type === "tool-Agent") &&
            typeof p.toolCallId === "string",
        )
        .map((p) => p.toolCallId!),
    );
    const nestedToolsMap = new Map<string, ToolPartBase[]>();
    const nestedToolIds = new Set<string>();

    for (const part of parts) {
      if (!isV5ToolPart(part)) continue;
      if (part.type === "tool-TaskOutput") continue;
      if (!part.toolCallId || !part.toolCallId.includes(":")) continue;
      const parentId = part.toolCallId.split(":")[0];
      if (!taskPartIds.has(parentId)) continue;
      if (!nestedToolsMap.has(parentId)) {
        nestedToolsMap.set(parentId, []);
      }
      nestedToolsMap.get(parentId)!.push(part);
      nestedToolIds.add(part.toolCallId);
    }

    let i = 0;
    while (i < parts.length) {
      const part = parts[i]!;

      if (isV5ToolPart(part) && part.type === "tool-TaskOutput") {
        i++;
        continue;
      }

      if (isTextPart(part)) {
        const text = part.text;
        if (text) {
          elems.push(
            <div
              key={`${msg.id}-text-${i}`}
              className="group/assistant-text text-[13px]"
            >
              <Markdown
                content={text}
                className="leading-relaxed [&_p]:leading-relaxed"
              />
            </div>,
          );
        }
        i++;
        continue;
      }

      if (isErrorPart(part)) {
        elems.push(
          <ErrorMessage
            key={`${msg.id}-error-${i}`}
            title={part.title}
            message={part.message}
          />,
        );
        i++;
        continue;
      }

      if (isV5ToolPart(part)) {
        if (suppressQuestionTool && part.type === "tool-Question") {
          i++;
          continue;
        }
        if (part.toolCallId && nestedToolIds.has(part.toolCallId)) {
          i++;
          continue;
        }

        const chatStreamingStatus =
          isLast && isStreaming ? "streaming" : undefined;
        const toolCallId = part.toolCallId;
        const nestedTools =
          (part.type === "tool-Task" || part.type === "tool-Agent") &&
          toolCallId
            ? nestedToolsMap.get(toolCallId) || []
            : undefined;
        elems.push(
          <ToolRendererComponent
            key={part.toolCallId ?? `${msg.id}-tool-${i}`}
            part={part}
            nestedTools={nestedTools}
            chatStatus={chatStreamingStatus}
            toolRenderers={toolRenderers}
          />,
        );
        i++;
        continue;
      }

      i++;
    }

    return { elements: elems };
  }, [
    parts,
    msg.id,
    isLast,
    isStreaming,
    suppressQuestionTool,
    ToolRendererComponent,
    toolRenderers,
  ]);

  if (elements.length > 1) {
    return (
      <div className="group/assistant-turn flex flex-col gap-3">{elements}</div>
    );
  }

  return <div className="group/assistant-turn">{elements}</div>;
}
