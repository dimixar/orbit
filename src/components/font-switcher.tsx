'use client'

import { CheckIcon, ChevronDownIcon } from '@heroicons/react/24/outline'
import {
  Menu,
  MenuContent,
  MenuItem,
  MenuLabel,
  MenuTrigger,
} from '@/components/ui/menu'

interface FontOption<Id extends string> {
  value: Id
  label: string
  /** Font family used to render the item's own label — the preview. */
  family: string
  /** Optional second line for ambiguous entries (e.g. "Default"). */
  hint?: string
}

const EMOJI_STACK =
  '"Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji"'

/**
 * Font picker for Settings → Appearance: a dropdown whose items render their
 * own labels in their own typeface, so the choice is visible before picking.
 * Built on the app's Menu stack (keyboard nav + typeahead included).
 */
export function FontSwitcher<Id extends string>({
  value,
  options,
  onChange,
  ariaLabel,
}: {
  value: Id
  options: readonly FontOption<Id>[]
  onChange: (next: Id) => void
  ariaLabel: string
}) {
  const current = options.find((o) => o.value === value)

  return (
    <Menu>
      <MenuTrigger
        aria-label={ariaLabel}
        className="inline-flex h-8 cursor-pointer items-center gap-x-2 rounded-lg border border-border bg-bg px-2.5 text-fg outline-none transition-colors duration-100 hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
      >
        <span
          className="max-w-44 truncate text-sm font-medium"
          style={{ fontFamily: current?.family ? `${current.family}, ${EMOJI_STACK}` : undefined }}
        >
          {current?.label}
        </span>
        <ChevronDownIcon data-slot="chevron" className="size-3.5 shrink-0 text-muted-fg" />
      </MenuTrigger>
      <MenuContent placement="bottom end" className="min-w-56">
        {options.map((option) => (
          <MenuItem
            key={option.value}
            id={option.value}
            textValue={option.label}
            onAction={() => onChange(option.value)}
          >
            <MenuLabel
              className="min-w-0 shrink font-normal"
              style={{ fontFamily: `${option.family}, ${EMOJI_STACK}` }}
            >
              {option.label}
              {option.hint && (
                <span className="ml-2 text-muted-fg text-xs">{option.hint}</span>
              )}
            </MenuLabel>
            {option.value === value && (
              <CheckIcon className="size-3.5 shrink-0" strokeWidth={2} />
            )}
          </MenuItem>
        ))}
      </MenuContent>
    </Menu>
  )
}

/** Curated interface-font options (ids must match index.css + use-theme). */
export const UI_FONT_OPTIONS = [
  { value: 'inter', label: 'Inter', family: 'Inter' },
  { value: 'geist', label: 'Geist Sans', family: 'Geist' },
  { value: 'atkinson', label: 'Atkinson Hyperlegible', family: '"Atkinson Hyperlegible"' },
  { value: 'source-sans-3', label: 'Source Sans 3', family: '"Source Sans 3"' },
  { value: 'roboto', label: 'Roboto', family: 'Roboto' },
  { value: 'noto-sans', label: 'Noto Sans', family: '"Noto Sans"' },
  { value: 'dm-sans', label: 'DM Sans', family: '"DM Sans"' },
  { value: 'manrope', label: 'Manrope', family: 'Manrope' },
  { value: 'default', label: 'System', family: 'ui-sans-serif, system-ui' },
] as const

/** Curated code-font options (ids must match index.css + use-theme). */
export const CODE_FONT_OPTIONS = [
  { value: 'jetbrains', label: 'JetBrains Mono', family: '"JetBrains Mono"' },
  { value: 'fira-code', label: 'Fira Code', family: '"Fira Code"' },
  { value: 'geist-mono', label: 'Geist Mono', family: '"Geist Mono"' },
  { value: 'commit-mono', label: 'Commit Mono', family: '"Commit Mono"' },
  { value: 'source-code-pro', label: 'Source Code Pro', family: '"Source Code Pro"' },
  { value: 'cascadia-code', label: 'Cascadia Code', family: '"Cascadia Code"' },
  { value: 'roboto-mono', label: 'Roboto Mono', family: '"Roboto Mono"' },
  { value: 'iosevka', label: 'Iosevka', family: 'Iosevka' },
  { value: 'system', label: 'System Mono', family: 'ui-monospace, SFMono-Regular, Menlo' },
  { value: 'default', label: 'Default', family: '"IBM Plex Mono"', hint: 'IBM Plex Mono' },
] as const

/** Re-exported for consumers typing against the switcher's option shape. */
export type { FontOption }