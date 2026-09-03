"use client";

/**
 * Coding-agent diff component.
 *
 * Parses a unified diff patch into typed lines and renders them with
 * additions/deletions coloring. The parser is isolated here so a dedicated
 * diff library (e.g. `@pierre/diffs`) can be swapped in later without
 * touching the rest of the chat system.
 */

import { memo, useMemo, useState } from "react";
import { cn } from "cn";
import type { DiffPart } from "@/lib/agent/types";
import { CollapsibleSection, Chevron } from "./CollapsibleSection";

export type DiffLineType = "add" | "del" | "context" | "hunk" | "meta";

export type DiffLine = {
  type: DiffLineType;
  content: string;
};

/** Parses a unified diff patch into typed lines. Pure and testable. */
export function parseDiff(patch: string): DiffLine[] {
  const lines = patch.split("\n");
  const result: DiffLine[] = [];
  for (const raw of lines) {
    const content = raw;
    if (content.startsWith("+++") || content.startsWith("---")) {
      result.push({ type: "meta", content });
    } else if (content.startsWith("@@")) {
      result.push({ type: "hunk", content });
    } else if (content.startsWith("+")) {
      result.push({ type: "add", content });
    } else if (content.startsWith("-")) {
      result.push({ type: "del", content });
    } else {
      result.push({ type: "context", content });
    }
  }
  return result;
}

const LINE_STYLES: Record<DiffLineType, string> = {
  add: "bg-success-subtle/40 text-success-subtle-fg",
  del: "bg-danger-subtle/40 text-danger-subtle-fg",
  context: "text-fg/80",
  hunk: "bg-primary-subtle/30 text-primary-subtle-fg",
  meta: "text-muted-fg",
};

const LINE_PREFIX: Record<DiffLineType, string> = {
  add: "+",
  del: "-",
  context: " ",
  hunk: "@",
  meta: "",
};

export const DiffViewer = memo(function DiffViewer(part: DiffPart) {
  const { file, patch, additions, deletions } = part;
  const [copied, setCopied] = useState(false);

  const lines = useMemo(() => parseDiff(patch), [patch]);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(patch);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // ignore
    }
  };

  return (
    <CollapsibleSection
      defaultOpen
      label={`Diff for ${file}`}
      header={
        <span className="flex w-full items-center gap-2 rounded-md border border-border bg-card px-3 py-2 text-xs">
          <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5 shrink-0 text-muted-fg">
            <path d="M2 4h12M2 8h12M2 12h12" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
            <path d="M6.5 2v12M9.5 2v12" stroke="currentColor" strokeWidth="1" strokeLinecap="round" opacity="0.5" />
          </svg>
          <span className="min-w-0 truncate font-mono text-fg/90">{file}</span>
          <span className="ml-auto flex shrink-0 items-center gap-2">
            {(additions ?? 0) > 0 && (
              <span className="tabular-nums text-success-subtle-fg">+{additions}</span>
            )}
            {(deletions ?? 0) > 0 && (
              <span className="tabular-nums text-danger-subtle-fg">-{deletions}</span>
            )}
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                void copy();
              }}
              className="cursor-pointer rounded px-1.5 py-0.5 text-muted-fg transition-colors hover:bg-muted hover:text-fg"
            >
              {copied ? "Copied" : "Copy patch"}
            </button>
            <Chevron />
          </span>
        </span>
      }
    >
      <div className="mt-1.5 overflow-hidden rounded-md border border-border/70">
        <div className="overflow-x-auto">
          <pre className="min-w-full font-mono text-[11px] leading-[1.55]">
            {lines.map((line, i) => (
              <div
                key={i}
                className={cn("flex whitespace-pre", LINE_STYLES[line.type])}
              >
                <span className="w-6 shrink-0 select-none text-right pr-2 opacity-40">
                  {LINE_PREFIX[line.type]}
                </span>
                <span className="min-w-0 flex-1 px-1">{line.content}</span>
              </div>
            ))}
          </pre>
        </div>
      </div>
    </CollapsibleSection>
  );
});
