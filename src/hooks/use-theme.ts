'use client'

import { useEffect, useState } from 'react'

export type Theme = 'light' | 'dark' | 'system'

export type Accent = 'teal' | 'blue' | 'violet' | 'rose' | 'amber' | 'green' | 'slate'

export const ACCENTS: Accent[] = ['teal', 'blue', 'violet', 'rose', 'amber', 'green', 'slate']

/** Interface font choice — ids match the [data-font-ui] rules in index.css. */
export type UiFont =
  | 'default'
  | 'inter'
  | 'geist'
  | 'atkinson'
  | 'source-sans-3'
  | 'roboto'
  | 'noto-sans'
  | 'dm-sans'
  | 'manrope'

/** Code font choice — ids match the [data-font-code] rules in index.css. */
export type CodeFont =
  | 'default'
  | 'system'
  | 'jetbrains'
  | 'fira-code'
  | 'geist-mono'
  | 'commit-mono'
  | 'source-code-pro'
  | 'cascadia-code'
  | 'roboto-mono'
  | 'iosevka'

export const UI_FONTS: UiFont[] = [
  'default',
  'inter',
  'geist',
  'atkinson',
  'source-sans-3',
  'roboto',
  'noto-sans',
  'dm-sans',
  'manrope',
]
export const CODE_FONTS: CodeFont[] = [
  'default',
  'system',
  'jetbrains',
  'fira-code',
  'geist-mono',
  'commit-mono',
  'source-code-pro',
  'cascadia-code',
  'roboto-mono',
  'iosevka',
]

/** Interface scale (%) — root font size; rem text and spacing scale together. */
export const UI_SCALE_RANGE = { min: 80, max: 130, step: 5, default: 100 } as const

/** Code font size (px) — chat code blocks and edit diffs (--app-code-size). */
export const CODE_SIZE_RANGE = { min: 10, max: 18, step: 1, default: 12 } as const

/** Spacing density (%) — scales the Tailwind spacing unit (--spacing). */
export const DENSITY_RANGE = { min: 85, max: 115, step: 5, default: 100 } as const

const UI_SCALE_KEY = 'orbit-ui-scale'
const CODE_SIZE_KEY = 'orbit-code-size'
const DENSITY_KEY = 'orbit-density'

function readNumber(
  key: string,
  range: { min: number; max: number; default: number },
): number {
  const raw = Number.parseFloat(localStorage.getItem(key) ?? '')
  if (!Number.isFinite(raw)) return range.default
  return Math.min(range.max, Math.max(range.min, raw))
}

const THEME_KEY = 'orbit-theme'
const ACCENT_KEY = 'orbit-accent'
const UI_FONT_KEY = 'orbit-font-ui'
const CODE_FONT_KEY = 'orbit-font-code'

/** Resolve the effective color scheme for a stored preference. */
export function resolveDark(theme: Theme): boolean {
  return theme === 'dark' || (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches)
}

/**
 * Theme state, persisted to localStorage and applied by toggling the `.dark`
 * class on <html> (matches the `@custom-variant dark` in index.css). The
 * pre-paint boot script in index.html keeps the first frame correct; this
 * hook keeps it correct afterwards, including live `prefers-color-scheme`
 * changes while set to "system".
 */
export function useTheme() {
  const [theme, setThemeState] = useState<Theme>(() => {
    const stored = localStorage.getItem(THEME_KEY)
    return stored === 'light' || stored === 'dark' || stored === 'system' ? stored : 'system'
  })
  const [accent, setAccentState] = useState<Accent>(() => {
    const stored = localStorage.getItem(ACCENT_KEY)
    return (ACCENTS as string[]).includes(stored ?? '') ? (stored as Accent) : 'teal'
  })
  const [uiFont, setUiFontState] = useState<UiFont>(() => {
    const stored = localStorage.getItem(UI_FONT_KEY)
    return (UI_FONTS as string[]).includes(stored ?? '') ? (stored as UiFont) : 'default'
  })
  const [codeFont, setCodeFontState] = useState<CodeFont>(() => {
    const stored = localStorage.getItem(CODE_FONT_KEY)
    return (CODE_FONTS as string[]).includes(stored ?? '') ? (stored as CodeFont) : 'default'
  })
  const [uiScale, setUiScaleState] = useState(() => readNumber(UI_SCALE_KEY, UI_SCALE_RANGE))
  const [codeSize, setCodeSizeState] = useState(() => readNumber(CODE_SIZE_KEY, CODE_SIZE_RANGE))
  const [density, setDensityState] = useState(() => readNumber(DENSITY_KEY, DENSITY_RANGE))

  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)')
    const apply = () => {
      document.documentElement.classList.toggle('dark', resolveDark(theme))
    }
    apply()
    media.addEventListener('change', apply)
    return () => media.removeEventListener('change', apply)
  }, [theme])

  const setTheme = (next: Theme) => {
    localStorage.setItem(THEME_KEY, next)
    setThemeState(next)
  }

  useEffect(() => {
    document.documentElement.dataset.accent = accent
  }, [accent])

  const setAccent = (next: Accent) => {
    localStorage.setItem(ACCENT_KEY, next)
    setAccentState(next)
  }

  // Fonts ride on data attributes so the stacks themselves live in index.css
  // (single source of truth, and the pre-paint boot script can mirror it).
  useEffect(() => {
    const root = document.documentElement
    if (uiFont === 'default') delete root.dataset.fontUi
    else root.dataset.fontUi = uiFont
  }, [uiFont])

  useEffect(() => {
    const root = document.documentElement
    if (codeFont === 'default') delete root.dataset.fontCode
    else root.dataset.fontCode = codeFont
  }, [codeFont])

  const setUiFont = (next: UiFont) => {
    localStorage.setItem(UI_FONT_KEY, next)
    setUiFontState(next)
  }

  const setCodeFont = (next: CodeFont) => {
    localStorage.setItem(CODE_FONT_KEY, next)
    setCodeFontState(next)
  }

  // Scale / size / density ride on the root element's inline style so the
  // pre-paint boot script in index.html can mirror them exactly.
  useEffect(() => {
    const root = document.documentElement
    if (uiScale === UI_SCALE_RANGE.default) root.style.removeProperty('font-size')
    else root.style.fontSize = `${uiScale}%`
  }, [uiScale])

  useEffect(() => {
    const root = document.documentElement
    if (codeSize === CODE_SIZE_RANGE.default) root.style.removeProperty('--app-code-size')
    else root.style.setProperty('--app-code-size', `${codeSize}px`)
  }, [codeSize])

  useEffect(() => {
    const root = document.documentElement
    // px-based so density stays orthogonal to the interface font size scale
    if (density === DENSITY_RANGE.default) root.style.removeProperty('--spacing')
    else root.style.setProperty('--spacing', `${((4 * density) / 100).toFixed(2)}px`)
  }, [density])

  const setUiScale = (next: number) => {
    const clamped = Math.min(UI_SCALE_RANGE.max, Math.max(UI_SCALE_RANGE.min, next))
    localStorage.setItem(UI_SCALE_KEY, String(clamped))
    setUiScaleState(clamped)
  }

  const setCodeSize = (next: number) => {
    const clamped = Math.min(CODE_SIZE_RANGE.max, Math.max(CODE_SIZE_RANGE.min, next))
    localStorage.setItem(CODE_SIZE_KEY, String(clamped))
    setCodeSizeState(clamped)
  }

  const setDensity = (next: number) => {
    const clamped = Math.min(DENSITY_RANGE.max, Math.max(DENSITY_RANGE.min, next))
    localStorage.setItem(DENSITY_KEY, String(clamped))
    setDensityState(clamped)
  }

  return {
    theme,
    setTheme,
    accent,
    setAccent,
    uiFont,
    setUiFont,
    codeFont,
    setCodeFont,
    uiScale,
    setUiScale,
    codeSize,
    setCodeSize,
    density,
    setDensity,
  }
}
