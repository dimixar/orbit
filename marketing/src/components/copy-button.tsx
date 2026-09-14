"use client";

import { Check, CopySimple } from "@phosphor-icons/react/dist/ssr";
import { useState } from "react";
import { Button } from "@/components/ui/button";

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
    <Button
      type="button"
      variant="outline"
      size="xs"
      onClick={copy}
      aria-live="polite"
      className="font-mono text-[10.5px]"
    >
      {copied ? (
        <Check data-icon="inline-start" />
      ) : (
        <CopySimple data-icon="inline-start" />
      )}
      {copied ? "Copied" : label}
    </Button>
  );
}
