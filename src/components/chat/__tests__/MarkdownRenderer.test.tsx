import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MarkdownRenderer, sanitizeHref } from "../markdown/MarkdownRenderer";

// Shiki is async — mock it so code blocks render synchronously.
vi.mock("@/lib/markdown/highlighter", () => ({
  highlightCode: (code: string) =>
    Promise.resolve(`<pre class="shiki"><code>${code}</code></pre>`),
  normalizeLanguage: (l?: string) => l ?? "text",
}));

// Mermaid is heavy and not needed for these tests.
vi.mock("../markdown/MermaidViewer", () => ({
  MermaidViewer: ({ code }: { code: string }) => (
    <div data-testid="mermaid">{code}</div>
  ),
}));

describe("MarkdownRenderer", () => {
  it("renders headings, paragraphs, bold and lists", () => {
    render(
      <MarkdownRenderer
        content={"## Title\n\nSome **bold** text.\n\n- one\n- two"}
      />,
    );
    expect(screen.getByRole("heading", { level: 2 })).toHaveTextContent("Title");
    expect(screen.getByText(/Some/)).toBeInTheDocument();
    expect(screen.getByText(/bold/)).toBeInTheDocument();
    expect(screen.getByText("one")).toBeInTheDocument();
    expect(screen.getByText("two")).toBeInTheDocument();
  });

  it("renders GFM tables", () => {
    render(
      <MarkdownRenderer
        content={"| A | B |\n|---|---|\n| 1 | 2 |"}
      />,
    );
    expect(screen.getByRole("table")).toBeInTheDocument();
    expect(screen.getByText("1")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
  });

  it("renders fenced code blocks with a language label", () => {
    render(<MarkdownRenderer content={"```ts\nconst x = 1\n```"} />);
    expect(screen.getByText("const x = 1")).toBeInTheDocument();
    expect(screen.getByText("ts")).toBeInTheDocument();
  });

  it("renders inline code", () => {
    render(<MarkdownRenderer content={"Run `npm test` now"} />);
    expect(screen.getByText("npm test")).toBeInTheDocument();
  });

  it("renders mermaid blocks through the MermaidViewer", () => {
    render(
      <MarkdownRenderer
        content={"```mermaid\ngraph TD\n  A --> B\n```"}
      />,
    );
    expect(screen.getByTestId("mermaid")).toHaveTextContent("graph TD");
  });

  it("sanitizes unsafe link schemes", () => {
    expect(sanitizeHref("https://example.com")).toBe("https://example.com");
    expect(sanitizeHref("mailto:a@b.com")).toBe("mailto:a@b.com");
    expect(sanitizeHref("/relative/path")).toBe("/relative/path");
    expect(sanitizeHref("javascript:alert(1)")).toBeUndefined();
    expect(sanitizeHref("data:text/html,<script>")).toBeUndefined();
  });

  it("does not render raw HTML from agent markdown", () => {
    render(
      <MarkdownRenderer
        content={"Hello <script>window.pwned = true</script> <img src=x onerror=alert(1)>"}
      />,
    );
    expect(document.querySelector("script")).toBeNull();
    expect(document.querySelector("img")).toBeNull();
    // The raw HTML is shown as escaped text.
    expect(screen.getByText(/<script>/)).toBeInTheDocument();
  });

  it("renders blockquotes and links", () => {
    render(
      <MarkdownRenderer
        content={"> quoted text\n\n[link](https://example.com)"}
      />,
    );
    expect(screen.getByText("quoted text")).toBeInTheDocument();
    const link = screen.getByRole("link", { name: "link" });
    expect(link).toHaveAttribute("href", "https://example.com");
    expect(link).toHaveAttribute("target", "_blank");
  });
});
