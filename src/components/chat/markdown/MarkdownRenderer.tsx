"use client";

/**
 * Markdown renderer for normal assistant text.
 *
 * - `react-markdown` + `remark-gfm` (tables, strikethrough, task lists, …)
 * - Shiki syntax highlighting via `CodeBlock`
 * - Mermaid diagrams via `MermaidViewer`
 * - NO raw HTML: `rehype-raw` is intentionally not used, so agent-generated
 *   HTML is rendered as plain text, never executed.
 * - Links are sanitized (only http/https/mailto/relative).
 *
 * Tool calls, diffs, plans, reasoning and file operations are NOT rendered
 * here — they arrive as structured `AgentPart`s with dedicated components.
 */

import { memo } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { cn } from "cn";
import { CodeBlock } from "./CodeBlock";
import { MermaidViewer } from "./MermaidViewer";

export type MarkdownRendererProps = {
  content: string;
  className?: string;
};

/** Only allow safe URL schemes + relative paths. */
export function sanitizeHref(href: string | undefined): string | undefined {
  if (!href) return undefined;
  const trimmed = href.trim();
  if (
    trimmed.startsWith("http://") ||
    trimmed.startsWith("https://") ||
    trimmed.startsWith("mailto:") ||
    trimmed.startsWith("/") ||
    trimmed.startsWith("#") ||
    trimmed.startsWith("./") ||
    trimmed.startsWith("../")
  ) {
    return trimmed;
  }
  return undefined;
}

const components: Components = {
  h1: ({ children }) => (
    <h1 className="mb-2 mt-4 text-lg font-semibold tracking-[-0.01em] text-fg first:mt-0">
      {children}
    </h1>
  ),
  h2: ({ children }) => (
    <h2 className="mb-2 mt-4 text-base font-semibold tracking-[-0.01em] text-fg first:mt-0">
      {children}
    </h2>
  ),
  h3: ({ children }) => (
    <h3 className="mb-1.5 mt-3 text-sm font-semibold text-fg first:mt-0">{children}</h3>
  ),
  h4: ({ children }) => (
    <h4 className="mb-1.5 mt-3 text-sm font-medium text-fg first:mt-0">{children}</h4>
  ),
  p: ({ children }) => (
    <p className="my-2 text-sm leading-relaxed text-fg/85 first:mt-0 last:mb-0">{children}</p>
  ),
  ul: ({ children }) => (
    <ul className="my-2 list-disc space-y-1 pl-5 text-sm leading-relaxed text-fg/85 marker:text-muted-fg">
      {children}
    </ul>
  ),
  ol: ({ children }) => (
    <ol className="my-2 list-decimal space-y-1 pl-5 text-sm leading-relaxed text-fg/85 marker:text-muted-fg">
      {children}
    </ol>
  ),
  li: ({ children }) => <li className="pl-0.5">{children}</li>,
  strong: ({ children }) => <strong className="font-semibold text-fg">{children}</strong>,
  em: ({ children }) => <em className="italic">{children}</em>,
  blockquote: ({ children }) => (
    <blockquote className="my-2 border-l-2 border-primary/40 pl-3 text-sm italic text-muted-fg">
      {children}
    </blockquote>
  ),
  hr: () => <hr className="my-4 border-border" />,
  a: ({ href, children }) => {
    const safe = sanitizeHref(href);
    if (!safe) {
      return <span className="text-muted-fg line-through decoration-muted-fg/50">{children}</span>;
    }
    const external = safe.startsWith("http");
    return (
      <a
        href={safe}
        target={external ? "_blank" : undefined}
        rel={external ? "noopener noreferrer" : undefined}
        className="font-medium text-primary-subtle-fg underline decoration-primary-subtle-fg/40 underline-offset-2 hover:decoration-primary-subtle-fg"
      >
        {children}
      </a>
    );
  },
  img: ({ src, alt }) => {
    if (!src) return null;
    const safe =
      src.startsWith("http://") ||
      src.startsWith("https://") ||
      src.startsWith("data:image/")
        ? src
        : undefined;
    if (!safe) return null;
    return (
      <img
        src={safe}
        alt={alt ?? ""}
        className="my-2 max-h-96 max-w-full rounded-lg border border-border"
      />
    );
  },
  table: ({ children }) => (
    <div className="my-3 overflow-x-auto rounded-lg border border-border">
      <table className="w-full border-collapse text-sm">{children}</table>
    </div>
  ),
  thead: ({ children }) => <thead className="bg-muted/60">{children}</thead>,
  th: ({ children }) => (
    <th className="border-b border-border px-3 py-2 text-left font-medium text-fg">{children}</th>
  ),
  td: ({ children }) => (
    <td className="border-b border-border/60 px-3 py-2 align-top text-fg/85 last:border-b-0">
      {children}
    </td>
  ),
  code: ({ className, children }) => {
    const match = /language-([\w-]+)/.exec(className ?? "");
    const raw = String(children).replace(/\n$/, "");
    const isInline = !match && !raw.includes("\n");

    if (isInline) {
      return (
        <code className="rounded-sm border border-border bg-muted px-1 py-px font-mono text-[0.8125em] text-fg">
          {children}
        </code>
      );
    }

    const language = match?.[1] ?? "text";
    if (language === "mermaid") {
      return <MermaidViewer code={raw} className="my-2" />;
    }
    return <CodeBlock code={raw} language={language} className="my-2" />;
  },
  pre: ({ children }) => <>{children}</>,
};

export const MarkdownRenderer = memo(function MarkdownRenderer({
  content,
  className,
}: MarkdownRendererProps) {
  return (
    <div className={cn("min-w-0 break-words", className)}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
        {content}
      </ReactMarkdown>
    </div>
  );
});
