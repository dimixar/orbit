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
import { useCallback, useEffect, useMemo, useState } from 'react'
import type { PiModelInfo } from '@assistant-ui/react-pi'
import { piClient } from '@/lib/pi-client'
import { useScopedModels } from '@/lib/pi-sse'
import { ProviderIcon } from '@/components/chat-panel'
import { Button } from '@/components/ui/button'
import { Checkbox, CheckboxField } from '@/components/ui/checkbox'
import { Note } from '@/components/ui/note'
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
      setModels(await piClient.getAvailableModels())
    } catch {
      setModels(null)
    }
  }, [])
  useEffect(() => {
    void load()
  }, [load])

  const { scopedIds, patterns, saving, saveState, toggleScoped, resetScope } =
    useScopedModels()

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
    safeModels.length > 0 && safeModels.every((m) => scopedIds?.includes(modelKey(m)))
  const enabledCount = scopedIds === null ? safeModels.length : scopedIds.length

  return (
    <WorkbenchPage
      title="Scoped models"
      description="Choose which models pi treats as enabled — the subset quick-cycled with Ctrl+P in the CLI and used when picking a model for new sessions. Saved to ~/.pi/agent/settings.json, shared with pi /scoped-models."
      actions={
        <>
          <Button
            intent="outline"
            size="sm"
            isDisabled={saving || everythingEnabled}
            onPress={resetScope}
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
            intent={saveState === 'error' ? 'danger' : scopedIds === null ? 'info' : 'default'}
            className="mb-8"
          >
            {saveState === 'error' ? (
              <>
                <strong>Couldn't save the scope</strong> — the change was reverted. Is the pi
                agent server running?
              </>
            ) : scopedIds === null ? (
              <>
                <strong>No scope set</strong> — every available model is enabled. Uncheck models
                below to restrict the set.
              </>
            ) : (
              <>
                <strong>{enabledCount}</strong> of {safeModels.length} available models enabled.
                Unchecked models stay in the composer picker, but pi won't cycle to them or fall
                back to them.
              </>
            )}
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
                              isDisabled={saving}
                              onChange={() => toggleScoped(id, safeModels.map(modelKey))}
                              className="w-full px-4 py-2.5"
                            >
                              {/* Label content lives inside Checkbox: it renders
                                  its own control-label cell, aligned beside the
                                  indicator. A sibling Label would wrap below. */}
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
              <div className="border-border flex items-center gap-2 border-t px-4 py-2.5 text-muted-fg text-xs">
                <ArrowPathIcon className="size-3.5 shrink-0" />
                {saving
                  ? 'Saving…'
                  : saveState === 'saved'
                    ? 'Saved to ~/.pi/agent/settings.json — new sessions pick it up immediately, open sessions live.'
                    : 'Toggling a model saves the scope for new sessions and applies it to open ones.'}
              </div>
            </WorkbenchCard>
          </WorkbenchSection>

          {patterns !== null && (
            <WorkbenchSection title={`Patterns in settings.json (${patterns.length})`}>
              <WorkbenchCard className="p-4">
                <p className="mb-2 text-muted-fg text-xs leading-5">
                  The raw <code className="rounded bg-muted px-1 py-0.5 font-mono">enabledModels</code>{' '}
                  value — patterns may be globs written from the CLI that match more than the
                  resolved list above.
                </p>
                <div className="flex flex-wrap gap-1.5">
                  {patterns.map((p) => (
                    <code
                      key={p}
                      className="rounded bg-muted px-1.5 py-0.5 font-mono text-muted-fg text-xs"
                    >
                      {p}
                    </code>
                  ))}
                </div>
              </WorkbenchCard>
            </WorkbenchSection>
          )}

          <p className="text-muted-fg text-xs leading-5">
            Scope is shared with the pi CLI —{' '}
            <code className="rounded bg-muted px-1 py-0.5 font-mono">/scoped-models</code> edits
            the same patterns, including globs like{' '}
            <code className="rounded bg-muted px-1 py-0.5 font-mono">anthropic/*:high</code>.
          </p>
        </>
      )}
    </WorkbenchPage>
  )
}