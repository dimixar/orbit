import { AccentSwitcher } from '@/components/accent-switcher'
import { ThemeSwitcher } from '@/components/theme-switcher'
import {
  CODE_FONT_OPTIONS,
  FontSwitcher,
  UI_FONT_OPTIONS,
} from '@/components/font-switcher'
import { NumberStepper } from '@/components/number-stepper'
import type {
  Accent,
  CodeFont,
  Theme,
  UiFont,
} from '@/hooks/use-theme'
import {
  CODE_SIZE_RANGE,
  DENSITY_RANGE,
  UI_SCALE_RANGE,
} from '@/hooks/use-theme'
import { WorkbenchCard, WorkbenchPage, WorkbenchSection } from './page'

export function SettingsPage({
  theme,
  onThemeChange,
  accent,
  onAccentChange,
  uiFont,
  onUiFontChange,
  codeFont,
  onCodeFontChange,
  uiScale,
  onUiScaleChange,
  codeSize,
  onCodeSizeChange,
  density,
  onDensityChange,
}: {
  theme: Theme
  onThemeChange: (theme: Theme) => void
  accent: Accent
  onAccentChange: (accent: Accent) => void
  uiFont: UiFont
  onUiFontChange: (font: UiFont) => void
  codeFont: CodeFont
  onCodeFontChange: (font: CodeFont) => void
  uiScale: number
  onUiScaleChange: (scale: number) => void
  codeSize: number
  onCodeSizeChange: (size: number) => void
  density: number
  onDensityChange: (density: number) => void
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
          <div className="border-border border-t" />
          <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3 px-5 py-4">
            <div className="min-w-0">
              <p className="font-medium text-sm">Interface font</p>
              <p className="mt-0.5 text-muted-fg text-sm">
                The typeface used across the app's UI, headings included.
              </p>
            </div>
            <FontSwitcher
              ariaLabel="Interface font"
              value={uiFont}
              options={UI_FONT_OPTIONS}
              onChange={onUiFontChange}
            />
          </div>
          <div className="border-border border-t" />
          <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3 px-5 py-4">
            <div className="min-w-0">
              <p className="font-medium text-sm">Interface font size</p>
              <p className="mt-0.5 text-muted-fg text-sm">
                Scales interface text across the app.
              </p>
            </div>
            <NumberStepper
              ariaLabel="Interface font size"
              value={uiScale}
              onChange={onUiScaleChange}
              min={UI_SCALE_RANGE.min}
              max={UI_SCALE_RANGE.max}
              step={UI_SCALE_RANGE.step}
              defaultValue={UI_SCALE_RANGE.default}
              unit="%"
            />
          </div>
          <div className="border-border border-t" />
          <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3 px-5 py-4">
            <div className="min-w-0">
              <p className="font-medium text-sm">Code font</p>
              <p className="mt-0.5 text-muted-fg text-sm">
                Used for code blocks, tool output, and the composer.
              </p>
            </div>
            <FontSwitcher
              ariaLabel="Code font"
              value={codeFont}
              options={CODE_FONT_OPTIONS}
              onChange={onCodeFontChange}
            />
          </div>
          <div className="border-border border-t" />
          <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3 px-5 py-4">
            <div className="min-w-0">
              <p className="font-medium text-sm">Code font size</p>
              <p className="mt-0.5 text-muted-fg text-sm">
                Size of code blocks and diffs in the chat.
              </p>
            </div>
            <NumberStepper
              ariaLabel="Code font size"
              value={codeSize}
              onChange={onCodeSizeChange}
              min={CODE_SIZE_RANGE.min}
              max={CODE_SIZE_RANGE.max}
              step={CODE_SIZE_RANGE.step}
              defaultValue={CODE_SIZE_RANGE.default}
              unit="px"
            />
          </div>
          <div className="border-border border-t" />
          <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3 px-5 py-4">
            <div className="min-w-0">
              <p className="font-medium text-sm">Spacing density</p>
              <p className="mt-0.5 text-muted-fg text-sm">
                Tightens or loosens padding across the app.
              </p>
            </div>
            <NumberStepper
              ariaLabel="Spacing density"
              value={density}
              onChange={onDensityChange}
              min={DENSITY_RANGE.min}
              max={DENSITY_RANGE.max}
              step={DENSITY_RANGE.step}
              defaultValue={DENSITY_RANGE.default}
              unit="%"
            />
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
