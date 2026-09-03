"use client";

/**
 * Main tool-call UI.
 *
 * Collapsible row showing tool name, status, duration, input and output.
 * Default behavior:
 *   - running tools are expanded
 *   - completed tools can be collapsed
 *   - errors are automatically visible
 */

import { memo, useMemo } from "react";
import { cn } from "cn";
import type { ToolCallPart } from "@/lib/agent/types";
import { CollapsibleSection, Chevron } from "./CollapsibleSection";
import { formatDuration, formatToolValue, truncate, valueSize } from "./part-utils";

const TOOL_ICONS: Record<string, React.ReactNode> = {
  read_file: (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
      <path d="M3 2.5h6l4 4v7H3z" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round" />
      <path d="M9 2.5v4h4M5.5 8.5h5M5.5 11h3.5" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
    </svg>
  ),
  search: (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
      <circle cx="7" cy="7" r="4.5" stroke="currentColor" strokeWidth="1.3" />
      <path d="m10.5 10.5 3 3" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
    </svg>
  ),
  edit_file: (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
      <path d="M11.5 2.5 13.5 4.5 6 12l-2.5.5L4 10z" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round" />
    </svg>
  ),
  write_file: (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
      <path d="M11.5 2.5 13.5 4.5 6 12l-2.5.5L4 10z" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round" />
    </svg>
  ),
  terminal: (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
      <rect x="1.5" y="2.5" width="13" height="11" rx="1.5" stroke="currentColor" strokeWidth="1.3" />
      <path d="m4.5 6 2.5 2-2.5 2M8.5 10.5h3" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  ),
  bash: (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
      <rect x="1.5" y="2.5" width="13" height="11" rx="1.5" stroke="currentColor" strokeWidth="1.3" />
      <path d="m4.5 6 2.5 2-2.5 2M8.5 10.5h3" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  ),
  web_fetch: (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
      <circle cx="8" cy="8" r="5.5" stroke="currentColor" strokeWidth="1.3" />
      <path d="M2.5 8h11M8 2.5c1.8 1.8 1.8 9.2 0 11M8 2.5C6.2 4.3 6.2 11.7 8 13.5" stroke="currentColor" strokeWidth="1.3" />
    </svg>
  ),
  browser: (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
      <circle cx="8" cy="8" r="5.5" stroke="currentColor" strokeWidth="1.3" />
      <path d="M2.5 8h11M8 2.5c1.8 1.8 1.8 9.2 0 11M8 2.5C6.2 4.3 6.2 11.7 8 13.5" stroke="currentColor" strokeWidth="1.3" />
    </svg>
  ),
  npm: (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
      <rect x="1.5" y="3" width="13" height="10" rx="1.5" stroke="currentColor" strokeWidth="1.3" />
      <path d="M6 6v4M8 6v4M10 6v4" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
    </svg>
  ),
};

function toolIcon(tool: string): React.ReactNode {
  const key = tool.toLowerCase();
  for (const [name, icon] of Object.entries(TOOL_ICONS)) {
    if (key.includes(name)) return icon;
  }
  return (
    <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
      <path d="M9.5 2.5 13.5 6.5 6 14l-4 .5L2.5 10z" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round" />
    </svg>
  );
}

function StatusGlyph({ status }: { status: ToolCallPart["status"] }) {
  if (status === "running") {
    return (
      <span className="size-3 animate-spin rounded-full border-[1.5px] border-primary/25 border-t-primary" />
    );
  }
  if (status === "success") {
    return (
      <span className="flex size-3.5 items-center justify-center text-success-subtle-fg">
        <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
          <path d="M3 8.5 6.5 12 13 4.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </span>
    );
  }
  return (
    <span className="flex size-3.5 items-center justify-center text-danger-subtle-fg">
      <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
        <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
      </svg>
    </span>
  );
}

function InputSummary({ input }: { input: unknown }) {
  const text = useMemo(() => {
    if (typeof input === "string") return input;
    if (input && typeof input === "object") {
      const record = input as Record<string, unknown>;
      const path = record.path ?? record.file ?? record.command ?? record.url;
      if (typeof path === "string") return path;
      const first = Object.values(record)[0];
      if (typeof first === "string") return first;
    }
    return formatToolValue(input);
  }, [input]);

  return <span className="truncate font-mono text-[11px] text-muted-fg/80">{text}</span>;
}

export const ToolCallBlock = memo(function ToolCallBlock({
  tool,
  status,
  input,
  output,
  durationMs,
  showOutput = true,
}: ToolCallPart & { showOutput?: boolean }) {
  const isError = status === "error";
  const isRunning = status === "running";
  const hasOutput = showOutput && output !== undefined && output !== null && output !== "";
  const outputSize = valueSize(output);
  const renderOutput = hasOutput && outputSize > 0;

  return (
    <CollapsibleSection
      defaultOpen={isRunning || isError}
      label={`${tool} tool call`}
      header={
        <span
          className={cn(
            "flex w-full items-center gap-2 rounded-md border px-3 py-2 text-xs",
            isError
              ? "border-danger-subtle/50 bg-danger-subtle/20"
              : "border-border bg-card",
          )}
        >
          <span
            className={cn(
              "flex size-4 shrink-0 items-center justify-center",
              isError ? "text-danger-subtle-fg" : "text-muted-fg",
            )}
          >
            {toolIcon(tool)}
          </span>
          <span className="shrink-0 font-medium text-fg">{tool}</span>
          <InputSummary input={input} />
          <span className="ml-auto flex shrink-0 items-center gap-2">
            {durationMs !== undefined && !isRunning && (
              <span className="tabular-nums text-muted-fg/70">{formatDuration(durationMs)}</span>
            )}
            <StatusGlyph status={status} />
            <Chevron />
          </span>
        </span>
      }
    >
      <div className="mt-1.5 space-y-1.5">
        {input !== undefined && input !== null && (
          <div className="overflow-hidden rounded-md border border-border/70 bg-muted/30">
            <div className="border-b border-border/60 px-3 py-1 text-[10px] font-medium uppercase tracking-wide text-muted-fg/70">
              Input
            </div>
            <pre className="overflow-x-auto px-3 py-2 font-mono text-[11px] leading-relaxed text-fg/85">
              {formatToolValue(input)}
            </pre>
          </div>
        )}
        {renderOutput && (
          <div className="overflow-hidden rounded-md border border-border/70 bg-muted/30">
            <div className="border-b border-border/60 px-3 py-1 text-[10px] font-medium uppercase tracking-wide text-muted-fg/70">
              Output
            </div>
            <pre className="max-h-64 overflow-auto px-3 py-2 font-mono text-[11px] leading-relaxed text-fg/85">
              {truncate(formatToolValue(output), 4000)}
            </pre>
          </div>
        )}
      </div>
    </CollapsibleSection>
  );
});
