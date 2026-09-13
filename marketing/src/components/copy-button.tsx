"use client";

import { Check, CopySimple } from "@phosphor-icons/react/dist/ssr";
import { useState } from "react";

export function CopyButton({
  value,
  label = "Copy",
}: {
  value: string;
  label?: string;
}) {
  const [copied, setCopied] = useState(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
    } catch {
      setCopied(false);
    }
  }

  return (
    <button
      type="button"
      onClick={copy}
      className="inline-flex items-center gap-1.5 rounded-md border border-hairline bg-raised px-2 py-1 font-mono text-[10.5px] text-ink-2 transition-colors hover:border-[#3a3a3a] hover:text-ink"
      aria-live="polite"
    >
      {copied ? (
        <Check className="size-3 text-success" />
      ) : (
        <CopySimple className="size-3" />
      )}
      {copied ? "Copied" : label}
    </button>
  );
}
