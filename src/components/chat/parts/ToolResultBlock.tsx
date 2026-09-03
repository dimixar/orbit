"use client";

/**
 * Tool output rendered separately from the tool call. Large output is
 * collapsed by default with a size label; clicking expands it.
 */

import { memo, useMemo, useState } from "react";
import { cn } from "cn";
import type { ToolResultPart } from "@/lib/agent/types";
import { CollapsibleSection, Chevron } from "./CollapsibleSection";
import { formatBytes, formatDuration, formatToolValue, truncate, valueSize } from "./part-utils";

const COLLAPSE_THRESHOLD = 2000; // chars

export const ToolResultBlock = memo(function ToolResultBlock(part: ToolResultPart) {
  const { toolCallId, status, output, durationMs, error } = part;
  const [copied, setCopied] = useState(false);

  const text = useMemo(() => formatToolValue(output), [output]);
  const size = valueSize(output);
  const large = size > COLLAPSE_THRESHOLD;
  const isError = status === "error";

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // ignore
    }
  };

  return (
    <CollapsibleSection
      defaultOpen={!large || isError}
      label={`Tool output for ${toolCallId}`}
      header={
        <span
          className={cn(
            "flex w-full items-center gap-2 rounded-md border px-3 py-1.5 text-xs",
            isError ? "border-danger-subtle/50 bg-danger-subtle/20" : "border-border/70 bg-muted/30",
          )}
        >
          <span
            className={cn(
              "flex size-3.5 shrink-0 items-center justify-center",
              isError ? "text-danger-subtle-fg" : "text-muted-fg",
            )}
          >
            {isError ? (
              <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
                <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
            ) : (
              <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
                <path d="M2.5 8h11M8 2.5v11" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
              </svg>
            )}
          </span>
          <span className="font-medium text-fg/90">
            {isError ? "Tool failed" : "Tool output"}
          </span>
          {large && <span className="text-muted-fg/70">· {formatBytes(size)}</span>}
          {durationMs !== undefined && (
            <span className="tabular-nums text-muted-fg/70">· {formatDuration(durationMs)}</span>
          )}
          <span className="ml-auto flex shrink-0 items-center gap-1">
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                void copy();
              }}
              className="cursor-pointer rounded px-1.5 py-0.5 text-muted-fg transition-colors hover:bg-muted hover:text-fg"
            >
              {copied ? "Copied" : "Copy"}
            </button>
            <Chevron />
          </span>
        </span>
      }
    >
      <div className="mt-1.5 overflow-hidden rounded-md border border-border/70 bg-muted/30">
        {error && (
          <div className="border-b border-danger-subtle/40 bg-danger-subtle/20 px-3 py-1.5 text-[11px] text-danger-subtle-fg">
            {error}
          </div>
        )}
        <pre className="max-h-96 overflow-auto px-3 py-2 font-mono text-[11px] leading-relaxed text-fg/85">
          {truncate(text, 20000)}
        </pre>
      </div>
    </CollapsibleSection>
  );
});
