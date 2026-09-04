"use client";

/**
 * Maps each `AgentPart` to its dedicated component. Adding a new part type
 * means adding one case here (plus the type and reducer mapping) — the rest
 * of the chat system stays untouched.
 */

import { memo } from "react";
import type { AgentPart } from "@/lib/agent/types";
import { TextPart } from "./parts/TextPart";
import { ReasoningBlock } from "./parts/ReasoningBlock";
import { PlanBlock } from "./parts/PlanBlock";
import { ToolCallBlock } from "./parts/ToolCallBlock";
import { ToolResultBlock } from "./parts/ToolResultBlock";
import { CodePart } from "./parts/CodePart";
import { DiffViewer } from "./parts/DiffViewer";
import { FileChangeBlock } from "./parts/FileChangeBlock";
import { ErrorBlock } from "./parts/ErrorBlock";
import { ApprovalBlock } from "./parts/ApprovalBlock";

export type AgentMessagePartsProps = {
  parts: AgentPart[];
  onFileClick?: (path: string) => void;
  onApprove?: (id: string) => void;
  onReject?: (id: string) => void;
};

export const AgentMessageParts = memo(function AgentMessageParts({
  parts,
  onFileClick,
  onApprove,
  onReject,
}: AgentMessagePartsProps) {
  return (
    <div className="flex flex-col gap-2.5">
      {parts.map((part, index) => {
        switch (part.type) {
          case "text":
            return <TextPart key={index} {...part} />;

          case "reasoning":
            return <ReasoningBlock key={index} {...part} />;

          case "plan":
            return <PlanBlock key={index} {...part} />;

          case "tool_call": {
            // When a matching result part follows, the output lives in the
            // ToolResultBlock — don't duplicate it inside the call row.
            // Keyed with the index suffix: some models re-emit the same
            // tool-call id across turns, and a bare `part.id` would collide.
            const next = parts[index + 1];
            const hasResult =
              next?.type === "tool_result" && next.toolCallId === part.id;
            return (
              <ToolCallBlock
                key={`${part.id}:${index}`}
                {...part}
                showOutput={!hasResult}
              />
            );
          }

          case "tool_result":
            return <ToolResultBlock key={`${part.toolCallId}:${index}`} {...part} />;

          case "code":
            return <CodePart key={index} {...part} />;

          case "diff":
            return <DiffViewer key={part.file} {...part} />;

          case "file":
            return (
              <FileChangeBlock key={index} {...part} onFileClick={onFileClick} />
            );

          case "approval":
            return (
              <ApprovalBlock
                key={part.id}
                {...part}
                onApprove={onApprove}
                onReject={onReject}
              />
            );

          case "error":
            return <ErrorBlock key={index} {...part} />;

          default:
            return null;
        }
      })}
    </div>
  );
});
