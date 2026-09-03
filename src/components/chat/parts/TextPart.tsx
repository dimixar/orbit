"use client";

/**
 * Normal assistant prose. Contains almost no Markdown logic — it lazy-loads
 * the Markdown renderer so the heavy markdown/Shiki/Mermaid deps are
 * code-split out of the initial bundle.
 */

import { Suspense, lazy, memo } from "react";
import type { TextPart as TextPartType } from "@/lib/agent/types";

const MarkdownRenderer = lazy(() =>
  import("../markdown/MarkdownRenderer").then((m) => ({
    default: m.MarkdownRenderer,
  })),
);

export const TextPart = memo(function TextPart({ content }: TextPartType) {
  return (
    <div className="text-sm">
      <Suspense
        fallback={
          <div className="whitespace-pre-wrap text-sm leading-relaxed text-fg/85">
            {content}
          </div>
        }
      >
        <MarkdownRenderer content={content} />
      </Suspense>
    </div>
  );
});
