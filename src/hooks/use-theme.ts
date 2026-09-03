'use client'

import { useEffect, useState } from 'react'

export type Theme = 'light' | 'dark' | 'system'

export type Accent = 'teal' | 'blue' | 'violet' | 'rose' | 'amber' | 'green' | 'slate'

export const ACCENTS: Accent[] = ['teal', 'blue', 'violet', 'rose', 'amber', 'green', 'slate']

const THEME_KEY = 'orbit-theme'
const ACCENT_KEY = 'orbit-accent'

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

  return { theme, setTheme, accent, setAccent }
}
