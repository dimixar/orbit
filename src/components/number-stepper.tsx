'use client'

import { MinusIcon, PlusIcon } from '@heroicons/react/20/solid'
import { ArrowPathIcon } from '@heroicons/react/24/outline'

const stepButtonClass =
  'flex size-7 cursor-pointer items-center justify-center text-muted-fg outline-none transition-colors duration-100 hover:bg-muted hover:text-fg focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-40 first:rounded-l-lg last:rounded-r-lg'

/**
 * Compact numeric stepper: [−] value [+] with a unit label and a reset
 * affordance that appears whenever the value drifts from the default.
 */
export function NumberStepper({
  value,
  onChange,
  min,
  max,
  step = 1,
  unit,
  defaultValue,
  ariaLabel,
}: {
  value: number
  onChange: (next: number) => void
  min: number
  max: number
  step?: number
  unit?: string
  defaultValue: number
  ariaLabel: string
}) {
  return (
    <div className="flex items-center gap-2">
      {value !== defaultValue && (
        <button
          type="button"
          aria-label={`Reset ${ariaLabel} to default`}
          title="Reset to default"
          onClick={() => onChange(defaultValue)}
          className="flex size-7 cursor-pointer items-center justify-center rounded-lg text-muted-fg outline-none transition-colors duration-100 hover:bg-muted hover:text-fg focus-visible:ring-2 focus-visible:ring-ring"
        >
          <ArrowPathIcon className="size-3.5" />
        </button>
      )}
      <div
        role="group"
        aria-label={ariaLabel}
        className="flex items-center rounded-lg border border-border bg-bg"
      >
        <button
          type="button"
          aria-label={`Decrease ${ariaLabel}`}
          disabled={value <= min}
          onClick={() => onChange(Math.max(min, value - step))}
          className={stepButtonClass}
        >
          <MinusIcon className="size-3.5" />
        </button>
        <span
          aria-live="polite"
          className="min-w-12 text-center text-sm tabular-nums text-fg"
        >
          {value}
        </span>
        <button
          type="button"
          aria-label={`Increase ${ariaLabel}`}
          disabled={value >= max}
          onClick={() => onChange(Math.min(max, value + step))}
          className={stepButtonClass}
        >
          <PlusIcon className="size-3.5" />
        </button>
      </div>
      {unit && <span className="text-muted-fg text-xs">{unit}</span>}
    </div>
  )
}