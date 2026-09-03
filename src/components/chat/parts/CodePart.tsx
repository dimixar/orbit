"use client";

/**
 * Standalone code returned as structured agent data. Reuses the markdown
 * CodeBlock — no duplicated Shiki logic.
 */

import { memo } from "react";
import type { CodePart as CodePartType } from "@/lib/agent/types";
import { CodeBlock } from "../markdown/CodeBlock";

export const CodePart = memo(function CodePart(part: CodePartType) {
  return (
    <CodeBlock
      code={part.code}
      language={part.language}
      filename={part.filename}
      showLineNumbers
    />
  );
});
