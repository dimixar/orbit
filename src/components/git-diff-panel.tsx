"use client";

/**
 * Git diff panel — the app's source-control surface, opened from a turn's
 * "Review" action or the header panel toggle.
 *
 * Layout mirrors the desktop IDE pattern: scope selector + totals + refresh
 * on top; the selected file's unified diff on the left; a filterable folder
 * tree of changed files (with git status badges) on the right. Scope "Last
 * turn" scopes to the files the agent touched in the most recent turn;
 * "All changes" shows the whole working tree.
 */

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ChevronDownIcon,
  ChevronRightIcon,
  DocumentTextIcon,
  FolderIcon,
  FolderOpenIcon,
  MagnifyingGlassIcon,
  ArrowPathIcon,
  XMarkIcon,
} from "@heroicons/react/24/outline";
import {
  fetchWorkspaceFileDiff,
  fetchWorkspaceStatus,
  type WorkspaceFileChange,
} from "@/lib/pi-client";
import { cn } from "@/components/agent-elements/utils/cn";
import {
  Menu,
  MenuContent,
  MenuItem,
  MenuLabel,
  MenuTrigger,
} from "@/components/ui/menu";
import { CheckIcon } from "@heroicons/react/20/solid";

export type GitDiffScope = "turn" | "all";

type DiffLine = {
  kind: "context" | "add" | "del" | "hunk" | "meta";
  oldNum?: number;
  newNum?: number;
  text: string;
};

/** Parse a unified diff into line rows with old/new line numbers. */
function parseDiff(text: string): DiffLine[] {
  const out: DiffLine[] = [];
  let oldNo = 0;
  let newNo = 0;
  for (const line of text.split("\n")) {
    if (line.startsWith("@@")) {
      const m = line.match(/@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
      if (m) {
        oldNo = Number(m[1]);
        newNo = Number(m[2]);
      }
      out.push({ kind: "hunk", text: line });
    } else if (line.startsWith("+")) {
      out.push({ kind: "add", newNum: newNo++, text: line.slice(1) });
    } else if (line.startsWith("-")) {
      out.push({ kind: "del", oldNum: oldNo++, text: line.slice(1) });
    } else if (
      line.startsWith("diff ") ||
      line.startsWith("index ") ||
      line.startsWith("--- ") ||
      line.startsWith("+++ ") ||
      line.startsWith("\\ No newline")
    ) {
      out.push({ kind: "meta", text: line });
    } else {
      out.push({
        kind: "context",
        oldNum: oldNo++,
        newNum: newNo++,
        text: line.slice(1),
      });
    }
  }
  return out;
}

type TreeNode = {
  name: string;
  path: string;
  children?: Map<string, TreeNode>;
  file?: WorkspaceFileChange;
};

/** Build a folder tree from flat changed-file paths. */
function buildTree(files: WorkspaceFileChange[]): Map<string, TreeNode> {
  const root: Map<string, TreeNode> = new Map();
  for (const file of files) {
    const segments = file.path.split("/");
    let level = root;
    let prefix = "";
    for (let i = 0; i < segments.length; i++) {
      const segment = segments[i];
      prefix = prefix ? `${prefix}/${segment}` : segment;
      let node = level.get(segment);
      if (!node) {
        node = { name: segment, path: prefix, children: undefined };
        level.set(segment, node);
      }
      if (i === segments.length - 1) {
        node.file = file;
      } else {
        node.children ??= new Map();
        level = node.children;
      }
    }
  }
  return root;
}

const STATUS_BADGE: Record<
  WorkspaceFileChange["status"],
  { label: string; className: string }
> = {
  modified: {
    label: "M",
    className: "bg-warning-subtle text-warning-subtle-fg",
  },
  added: { label: "A", className: "bg-success-subtle text-success-subtle-fg" },
  deleted: { label: "D", className: "bg-danger-subtle text-danger-subtle-fg" },
};

function TreeNodeRow({
  node,
  depth,
  expanded,
  onToggleFolder,
  selectedPath,
  onSelectFile,
}: {
  node: TreeNode;
  depth: number;
  expanded: Set<string>;
  onToggleFolder: (path: string) => void;
  selectedPath: string | undefined;
  onSelectFile: (path: string) => void;
}) {
  const isFolder = node.children !== undefined && node.file === undefined;
  const isOpen = expanded.has(node.path);
  const badge = node.file ? STATUS_BADGE[node.file.status] : undefined;

  if (isFolder) {
    return (
      <>
        <button
          type="button"
          onClick={() => onToggleFolder(node.path)}
          className="flex w-full cursor-pointer items-center gap-1 rounded-md px-1.5 py-1 text-left text-xs text-muted-fg outline-none transition-colors duration-100 hover:bg-muted hover:text-fg focus-visible:ring-2 focus-visible:ring-ring"
          style={{ paddingLeft: `${8 + depth * 12}px` }}
        >
          {isOpen ? (
            <ChevronDownIcon className="size-3 shrink-0" strokeWidth={2} />
          ) : (
            <ChevronRightIcon className="size-3 shrink-0" strokeWidth={2} />
          )}
          {isOpen ? (
            <FolderOpenIcon className="size-3.5 shrink-0" strokeWidth={1.6} />
          ) : (
            <FolderIcon className="size-3.5 shrink-0" strokeWidth={1.6} />
          )}
          <span className="min-w-0 truncate">{node.name}</span>
        </button>
        {isOpen &&
          [...node.children!.values()]
            .sort((a, b) => {
              const aFolder = a.children !== undefined && a.file === undefined;
              const bFolder = b.children !== undefined && b.file === undefined;
              if (aFolder !== bFolder) return aFolder ? -1 : 1;
              return a.name.localeCompare(b.name);
            })
            .map((child) => (
              <TreeNodeRow
                key={child.path}
                node={child}
                depth={depth + 1}
                expanded={expanded}
                onToggleFolder={onToggleFolder}
                selectedPath={selectedPath}
                onSelectFile={onSelectFile}
              />
            ))}
      </>
    );
  }

  const brandName = node.file ? STATUS_BADGE[node.file.status] : undefined;
  return (
    <button
      type="button"
      onClick={() => node.file && onSelectFile(node.path)}
      className={cn(
        "flex w-full items-center gap-1.5 rounded-md px-1.5 py-1 text-left text-xs outline-none transition-colors duration-100 focus-visible:ring-2 focus-visible:ring-ring",
        selectedPath === node.path
          ? "bg-muted text-fg"
          : "text-muted-fg hover:bg-muted hover:text-fg",
      )}
      style={{ paddingLeft: `${8 + depth * 12}px` }}
    >
      <DocumentTextIcon className="size-3.5 shrink-0" strokeWidth={1.6} />
      <span className="min-w-0 flex-1 truncate">{node.name}</span>
      {node.file && (
        <span className="flex shrink-0 items-center gap-1.5 text-[10px] tabular-nums text-muted-fg/70">
          {node.file.additions > 0 && (
            <span className="text-success-subtle-fg">
              +{node.file.additions}
            </span>
          )}
          {node.file.deletions > 0 && (
            <span className="text-danger-subtle-fg">
              -{node.file.deletions}
            </span>
          )}
        </span>
      )}
      {brandName && (
        <span
          className={cn(
            "flex size-4 shrink-0 items-center justify-center rounded-sm text-[9px] font-semibold",
            badge?.className ?? brandName.className,
          )}
        >
          {badge?.label ?? brandName.label}
        </span>
      )}
    </button>
  );
}

function DiffViewer({ diff }: { diff: string }) {
  const lines = useMemo(() => parseDiff(diff), [diff]);
  if (lines.length === 0) {
    return (
      <div className="grid h-full place-content-center text-xs text-muted-fg">
        No changes in this file
      </div>
    );
  }
  return (
    <div className="min-w-0 font-mono text-[11px] leading-[18px]">
      {lines.map((line, i) => {
        const isAdd = line.kind === "add";
        const isDel = line.kind === "del";
        const num =
          line.kind === "del" ? line.oldNum : (line.newNum ?? line.oldNum);
        return (
          <div
            key={i}
            className={cn(
              "flex items-start whitespace-pre",
              // Full-strength brand colors at a visible alpha — the subtle
              // tokens are already ~10% alpha and vanish on dark surfaces.
              isAdd && "bg-success/20",
              isDel && "bg-danger/25",
              line.kind === "hunk" && "bg-muted/50 text-muted-fg",
            )}
          >
            <span
              className={cn(
                "w-11 shrink-0 select-none pr-2 text-right tabular-nums",
                isAdd && "text-success/70",
                isDel && "text-danger/70",
                !isAdd && !isDel && "text-muted-fg/50",
              )}
            >
              {num ?? ""}
            </span>
            <span
              className={cn(
                "w-4 shrink-0 select-none text-center font-semibold",
                isAdd && "text-success",
                isDel && "text-danger",
              )}
            >
              {isAdd ? "+" : isDel ? "-" : ""}
            </span>
            <span
              className={cn(
                "min-w-0 flex-1 pr-4",
                isAdd && "text-fg",
                isDel && "text-fg/70",
                line.kind === "context" && "text-fg/85",
              )}
            >
              {line.text || " "}
            </span>
          </div>
        );
      })}
    </div>
  );
}

export function GitDiffPanel({
  workspacePath,
  turnPaths,
  onClose,
}: {
  workspacePath: string;
  /** Files the agent touched in the last turn — powers the "Last turn" scope. */
  turnPaths: string[];
  onClose: () => void;
}) {
  const [scope, setScope] = useState<GitDiffScope>(
    turnPaths.length > 0 ? "turn" : "all",
  );
  const [files, setFiles] = useState<WorkspaceFileChange[] | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | undefined>(
    undefined,
  );
  const [diff, setDiff] = useState<string | null>(null);
  const [diffError, setDiffError] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [refreshing, setRefreshing] = useState(false);
  const [nonce, setNonce] = useState(0);

  const refresh = useCallback(() => setNonce((n) => n + 1), []);

  // Working-tree status, refetched on scope change and manual refresh.
  useEffect(() => {
    let cancelled = false;
    setRefreshing(true);
    void fetchWorkspaceStatus(workspacePath)
      .then((next) => {
        if (cancelled) return;
        setFiles(next);
      })
      .finally(() => {
        if (!cancelled) setRefreshing(false);
      });
    return () => {
      cancelled = true;
    };
  }, [workspacePath, nonce]);

  // Files visible in the current scope, then filtered by the search box.
  const scopedFiles = useMemo(() => {
    if (!files) return null;
    const base =
      scope === "turn"
        ? files.filter((f) => turnPaths.includes(f.path))
        : files;
    const query = filter.trim().toLowerCase();
    if (!query) return base;
    return base.filter((f) => f.path.toLowerCase().includes(query));
  }, [files, scope, turnPaths, filter]);

  const tree = useMemo(() => buildTree(scopedFiles ?? []), [scopedFiles]);

  // Auto-select the first file in scope; auto-expand its folders.
  useEffect(() => {
    if (scopedFiles === null) return;
    if (scopedFiles.length === 0) {
      setSelectedPath(undefined);
      return;
    }
    if (!selectedPath || !scopedFiles.some((f) => f.path === selectedPath)) {
      setSelectedPath(scopedFiles[0]!.path);
    }
  }, [scopedFiles, selectedPath]);

  // Auto-expand folders along the selected file's path.
  useEffect(() => {
    if (!selectedPath) return;
    setExpanded((prev) => {
      const next = new Set(prev);
      const segments = selectedPath.split("/");
      segments.pop();
      let prefix = "";
      for (const segment of segments) {
        prefix = prefix ? `${prefix}/${segment}` : segment;
        next.add(prefix);
      }
      return next;
    });
  }, [selectedPath]);

  // Fetch the selected file's diff.
  useEffect(() => {
    if (!selectedPath) {
      setDiff(null);
      return;
    }
    let cancelled = false;
    setDiff(null);
    setDiffError(null);
    void fetchWorkspaceFileDiff(workspacePath, selectedPath)
      .then((text) => {
        if (!cancelled) setDiff(text);
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setDiffError(error instanceof Error ? error.message : String(error));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [workspacePath, selectedPath, nonce]);

  const selectedFile = scopedFiles?.find((f) => f.path === selectedPath);
  const totalAdditions = (scopedFiles ?? []).reduce(
    (sum, f) => sum + f.additions,
    0,
  );
  const totalDeletions = (scopedFiles ?? []).reduce(
    (sum, f) => sum + f.deletions,
    0,
  );

  const toggleFolder = useCallback((path: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);

  return (
    <aside className="flex w-[min(60vw,760px)] min-w-[380px] shrink-0 flex-col border-l border-border bg-bg">
      {/* Panel header: scope, totals, refresh, close */}
      <div className="flex h-12 shrink-0 items-center gap-2 border-b border-border px-3">
        <MenuScopeSelector
          scope={scope}
          onScopeChange={(next) => {
            setScope(next);
            setSelectedPath(undefined);
          }}
          turnAvailable={turnPaths.length > 0}
        />
        <span className="flex items-center gap-2 text-xs font-medium tabular-nums">
          <span className="text-success-subtle-fg">+{totalAdditions}</span>
          <span className="text-danger-subtle-fg">-{totalDeletions}</span>
        </span>
        <div className="ml-auto flex items-center gap-1">
          <button
            type="button"
            onClick={refresh}
            aria-label="Refresh changes"
            title="Refresh changes"
            className="flex size-6 cursor-pointer items-center justify-center rounded-md outline-none transition-colors duration-100 hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
          >
            <ArrowPathIcon
              className={cn("size-3.5", refreshing && "animate-spin")}
              strokeWidth={1.6}
            />
          </button>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close git changes panel"
            title="Close"
            className="flex size-6 cursor-pointer items-center justify-center rounded-md outline-none transition-colors duration-100 hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
          >
            <XMarkIcon className="size-4" strokeWidth={1.6} />
          </button>
        </div>
      </div>

      <div className="flex min-h-0 flex-1">
        {/* Diff viewer */}
        <div className="flex min-w-0 flex-1 flex-col border-r border-border">
          {selectedFile && (
            <div className="flex shrink-0 items-center gap-2 border-b border-border px-3 py-2">
              <DocumentTextIcon
                className="size-3.5 shrink-0 text-muted-fg"
                strokeWidth={1.6}
              />
              <span
                className="min-w-0 truncate text-xs font-medium text-fg"
                title={selectedFile.path}
              >
                {selectedFile.path}
              </span>
              <span className="ml-auto flex shrink-0 items-center gap-2 text-xs tabular-nums">
                <span className="text-success-subtle-fg">
                  +{selectedFile.additions}
                </span>
                <span className="text-danger-subtle-fg">
                  -{selectedFile.deletions}
                </span>
              </span>
            </div>
          )}
          <div className="min-h-0 flex-1 overflow-auto py-2">
            {diffError ? (
              <div className="px-3 py-2 text-xs text-danger-subtle-fg">
                {diffError}
              </div>
            ) : diff === null ? (
              <div className="grid place-content-center py-10 text-xs text-muted-fg">
                {selectedPath ? "Loading diff…" : "Select a file"}
              </div>
            ) : (
              <DiffViewer diff={diff} />
            )}
          </div>
        </div>

        {/* File tree */}
        <div className="flex w-56 shrink-0 flex-col">
          <div className="shrink-0 p-2">
            <div className="relative">
              <MagnifyingGlassIcon
                data-slot="icon"
                className="pointer-events-none absolute start-2.5 top-1/2 z-10 size-3.5 -translate-y-1/2 text-muted-fg"
              />
              <input
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                placeholder="Filter files..."
                aria-label="Filter changed files"
                className="block w-full appearance-none rounded-lg bg-secondary py-1.5 pe-2 ps-8 text-xs text-fg outline-none transition-colors placeholder:text-muted-fg focus:ring-2 focus:ring-ring"
              />
            </div>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto px-1 pb-2">
            {scopedFiles === null ? (
              <p className="px-2 py-1.5 text-[11px] text-muted-fg">
                Loading changes…
              </p>
            ) : scopedFiles.length === 0 ? (
              <p className="px-2 py-1.5 text-[11px] text-muted-fg">
                {files && files.length > 0
                  ? "No files match your filter"
                  : "No uncommitted changes"}
              </p>
            ) : (
              [...tree.values()]
                .sort((a, b) => {
                  const aFolder =
                    a.children !== undefined && a.file === undefined;
                  const bFolder =
                    b.children !== undefined && b.file === undefined;
                  if (aFolder !== bFolder) return aFolder ? -1 : 1;
                  return a.name.localeCompare(b.name);
                })
                .map((node) => (
                  <TreeNodeRow
                    key={node.path}
                    node={node}
                    depth={0}
                    expanded={expanded}
                    onToggleFolder={toggleFolder}
                    selectedPath={selectedPath}
                    onSelectFile={setSelectedPath}
                  />
                ))
            )}
          </div>
        </div>
      </div>
    </aside>
  );
}

/** "Last turn / All changes" scope selector — the app's Menu kit. */
function MenuScopeSelector({
  scope,
  onScopeChange,
  turnAvailable,
}: {
  scope: GitDiffScope;
  onScopeChange: (scope: GitDiffScope) => void;
  turnAvailable: boolean;
}) {
  const label = scope === "turn" ? "Last Turn" : "All Changes";
  return (
    <Menu>
      <MenuTrigger
        aria-label="Change diff scope"
        className="flex cursor-pointer items-center gap-1 rounded-md px-1.5 py-1 text-xs font-medium text-fg outline-none transition-colors duration-100 hover:bg-muted data-[pressed]:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
      >
        {label}
        <ChevronDownIcon className="size-3 shrink-0" strokeWidth={1.8} />
      </MenuTrigger>
      <MenuContent placement="bottom start" className="w-40">
        <MenuItem
          id="turn"
          textValue="Last Turn"
          isDisabled={!turnAvailable}
          onAction={() => onScopeChange("turn")}
        >
          <MenuLabel>Last Turn</MenuLabel>
          {scope === "turn" && (
            <CheckIcon className="size-3.5 shrink-0" strokeWidth={2} />
          )}
        </MenuItem>
        <MenuItem
          id="all"
          textValue="All Changes"
          onAction={() => onScopeChange("all")}
        >
          <MenuLabel>All Changes</MenuLabel>
          {scope === "all" && (
            <CheckIcon className="size-3.5 shrink-0" strokeWidth={2} />
          )}
        </MenuItem>
      </MenuContent>
    </Menu>
  );
}
