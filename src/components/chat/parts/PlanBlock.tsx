"use client";

/**
 * The agent's execution plan. Steps update in place as events arrive.
 */

import { memo } from "react";
import { cn } from "cn";
import type { PlanPart, PlanStepStatus } from "@/lib/agent/types";
import { CollapsibleSection, Chevron } from "./CollapsibleSection";

const STEP_ICONS: Record<PlanStepStatus, React.ReactNode> = {
  pending: (
    <span className="flex size-4 shrink-0 items-center justify-center">
      <span className="size-2 rounded-full border-[1.5px] border-muted-fg/50" />
    </span>
  ),
  running: (
    <span className="flex size-4 shrink-0 items-center justify-center">
      <span className="size-2 animate-pulse rounded-full bg-primary" />
    </span>
  ),
  complete: (
    <span className="flex size-4 shrink-0 items-center justify-center text-success-subtle-fg">
      <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
        <path
          d="M3 8.5 6.5 12 13 4.5"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    </span>
  ),
  error: (
    <span className="flex size-4 shrink-0 items-center justify-center text-danger-subtle-fg">
      <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
        <path
          d="M4 4l8 8M12 4l-8 8"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
        />
      </svg>
    </span>
  ),
};

function stepLabel(status: PlanStepStatus): string {
  switch (status) {
    case "pending":
      return "Pending";
    case "running":
      return "Running";
    case "complete":
      return "Complete";
    case "error":
      return "Failed";
  }
}

export const PlanBlock = memo(function PlanBlock({ steps }: PlanPart) {
  const completed = steps.filter((s) => s.status === "complete").length;
  const failed = steps.filter((s) => s.status === "error").length;
  const running = steps.filter((s) => s.status === "running").length;
  const progress = steps.length === 0 ? 0 : Math.round((completed / steps.length) * 100);
  const allDone = steps.length > 0 && completed + failed === steps.length;

  return (
    <CollapsibleSection
      defaultOpen={!allDone}
      label="Plan"
      header={
        <span className="flex w-full items-center gap-2 rounded-md border border-border bg-card px-3 py-2 text-xs">
          <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5 text-muted-fg">
            <rect x="2" y="3" width="12" height="10.5" rx="1.5" stroke="currentColor" strokeWidth="1.3" />
            <path d="M2 6.5h12M5.5 3v10.5" stroke="currentColor" strokeWidth="1.3" />
          </svg>
          <span className="font-medium text-fg">Plan</span>
          <span className="ml-auto flex items-center gap-2 text-muted-fg">
            {running > 0 && <span className="text-primary-subtle-fg">{running} running</span>}
            {failed > 0 && <span className="text-danger-subtle-fg">{failed} failed</span>}
            <span className="tabular-nums">
              {completed}/{steps.length}
            </span>
          </span>
          <Chevron />
        </span>
      }
    >
      <div className="mt-1.5 overflow-hidden rounded-md border border-border/70 bg-card">
        <div className="h-0.5 w-full bg-muted">
          <div
            className={cn("h-full bg-primary transition-all duration-300", failed > 0 && "bg-danger")}
            style={{ width: `${progress}%` }}
          />
        </div>
        <ol className="divide-y divide-border/60">
          {steps.map((step) => (
            <li key={step.id} className="flex items-center gap-2.5 px-3 py-2">
              {STEP_ICONS[step.status]}
              <span
                className={cn(
                  "min-w-0 flex-1 truncate text-xs",
                  step.status === "complete" && "text-muted-fg",
                  step.status === "pending" && "text-muted-fg/70",
                  step.status === "error" && "text-danger-subtle-fg",
                )}
              >
                {step.title}
              </span>
              <span className="shrink-0 text-[10px] uppercase tracking-wide text-muted-fg/60">
                {stepLabel(step.status)}
              </span>
            </li>
          ))}
        </ol>
      </div>
    </CollapsibleSection>
  );
});
