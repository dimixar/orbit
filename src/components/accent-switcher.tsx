'use client'

import { CheckIcon } from '@heroicons/react/20/solid'
import {
  ToggleButton,
  ToggleButtonGroup,
} from 'react-aria-components/ToggleButtonGroup'
import { twMerge } from 'tailwind-merge'
import type { Accent } from '@/hooks/use-theme'

// Swatch colors mirror each accent's light-mode --primary token (index.css).
const SWATCHES: { value: Accent; label: string; color: string }[] = [
  { value: 'teal', label: 'Teal', color: 'oklch(0.6 0.118 184.704)' },
  { value: 'blue', label: 'Blue', color: 'oklch(0.55 0.14 255)' },
  { value: 'violet', label: 'Violet', color: 'oklch(0.55 0.17 295)' },
  { value: 'rose', label: 'Rose', color: 'oklch(0.6 0.16 15)' },
  { value: 'amber', label: 'Amber', color: 'oklch(0.7 0.15 70)' },
  { value: 'green', label: 'Green', color: 'oklch(0.6 0.14 152)' },
  { value: 'slate', label: 'Slate', color: 'oklch(0.55 0.025 250)' },
]

/**
 * Accent color picker — a row of swatches on react-aria's ToggleButtonGroup
 * (keyboard navigable). The check mark uses the active accent's --primary-fg
 * so it stays legible on every swatch (e.g. dark ink on amber).
 */
export function AccentSwitcher({
  accent,
  onChange,
}: {
  accent: Accent
  onChange: (accent: Accent) => void
}) {
  return (
    <div className="flex items-center gap-3">
      <ToggleButtonGroup
        aria-label="Accent color"
        selectionMode="single"
        selectedKeys={[accent]}
        onSelectionChange={(keys) => {
          const [first] = keys
          if (typeof first === 'string') {
            onChange(first as Accent)
          }
        }}
        className="flex items-center gap-1.5"
      >
        {SWATCHES.map(({ value, label, color }) => (
          <ToggleButton
            key={value}
            id={value}
            aria-label={label}
            className={({ isSelected, isFocusVisible }) =>
              twMerge(
                'grid size-7 place-items-center rounded-full outline-hidden',
                isSelected
                  ? 'ring-2 ring-fg ring-offset-2 ring-offset-card'
                  : 'hover:ring-2 hover:ring-muted-fg/30 hover:ring-offset-2 hover:ring-offset-card',
                isFocusVisible && 'inset-ring-2 inset-ring-ring',
              )
            }
          >
            <span className="flex size-5.5 items-center justify-center rounded-full" style={{ backgroundColor: color }}>
              {accent === value && <CheckIcon className="size-3.5 text-primary-fg" />}
            </span>
          </ToggleButton>
        ))}
      </ToggleButtonGroup>
      <span className="w-14 text-sm text-muted-fg">
        {SWATCHES.find((s) => s.value === accent)?.label}
      </span>
    </div>
  )
}
