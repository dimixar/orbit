import { cn } from "./utils/cn";

export type ThinkingStepsProps = {
  className?: string;
};

/**
 * "Thinking" indicator: a stack of five dashes that light up in sequence,
 * shown while the agent is working and no output has streamed yet.
 * Without motion (prefers-reduced-motion) the dashes rest at full opacity.
 */
export function ThinkingSteps({ className }: ThinkingStepsProps) {
  return (
    <span
      role="status"
      aria-label="Agent working"
      className={cn("flex w-3.5 flex-col items-stretch gap-[1.5px]", className)}
    >
      {[0, 1, 2, 3, 4].map((i) => (
        <span
          key={i}
          className="h-[2px] origin-left rounded-full bg-current motion-safe:animate-thinking-dash"
          style={{ animationDelay: `${i * 0.16}s` }}
        />
      ))}
    </span>
  );
}