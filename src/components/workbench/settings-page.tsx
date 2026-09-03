import { AccentSwitcher } from '@/components/accent-switcher'
import { ThemeSwitcher } from '@/components/theme-switcher'
import type { Accent, Theme } from '@/hooks/use-theme'
import { WorkbenchCard, WorkbenchPage, WorkbenchSection } from './page'

export function SettingsPage({
  theme,
  onThemeChange,
  accent,
  onAccentChange,
}: {
  theme: Theme
  onThemeChange: (theme: Theme) => void
  accent: Accent
  onAccentChange: (accent: Accent) => void
}) {
  return (
    <WorkbenchPage
      title="Settings"
      description="Appearance for the Orbit desktop app. The chat runs on the pi agent over SSE (agent/sse-server.ts)."
    >
      <WorkbenchSection title="Appearance">
        <WorkbenchCard className="p-0">
          <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3 px-5 py-4">
            <div className="min-w-0">
              <p className="font-medium text-sm">Theme</p>
              <p className="mt-0.5 text-muted-fg text-sm">
                Light, dark, or follow the system appearance.
              </p>
            </div>
            <ThemeSwitcher theme={theme} onChange={onThemeChange} />
          </div>
          <div className="border-border border-t" />
          <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3 px-5 py-4">
            <div className="min-w-0">
              <p className="font-medium text-sm">Accent color</p>
              <p className="mt-0.5 text-muted-fg text-sm">
                Sets the tone for buttons, focus rings, the sidebar, and charts.
              </p>
            </div>
            <AccentSwitcher accent={accent} onChange={onAccentChange} />
          </div>
        </WorkbenchCard>
      </WorkbenchSection>

      <WorkbenchSection title="About">
        <WorkbenchCard>
          <p className="text-muted-fg text-sm leading-6">
            Orbit is a desktop GUI for the{' '}
            <a
              href="https://pi.dev"
              target="_blank"
              rel="noreferrer"
              className="text-primary-subtle-fg underline decoration-primary/40 underline-offset-2 hover:decoration-primary"
            >
              pi coding agent
            </a>
            . The chat connects over SSE to{' '}
            <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">
              agent/sse-server.ts
            </code>{' '}
            (run with{' '}
            <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">pnpm agent:sse</code>
            ), which drives the pi SDK in-process. Sessions are stored in pi's
            own format at{' '}
            <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">
              ~/.pi/agent/sessions
            </code>{' '}
            — anything you do here works in the CLI too, and vice versa.
          </p>
        </WorkbenchCard>
      </WorkbenchSection>
    </WorkbenchPage>
  )
}
