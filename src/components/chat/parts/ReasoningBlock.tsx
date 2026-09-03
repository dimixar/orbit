"use client";

/**
 * Agent status/progress summary — visually secondary, collapsible.
 *
 * Only renders safe status summaries supplied for UI display. Never renders
 * private chain-of-thought.
 */

import { memo } from "react";
import { cn } from "cn";
import type { ReasoningPart } from "@/lib/agent/types";
import { CollapsibleSection, Chevron } from "./CollapsibleSection";

export const ReasoningBlock = memo(function ReasoningBlock({
  content,
  status,
}: ReasoningPart) {
  const streaming = status === "streaming";

  return (
    <CollapsibleSection
      defaultOpen={streaming}
      label={streaming ? "Working" : "Working notes"}
      header={
        <span className="flex w-full items-center gap-2 py-1 text-xs text-muted-fg">
          <span className="relative flex size-2 shrink-0">
            {streaming && (
              <span className="absolute inline-flex size-full animate-ping rounded-full bg-primary/60" />
            )}
            <span
              className={cn(
                "relative inline-flex size-2 rounded-full",
                streaming ? "bg-primary" : "bg-muted-fg/40",
              )}
            />
          </span>
          <span className="font-medium">{streaming ? "Working" : "Working notes"}</span>
          <Chevron className="ml-auto" />
        </span>
      }
    >
      <div className="rounded-md border border-border/70 bg-muted/30 px-3 py-2 text-xs leading-relaxed text-muted-fg">
        {content}
        {streaming && <span className="ml-0.5 inline-block h-3 w-0.5 animate-pulse bg-primary align-middle" />}
      </div>
    </CollapsibleSection>
  );
});
