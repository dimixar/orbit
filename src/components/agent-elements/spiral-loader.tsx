import { cn } from "./utils/cn";

export type SpiralLoaderProps = {
  size?: number;
  className?: string;
};

/** Lightweight spinning arc used as the "Processing…" indicator. */
export function SpiralLoader({ size = 16, className }: SpiralLoaderProps) {
  return (
    <svg
      viewBox="0 0 16 16"
      role="status"
      aria-label="Processing"
      className={cn("animate-spin text-an-foreground-muted", className)}
      style={{ width: size, height: size, animationDuration: "0.9s" }}
    >
      <circle
        cx="8"
        cy="8"
        r="6"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeDasharray="28"
        strokeDashoffset="18"
      />
    </svg>
  );
}
