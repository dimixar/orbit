"use client";

/**
 * The main chat — the original Orbit chat UI, now powered by the Pi runtime
 * over SSE (agent/sse-server.ts) instead of the WebSocket daemon.
 *
 *   browser (this webview)                    Node server
 *   ─────────────────                         ───────────
 *   usePiRuntime (SSE)  ──HTTP/SSE──▶        agent/sse-server.ts
 *   usePiSseChat                              └ createPiNodeClient → Pi SDK
 *
 * Start the SSE server first:  pnpm agent:sse
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowPathIcon,
  ArrowUpIcon,
  ChevronDownIcon,
  CodeBracketIcon,
  CommandLineIcon,
  ComputerDesktopIcon,
  FolderIcon,
  InformationCircleIcon,
  Squares2X2Icon,
  StopIcon,
} from "@heroicons/react/24/outline";
import { CheckIcon, PlusIcon } from "@heroicons/react/20/solid";
import { twMerge } from "tailwind-merge";
import type { ChatStatus, UIMessage } from "ai";
import { useAui, useAuiState } from "@assistant-ui/react";
import { usePiRuntimeExtras, usePiSession } from "@assistant-ui/react-pi";
import type { PiContextUsage } from "@assistant-ui/react-pi";
import {
  MessageList as AgentMessageList,
  collectEditedPaths,
} from "@/components/agent-elements/message-list";
import { GitDiffPanel } from "@/components/git-diff-panel";
import {
  Menu,
  MenuContent,
  MenuDescription,
  MenuItem,
  MenuLabel,
  MenuSection,
  MenuTrigger,
  menuContentStyles,
} from "@/components/ui/menu";
import {
  Popover,
  PopoverBody,
  PopoverContent,
  PopoverHeader,
  PopoverTitle,
  PopoverTrigger,
} from "@/components/ui/popover";
import { Dialog, DialogClose } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { SearchField, SearchInput } from "@/components/ui/search-field";
import { Autocomplete, useFilter } from "react-aria-components/Autocomplete";
import { Menu as MenuPrimitive } from "react-aria-components/Menu";
import { usePiSseChat, usePiModels } from "@/lib/pi-sse";
import StackIcon, { type IconName } from "tech-stack-icons";
import { SyntheticIcon } from "@/components/icons/synthetic-icon";
import {
  checkoutWorkspaceBranch,
  createWorkspaceBranch,
  fetchWorkspaceApps,
  fetchWorkspaceBranch,
  fetchWorkspaceBranches,
  fetchWorkspaceStatus,
  openWorkspaceInApp,
  piClient,
  type OpenInApp,
} from "@/lib/pi-client";
import { isTauri, pathBasename, pickWorkspaceFolder } from "@/lib/pick-folder";
import type { ChatPart } from "@/lib/pi-agent";

/** toolName → tool-card part type: bash → tool-Bash, web_fetch → tool-WebFetch. */
function pascalToolName(name: string): string {
  return name.replace(/(?:^|_)([a-z])/g, (_, c: string) => c.toUpperCase());
}

/** Map one daemon ChatPart onto the Agent Elements UIMessage part(s). */
function toUIPart(part: ChatPart, key: string): UIMessage["parts"] {
  if (part.type === "text") {
    return part.text ? [{ type: "text", text: part.text }] : [];
  }
  if (part.type === "thinking") {
    const done = part.done === true;
    return [
      {
        type: "tool-Thinking" as const,
        toolCallId: `${key}-think`,
        state: done ? "output-available" : "input-streaming",
        input: { thought: part.text },
        ...(done ? { output: part.text } : {}),
      },
    ] as unknown as UIMessage["parts"];
  }
  return [
    {
      type: `tool-${pascalToolName(part.toolName)}` as const,
      toolCallId: part.toolCallId || `${key}-tool`,
      state: part.isError
        ? "output-error"
        : part.output !== undefined
          ? "output-available"
          : "call",
      input: part.input ?? {},
      ...(part.output !== undefined ? { output: part.output } : {}),
    },
  ] as unknown as UIMessage["parts"];
}

/* ------------------------------------------------------------------ */
/* Marks                                                               */
/* ------------------------------------------------------------------ */

/** Six-spoke coral asterisk used above the empty-state prompt. */
function Asterisk({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden="true"
      className={twMerge("text-warning", className)}
    >
      <g stroke="currentColor" strokeWidth="1.9" strokeLinecap="round">
        <line x1="8" y1="1.6" x2="8" y2="14.4" />
        <line x1="2.8" y1="4.8" x2="13.2" y2="11.2" />
        <line x1="2.8" y1="11.2" x2="13.2" y2="4.8" />
      </g>
    </svg>
  );
}

/** White "Pᵢ" tile: the agent mark, optionally tinted (header variant). */
function PiTile({
  className,
  tint = "oklch(0.228 0.013 107.4)",
}: {
  className?: string;
  tint?: string;
}) {
  return (
    <span
      aria-hidden="true"
      className={twMerge(
        "flex size-3.5 shrink-0 items-center justify-center rounded-[3px] bg-fg",
        className,
      )}
    >
      <svg viewBox="0 0 10 10" className="size-2.5" fill="none">
        <path
          d="M2.6 8.4V1.6"
          stroke={tint}
          strokeWidth="2"
          strokeLinecap="round"
        />
        <path
          d="M2.6 1.6h2a1.9 1.9 0 0 1 0 3.8h-2"
          stroke={tint}
          strokeWidth="2"
          strokeLinecap="round"
        />
        <rect x="6.4" y="6.3" width="2.1" height="2.1" rx="0.4" fill={tint} />
      </svg>
    </span>
  );
}

/** Git-branch glyph (heroicons has none). */
function BranchIcon({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      aria-hidden="true"
      className={className}
    >
      <circle cx="4.5" cy="3.6" r="1.8" />
      <circle cx="4.5" cy="12.4" r="1.8" />
      <circle cx="11.8" cy="5.6" r="1.8" />
      <path d="M4.5 5.4v5.2" />
      <path d="M11.8 7.4c0 2.2-2.2 3.3-5 3.5" />
    </svg>
  );
}

/** Right-hand panel glyph (heroicons only ships the left variant). */
function RightPanelIcon({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 20 20"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      aria-hidden="true"
      className={className}
    >
      <rect x="2.8" y="4" width="14.4" height="12" rx="2.2" />
      <path d="M13.2 4v12" />
    </svg>
  );
}

/** Small square icon button, muted until hover. */
function IconButton({
  label,
  className,
  children,
  onPress,
}: {
  label: string;
  className?: string;
  children: React.ReactNode;
  onPress?: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onPress}
      className={twMerge(
        "flex size-7 cursor-pointer items-center justify-center rounded-lg text-muted-fg transition-colors duration-100 hover:bg-muted hover:text-fg",
        className,
      )}
    >
      {children}
    </button>
  );
}

/* ------------------------------------------------------------------ */
/* Header                                                              */
/* ------------------------------------------------------------------ */

/**
 * Live working-tree diff totals for the header (+adds −dels). Fetched from
 * the agent's SSE server on mount, every 20s, and whenever the git changes
 * panel closes (turns likely ended). Clicking opens the changes panel.
 */
function GitStatsChip({
  workspacePath,
  onOpen,
}: {
  workspacePath: string;
  onOpen?: () => void;
}) {
  const [totals, setTotals] = useState<{
    additions: number;
    deletions: number;
  } | null>(null);

  useEffect(() => {
    let cancelled = false;
    const refresh = () => {
      void fetchWorkspaceStatus(workspacePath).then((files) => {
        if (cancelled || !files) return;
        setTotals({
          additions: files.reduce((sum, f) => sum + f.additions, 0),
          deletions: files.reduce((sum, f) => sum + f.deletions, 0),
        });
      });
    };
    refresh();
    const timer = setInterval(refresh, 20_000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [workspacePath]);

  return (
    <button
      type="button"
      onClick={onOpen}
      title="Open git changes panel"
      aria-label={`Working tree changes: +${totals?.additions ?? 0} additions, -${totals?.deletions ?? 0} deletions — open panel`}
      className="flex cursor-pointer items-center gap-2 rounded-md px-1 py-0.5 text-xs font-medium tabular-nums outline-none transition-colors duration-100 hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
    >
      <span className="text-success-subtle-fg">+{totals?.additions ?? 0}</span>
      <span className="text-danger-subtle-fg">-{totals?.deletions ?? 0}</span>
    </button>
  );
}

function ChatHeader({
  title,
  workspacePath,
  onTogglePanel,
}: {
  title: string;
  /** Absolute path of the active workspace — enables the “Open in” picker. */
  workspacePath?: string;
  /** Toggles the git changes panel. */
  onTogglePanel?: () => void;
}) {
  return (
    <header className="flex h-12 shrink-0 items-center justify-between pl-4 pr-3.5">
      <h1 className="truncate text-[13px] font-medium tracking-[-0.01em] text-fg">
        {title}
      </h1>

      <div className="flex shrink-0 items-center gap-4">
        {workspacePath && <OpenInMenu workspacePath={workspacePath} />}
        {workspacePath && (
          <GitStatsChip workspacePath={workspacePath} onOpen={onTogglePanel} />
        )}

        <div className="flex items-center gap-2">
          <IconButton label="Session info">
            <InformationCircleIcon className="size-4" strokeWidth={1.6} />
          </IconButton>

          <IconButton label="Toggle panel" onPress={onTogglePanel}>
            <RightPanelIcon className="size-4" />
          </IconButton>
        </div>
      </div>
    </header>
  );
}

/* ------------------------------------------------------------------ */
/* Context meter                                                       */
/* ------------------------------------------------------------------ */

/** Compact token count: 41234 → "41.2K". */
function compactTokens(n: number): string {
  return new Intl.NumberFormat("en", {
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(n);
}

/** Exact token count with grouping: 41234 → "41,234". */
function fullTokens(n: number): string {
  return new Intl.NumberFormat("en").format(n);
}

/** Shared escalation: muted → warning at 70% → danger at 90%. */
function contextLevel(pct: number): string {
  return pct >= 90
    ? "text-danger-subtle-fg"
    : pct >= 70
      ? "text-warning-subtle-fg"
      : "text-muted-fg";
}

/**
 * Branch switcher for the composer's status row, built on the app's Menu
 * stack (Intent UI): the branch chip is a MenuTrigger; its MenuContent lists
 * every local branch (arrow keys, typeahead, current one checked) with
 * click-to-checkout, and “Create and checkout new branch…” opens a small
 * controlled Popover for the name — the kit's dialog-from-a-menu-item
 * pattern. Actions run through the agent's SSE server and surface git's own
 * `fatal:` line on failure — the row only ever reflects what git did.
 */
function BranchPicker({
  workspacePath,
  branch,
  onSwitched,
}: {
  workspacePath: string;
  branch: string;
  /** Called after a successful checkout/create so the chip updates at once. */
  onSwitched: () => void;
}) {
  const [data, setData] = useState<{
    current: string | null;
    branches: string[];
  } | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [newName, setNewName] = useState("");
  /** Branch with an in-flight checkout/create — renders as a spinner row. */
  const [busyBranch, setBusyBranch] = useState<string | null>(null);
  const busy = busyBranch !== null;
  const triggerRef = useRef<HTMLButtonElement>(null);

  // Fresh branches every time the picker opens — never a stale list.
  useEffect(() => {
    void fetchWorkspaceBranches(workspacePath).then((next) => setData(next));
  }, [workspacePath]);

  const current = data?.current ?? null;

  const checkout = async (name: string) => {
    if (busy || name === current) return;
    setBusyBranch(name);
    setActionError(null);
    try {
      await checkoutWorkspaceBranch(workspacePath, name);
      onSwitched();
    } catch (error) {
      setActionError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusyBranch(null);
    }
  };

  const create = async () => {
    const name = newName.trim();
    if (!name || busy) return;
    setBusyBranch(name);
    setActionError(null);
    try {
      await createWorkspaceBranch(workspacePath, name);
      setCreateOpen(false);
      setNewName("");
      onSwitched();
    } catch (error) {
      setActionError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusyBranch(null);
    }
  };

  return (
    <>
      <Menu>
        <MenuTrigger
          ref={triggerRef}
          aria-label={`Git branch: ${branch === "HEAD" ? "detached HEAD" : branch} — switch branch`}
          className="flex cursor-pointer items-center gap-1.5 rounded-md px-1 py-0.5 -mx-1 text-[11px] text-muted-fg outline-hidden transition-colors duration-100 hover:bg-muted hover:text-fg data-[pressed]:bg-muted data-[pressed]:text-fg focus-visible:ring-2 focus-visible:ring-ring"
        >
          <BranchIcon className="size-3.5" />
          <span className="max-w-40 truncate font-mono">
            {branch === "HEAD" ? "detached" : branch}
          </span>
        </MenuTrigger>
        <MenuContent
          placement="top"
          popover={{ className: "w-64" }}
          className="p-0"
          disabledKeys={busyBranch ? [busyBranch] : []}
        >
          <MenuSection label="Branches">
            {data === null ? (
              <MenuItem isDisabled className="px-2 py-1 sm:px-2 sm:py-1">
                <MenuLabel className="text-[11px] text-muted-fg">
                  Loading branches…
                </MenuLabel>
              </MenuItem>
            ) : data.branches.length === 0 ? (
              <MenuItem isDisabled className="px-2 py-1 sm:px-2 sm:py-1">
                <MenuLabel className="text-[11px] text-muted-fg">
                  No local branches
                </MenuLabel>
              </MenuItem>
            ) : (
              data.branches.map((name) => (
                <MenuItem
                  key={name}
                  id={name}
                  textValue={name}
                  onAction={() => void checkout(name)}
                  className="px-2 py-1 sm:px-2 sm:py-1"
                >
                  <BranchIcon className="size-3.5 shrink-0" />
                  <MenuLabel className="min-w-0 truncate font-mono text-xs">
                    {name}
                    {name === current && (
                      <span className="sr-only"> (current branch)</span>
                    )}
                  </MenuLabel>
                  {busyBranch === name ? (
                    <ArrowPathIcon className="size-3.5 shrink-0 animate-spin" />
                  ) : (
                    name === current && (
                      <CheckIcon
                        className="size-3.5 shrink-0"
                        strokeWidth={2}
                      />
                    )
                  )}
                </MenuItem>
              ))
            )}
          </MenuSection>
          <MenuSection>
            <MenuItem
              id="__create"
              textValue="Create and checkout new branch"
              onAction={() => {
                setNewName("");
                setActionError(null);
                setCreateOpen(true);
              }}
              className="px-2 py-1 sm:px-2 sm:py-1"
            >
              <PlusIcon className="size-3.5 shrink-0" strokeWidth={2} />
              <MenuLabel className="text-xs">
                Create and checkout new branch…
              </MenuLabel>
            </MenuItem>
          </MenuSection>
        </MenuContent>
      </Menu>

      {/* Branch-name entry — a controlled popover anchored to the chip, the
          kit's dialog-from-a-menu-item pattern. */}
      <PopoverContent
        isOpen={createOpen}
        onOpenChange={setCreateOpen}
        triggerRef={triggerRef}
        placement="top"
        className="w-64"
      >
        <Dialog className="[--gutter:--spacing(2)] p-(--gutter)">
          <Input
            autoFocus
            aria-label="New branch name"
            placeholder="Branch name"
            value={newName}
            disabled={busy}
            onChange={(e) => setNewName(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void create();
            }}
            className="h-7 py-1 text-xs"
          />
          {actionError && (
            <p className="mt-1.5 text-[11px] text-danger-subtle-fg">
              {actionError}
            </p>
          )}
          <div className="mt-2.5 flex items-center justify-end gap-1.5">
            <DialogClose intent="outline" size="xs" className="text-xs">
              Cancel
            </DialogClose>
            <Button
              intent="primary"
              size="xs"
              isDisabled={!newName.trim() || busy}
              onPress={() => void create()}
            >
              Create branch
            </Button>
          </div>
        </Dialog>
      </PopoverContent>
    </>
  );
}

/**
 * Round context-usage meter for the composer's status row: a small ring whose
 * arc is the share of the model's context window the session occupies, with
 * the exact percent beside it. Clicking it opens a details popover — progress
 * bar, used / remaining / window figures, and the active model — so the
 * one-line status row stays quiet while the full picture stays one click away.
 * Data comes from the daemon's `context_usage` event — nothing is estimated
 * here, so the meter stays hidden until the session has a real reading
 * (brand-new chats).
 *
 * Color escalates as the window fills: muted → warning at 70% → danger at
 * 90%. While pi is compacting the context, the meter pulses and says so.
 */
function ContextMeter({
  usage,
  compacting,
  model,
}: {
  usage?: PiContextUsage;
  compacting: boolean;
  model?: string;
}) {
  const pct =
    usage?.percent ??
    (usage?.tokens != null && usage.contextWindow > 0
      ? (usage.tokens / usage.contextWindow) * 100
      : null);
  if (!usage || pct == null) return null;
  const clamped = Math.min(100, Math.max(0, pct));
  const level = contextLevel(clamped);
  const label = compacting
    ? "Compacting context…"
    : `Context details: ${Math.round(clamped)}% of the context window used`;
  const remaining = Math.max(0, usage.contextWindow - (usage.tokens ?? 0));
  // r=6.5 in a 16px viewBox → circumference 2πr; the arc starts at 12 o'clock
  // (rotate -90°) and sweeps clockwise, capped at a full ring.
  const r = 6.5;
  const circumference = 2 * Math.PI * r;
  return (
    <Popover>
      <PopoverTrigger
        aria-label={label}
        className={twMerge(
          "flex -mx-1 -my-0.5 cursor-pointer items-center gap-1.5 rounded-md px-1 py-0.5 text-[11px] outline-hidden transition-colors hover:bg-muted data-[pressed]:bg-muted focus-visible:ring-2 focus-visible:ring-ring",
          level,
          compacting && "motion-safe:animate-pulse",
        )}
      >
        <svg viewBox="0 0 16 16" aria-hidden="true" className="size-3.5">
          <circle
            cx="8"
            cy="8"
            r={r}
            fill="none"
            strokeWidth="2.5"
            className="stroke-fg/15"
          />
          <circle
            cx="8"
            cy="8"
            r={r}
            fill="none"
            strokeWidth="2.5"
            strokeLinecap="round"
            strokeDasharray={circumference}
            strokeDashoffset={circumference * (1 - clamped / 100)}
            transform="rotate(-90 8 8)"
            className="stroke-current transition-[stroke-dashoffset] duration-700 ease-out"
          />
        </svg>
        <span className="tabular-nums">{Math.round(clamped)}%</span>
      </PopoverTrigger>
      <PopoverContent placement="top" className="w-64">
        <Dialog className="[--gutter:--spacing(2)] p-(--gutter)">
          <PopoverHeader className="p-0 pb-0">
            <div className="flex items-baseline justify-between gap-2">
              <PopoverTitle className="text-xs font-semibold">
                Context
              </PopoverTitle>
              {compacting ? (
                <span className="flex items-center gap-1.5 text-[11px] text-warning-subtle-fg motion-safe:animate-pulse">
                  <span className="size-1.5 rounded-full bg-current" />
                  Compacting…
                </span>
              ) : (
                <span className={`text-xs font-medium tabular-nums ${level}`}>
                  {Math.round(clamped)}%
                </span>
              )}
            </div>
          </PopoverHeader>
          <PopoverBody className="px-0 pt-1.5 pb-0">
            {/* Linear counterpart of the ring, same escalation thresholds */}
            <div
              role="progressbar"
              aria-valuenow={Math.round(clamped)}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-label="Context window used"
              className="mt-2 h-1.5 overflow-hidden rounded-full bg-fg/10"
            >
              <div
                className={twMerge(
                  "h-full rounded-full transition-[width] duration-700 ease-out",
                  clamped >= 90
                    ? "bg-danger-subtle-fg"
                    : clamped >= 70
                      ? "bg-warning-subtle-fg"
                      : "bg-fg/60",
                )}
                style={{ width: `${clamped}%` }}
              />
            </div>

            <dl className="mt-2 space-y-0.5 text-[11px]">
              <div className="flex items-baseline justify-between gap-2">
                <dt className="text-muted-fg">Used</dt>
                <dd className="tabular-nums text-fg">
                  {fullTokens(usage.tokens ?? 0)}
                  <span className="text-muted-fg">
                    {" "}
                    · {compactTokens(usage.tokens ?? 0)}
                  </span>
                </dd>
              </div>
              <div className="flex items-baseline justify-between gap-2">
                <dt className="text-muted-fg">Remaining</dt>
                <dd
                  className={twMerge(
                    "tabular-nums",
                    level !== "text-muted-fg" && level,
                  )}
                >
                  {fullTokens(remaining)}
                  <span className="text-muted-fg">
                    {" "}
                    · {compactTokens(remaining)}
                  </span>
                </dd>
              </div>
              <div className="flex items-baseline justify-between gap-2">
                <dt className="text-muted-fg">Window</dt>
                <dd className="tabular-nums text-fg">
                  {fullTokens(usage.contextWindow)}
                  <span className="text-muted-fg">
                    {" "}
                    · {compactTokens(usage.contextWindow)}
                  </span>
                </dd>
              </div>
              {model && (
                <div className="flex items-baseline justify-between gap-2">
                  <dt className="text-muted-fg">Model</dt>
                  <dd className="truncate font-mono text-fg" title={model}>
                    {model}
                  </dd>
                </div>
              )}
            </dl>
          </PopoverBody>
        </Dialog>
      </PopoverContent>
    </Popover>
  );
}

/* ------------------------------------------------------------------ */
/* Composer                                                            */
/* ------------------------------------------------------------------ */

/**
 * Current git branch of the active workspace, best-effort. Resolved by the
 * agent's SSE server (the only side with filesystem access); `null` when the
 * folder isn't a repo, git is missing, or the server is offline. Re-polls on
 * a 30s cadence and on window focus — checkouts can happen outside the app
 * while a session is open, and the status row must reflect the truth.
 * Returns a `refresh` so checkout/create in the branch picker updates the
 * chip immediately instead of waiting for the next poll.
 */
function useWorkspaceGit(workspacePath: string | undefined): {
  branch: string | null;
  refresh: () => void;
} {
  const [branch, setBranch] = useState<string | null>(null);

  const refresh = useCallback(() => {
    if (!workspacePath) return;
    void fetchWorkspaceBranch(workspacePath).then((next) => setBranch(next));
  }, [workspacePath]);

  useEffect(() => {
    if (!workspacePath) {
      setBranch(null);
      return;
    }
    refresh();
    const timer = setInterval(refresh, 30_000);
    window.addEventListener("focus", refresh);
    return () => {
      clearInterval(timer);
      window.removeEventListener("focus", refresh);
    };
  }, [workspacePath, refresh]);

  return { branch, refresh };
}

const THINKING_LEVEL_LABELS: Record<string, string> = {
  off: "Off",
  minimal: "Minimal",
  low: "Low",
  medium: "Medium",
  high: "High",
  xhigh: "Extra high",
  max: "Max",
};

/** Shared look for the composer's selector chips: muted text, hover wash, chevron. */
const chipTriggerClass =
  "flex shrink-0 cursor-pointer items-center gap-1.5 rounded-md px-1.5 py-0.5 text-xs text-muted-fg transition-colors duration-100 hover:bg-muted hover:text-fg data-[pressed]:bg-muted disabled:opacity-50";

const OPEN_IN_LAST_KEY = "orbit:open-in:last";

/** Provider brand icons (tech-stack-icons); unknown providers fall back to
 *  the pi glyph. Keys are pi provider ids, matched case-insensitively.
 *  Providers the library doesn't ship map to custom mark components. */
type ProviderIconDef = IconName | React.ComponentType<{ className?: string }>;
const PROVIDER_ICONS: Record<string, ProviderIconDef> = {
  ollama: "ollama",
  anthropic: "anthropic",
  claude: "claude",
  openai: "openai",
  "openai-nosession": "openai",
  google: "google",
  "google-vertex": "google",
  gemini: "gemini",
  groq: "groq",
  mistral: "mistral",
  deepseek: "deepseek",
  xai: "grok",
  meta: "meta",
  qwen: "qwen",
  kimi: "kimi",
  moonshot: "kimi",
  together: "together",
  // openrouter: no brand icon in tech-stack-icons (their asset is a PNG) —
  // falls back to the pi glyph rather than borrowing another brand's mark.
  azure: "azure",
  bedrock: "bedrock",
  "github-copilot": "copilotgithub",
  copilot: "copilotgithub",
  "vercel-ai-gateway": "vercel",
  vercel: "vercel",
  opencode: "opencode",
  "opencode-go": "opencode",
  "opencode-zen": "opencode",
  cursor: "cursor",
  cline: "cline",
  fireworks: "fireworks",
  cerebras: "cerebras",
  huggingface: "huggingface",
  "hugging-face": "huggingface",
  novita: "novita",
  deepinfra: "deepinfra",
  hyperbolic: "hyperbolic",
  zhipu: "zhipu",
  zai: "zhipu",
  synthetic: SyntheticIcon,
};

/** Provider brand icon, or null when the provider has no known mark. */
export function ProviderIcon({
  provider,
  className,
}: {
  provider?: string;
  className?: string;
}) {
  const icon = provider ? PROVIDER_ICONS[provider.toLowerCase()] : undefined;
  if (!icon) return null;
  if (typeof icon === "string") {
    return <StackIcon name={icon} variant="dark" className={className} />;
  }
  const Custom = icon;
  return <Custom className={className} />;
}

const OPEN_IN_KIND_ICONS: Record<
  OpenInApp["kind"],
  React.ComponentType<{ className?: string; strokeWidth?: number }>
> = {
  editor: CodeBracketIcon,
  terminal: CommandLineIcon,
  files: FolderIcon,
};

/** Brand icons from tech-stack-icons where the catalog has them; apps not
 *  covered (Terminal, iTerm2, Warp, Ghostty, Kitty, Finder) fall back to the
 *  kind glyph. */
const OPEN_IN_APP_ICONS: Record<string, IconName> = {
  vscode: "vscode",
  cursor: "cursor",
  zed: "zed",
  xcode: "xcode",
  "android-studio": "android",
  webstorm: "webstorm",
  sublime: "sublime",
  alacritty: "alacritty",
};

/** Brand icon where tech-stack-icons covers the app; kind glyph otherwise. */
function AppIcon({ app }: { app: OpenInApp }) {
  const brandIcon = OPEN_IN_APP_ICONS[app.id];
  if (brandIcon) {
    return (
      <StackIcon name={brandIcon} variant="dark" className="size-4 shrink-0" />
    );
  }
  const KindIcon = OPEN_IN_KIND_ICONS[app.kind];
  return <KindIcon className="size-4 shrink-0" strokeWidth={1.6} />;
}

/**
 * “Open in” picker for the header's top-right cluster: lists the IDEs and
 * terminals installed on this machine (probed by the agent's SSE server) and
 * opens the active workspace folder in the chosen app. The last-used app is
 * remembered locally and checked; apps that aren't installed never appear.
 */
function OpenInMenu({ workspacePath }: { workspacePath: string }) {
  const [apps, setApps] = useState<OpenInApp[] | null>(null);
  const [lastUsed, setLastUsed] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  useEffect(() => {
    setLastUsed(localStorage.getItem(OPEN_IN_LAST_KEY));
    void fetchWorkspaceApps().then((next) => {
      // Drop a stale last-used pointer if that app is gone.
      if (next && lastUsed && !next.some((a) => a.id === lastUsed)) {
        localStorage.removeItem(OPEN_IN_LAST_KEY);
        setLastUsed(null);
      }
      setApps(next);
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [workspacePath]);

  /** Trigger + check-mark target: last used, else VS Code, else first app. */
  const activeAppId =
    lastUsed ??
    (apps?.some((a) => a.id === "vscode") ? "vscode" : (apps?.[0]?.id ?? null));
  const activeApp = apps?.find((a) => a.id === activeAppId);

  const openIn = async (app: OpenInApp) => {
    if (busyId) return;
    setBusyId(app.id);
    setActionError(null);
    try {
      await openWorkspaceInApp(workspacePath, app.id);
      localStorage.setItem(OPEN_IN_LAST_KEY, app.id);
      setLastUsed(app.id);
    } catch (error) {
      setActionError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusyId(null);
    }
  };

  return (
    <div className="flex shrink-0 items-center rounded-md text-xs text-muted-fg">
      {/* Left segment — one click opens the folder in the current app. */}
      <button
        type="button"
        disabled={!activeApp || busyId !== null}
        aria-label={
          activeApp ? `Open project in ${activeApp.name}` : "Open project"
        }
        title={
          activeApp
            ? `Open this project in ${activeApp.name}`
            : "Open this project in an app"
        }
        onClick={() => activeApp && void openIn(activeApp)}
        className="flex h-7 cursor-pointer items-center gap-1.5 overflow-hidden rounded-l-md py-0.5 pr-1 pl-1.5 outline-none transition-colors duration-100 hover:bg-muted hover:text-fg focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-default disabled:opacity-50"
      >
        {activeApp ? (
          <AppIcon app={activeApp} />
        ) : (
          <Squares2X2Icon className="size-3.5 shrink-0" strokeWidth={1.6} />
        )}
        <span className="truncate">{activeApp?.name ?? "Open in"}</span>
      </button>
      {/* Right segment — opens the picker for a different app. */}
      <Menu>
        <MenuTrigger
          aria-label="Choose another app to open the project in"
          className="flex h-7 cursor-pointer items-center rounded-r-md py-0.5 pr-1.5 pl-0.5 outline-none transition-colors duration-100 hover:bg-muted hover:text-fg data-[pressed]:bg-muted data-[pressed]:text-fg focus-visible:ring-2 focus-visible:ring-ring"
        >
          <ChevronDownIcon className="size-3 shrink-0" strokeWidth={1.8} />
        </MenuTrigger>
        <MenuContent placement="bottom end" className="w-56">
          {apps === null ? (
            <MenuItem isDisabled>
              <MenuLabel className="text-[11px] text-muted-fg">
                Detecting installed apps…
              </MenuLabel>
            </MenuItem>
          ) : apps.length === 0 ? (
            <MenuItem isDisabled>
              <MenuLabel className="text-[11px] text-muted-fg">
                No supported apps found
              </MenuLabel>
            </MenuItem>
          ) : (
            apps.map((app) => {
              return (
                <MenuItem
                  key={app.id}
                  id={app.id}
                  textValue={app.name}
                  onAction={() => void openIn(app)}
                >
                  <AppIcon app={app} />
                  <MenuLabel className="min-w-0 truncate">{app.name}</MenuLabel>
                  {busyId === app.id ? (
                    <ArrowPathIcon className="size-3.5 shrink-0 animate-spin" />
                  ) : (
                    activeAppId === app.id && (
                      <CheckIcon
                        className="size-3.5 shrink-0"
                        strokeWidth={2}
                      />
                    )
                  )}
                </MenuItem>
              );
            })
          )}
          {actionError && (
            <p className="col-span-full px-2.5 pb-1 text-[11px] text-danger-subtle-fg">
              {actionError}
            </p>
          )}
        </MenuContent>
      </Menu>
    </div>
  );
}

/** Signal bars used for thinking-effort levels; `lit` of 4 bars filled. */
function EffortGlyph({
  lit,
  className,
}: {
  lit: 0 | 1 | 2 | 3 | 4;
  className?: string;
}) {
  const heights = [4, 6.5, 9, 11.5];
  return (
    <svg viewBox="0 0 14 14" aria-hidden="true" className={className}>
      {heights.map((h, i) => (
        <rect
          key={i}
          x={1.4 + i * 3.3}
          y={12.6 - h}
          width={2.1}
          height={h}
          rx={0.8}
          className={i < lit ? "fill-current" : "fill-current opacity-25"}
        />
      ))}
    </svg>
  );
}

/** Dot-in-circle used for the "off" effort level. */
function EffortOffGlyph({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 14 14"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.4}
      aria-hidden="true"
      className={className}
    >
      <circle cx="7" cy="7" r="4.6" />
      <path d="M3.9 3.9l6.2 6.2" />
    </svg>
  );
}

const EFFORT_ICONS: Record<string, (className: string) => React.ReactNode> = {
  off: (c) => <EffortOffGlyph className={c} />,
  minimal: (c) => <EffortGlyph lit={1} className={c} />,
  low: (c) => <EffortGlyph lit={2} className={c} />,
  medium: (c) => <EffortGlyph lit={3} className={c} />,
  high: (c) => <EffortGlyph lit={4} className={c} />,
  xhigh: (c) => <EffortGlyph lit={4} className={c} />,
  max: (c) => <EffortGlyph lit={4} className={c} />,
};

function Composer({
  modelLabel,
  models,
  currentModel,
  thinkingLevel,
  thinkingLevels,
  connected,
  isStreaming,
  workspaceName,
  canPickFolder,
  workspacePath,
  onOpenFolder,
  onSend,
  onAbort,
  onSelectModel,
  onSelectThinkingLevel,
  contextUsage,
  compacting,
  quoteRequest,
}: {
  modelLabel: string;
  models: {
    provider: string;
    modelId: string;
    name?: string;
    supportsThinking?: boolean;
    availableThinkingLevels?: readonly string[];
  }[];
  /** The active model as "provider/modelId" — the key model menu items use. */
  currentModel?: string;
  thinkingLevel: string;
  thinkingLevels: string[];
  connected: boolean;
  isStreaming: boolean;
  /** Display name of the active workspace folder, if a folder was picked. */
  workspaceName?: string;
  /** Native picker availability — false outside the Tauri webview. */
  canPickFolder: boolean;
  /** Absolute path of the active workspace, when the session has one. */
  workspacePath?: string;
  /** Opens the native picker and starts a new session in the chosen folder. */
  onOpenFolder: () => void;
  /** Live context-window usage for the active session, when reported. */
  contextUsage?: PiContextUsage;
  /** True while pi is compacting the session's context. */
  compacting: boolean;
  onSend: (text: string) => void;
  onAbort: () => void;
  onSelectModel: (provider: string, modelId: string) => void;
  onSelectThinkingLevel: (level: string) => void;
  /** Incoming “quote” from a message's options menu — appended to the draft. */
  quoteRequest?: { text: string; nonce: number } | null;
}) {
  const [draft, setDraft] = useState("");
  const taRef = useRef<HTMLTextAreaElement>(null);
  const canSend = draft.trim().length > 0;
  // Git state of the active workspace — drives the branch chip + picker.
  const git = useWorkspaceGit(workspacePath);

  useEffect(() => {
    const el = taRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  }, [draft]);

  // Quote delivery: append the blockquote to whatever is drafted, then focus
  // the textarea so the user can start typing their follow-up immediately.
  useEffect(() => {
    if (!quoteRequest) return;
    setDraft((prev) =>
      prev.trim()
        ? `${prev.trimEnd()}\n${quoteRequest.text}\n`
        : `${quoteRequest.text}\n`,
    );
    taRef.current?.focus();
  }, [quoteRequest]);

  const submit = () => {
    if (isStreaming) {
      onAbort();
      return;
    }
    const text = draft.trim();
    if (!text) return;
    setDraft("");
    if (taRef.current) taRef.current.style.height = "auto";
    onSend(text);
  };

  const { contains } = useFilter({ sensitivity: "base" });

  // Thinking levels follow the active model: when the model declares its own
  // capability list (`availableThinkingLevels`), only those are offered —
  // pi rejects anything else. Without a declared list (catalog entry missing
  // or not capability-aware) fall back to the standard ladder.
  const activeModel = models.find(
    (m) =>
      currentModel !== undefined &&
      `${m.provider}/${m.modelId}` === currentModel,
  );
  const modelThinkingLevels = activeModel?.availableThinkingLevels;
  const visibleThinkingLevels =
    modelThinkingLevels && modelThinkingLevels.length > 0
      ? [...modelThinkingLevels].sort(
          (a, b) => thinkingLevels.indexOf(a) - thinkingLevels.indexOf(b),
        )
      : thinkingLevels;

  return (
    <div className="shrink-0 px-4 pb-4">
      <form
        className="mx-auto w-full max-w-[720px]"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <div className="rounded-[10px] border border-border bg-card px-4 pt-3.5 pb-2.5 shadow-[0_8px_24px_rgba(0,0,0,0.35)]">
          <textarea
            ref={taRef}
            rows={1}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                submit();
              }
            }}
            placeholder="Do anything…"
            aria-label="Prompt"
            spellCheck={false}
            className="block w-full resize-none bg-transparent text-[13px] leading-[18px] text-fg outline-none placeholder:text-muted-fg"
          />

          <div className="mt-3 flex items-center gap-x-2">
            <Menu>
              <MenuTrigger
                aria-label="Model"
                isDisabled={!connected}
                className={chipTriggerClass}
              >
                <ProviderIcon
                  provider={currentModel?.split("/")[0]}
                  className="size-3.5 shrink-0"
                />
                {!currentModel && <PiTile />}
                <span className="truncate">{modelLabel}</span>
                <ChevronDownIcon
                  className="size-3 shrink-0 text-muted-fg"
                  strokeWidth={1.8}
                />
              </MenuTrigger>
              <PopoverContent
                placement="top start"
                className="w-72 *:data-[slot=popover-inner]:overflow-hidden"
              >
                <Autocomplete filter={contains}>
                  <SearchField
                    aria-label="Search models"
                    autoFocus
                    className="border-b sm:**:[svg]:top-2.5"
                  >
                    <SearchInput
                      placeholder="Search models"
                      className="rounded-none border-0 focus:border-input focus:ring-0 hover:focus:border-transparent sm:py-1.5"
                    />
                  </SearchField>
                  <MenuPrimitive
                    aria-label="Models"
                    className={menuContentStyles()}
                    renderEmptyState={() => (
                      <div className="col-span-full grid min-h-20 place-content-center text-[11px] text-muted-fg">
                        <span>
                          {models.length === 0
                            ? "No models — is the SSE server running?"
                            : "No models match your search"}
                        </span>
                      </div>
                    )}
                  >
                    <MenuSection label="Available models">
                      {models.map((m) => {
                        const id = `${m.provider}/${m.modelId}`;
                        const selected = id === currentModel;
                        return (
                          <MenuItem
                            key={id}
                            id={id}
                            textValue={`${m.name ?? m.modelId} ${m.provider}`}
                            onAction={() =>
                              onSelectModel(m.provider, m.modelId)
                            }
                          >
                            <ProviderIcon
                              provider={m.provider}
                              className="size-4 shrink-0"
                            />
                            <MenuLabel className="min-w-0 truncate">
                              {m.name ?? m.modelId}
                            </MenuLabel>
                            {selected && (
                              <CheckIcon
                                className="size-3.5 shrink-0"
                                strokeWidth={2}
                              />
                            )}
                            <MenuDescription>{m.provider}</MenuDescription>
                          </MenuItem>
                        );
                      })}
                    </MenuSection>
                  </MenuPrimitive>
                </Autocomplete>
              </PopoverContent>
            </Menu>

            <Menu>
              <MenuTrigger
                aria-label="Thinking effort"
                isDisabled={!connected}
                className={chipTriggerClass}
              >
                {THINKING_LEVEL_LABELS[thinkingLevel] ?? thinkingLevel}
                <ChevronDownIcon
                  className="size-3 shrink-0 text-muted-fg"
                  strokeWidth={1.8}
                />
              </MenuTrigger>
              <MenuContent placement="top start">
                {visibleThinkingLevels.map((level) => (
                  <MenuItem
                    key={level}
                    id={level}
                    textValue={THINKING_LEVEL_LABELS[level] ?? level}
                    onAction={() => onSelectThinkingLevel(level)}
                  >
                    {EFFORT_ICONS[level]?.("size-4") ?? (
                      <EffortGlyph lit={3} className="size-4" />
                    )}
                    <MenuLabel>
                      {THINKING_LEVEL_LABELS[level] ?? level}
                    </MenuLabel>
                    {/* Check sits between label and description so the kit's
                        `[slot=label] + svg` rule pins it to the row edge —
                        after the description it would fall out of the grid. */}
                    {level === thinkingLevel && (
                      <CheckIcon
                        className="size-3.5 shrink-0"
                        strokeWidth={2}
                      />
                    )}
                    <MenuDescription>{level}</MenuDescription>
                  </MenuItem>
                ))}
              </MenuContent>
            </Menu>

            <button
              type="submit"
              aria-label={isStreaming ? "Stop" : "Send"}
              className={twMerge(
                "ml-auto flex size-6.5 shrink-0 cursor-pointer items-center justify-center rounded-full transition-colors duration-150",
                isStreaming
                  ? "bg-secondary text-fg hover:bg-muted"
                  : canSend
                    ? "bg-fg text-bg hover:bg-fg/80"
                    : "cursor-default bg-secondary text-muted-fg/60",
              )}
            >
              {isStreaming ? (
                <StopIcon className="size-2.5 fill-current" />
              ) : (
                <ArrowUpIcon className="size-3.5" strokeWidth={2} />
              )}
            </button>
          </div>
        </div>

        <div className="mt-3 flex items-center gap-5 px-4 text-[11px] text-muted-fg">
          {canPickFolder ? (
            <button
              type="button"
              onClick={onOpenFolder}
              title="Open a folder from your device"
              className="flex cursor-pointer items-center gap-1.5 rounded-md outline-hidden transition-colors duration-100 hover:text-fg focus-visible:ring-2 focus-visible:ring-ring"
            >
              <FolderIcon className="size-3.5" strokeWidth={1.6} />
              {workspaceName ?? "Open folder"}
            </button>
          ) : (
            <span className="flex items-center gap-1.5">
              <FolderIcon className="size-3.5" strokeWidth={1.6} />
              {workspaceName ?? "Local"}
            </span>
          )}
          <span className="flex items-center gap-1.5">
            <ComputerDesktopIcon className="size-3.5" strokeWidth={1.6} />
            Local
          </span>
          {git.branch && workspacePath && (
            <BranchPicker
              workspacePath={workspacePath}
              branch={git.branch}
              onSwitched={git.refresh}
            />
          )}
          {/* Connected/disconnected lives in the header + sidebar; the bottom
              row ends with the context meter. */}
          <span className="ml-auto">
            <ContextMeter
              usage={contextUsage}
              compacting={compacting}
              model={modelLabel}
            />
          </span>
        </div>
      </form>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Empty state                                                         */
/* ------------------------------------------------------------------ */

function EmptyState({
  workspaceName,
  canPickFolder,
  onOpenFolder,
}: {
  /** Display name of the active workspace folder, if a folder was picked. */
  workspaceName?: string;
  /** Native picker availability — false outside the Tauri webview. */
  canPickFolder: boolean;
  onOpenFolder: () => void;
}) {
  const chip = (
    <span className="inline-flex translate-y-[3px] items-center gap-1.5 rounded-lg bg-secondary px-2.5 py-1 text-[20px] text-fg">
      <FolderIcon className="size-4 text-muted-fg" strokeWidth={1.7} />
      {workspaceName ?? "open a folder"}
    </span>
  );
  return (
    <div className="relative flex flex-1 flex-col items-center justify-center overflow-hidden pb-12">
      <Asterisk className="relative size-4 motion-safe:animate-rise" />
      <h2 className="relative mt-6 font-display text-2xl font-semibold tracking-[-0.02em] text-balance text-fg motion-safe:animate-rise motion-safe:[animation-delay:120ms]">
        What should we build in{" "}
        {canPickFolder ? (
          <button
            type="button"
            onClick={onOpenFolder}
            title="Open a folder from your device"
            className="cursor-pointer rounded-lg outline-hidden transition-colors duration-100 hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
          >
            {chip}
          </button>
        ) : (
          chip
        )}
        ?
      </h2>
      {canPickFolder && (
        <p className="relative mt-3 text-xs text-muted-fg motion-safe:animate-rise motion-safe:[animation-delay:200ms]">
          Pick a project folder to start the agent there
        </p>
      )}
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Panel                                                               */
/* ------------------------------------------------------------------ */

export default function ChatPanel() {
  const aui = useAui();
  const {
    messages,
    isStreaming,
    connected,
    model,
    provider,
    cancel,
    setModel,
    setThinkingLevel,
  } = usePiSseChat();
  const models = usePiModels();
  // contextUsage arrives via the daemon's `context_usage` event (tokens used,
  // window size, percent); compaction is pi compacting the context when the
  // window nears its limit. Both feed the composer's bottom-right meter.
  const { contextUsage, compaction, metadata } = usePiRuntimeExtras();
  // The real thinking level lives in the thread metadata — the daemon's
  // `thinking_level_changed` events land there via the runtime's reducer, so
  // the chip follows the session truth (including levels set from the CLI).
  const thinkingLevel = metadata?.config?.thinkingLevel ?? "medium";
  const thinkingLevels = [
    "off",
    "minimal",
    "low",
    "medium",
    "high",
    "xhigh",
    "max",
  ];

  // Header title follows the active session: prefer the thread snapshot's
  // metadata title (server-derived session name / first-message snippet),
  // falling back to the aui thread-list item title (already loaded, so the
  // header is correct the instant a session is opened), then “New task”.
  const session = usePiSession();
  const listItem = useAuiState((s) => s.threadListItem);
  const listItemTitle = listItem?.title;
  const listItemWorkspace = listItem?.custom?.workspacePath as
    string | undefined;
  const headerTitle =
    session?.title?.trim() || listItemTitle?.trim() || "New task";

  // Auto-title: pi names sessions only when explicitly told to
  // (setSessionName — the sidebar's inline rename), so a brand-new chat would
  // stay “New task” in the header and “Untitled task” in the sidebar
  // forever. Once the first user message lands on an unnamed session, adopt
  // pi's own fallback — a single-line snippet of that message — as the real
  // session name. The rename goes through pi (setSessionName), so the header
  // updates live (session_info_changed → metadata refresh) and the sidebar
  // picks the name up on its next metadata fetch. Existing titles — including
  // user renames and CLI `/name` — are never overwritten.
  const remoteThreadId = listItem?.remoteId ?? listItem?.externalId;
  const knownTitle = session?.title?.trim() || listItemTitle?.trim() || "";
  const autoTitledThreads = useRef(new Set<string>());
  useEffect(() => {
    if (
      !remoteThreadId ||
      knownTitle ||
      autoTitledThreads.current.has(remoteThreadId)
    )
      return;
    const firstUserText = messages
      .find((m) => m.role === "user")
      ?.parts.find(
        (p): p is Extract<ChatPart, { type: "text" }> => p.type === "text",
      )?.text;
    const title = firstUserText?.replace(/\s+/g, " ").trim().slice(0, 60);
    if (!title) return;
    autoTitledThreads.current.add(remoteThreadId);
    piClient
      .renameThread(remoteThreadId, title)
      .then(() => {
        // Nudge the sidebar so the row shows the new sessionName right away
        // instead of waiting for its 30s metadata poll.
        window.dispatchEvent(new Event("orbit:threads-updated"));
      })
      .catch((error: unknown) => {
        autoTitledThreads.current.delete(remoteThreadId);
        console.error("[orbit] auto-title failed:", error);
      });
  }, [remoteThreadId, knownTitle, messages]);

  // The working directory of the active session, if it has one. Each pi
  // session is bound to a single folder, so “open folder” always starts a
  // new session rooted at the picked directory.
  const activeWorkspace = session?.workspacePath ?? listItemWorkspace;
  const workspaceName = activeWorkspace
    ? pathBasename(activeWorkspace)
    : undefined;

  /** Native folder picker → new pi session in the chosen directory. */
  const handleOpenFolder = async () => {
    const dir = await pickWorkspaceFolder();
    if (!dir) return;
    try {
      const snapshot = await piClient.createThread({ workspacePath: dir });
      await aui.threads.switchToThread(snapshot.metadata.id);
    } catch (error) {
      console.error("[orbit] failed to open folder as a new session:", error);
    }
  };
  const canPickFolder = isTauri();

  const modelLabel = model ?? provider ?? "pi";

  // Adapter: daemon turns + parts → AI SDK UIMessage[] (ids stable; list only appends).
  const uiMessages = useMemo<UIMessage[]>(
    () =>
      messages.map((m, i) => {
        const key = `${m.role}-${i}`;
        return {
          id: key,
          role: m.role,
          // Pi messages carry epoch-ms timestamps; the message list uses them
          // for the per-message date/time in the hover toolbar.
          createdAt: m.createdAt,
          parts: m.parts.flatMap((p, pi) => toUIPart(p, `${key}-${pi}`)),
        } as UIMessage;
      }),
    [messages],
  );
  const status: ChatStatus = isStreaming ? "streaming" : "ready";
  // Nonce-stamped so quoting the same message twice re-triggers the effect.
  const [quoteRequest, setQuoteRequest] = useState<{
    text: string;
    nonce: number;
  } | null>(null);

  // Git changes panel — opened from a turn's Review action or the header toggle.
  const [diffPanelOpen, setDiffPanelOpen] = useState(false);
  // Files the agent touched in the latest turn — the panel's "Last Turn" scope.
  const lastTurnPaths = useMemo(() => {
    const roles = uiMessages.map((m) => m.role);
    const lastUserIdx = roles.lastIndexOf("user");
    return collectEditedPaths(
      uiMessages.slice(lastUserIdx === -1 ? 0 : lastUserIdx),
    );
  }, [uiMessages]);

  const handleSend = (text: string) => {
    aui.composer.setText(text);
    aui.composer.send();
  };

  /** Insert a message into the composer as a markdown blockquote. */
  const handleQuote = (text: string) => {
    const quoted = text
      .split("\n")
      .map((line) => `> ${line}`)
      .join("\n");
    // The composer's textarea is driven by Composer's local draft state (not
    // the aui composer store), so the quote must be delivered as a prop.
    setQuoteRequest({ text: quoted, nonce: Date.now() });
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col bg-bg">
      <ChatHeader
        title={headerTitle}
        workspacePath={activeWorkspace}
        onTogglePanel={() => setDiffPanelOpen((v) => !v)}
      />

      <div className="flex min-h-0 flex-1">
        <div className="flex min-w-0 flex-1 flex-col">
          {messages.length === 0 ? (
            <EmptyState
              workspaceName={workspaceName}
              canPickFolder={canPickFolder}
              onOpenFolder={() => void handleOpenFolder()}
            />
          ) : (
            <AgentMessageList
              messages={uiMessages}
              status={status}
              onQuote={handleQuote}
              workspacePath={activeWorkspace}
              onReviewChanges={() => setDiffPanelOpen(true)}
            />
          )}

          <Composer
            modelLabel={modelLabel}
            models={models}
            currentModel={
              model && provider ? `${provider}/${model}` : undefined
            }
            thinkingLevel={thinkingLevel}
            thinkingLevels={thinkingLevels}
            connected={connected}
            isStreaming={isStreaming}
            workspaceName={workspaceName}
            quoteRequest={quoteRequest}
            canPickFolder={canPickFolder}
            workspacePath={activeWorkspace}
            onOpenFolder={() => void handleOpenFolder()}
            contextUsage={contextUsage}
            compacting={compaction?.active ?? false}
            onSend={handleSend}
            onAbort={() => void cancel()}
            onSelectModel={(provider, modelId) =>
              void setModel({ provider, modelId })
            }
            onSelectThinkingLevel={(level) =>
              void setThinkingLevel(level as never)
            }
          />
        </div>

        {diffPanelOpen && activeWorkspace && (
          <GitDiffPanel
            workspacePath={activeWorkspace}
            turnPaths={lastTurnPaths}
            onClose={() => setDiffPanelOpen(false)}
          />
        )}
      </div>
    </div>
  );
}
