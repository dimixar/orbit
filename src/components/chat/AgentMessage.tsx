"use client";

/**
 * Top-level assistant message: agent identity header + status + parts.
 * Renders parts in order via `AgentMessageParts`; no per-tool logic here.
 */

import { memo } from "react";
import type { AgentMessage as AgentMessageType } from "@/lib/agent/types";
import { AgentMessageParts } from "./AgentMessageParts";

export type AgentMessageProps = {
  message: AgentMessageType;
  onFileClick?: (path: string) => void;
  onApprove?: (id: string) => void;
  onReject?: (id: string) => void;
};

function StatusBadge({ status }: { status?: AgentMessageType["status"] }) {
  if (!status || status === "complete") return null;
  if (status === "error") {
    return (
      <span className="flex items-center gap-1 rounded-sm bg-danger-subtle px-1.5 py-px text-[10px] font-medium text-danger-subtle-fg">
        <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-2.5">
          <path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
        </svg>
        Failed
      </span>
    );
  }
  return (
    <span className="flex items-center gap-1 rounded-sm bg-primary-subtle px-1.5 py-px text-[10px] font-medium text-primary-subtle-fg">
      <span className="size-1.5 animate-pulse rounded-full bg-primary" />
      Streaming
    </span>
  );
}

export const AgentMessage = memo(function AgentMessage({
  message,
  onFileClick,
  onApprove,
  onReject,
}: AgentMessageProps) {
  return (
    <div className="group/agent-message flex gap-3">
      {/* Agent avatar */}
      <div className="mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-md bg-fg text-bg">
        <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
          <path d="M4 12.5V3.5" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
          <path d="M4 3.5h3a2.5 2.5 0 0 1 0 5H4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
          <rect x="9.5" y="9" width="3" height="3" rx="0.6" fill="currentColor" />
        </svg>
      </div>

      <div className="min-w-0 flex-1">
        <div className="mb-1.5 flex items-center gap-2">
          <span className="text-xs font-semibold text-fg">Agent</span>
          <StatusBadge status={message.status} />
          {message.runId && (
            <span className="font-mono text-[10px] text-muted-fg/50">{message.runId}</span>
          )}
        </div>

        {message.parts.length === 0 ? (
          <div className="flex items-center gap-2 py-1 text-xs text-muted-fg">
            <span className="size-3 animate-spin rounded-full border-[1.5px] border-primary/25 border-t-primary" />
            Thinking…
          </div>
        ) : (
          <AgentMessageParts
            parts={message.parts}
            onFileClick={onFileClick}
            onApprove={onApprove}
            onReject={onReject}
          />
        )}
      </div>
    </div>
  );
});
