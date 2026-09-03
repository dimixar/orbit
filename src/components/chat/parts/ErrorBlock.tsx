"use client";

/**
 * Agent/tool errors. Never breaks the message renderer — always renders a
 * contained, dismissible-looking block with expandable details.
 */

import { memo, useState } from "react";
import { cn } from "cn";
import type { ErrorPart } from "@/lib/agent/types";
import { formatToolValue } from "./part-utils";

export const ErrorBlock = memo(function ErrorBlock(part: ErrorPart) {
  const { message, details } = part;
  const [showDetails, setShowDetails] = useState(false);
  const hasDetails = details !== undefined && details !== null;

  return (
    <div className="overflow-hidden rounded-lg border border-danger-subtle/50 bg-danger-subtle/20">
      <div className="flex items-start gap-2.5 px-3 py-2.5">
        <span className="mt-px flex size-4 shrink-0 items-center justify-center text-danger-subtle-fg">
          <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-4">
            <path
              d="M8 1.5 14.5 13.5H1.5L8 1.5Z"
              stroke="currentColor"
              strokeWidth="1.3"
              strokeLinejoin="round"
            />
            <path d="M8 6v3.5" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
            <circle cx="8" cy="11.5" r="0.8" fill="currentColor" />
          </svg>
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-xs font-medium leading-relaxed text-danger-subtle-fg">{message}</p>
          {hasDetails && (
            <button
              type="button"
              onClick={() => setShowDetails((v) => !v)}
              className="mt-1 cursor-pointer text-[11px] text-danger-subtle-fg/70 underline decoration-danger-subtle-fg/30 underline-offset-2 hover:text-danger-subtle-fg"
            >
              {showDetails ? "Hide details" : "Show details"}
            </button>
          )}
        </div>
      </div>
      {hasDetails && showDetails && (
        <pre
          className={cn(
            "max-h-64 overflow-auto border-t border-danger-subtle/30 bg-danger-subtle/10 px-3 py-2 font-mono text-[11px] leading-relaxed text-danger-subtle-fg/90",
          )}
        >
          {formatToolValue(details)}
        </pre>
      )}
    </div>
  );
});
