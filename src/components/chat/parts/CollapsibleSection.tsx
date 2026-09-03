"use client";

/**
 * Collapsible section used by reasoning, plan, tool, diff and result blocks.
 * Wraps Base UI's Collapsible with the app's panel animation classes.
 */

import { Collapsible } from "@base-ui/react/collapsible";
import { cn } from "cn";
import type { ReactNode } from "react";

export type CollapsibleSectionProps = {
  header: ReactNode;
  children: ReactNode;
  defaultOpen?: boolean;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  disabled?: boolean;
  className?: string;
  contentClassName?: string;
  /** aria-label for the trigger button. */
  label?: string;
};

export function CollapsibleSection({
  header,
  children,
  defaultOpen = false,
  open,
  onOpenChange,
  disabled = false,
  className,
  contentClassName,
  label,
}: CollapsibleSectionProps) {
  const rootProps =
    open === undefined ? { defaultOpen } : { open, onOpenChange };

  return (
    <Collapsible.Root className={cn("w-full", className)} {...rootProps}>
      <Collapsible.Trigger
        disabled={disabled}
        aria-label={label}
        className={cn(
          "group flex w-full items-center rounded-md text-left outline-none",
          !disabled && "cursor-pointer",
          "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring",
        )}
      >
        {header}
      </Collapsible.Trigger>
      <Collapsible.Panel
        className={cn(
          "overflow-hidden",
          "h-[var(--collapsible-panel-height)] transition-all duration-150 ease-out",
          "data-ending-style:h-0 data-starting-style:h-0",
          "[&[hidden]:not([hidden='until-found'])]:hidden",
          contentClassName,
        )}
      >
        {children}
      </Collapsible.Panel>
    </Collapsible.Root>
  );
}

/** Small chevron that rotates when its parent collapsible is open. */
export function Chevron({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden="true"
      className={cn(
        "size-3 shrink-0 text-muted-fg transition-transform duration-150 group-data-panel-open:rotate-90",
        className,
      )}
    >
      <path
        d="M6 3.5 10.5 8 6 12.5"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
