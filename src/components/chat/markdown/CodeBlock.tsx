"use client";

/**
 * Reusable syntax-highlighted code block.
 *
 * - Shiki highlighting (async, cached in `lib/markdown/highlighter.ts`)
 * - copy button, filename, language label
 * - optional line numbers and code-wrapping toggle
 * - horizontal scrolling
 * - loading state (plain escaped code until highlight resolves)
 *
 * The highlighted HTML is produced by Shiki, which escapes all code content,
 * so injecting it via `dangerouslySetInnerHTML` is safe — agent code is
 * never executed.
 */

import { memo, useEffect, useMemo, useState } from "react";
import { cn } from "cn";
import { highlightCode, normalizeLanguage } from "@/lib/markdown/highlighter";
import { Button } from "@/components/ui/button";
import { TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

export type CodeBlockProps = {
  code: string;
  language?: string;
  filename?: string;
  /** Show line numbers (default false). */
  showLineNumbers?: boolean;
  /** Wrap long lines (default false → horizontal scroll). */
  wrap?: boolean;
  className?: string;
};

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard unavailable (e.g. non-secure context) — ignore.
    }
  };
  return (
    <TooltipTrigger>
      <Button
        size="sq-xs"
        intent="plain"
        aria-label={copied ? "Copied" : "Copy code"}
        onPress={copy}
        className="text-muted-fg"
      >
        {copied ? (
          <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
            <path
              d="M3 8.5 6.5 12 13 4.5"
              stroke="currentColor"
              strokeWidth="1.8"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        ) : (
          <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
            <rect x="5.5" y="5.5" width="8" height="8" rx="1.5" stroke="currentColor" strokeWidth="1.4" />
            <path d="M10.5 5.5v-1A1.5 1.5 0 0 0 9 3H4.5A1.5 1.5 0 0 0 3 4.5V9a1.5 1.5 0 0 0 1.5 1.5h1" stroke="currentColor" strokeWidth="1.4" />
          </svg>
        )}
      </Button>
      <TooltipContent>{copied ? "Copied" : "Copy code"}</TooltipContent>
    </TooltipTrigger>
  );
}

export const CodeBlock = memo(function CodeBlock({
  code,
  language,
  filename,
  showLineNumbers = false,
  wrap = false,
  className,
}: CodeBlockProps) {
  const [html, setHtml] = useState<string | null>(null);
  const [error, setError] = useState(false);
  const [lineNumbers, setLineNumbers] = useState(showLineNumbers);
  const [wrapped, setWrapped] = useState(wrap);

  const lang = useMemo(() => normalizeLanguage(language), [language]);

  useEffect(() => {
    let cancelled = false;
    setHtml(null);
    setError(false);
    highlightCode(code, language)
      .then((result) => {
        if (!cancelled) setHtml(result);
      })
      .catch(() => {
        if (!cancelled) setError(true);
      });
    return () => {
      cancelled = true;
    };
  }, [code, language]);

  const lineCount = useMemo(() => code.split("\n").length, [code]);

  return (
    <div
      className={cn(
        "group/code overflow-hidden rounded-lg border border-border bg-muted/40",
        className,
      )}
    >
      {(filename || language) && (
        <div className="flex h-8 items-center gap-2 border-b border-border bg-muted/60 px-3">
          {filename && (
            <span className="truncate font-mono text-xs text-muted-fg">{filename}</span>
          )}
          {language && (
            <span className="ml-auto rounded-sm bg-secondary px-1.5 py-px font-mono text-[10px] uppercase tracking-wide text-muted-fg">
              {lang}
            </span>
          )}
        </div>
      )}

      <div className="flex items-center justify-end gap-0.5 border-b border-border/60 px-2 py-1">
        <div className="flex items-center gap-0.5">
          {lineCount > 1 && (
            <TooltipTrigger>
              <Button
                size="sq-xs"
                intent="plain"
                aria-label={lineNumbers ? "Hide line numbers" : "Show line numbers"}
                aria-pressed={lineNumbers}
                onPress={() => setLineNumbers((v) => !v)}
                className={cn("text-muted-fg", lineNumbers && "bg-secondary text-fg")}
              >
                <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
                  <path d="M2 4h12M2 8h12M2 12h12" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
                  <path d="M6.5 2v12M9.5 2v12" stroke="currentColor" strokeWidth="1" strokeLinecap="round" opacity="0.5" />
                </svg>
              </Button>
              <TooltipContent>{lineNumbers ? "Hide line numbers" : "Show line numbers"}</TooltipContent>
            </TooltipTrigger>
          )}
          <TooltipTrigger>
            <Button
              size="sq-xs"
              intent="plain"
              aria-label={wrapped ? "Disable wrapping" : "Wrap lines"}
              aria-pressed={wrapped}
              onPress={() => setWrapped((v) => !v)}
              className={cn("text-muted-fg", wrapped && "bg-secondary text-fg")}
            >
              <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" className="size-3.5">
                <path d="M2 4h12M2 8h7M2 12h4" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
                <path d="M9 8l3 2-3 2" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </Button>
            <TooltipContent>{wrapped ? "Disable wrapping" : "Wrap lines"}</TooltipContent>
          </TooltipTrigger>
          <CopyButton text={code} />
        </div>
      </div>

      <div className="overflow-x-auto">
        {error ? (
          <pre className="px-4 py-3 font-mono text-xs text-danger-subtle-fg">
            Failed to highlight code
          </pre>
        ) : html ? (
          <div className={cn("flex min-h-full", lineNumbers && "pl-0")}>
            {lineNumbers && (
              <div
                aria-hidden="true"
                className="sticky left-0 z-10 shrink-0 select-none border-r border-border/60 bg-muted/40 px-2 py-3 text-right font-mono text-xs leading-[1.6] text-muted-fg/50"
              >
                {Array.from({ length: lineCount }, (_, i) => (
                  <div key={i}>{i + 1}</div>
                ))}
              </div>
            )}
            <div
              className={cn(
                "min-w-0 flex-1 [&_pre]:px-4 [&_pre]:py-3 [&_pre]:font-mono [&_pre]:text-xs [&_pre]:leading-[1.6]",
                wrapped
                  ? "[&_pre]:whitespace-pre-wrap [&_pre]:break-words"
                  : "[&_pre]:whitespace-pre",
              )}
              // Shiki escapes all code content — safe to inject.
              dangerouslySetInnerHTML={{ __html: html }}
            />
          </div>
        ) : (
          <pre className="px-4 py-3 font-mono text-xs leading-[1.6] text-muted-fg">
            {code}
          </pre>
        )}
      </div>
    </div>
  );
});
