'use client'

import { ComputerDesktopIcon, MoonIcon, SunIcon } from '@heroicons/react/24/outline'
import {
  ToggleButton,
  ToggleButtonGroup,
} from 'react-aria-components/ToggleButtonGroup'
import { twMerge } from 'tailwind-merge'
import type { Theme } from '@/hooks/use-theme'

const OPTIONS = [
  { value: 'light', label: 'Light', Icon: SunIcon },
  { value: 'dark', label: 'Dark', Icon: MoonIcon },
  { value: 'system', label: 'System', Icon: ComputerDesktopIcon },
] as const

/**
 * Segmented Light / Dark / System switcher built on react-aria's
 * ToggleButtonGroup (roving tabindex + arrow-key navigation included),
 * styled with design tokens: muted track, elevated selected segment.
 */
export function ThemeSwitcher({
  theme,
  onChange,
}: {
  theme: Theme
  onChange: (theme: Theme) => void
}) {
  return (
    <ToggleButtonGroup
      aria-label="Theme"
      selectionMode="single"
      selectedKeys={[theme]}
      onSelectionChange={(keys) => {
        const [first] = keys
        if (typeof first === 'string') {
          onChange(first as Theme)
        }
      }}
      className="inline-flex items-center rounded-lg border border-border bg-muted p-0.5"
    >
      {OPTIONS.map(({ value, label, Icon }) => (
        <ToggleButton
          key={value}
          id={value}
          className={({ isSelected, isFocusVisible }) =>
            twMerge(
              'flex min-w-16 items-center justify-center gap-1.5 rounded-md px-2.5 py-1.5 text-sm font-medium outline-hidden transition-colors',
              isSelected ? 'bg-bg text-fg shadow-xs' : 'text-muted-fg hover:text-fg',
              isFocusVisible && 'inset-ring inset-ring-ring',
            )
          }
        >
          <Icon className="size-4" />
          {label}
        </ToggleButton>
      ))}
    </ToggleButtonGroup>
  )
}
