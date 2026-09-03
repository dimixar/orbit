"use client";

/**
 * Renders Mermaid diagram source inside a fenced code block.
 *
 * Mermaid is heavy (~2 MB), so it is lazy-loaded on first use and the
 * component stays isolated from `MarkdownRenderer`. Supports loading, error,
 * copy-source, and an expandable view for tall diagrams.
 */

import { memo, useEffect, useId, useRef, useState } from "react";
import { cn } from "cn";
import { Button } from "@/components/ui/button";
import { TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

type MermaidModule = typeof import("mermaid");

let mermaidPromise: Promise<MermaidModule> | null = null;

function loadMermaid(): Promise<MermaidModule> {
  if (!mermaidPromise) {
    mermaidPromise = import("mermaid").then((mod) => {
      mod.default.initialize({
        startOnLoad: false,
        securityLevel: "strict",
        theme: "base",
        themeVariables: {
          fontFamily: "ui-sans-serif, system-ui, sans-serif",
          fontSize: "13px",
        },
        flowchart: { htmlLabels: true, curve: "basis" },
      });
      return mod;
    });
  }
  return mermaidPromise;
}

export type MermaidViewerProps = {
  code: string;
  className?: string;
};

export const MermaidViewer = memo(function MermaidViewer({
  code,
  className,
}: MermaidViewerProps) {
  const [svg, setSvg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [copied, setCopied] = useState(false);
  const renderId = useId().replace(/[^a-zA-Z0-9]/g, "");
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    setSvg(null);
    setError(null);

    loadMermaid()
      .then(async (mod) => {
        const { svg: rendered } = await mod.default.render(`mermaid-${renderId}`, code);
        if (!cancelled) setSvg(rendered);
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [code, renderId]);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // ignore
    }
  };

  if (error) {
    return (
      <div className={cn("overflow-hidden rounded-lg border border-danger-subtle/50", className)}>
        <div className="flex items-center justify-between border-b border-border bg-muted/60 px-3 py-1.5">
          <span className="text-xs font-medium text-danger-subtle-fg">Invalid diagram</span>
          <Button size="sq-xs" intent="plain" aria-label="Copy diagram source" onPress={copy}>
            {copied ? "Copied" : "Copy source"}
          </Button>
        </div>
        <pre className="overflow-x-auto px-4 py-3 font-mono text-xs text-muted-fg">{code}</pre>
      </div>
    );
  }

  return (
    <div className={cn("overflow-hidden rounded-lg border border-border bg-card", className)}>
      <div className="flex items-center justify-between border-b border-border bg-muted/60 px-3 py-1.5">
        <span className="flex items-center gap-1.5 text-xs font-medium text-muted-fg">
          <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
            <path d="M2 4.5 6 8l-4 3.5M8 12h6" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          Diagram
        </span>
        <div className="flex items-center gap-0.5">
          <TooltipTrigger>
            <Button size="sq-xs" intent="plain" aria-label="Copy diagram source" onPress={copy}>
              {copied ? (
                <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
                  <path d="M3 8.5 6.5 12 13 4.5" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              ) : (
                <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
                  <rect x="5.5" y="5.5" width="8" height="8" rx="1.5" stroke="currentColor" strokeWidth="1.4" />
                  <path d="M10.5 5.5v-1A1.5 1.5 0 0 0 9 3H4.5A1.5 1.5 0 0 0 3 4.5V9a1.5 1.5 0 0 0 1.5 1.5h1" stroke="currentColor" strokeWidth="1.4" />
                </svg>
              )}
            </Button>
            <TooltipContent>{copied ? "Copied" : "Copy source"}</TooltipContent>
          </TooltipTrigger>
        </div>
      </div>

      <div
        ref={containerRef}
        className={cn(
          "relative overflow-auto bg-card p-4 [&_svg]:mx-auto [&_svg]:h-auto [&_svg]:max-w-full",
          !expanded && "max-h-80",
        )}
      >
        {svg ? (
          <div
            className="mermaid-rendered [&_svg]:block"
            // Mermaid escapes diagram text; rendered with strict security level.
            dangerouslySetInnerHTML={{ __html: svg }}
          />
        ) : (
          <div className="flex items-center gap-2 py-6 text-xs text-muted-fg">
            <span className="size-3 animate-spin rounded-full border-2 border-border border-t-primary" />
            Rendering diagram…
          </div>
        )}
      </div>

      {!expanded && (
        <button
          type="button"
          onClick={() => setExpanded(true)}
          className="block w-full cursor-pointer border-t border-border bg-muted/40 py-1.5 text-center text-xs text-muted-fg transition-colors hover:bg-muted hover:text-fg"
        >
          Expand diagram
        </button>
      )}
    </div>
  );
});
