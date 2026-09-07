import React, {
  memo,
  useRef,
  useEffect,
  useCallback,
  useState,
  useMemo,
} from "react";
import type { UIMessage, ChatStatus } from "ai";
import {
  MessageScroller,
  useMessageScroller,
} from "@shadcn/react/message-scroller";
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
import { useStickToLatest } from "./use-stick-to-latest";
import { ThinkingSteps } from "./thinking-steps";
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
  /**
   * Notifies when the turn rail takes over the left gutter (content
   * overflows), so the chat panel can align the composer with the padded
   * transcript column.
   */
  onRailVisibleChange?: (visible: boolean) => void;
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

/**
 * Content fingerprint of a user message. The pi runtime re-projects the whole
 * transcript on every snapshot refresh (model change, thinking-level change,
 * agent_end), which hands the list brand-new message objects *with new ids*.
 * Comparing text keeps scroll intent stable across those rebuilds: a changed
 * fingerprint means the user actually sent something; an identical fingerprint
 * with a new id is just the runtime re-delivering history.
 */
function userMessageFingerprint(msg: UIMessage | undefined): string | null {
  if (!msg || msg.role !== "user") return null;
  return getTextFromParts(msg.parts ?? [], "\n");
}
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
    // mt-4 lifts the card clear of the message prose above it (which sits at
    // a 12px internal rhythm), so the turn's outcome reads as its own group.
    <div className="mt-4 overflow-hidden rounded-lg border border-border bg-card">
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
type Turn = { userMsg?: UIMessage; assistantMsgs: UIMessage[] };

function groupMessagesIntoTurns(messages: UIMessage[]): Turn[] {
  const turns: Turn[] = [];
  let current: Turn | null = null;

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

/**
 * Chat transcript scroller. Opening position and the jump-to-latest chip
 * stay on MessageScroller. Stick-to-bottom while a reply streams is owned
 * by `useStickToLatest` — the primitive otherwise pins a new turn to the
 * *start* of the viewport, which hides the latest tokens as they arrive.
 *
 * `autoScroll` stays off. A Pi snapshot refresh (model / thinking-level
 * change) remounts the same turns; MessageScroller then treats that as a
 * content change, zeros its end-spacer, and align-starts the last item —
 * which looks like the chat jumped to the middle. We already follow the
 * live edge ourselves.
 */
export const MessageList = memo(function MessageList(props: MessageListProps) {
  return (
    <MessageScroller.Provider
      autoScroll={false}
      defaultScrollPosition={
        props.initialScrollBehavior === "top" ? "start" : "end"
      }
    >
      <MessageListInner {...props} />
    </MessageScroller.Provider>
  );
});

const MessageListInner = memo(function MessageListInner({
  messages,
  status,
  className,
  showCopyToolbar = true,
  suppressQuestionTool = false,
  enableImagePreview = true,
  slots,
  classNames,
  toolRenderers,
  onQuote,
  workspacePath,
  onReviewChanges,
  onRailVisibleChange,
}: MessageListProps) {
  const { scrollToEnd } = useMessageScroller();
  const [activeCopyId, setActiveCopyId] = useState<string | null>(null);
  const viewportElRef = useRef<HTMLDivElement | null>(null);
  const contentElRef = useRef<HTMLDivElement | null>(null);
  // The rail replaces the scrollbar while visible: pad the transcript away
  // from the rail and hide the native scrollbar (beUI PreviewRail behavior).
  const [railVisible, setRailVisible] = useState(false);
  useEffect(() => {
    onRailVisibleChange?.(railVisible);
  }, [railVisible, onRailVisibleChange]);

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

  const normalizedMessages = useMemo(
    () => normalizeMessages(messages),
    [messages],
  );
  const lastUserMessageId = useMemo(
    () => getLastUserMessageId(normalizedMessages),
    [normalizedMessages],
  );
  const lastUserFingerprint = useMemo(() => {
    for (let i = normalizedMessages.length - 1; i >= 0; i -= 1) {
      const fp = userMessageFingerprint(normalizedMessages[i]);
      if (fp !== null) return fp;
    }
    return null;
  }, [normalizedMessages]);

  const followKey = useMemo(() => {
    const last = normalizedMessages[normalizedMessages.length - 1];
    return [
      normalizedMessages.length,
      last?.id ?? "",
      last ? getTextFromParts(last.parts ?? [], "").length : 0,
      last?.parts?.length ?? 0,
    ].join(":");
  }, [normalizedMessages]);

  const { onViewportScroll, pinLatest } = useStickToLatest({
    viewportRef: viewportElRef,
    contentRef: contentElRef,
    scrollToEnd,
    lastUserMessageId,
    lastUserFingerprint,
    followKey,
  });

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

  return (
    <MessageScroller.Root className="an-chat-root relative flex min-h-0 flex-1 flex-col overflow-hidden">
      <MessageScroller.Viewport
        ref={viewportElRef}
        onScroll={onViewportScroll}
        className={cn(
          "an-message-list min-h-0 flex-1 overflow-y-auto overflow-anchor-none overscroll-y-contain",
          // While the rail is up it replaces the scrollbar entirely and the
          // transcript pads away from the ticks (25px rail + 25px gap + 20px
          // clearance).
          railVisible &&
            "pl-[70px] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
          className,
        )}
      >
        <MessageScroller.Content
          ref={contentElRef}
          className="mx-auto w-full max-w-[calc(var(--an-max-width)+3rem)] space-y-2 px-6 py-6"
        >
          {turns.map((turn, turnIndex) => {
            const isLastTurn = turnIndex === turns.length - 1;
            const turnKey = turn.userMsg?.id ?? `turn-${turnIndex}`;

            return (
              <MessageScroller.Item
                key={turnKey}
                messageId={turnKey}
                scrollAnchor={false}
                className="relative space-y-2"
              >
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
                      (Boolean(text) && (showCopyToolbar || Boolean(onQuote)));
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
                    const editedPaths = collectEditedPaths(turn.assistantMsgs);

                    return (
                      <div className="group/assistant-turn">
                        <div className="flex flex-col gap-3">
                          {turn.assistantMsgs.map((msg, i) => {
                            const isLastMsg =
                              isLastTurn && i === turn.assistantMsgs.length - 1;
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
                    icon={<ThinkingSteps className="scale-75" />}
                    shimmerLabel={planningLabel}
                    completeLabel="Done"
                    isAnimating={true}
                  />
                )}
              </MessageScroller.Item>
            );
          })}
        </MessageScroller.Content>
      </MessageScroller.Viewport>
      <JumpToLatestButton onJump={pinLatest} />
      <MessageRail
        turns={turns}
        viewportElRef={viewportElRef}
        onOverflowChange={setRailVisible}
      />
    </MessageScroller.Root>
  );
});

/**
 * Jump-to-latest affordance, rendered by the MessageScroller primitive. It is
 * inert (and faded out) whenever the transcript already rests at the live
 * edge, so it only appears once the user has scrolled away from the bottom.
 */

const RAIL_TITLE_LENGTH = 56;
const RAIL_DESCRIPTION_LENGTH = 88;

/** Word-boundary-aware truncation with an ellipsis (beUI PreviewRail style). */
function excerpt(text: string, limit: number): string {
  if (text.length <= limit) return text;
  const slice = text.slice(0, limit);
  const boundary = slice.lastIndexOf(" ");
  const end = boundary > limit * 0.65 ? boundary : limit;
  return `${slice.slice(0, end).trim()}…`;
}

type RailItem = {
  id: string;
  label: string;
  description?: string;
  isLast: boolean;
};

/**
 * Message navigation rail — a slim column of ticks (one per turn) at the
 * right edge of the transcript, in the beUI PreviewRail style. The tick
 * nearest the reading position is bright and full-length; its neighbors
 * taper off. Hovering (or focusing) a tick reveals a preview card with the
 * message content; clicking scrolls that turn to the viewport center (the
 * last tick jumps to the live edge). The rail replaces the scrollbar while
 * visible, so the viewport gains right padding and loses its scrollbar.
 */
function MessageRail({
  turns,
  viewportElRef,
  onOverflowChange,
}: {
  turns: Turn[];
  viewportElRef: React.RefObject<HTMLDivElement | null>;
  onOverflowChange: (visible: boolean) => void;
}) {
  const { scrollToMessage, scrollToEnd } = useMessageScroller();
  const railRef = useRef<HTMLDivElement | null>(null);
  const frameRef = useRef(0);
  const [activeId, setActiveId] = useState("");
  const [hovered, setHovered] = useState<{ id: string; top: number } | null>(
    null,
  );
  const [rowHeight, setRowHeight] = useState(10);
  const [overflowing, setOverflowing] = useState(false);

  const items = useMemo<RailItem[]>(
    () =>
      turns.map((turn, i) => {
        // Must match the transcript Item's messageId exactly, otherwise the
        // jump silently fails for turns the DOM addresses differently.
        const id = turn.userMsg?.id ?? `turn-${i}`;
        const prompt = turn.userMsg
          ? getTextFromParts(turn.userMsg.parts ?? [], "")
          : "";
        const reply = getTextFromParts(
          turn.assistantMsgs.flatMap((msg) => msg.parts ?? []),
          "\n\n",
        );
        const isLast = i === turns.length - 1;
        if (prompt) {
          return {
            id,
            label: excerpt(prompt, RAIL_TITLE_LENGTH),
            description: reply
              ? excerpt(reply, RAIL_DESCRIPTION_LENGTH)
              : undefined,
            isLast,
          };
        }
        // Turn with an empty/attachment-only prompt (or assistant-only when
        // a session starts mid-conversation).
        return {
          id,
          label: excerpt(reply, RAIL_TITLE_LENGTH) || "Message",
          description:
            reply.length > RAIL_TITLE_LENGTH
              ? excerpt(
                  reply.slice(RAIL_TITLE_LENGTH).trim(),
                  RAIL_DESCRIPTION_LENGTH,
                )
              : undefined,
          isLast,
        };
      }),
    [turns],
  );

  // Nearest-to-center tick tracks the reading position; run inside a rAF so
  // scroll bursts coalesce into one measurement pass.
  const sync = useCallback(() => {
    if (frameRef.current) return;
    frameRef.current = requestAnimationFrame(() => {
      frameRef.current = 0;
      const viewport = viewportElRef.current;
      if (!viewport) return;
      const rows = viewport.querySelectorAll("[data-message-id]");
      const nextOverflowing =
        viewport.scrollHeight > viewport.clientHeight + 1 && rows.length > 1;
      setOverflowing(nextOverflowing);
      if (!nextOverflowing) return;
      setRowHeight((current) => {
        const next = Math.max(
          8,
          Math.min(14, Math.floor((viewport.clientHeight - 16) / rows.length)),
        );
        return current === next ? current : next;
      });
      const rect = viewport.getBoundingClientRect();
      const readingLine = rect.top + rect.height / 2;
      let bestId = "";
      let bestDistance = Number.POSITIVE_INFINITY;
      rows.forEach((row) => {
        const r = row.getBoundingClientRect();
        const distance = Math.abs(r.top + r.height / 2 - readingLine);
        if (distance < bestDistance) {
          bestDistance = distance;
          bestId = row.getAttribute("data-message-id") ?? "";
        }
      });
      if (bestId)
        setActiveId((current) => (current === bestId ? current : bestId));
    });
  }, [viewportElRef]);

  useEffect(() => {
    const viewport = viewportElRef.current;
    if (!viewport) return;
    sync();
    viewport.addEventListener("scroll", sync, { passive: true });
    const observer =
      typeof ResizeObserver === "undefined" ? null : new ResizeObserver(sync);
    observer?.observe(viewport);
    return () => {
      viewport.removeEventListener("scroll", sync);
      observer?.disconnect();
      if (frameRef.current) {
        cancelAnimationFrame(frameRef.current);
        // Reset the guard: a cancelled frame must not block future syncs
        // (StrictMode's effect double-invoke hits exactly this path).
        frameRef.current = 0;
      }
    };
  }, [sync, viewportElRef]);

  // Transcript content changed (new turn, streaming growth, rebuild).
  useEffect(() => {
    sync();
  }, [turns, sync]);

  useEffect(() => {
    onOverflowChange(overflowing);
  }, [overflowing, onOverflowChange]);

  if (!overflowing || items.length <= 1) return null;

  const hoveredItem = hovered
    ? items.find((item) => item.id === hovered.id)
    : undefined;

  return (
    <div
      ref={railRef}
      role="navigation"
      aria-label="Message navigation"
      className="pointer-events-none absolute inset-0 z-10"
    >
      {/* Pinned 25px from the chat area's left edge, vertically centered.
          Ticks expand rightward on hover; the preview opens to their right. */}
      <div className="absolute left-[25px] inset-y-0 flex w-11 flex-col items-start justify-center overflow-hidden py-1">
        {items.map((item) => {
          // Only the hovered/focused tick expands (20px → 44px). The active
          // reading position is a color cue, never a size change.
          const displayed = item.id === hovered?.id;
          const scale = displayed ? 1 : 20 / 44;
          return (
            <button
              key={item.id}
              type="button"
              data-message-rail-tick={item.id}
              aria-label={`Go to message: ${item.label}`}
              aria-current={item.id === activeId ? "true" : undefined}
              onClick={() => {
                if (item.isLast) {
                  scrollToEnd({ behavior: "smooth" });
                } else {
                  // Align the turn's beginning to the top of the viewport —
                  // centering tall turns lands you mid-message instead of at
                  // the message you clicked.
                  scrollToMessage(item.id, {
                    align: "start",
                    scrollMargin: 8,
                    behavior: "smooth",
                  });
                }
              }}
              onMouseEnter={(event) =>
                setHovered({
                  id: item.id,
                  top: event.currentTarget.getBoundingClientRect().top -
                    (railRef.current?.getBoundingClientRect().top ?? 0),
                })
              }
              onMouseLeave={() =>
                setHovered((current) =>
                  current?.id === item.id ? null : current,
                )
              }
              onFocus={(event) => {
                if (event.currentTarget.matches(":focus-visible")) {
                  setHovered({
                    id: item.id,
                    top: event.currentTarget.getBoundingClientRect().top -
                    (railRef.current?.getBoundingClientRect().top ?? 0),
                  });
                }
              }}
              onBlur={() =>
                setHovered((current) =>
                  current?.id === item.id ? null : current,
                )
              }
              className="pointer-events-auto relative flex cursor-pointer items-center justify-start rounded-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
              style={{ height: rowHeight, width: 35 }}
            >
              <span
                aria-hidden="true"
                className={cn(
                  "block h-0.5 w-11 origin-left rounded-full transition-[transform,background-color] duration-200 ease-out",
                  displayed || item.id === activeId
                    ? "bg-fg"
                    : "bg-muted-fg/50 group-hover:bg-muted-fg/80",
                )}
                style={{ transform: `scaleX(${scale})` }}
              />
            </button>
          );
          })}
      </div>
      {hoveredItem && hovered && (
          <div
            data-message-rail-preview
            className="pointer-events-none absolute left-[73px] z-20 w-72 -translate-y-1/2 rounded-xl border border-border bg-card/95 p-3 text-left shadow-[0_12px_32px_rgba(0,0,0,0.22),0_2px_6px_rgba(0,0,0,0.10)] backdrop-blur-sm motion-safe:animate-rise"
            style={{ top: hovered.top + rowHeight / 2 }}
          >
            <p className="text-[13px] font-medium leading-snug text-fg">
              {hoveredItem.label}
            </p>
            {hoveredItem.description && (
              <p className="mt-1 line-clamp-2 text-xs leading-snug text-muted-fg">
                {hoveredItem.description}
              </p>
            )}
          </div>
        )}
    </div>
  );
}

function JumpToLatestButton({ onJump }: { onJump: () => void }) {
  return (
    <MessageScroller.Button
      direction="end"
      onClick={(event) => {
        event.preventDefault();
        onJump();
      }}
      aria-label="Go to latest message"
      title="Go to latest message"
      className="absolute bottom-3 left-1/2 z-10 flex size-7 -translate-x-1/2 cursor-pointer items-center justify-center rounded-full border border-border bg-card text-muted-fg shadow-[0_8px_24px_rgba(0,0,0,0.18),0_1px_2px_rgba(0,0,0,0.08)] transition-[opacity,background-color,color] duration-150 hover:text-fg inert:pointer-events-none inert:opacity-0"
    >
      <IconArrowDown className="size-4" />
    </MessageScroller.Button>
  );
}

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
