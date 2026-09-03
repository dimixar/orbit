"use client";

/**
 * Filesystem operations: created / modified / deleted / read.
 * Clicking a file optionally triggers `onFileClick(path)`.
 */

import { memo } from "react";
import { cn } from "cn";
import type { FileAction, FilePart } from "@/lib/agent/types";

const ACTION_META: Record<
  FileAction,
  { label: string; className: string; icon: React.ReactNode }
> = {
  created: {
    label: "Created",
    className: "bg-success-subtle text-success-subtle-fg",
    icon: (
      <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
        <path d="M8 3.5v9M3.5 8h9" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
      </svg>
    ),
  },
  modified: {
    label: "Modified",
    className: "bg-primary-subtle text-primary-subtle-fg",
    icon: (
      <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
        <path d="M11.5 2.5 13.5 4.5 6 12l-2.5.5L4 10z" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round" />
      </svg>
    ),
  },
  deleted: {
    label: "Deleted",
    className: "bg-danger-subtle text-danger-subtle-fg",
    icon: (
      <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
        <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
      </svg>
    ),
  },
  read: {
    label: "Read",
    className: "bg-secondary text-secondary-fg",
    icon: (
      <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
        <path d="M3 2.5h6l4 4v7H3z" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round" />
        <path d="M9 2.5v4h4" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round" />
      </svg>
    ),
  },
};

export type FileChangeBlockProps = FilePart & {
  onFileClick?: (path: string) => void;
};

export const FileChangeBlock = memo(function FileChangeBlock({
  path,
  action,
  onFileClick,
}: FileChangeBlockProps) {
  const meta = ACTION_META[action];
  const clickable = Boolean(onFileClick);

  return (
    <button
      type="button"
      disabled={!clickable}
      onClick={() => onFileClick?.(path)}
      className={cn(
        "flex w-full items-center gap-2 rounded-md border border-border/70 bg-card px-3 py-1.5 text-left text-xs",
        clickable && "cursor-pointer transition-colors hover:bg-muted/50",
        !clickable && "cursor-default",
      )}
    >
      <span className={cn("flex shrink-0 items-center gap-1 rounded-sm px-1.5 py-0.5 text-[10px] font-medium", meta.className)}>
        {meta.icon}
        {meta.label}
      </span>
      <span className="min-w-0 truncate font-mono text-fg/85">{path}</span>
      {clickable && (
        <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="ml-auto size-3 shrink-0 text-muted-fg/60">
          <path d="M6 3.5 10.5 8 6 12.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      )}
    </button>
  );
});
