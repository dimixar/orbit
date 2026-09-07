'use client'

/**
 * Scoped Models — Orbit's surface for pi's `/scoped-models`.
 *
 * Every model in the auth-checked catalog, grouped by provider inside one
 * dense list card. Checked models form the "enabled" subset pi quick-cycles
 * (Ctrl+P in the CLI) and treats as the usable catalog for sessions; the set
 * persists as `enabledModels` patterns in `~/.pi/agent/settings.json`, so the
 * CLI sees the same scope.
 *
 * Scope semantics match the pi CLI selector exactly:
 *   - unscoped (no `enabledModels` in settings) = every model enabled
 *   - unchecking one model scopes to the rest
 *   - re-checking every model clears the setting entirely
 *
 * Raw patterns are shown in a quiet section at the bottom — they can be globs
 * ("anthropic/*:high") written from the CLI that match more than the resolved
 * checkbox list.
 */

import { ArrowPathIcon, CubeTransparentIcon } from '@heroicons/react/24/outline'
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import type { PiModelInfo } from '@assistant-ui/react-pi'
import { twMerge } from 'tailwind-merge'
import { piClient } from '@/lib/pi-client'
import { useScopedModels } from '@/lib/pi-sse'
import { ProviderIcon } from '@/components/chat-panel'
import { Button } from '@/components/ui/button'
import { Checkbox, CheckboxField } from '@/components/ui/checkbox'
import { Note } from '@/components/ui/note'
import { Code, Text } from '@/components/ui/text'
import { Skeleton, WorkbenchCard, WorkbenchPage, WorkbenchSection } from './page'

/** Canonical "provider/modelId" key shared by the picker, settings, and scope. */
const modelKey = (m: Pick<PiModelInfo, 'provider' | 'modelId'>) =>
  `${m.provider}/${m.modelId}`

export function ModelsPage() {
  // undefined = loading, null = server unreachable — the plugins-page
  // convention. A fresh fetch per mount (plus manual refresh) so scope edits
  // made in the CLI while Orbit was open show up here.
  const [models, setModels] = useState<PiModelInfo[] | null | undefined>(undefined)
  const load = useCallback(async () => {
    setModels(undefined)
    try {
      const list = await piClient.getAvailableModels()
      setModels(list)
      window.dispatchEvent(new Event('orbit:models-updated'))
    } catch {
      setModels(null)
    }
  }, [])
  useEffect(() => {
    void load()
  }, [load])

  const { loaded: scopeLoaded, scopedIds, patterns, saving, saveState, toggleScoped, resetScope } =
    useScopedModels()
  const scrollerRef = useRef<HTMLDivElement>(null)
  const pinnedScroll = useRef<number | null>(null)

  const pinScroll = useCallback(() => {
    pinnedScroll.current = scrollerRef.current?.scrollTop ?? 0
  }, [])

  // Restore the column after a toggle: status copy, footer text, and the
  // patterns block used to remount and scroll the focused (then disabled)
  // checkbox out of view.
  useLayoutEffect(() => {
    if (pinnedScroll.current == null) return
    const el = scrollerRef.current
    if (el) el.scrollTop = pinnedScroll.current
  }, [scopedIds, patterns, saving, saveState])

  const safeModels = models ?? []
  // Provider grouping, recomputed when the catalog lands.
  const byProvider = useMemo(() => {
    const groups = new Map<string, PiModelInfo[]>()
    for (const m of safeModels) {
      const list = groups.get(m.provider) ?? []
      list.push(m)
      groups.set(m.provider, list)
    }
    return [...groups.entries()].sort(([a], [b]) => a.localeCompare(b))
  }, [safeModels])

  // Unscoped (null) means every available model is enabled.
  const everythingEnabled =
    scopedIds === null ||
    (safeModels.length > 0 && safeModels.every((m) => scopedIds.includes(modelKey(m))))
  const enabledCount = scopedIds === null ? safeModels.length : scopedIds.length

  const statusLine =
    saveState === 'error'
      ? "Couldn't save the scope — the change was reverted. Is the pi agent running?"
      : !scopeLoaded
        ? 'Reading the current scope…'
        : scopedIds === null
          ? 'All available models are enabled.'
          : `${enabledCount} of ${safeModels.length} enabled.`

  return (
    <WorkbenchPage
      className="max-w-none"
      title="Scoped models"
      description="The enabled subset pi cycles with Ctrl+P and uses for new sessions. Saved to ~/.pi/agent/settings.json, shared with /scoped-models."
      scrollerRef={scrollerRef}
      actions={
        <>
          <Button
            intent="outline"
            size="sm"
            isDisabled={!scopeLoaded || everythingEnabled}
            onPress={() => {
              pinScroll()
              resetScope()
            }}
          >
            Enable all
          </Button>
          <Button
            intent="outline"
            size="sm"
            isPending={models === undefined}
            onPress={() => void load()}
          >
            <ArrowPathIcon data-slot="icon" />
            Refresh
          </Button>
        </>
      }
    >
      {models === undefined || models === null ? (
        models === undefined ? (
          <div className="space-y-3">
            {Array.from({ length: 3 }).map((_, i) => (
              <Skeleton key={i} className="h-20" />
            ))}
          </div>
        ) : (
          <WorkbenchCard className="py-14 text-center">
            <CubeTransparentIcon className="mx-auto mb-3 size-8 text-muted-fg" />
            <p className="font-medium">No models available</p>
            <p className="mx-auto mt-1.5 max-w-sm text-muted-fg text-sm leading-6">
              The pi agent server isn't reachable, or no provider has authentication. Start it
              with{' '}
              <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">pnpm agent:sse</code>{' '}
              and add credentials with{' '}
              <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">pi /login</code>.
            </p>
          </WorkbenchCard>
        )
      ) : (
        <>
          <Note
            intent={saveState === 'error' ? 'danger' : 'default'}
            indicator={false}
            role="status"
            aria-live="polite"
            className="mb-6 min-h-11 py-2.5"
          >
            {statusLine}
          </Note>

          <WorkbenchSection title="Models">
            <WorkbenchCard className="p-0">
              {byProvider.map(([provider, list], gi) => {
                const enabledInProvider = list.filter((m) =>
                  scopedIds === null ? true : scopedIds.includes(modelKey(m)),
                ).length
                return (
                  <section key={provider} className={gi > 0 ? 'border-border border-t' : ''}>
                    <div className="flex items-center gap-2 bg-muted/40 px-4 py-2">
                      <ProviderIcon provider={provider} className="size-3.5 shrink-0" />
                      <span className="text-muted-fg text-xs font-medium uppercase tracking-wider">
                        {provider}
                      </span>
                      <span className="ml-auto text-muted-fg text-xs tabular-nums">
                        {enabledInProvider}/{list.length}
                      </span>
                    </div>
                    <ul className="divide-border divide-y">
                      {list.map((m) => {
                        const id = modelKey(m)
                        const checked = scopedIds === null || scopedIds.includes(id)
                        return (
                          <li
                            key={id}
                            className="transition-colors hover:bg-muted/50"
                          >
                            <CheckboxField
                              isSelected={checked}
                              isDisabled={!scopeLoaded}
                              onChange={() => {
                                pinScroll()
                                toggleScoped(id, safeModels.map(modelKey))
                              }}
                              className="w-full px-4 py-2.5"
                            >
                              <Checkbox className="w-full">
                                <span className="flex min-w-0 items-center gap-2.5">
                                  <ProviderIcon
                                    provider={m.provider}
                                    className="size-4 shrink-0"
                                  />
                                  <span className="truncate text-sm/6 font-medium">
                                    {m.name ?? m.modelId}
                                  </span>
                                  <span className="ml-auto hidden shrink-0 font-mono text-muted-fg text-xs sm:block">
                                    {id}
                                  </span>
                                </span>
                              </Checkbox>
                            </CheckboxField>
                          </li>
                        )
                      })}
                    </ul>
                  </section>
                )
              })}
              <div className="border-border flex min-h-10 items-center gap-2 border-t px-4 py-2.5 text-muted-fg text-xs">
                <ArrowPathIcon
                  className={twMerge('size-3.5 shrink-0', saving && 'animate-spin')}
                />
                {saving
                  ? 'Saving…'
                  : saveState === 'saved'
                    ? 'Saved to ~/.pi/agent/settings.json'
                    : 'Changes apply to new sessions and open ones.'}
              </div>
            </WorkbenchCard>
          </WorkbenchSection>

          <WorkbenchSection title="Patterns in settings.json">
            <WorkbenchCard className="p-4">
              <Text className="mb-2 text-xs leading-5">
                Raw <Code>enabledModels</Code> value. CLI globs can match more than the
                checkboxes above.
              </Text>
              {patterns === null || patterns.length === 0 ? (
                <p className="text-muted-fg text-xs leading-5">
                  None written — the catalog is unscoped.
                </p>
              ) : (
                <div className="flex flex-wrap gap-1.5">
                  {patterns.map((p) => (
                    <Code key={p}>{p}</Code>
                  ))}
                </div>
              )}
            </WorkbenchCard>
          </WorkbenchSection>
        </>
      )}
    </WorkbenchPage>
  )
}