"use client";

/**
 * Human-approval gate. Renders a permission card with Reject/Approve
 * actions; the UI updates immediately to Approved/Rejected.
 */

import { memo, useState } from "react";
import { cn } from "cn";
import type { ApprovalPart, ApprovalStatus } from "@/lib/agent/types";
import { Button } from "@/components/ui/button";

export type ApprovalBlockProps = ApprovalPart & {
  onApprove?: (id: string) => void;
  onReject?: (id: string) => void;
};

export const ApprovalBlock = memo(function ApprovalBlock({
  id,
  title,
  description,
  status,
  onApprove,
  onReject,
}: ApprovalBlockProps) {
  const [localStatus, setLocalStatus] = useState<ApprovalStatus | null>(null);
  const resolved: ApprovalStatus = localStatus ?? status;
  const pending = resolved === "pending";

  const approve = () => {
    setLocalStatus("approved");
    onApprove?.(id);
  };
  const reject = () => {
    setLocalStatus("rejected");
    onReject?.(id);
  };

  return (
    <div className="overflow-hidden rounded-lg border border-border bg-card">
      <div className="flex items-start gap-2.5 px-3 py-2.5">
        <span
          className={cn(
            "mt-px flex size-4 shrink-0 items-center justify-center",
            resolved === "approved" && "text-success-subtle-fg",
            resolved === "rejected" && "text-danger-subtle-fg",
            pending && "text-warning-subtle-fg",
          )}
        >
          {resolved === "approved" ? (
            <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-4">
              <path d="M3 8.5 6.5 12 13 4.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          ) : resolved === "rejected" ? (
            <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-4">
              <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
            </svg>
          ) : (
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
          )}
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-xs font-medium text-fg">{title}</p>
          {description && (
            <p className="mt-0.5 text-xs leading-relaxed text-muted-fg">{description}</p>
          )}
        </div>
      </div>

      <div className="flex items-center justify-end gap-2 border-t border-border/70 bg-muted/30 px-3 py-2">
        {pending ? (
          <>
            <Button size="sm" intent="outline" onPress={reject}>
              Reject
            </Button>
            <Button size="sm" intent="primary" onPress={approve}>
              Approve
            </Button>
          </>
        ) : (
          <span
            className={cn(
              "flex items-center gap-1.5 text-xs font-medium",
              resolved === "approved" ? "text-success-subtle-fg" : "text-danger-subtle-fg",
            )}
          >
            {resolved === "approved" ? (
              <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
                <path d="M3 8.5 6.5 12 13 4.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            ) : (
              <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
                <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
              </svg>
            )}
            {resolved === "approved" ? "Approved" : "Rejected"}
          </span>
        )}
      </div>
    </div>
  );
});
