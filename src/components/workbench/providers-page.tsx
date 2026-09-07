'use client'

/**
 * Providers — Orbit's surface for pi's models.json.
 *
 * Custom model providers (OpenAI-compatible endpoints, Anthropic-style APIs,
 * local gateways) that pi reads from `~/.pi/agent/models.json` — the same
 * file the pi CLI and SDK read, so an entry added here works in the CLI too,
 * and one added from the CLI shows up here on refresh.
 *
 * The boundary is the design: Orbit edits ONLY the custom layer. Built-in
 * providers come from pi itself and authenticate through `pi /login`; they
 * are listed (with live model counts from the agent's catalog) but never
 * editable here. No fake "connect account" flows — every control writes
 * models.json through the SSE server.
 *
 * THESIS (extension of the workbench world, Operate mode): the provider list
 * is pi's own config file made legible — dense rows, honest source badges,
 * and an editor that maps 1:1 onto what pi reads (id / baseUrl / api /
 * apiKey / models[]). Inherits WorkbenchPage scaffolding, the dense list
 * card, quiet status notes, and inline delete confirmation from the
 * sidebar's session rows.
 */

import {
  ArrowPathIcon,
  KeyIcon,
  PencilSquareIcon,
  PlusIcon,
  ServerStackIcon,
  TrashIcon,
  XMarkIcon,
} from '@heroicons/react/24/outline'
import { useCallback, useEffect, useState } from 'react'
import {
  deleteProvider,
  fetchProviders,
  saveProvider,
} from '@/lib/pi-client'
import { ProviderIcon } from '@/components/chat-panel'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox, CheckboxField } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/field'
import { MagnifyingGlassIcon } from '@heroicons/react/20/solid'
import { Note } from '@/components/ui/note'
import {
  Sheet,
  SheetBody,
  SheetClose,
  SheetContent,
  SheetFooter,
  SheetHeader,
  SheetTrigger,
} from '@/components/ui/sheet'
import { Code } from '@/components/ui/text'
import type { PiProviderInfo, PiProvidersReport, PiSupportedProvider } from '@/lib/pi-agent'
import { Skeleton, WorkbenchCard, WorkbenchPage, WorkbenchSection } from './page'

/** API families pi's runtime can speak — fallback list when the runtime is
 *  unreachable; the server validates against its own live provider set. */
const PROVIDER_APIS = [
  'openai-completions',
  'openai-responses',
  'anthropic-messages',
  'google-generative-ai',
  'amazon-bedrock',
  'azure-openai-responses',
  'bedrock-converse-stream',
  'google-vertex',
  'mistral-conversations',
  'openai-codex-responses',
] as const

const PROVIDER_ID_PATTERN = /^[a-z0-9][a-z0-9._-]*$/i

type ModelRow = { id: string; name: string; contextWindow: string }

type ProviderForm = {
  id: string
  name: string
  baseUrl: string
  api: string
  /** Blank = keep a stored key; typed = replace; removeKey checkbox = clear. */
  apiKey: string
  removeKey: boolean
  models: ModelRow[]
  /** The supported provider this add started from (add mode only). */
  picked: PiSupportedProvider | null
}

const emptyForm = (): ProviderForm => ({
  id: '',
  name: '',
  baseUrl: '',
  api: 'openai-completions',
  apiKey: '',
  removeKey: false,
  models: [{ id: '', name: '', contextWindow: '' }],
  picked: null,
})

/** One custom-provider row, with hover-quieted edit/remove actions. */
function ProviderRow({
  provider,
  onEdit,
  onRemove,
  busy,
}: {
  provider: PiProviderInfo
  onEdit: () => void
  onRemove: () => void
  busy: boolean
}) {
  return (
    <li className="group/row transition-colors hover:bg-muted/50">
      <div className="flex min-w-0 items-center gap-3 px-4 py-3">
        <ProviderIcon provider={provider.id} className="size-4 shrink-0" />
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-2">
            <span className="truncate text-sm/6 font-medium">
              {provider.name ?? provider.id}
            </span>
            <Code className="shrink-0 text-[11px]">{provider.id}</Code>
            {provider.hasApiKey && (
              <span
                className="inline-flex shrink-0 items-center gap-1 text-muted-fg"
                title="An API key is stored in models.json"
              >
                <KeyIcon className="size-3" />
              </span>
            )}
          </div>
          <p className="mt-0.5 flex min-w-0 items-center gap-2 font-mono text-muted-fg text-xs">
            <span className="truncate">{provider.baseUrl || 'no base URL'}</span>
            {provider.api && (
              <>
                <span aria-hidden>·</span>
                <span className="shrink-0">{provider.api}</span>
              </>
            )}
            <span aria-hidden>·</span>
            <span className="shrink-0 tabular-nums">
              {provider.matchesBuiltin
                ? 'pi catalog'
                : `${provider.models.length} ${provider.models.length === 1 ? 'model' : 'models'}`}
            </span>
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1 opacity-0 transition-opacity group-hover/row:opacity-100 group-focus-within/row:opacity-100">
          <Button intent="outline" size="xs" onPress={onEdit} isDisabled={busy}>
            <PencilSquareIcon data-slot="icon" />
            Edit
          </Button>
          <Button
            intent="outline"
            size="xs"
            onPress={onRemove}
            isDisabled={busy}
            aria-label={`Remove ${provider.name ?? provider.id}`}
          >
            <TrashIcon data-slot="icon" />
          </Button>
        </div>
      </div>
    </li>
  )
}

export function ProvidersPage() {
  // undefined = loading, null = server unreachable — the plugins-page
  // convention. Fresh fetch per mount so CLI-side models.json edits show up.
  const [report, setReport] = useState<PiProvidersReport | null | undefined>(
    undefined,
  )

  const load = useCallback(async () => {
    setReport(undefined)
    try {
      const next = await fetchProviders()
      setReport(next)
    } catch {
      setReport(null)
    }
  }, [])
  useEffect(() => {
    void load()
  }, [load])

  // Editor state: null = closed; 'add' | provider id = open in that mode.
  const [editing, setEditing] = useState<'add' | string | null>(null)
  const [form, setForm] = useState<ProviderForm>(emptyForm)
  const [picking, setPicking] = useState(false)
  const [pickerQuery, setPickerQuery] = useState('')
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [removingId, setRemovingId] = useState<string | null>(null)

  const editingProvider =
    editing && editing !== 'add'
      ? report?.custom.find((p) => p.id === editing)
      : undefined

  const openAdd = () => {
    setForm(emptyForm())
    setSaveError(null)
    setPickerQuery('')
    setPicking(true)
    setEditing('add')
  }

  const openEdit = (provider: PiProviderInfo) => {
    setForm({
      id: provider.id,
      name: provider.name ?? '',
      baseUrl: provider.baseUrl,
      api: provider.api || 'openai-completions',
      apiKey: '',
      removeKey: false,
      models:
        provider.models.length > 0
          ? provider.models.map((m) => ({
              id: m.id,
              name: m.name ?? '',
              contextWindow: m.contextWindow ? String(m.contextWindow) : '',
            }))
          : [{ id: '', name: '', contextWindow: '' }],
      picked: null,
    })
    setSaveError(null)
    setPicking(false)
    setEditing(provider.id)
  }

  const closeEditor = () => {
    setEditing(null)
    setPicking(false)
  }

  // Client-side mirror of the server's validation, so obviously-broken input
  // never makes the round trip. A picked supported provider may carry no
  // models (pi's built-in catalog serves it); anything else needs one.
  const formErrors: string[] = []
  const trimmedId = form.id.trim()
  if (!PROVIDER_ID_PATTERN.test(trimmedId)) {
    formErrors.push(
      'ID must start with a letter or number — letters, numbers, dots, dashes and underscores only.',
    )
  }
  if (!/^https?:\/\//.test(form.baseUrl.trim())) {
    formErrors.push('Base URL must start with http:// or https://.')
  }
  const hasModels = form.models.some((m) => m.id.trim())
  if (!hasModels && !form.picked) {
    formErrors.push('Add at least one model ID.')
  }
  const duplicateModel =
    form.models.filter((m) => m.id.trim()).length !==
    new Set(form.models.map((m) => m.id.trim().toLowerCase()).filter(Boolean)).size
  if (duplicateModel) formErrors.push('Model IDs must be unique.')

  const save = async () => {
    if (formErrors.length > 0 || saving) return
    setSaving(true)
    setSaveError(null)
    try {
      const next = await saveProvider(trimmedId, {
        ...(form.name.trim() ? { name: form.name.trim() } : {}),
        baseUrl: form.baseUrl.trim(),
        api: form.api,
        apiKey: form.removeKey
          ? ''
          : form.apiKey
            ? form.apiKey
            : undefined,
        models: form.models
          .filter((m) => m.id.trim())
          .map((m) => ({
            id: m.id.trim(),
            ...(m.name.trim() ? { name: m.name.trim() } : {}),
            ...(Number(m.contextWindow) > 0
              ? { contextWindow: Number(m.contextWindow) }
              : {}),
          })),
      })
      setReport(next)
      setRemovingId(null)
      setEditing(null)
      setPicking(false)
      // The live catalog changed — the model picker picks this up.
      window.dispatchEvent(new Event('orbit:models-updated'))
    } catch (error) {
      setSaveError(error instanceof Error ? error.message : String(error))
    } finally {
      setSaving(false)
    }
  }

  const remove = async (id: string) => {
    try {
      const next = await deleteProvider(id)
      setReport(next)
      setRemovingId(null)
      window.dispatchEvent(new Event('orbit:models-updated'))
    } catch (error) {
      // Keep the row with the error inline — the report is unchanged.
      setSaveError(error instanceof Error ? error.message : String(error))
    }
  }

  const custom = report?.custom ?? []
  const catalog = report?.catalog ?? []

  const statusLine = !report
    ? ''
    : report.parseError
      ? 'models.json could not be read.'
      : `${custom.length} custom ${custom.length === 1 ? 'provider' : 'providers'} in models.json · ${catalog.length} ${catalog.length === 1 ? 'provider' : 'providers'} in the live catalog.`

  return (
    <WorkbenchPage
      title="Providers"
      description="Custom model providers from ~/.pi/agent/models.json — the same file the pi CLI reads, so entries work in both. Built-in providers authenticate through pi (/login) and are listed below, not editable here."
      actions={
        <>
          <Sheet
            isOpen={editing !== null}
            onOpenChange={(open) => {
              if (!open) closeEditor()
            }}
          >
            <SheetTrigger
              intent="primary"
              size="sm"
              onPress={openAdd}
            >
              <PlusIcon data-slot="icon" />
              Add provider
            </SheetTrigger>
            <SheetContent
              aria-label="Provider"
              side="right"
              className="sm:max-w-md"
            >
              <SheetHeader
                title={
                  editing === 'add'
                    ? picking
                      ? 'Add provider'
                      : 'Add provider — details'
                    : 'Edit provider'
                }
                description={
                  editing === 'add' && picking
                    ? 'Providers pi supports, from the running agent. Saved to ~/.pi/agent/models.json on the next step.'
                    : editing === 'add'
                      ? 'Prefilled from pi. Saved to ~/.pi/agent/models.json — pi picks it up on the next catalog refresh.'
                      : 'Saved to ~/.pi/agent/models.json — pi picks it up on the next catalog refresh.'
                }
              />
              <SheetBody>
                {editing === 'add' && picking ? (
                  /* Step 1: pick from the providers pi itself supports. */
                  <div className="flex min-h-0 flex-1 flex-col py-2">
                    <div className="relative">
                      <MagnifyingGlassIcon className="pointer-events-none absolute start-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-fg" />
                      <Input
                        autoFocus
                        aria-label="Search supported providers"
                        placeholder="Search providers"
                        value={pickerQuery}
                        onChange={(e) => setPickerQuery(e.currentTarget.value)}
                        onKeyDown={(e) => {
                          if (e.key === 'Escape' && pickerQuery) {
                            e.stopPropagation()
                            setPickerQuery('')
                          }
                        }}
                        className="ps-8"
                      />
                    </div>
                    {(() => {
                      const query = pickerQuery.trim().toLowerCase()
                      const existingIds = new Set(
                        (report?.custom ?? []).map((p) => p.id),
                      )
                      const matches = (report?.supported ?? []).filter(
                        (p) =>
                          !query ||
                          `${p.name} ${p.id}`.toLowerCase().includes(query),
                      )
                      return matches.length === 0 ? (
                        <p className="py-10 text-center text-muted-fg text-sm">
                          {report?.supported?.length
                            ? `No supported provider matches “${pickerQuery.trim()}”`
                            : 'The pi agent isn’t reachable — restart it to load its supported providers.'}
                        </p>
                      ) : (
                        <ul className="mt-2 min-h-0 flex-1 divide-border divide-y overflow-y-auto">
                          {matches.map((p) => {
                            const added = existingIds.has(p.id)
                            return (
                              <li key={p.id}>
                                <button
                                  type="button"
                                  disabled={added}
                                  onClick={() => {
                                    setForm((f) => ({
                                      ...f,
                                      id: p.id,
                                      name: p.name,
                                      baseUrl: p.baseUrl ?? '',
                                      api: p.api ?? f.api,
                                      models: [{ id: '', name: '', contextWindow: '' }],
                                      picked: p,
                                    }))
                                    setPicking(false)
                                  }}
                                  className="flex w-full min-w-0 items-center gap-3 px-2 py-2.5 text-left transition-colors hover:bg-muted/50 disabled:cursor-not-allowed"
                                >
                                  <ProviderIcon
                                    provider={p.id}
                                    className="size-4 shrink-0"
                                  />
                                  <span className="min-w-0 flex-1">
                                    <span className="block truncate text-sm/6 font-medium">
                                      {p.name}
                                    </span>
                                    <span className="block truncate font-mono text-muted-fg text-xs">
                                      {p.id}
                                      {p.api ? ` · ${p.api}` : ''}
                                    </span>
                                  </span>
                                  {added ? (
                                    <Badge className="shrink-0">in models.json</Badge>
                                  ) : (
                                    <span className="shrink-0 text-muted-fg text-xs tabular-nums">
                                      {p.modelCount}{' '}
                                      {p.modelCount === 1 ? 'model' : 'models'}
                                    </span>
                                  )}
                                </button>
                              </li>
                            )
                          })}
                        </ul>
                      )
                    })()}
                  </div>
                ) : (
                <div className="space-y-5 py-2">
                  {editing === 'add' && form.picked ? (
                    <div className="flex items-center gap-3 rounded-lg border border-border bg-muted/40 px-3 py-2.5">
                      <ProviderIcon
                        provider={form.picked.id}
                        className="size-4 shrink-0"
                      />
                      <div className="min-w-0 flex-1">
                        <p className="truncate text-sm/6 font-medium">
                          {form.picked.name}
                        </p>
                        <p className="truncate font-mono text-muted-fg text-xs">
                          {form.picked.id}
                          {form.picked.api ? ` · ${form.picked.api}` : ''}
                        </p>
                      </div>
                      <Button
                        intent="outline"
                        size="xs"
                        onPress={() => setPicking(true)}
                      >
                        Change
                      </Button>
                    </div>
                  ) : (
                  <div>
                    <Label>Provider ID</Label>
                    <Input
                      value={form.id}
                      disabled={editing !== 'add'}
                      placeholder="my-gateway"
                      onChange={(e) =>
                        setForm((f) => ({ ...f, id: e.currentTarget.value }))
                      }
                      className="mt-2 font-mono"
                    />
                    <p className="mt-1.5 text-muted-fg text-xs">
                      The key pi addresses the provider by — models become{' '}
                      <Code>&lt;id&gt;/&lt;model&gt;</Code>.
                    </p>
                  </div>
                  )}

                  <div>
                    <Label>
                      Display name <span className="text-muted-fg">(optional)</span>
                    </Label>
                    <Input
                      value={form.name}
                      placeholder="My gateway"
                      onChange={(e) =>
                        setForm((f) => ({ ...f, name: e.currentTarget.value }))
                      }
                      className="mt-2"
                    />
                  </div>

                  {form.picked ? (
                    // The picked provider dictates its API — pi owns this.
                    <div>
                      <Label>API type</Label>
                      <p className="mt-2 text-muted-fg text-xs">
                        <Code>{form.picked.api ?? 'provider default'}</Code>{' '}
                        — set by pi for this provider.
                      </p>
                    </div>
                  ) : (
                  <div>
                    <Label>API type</Label>
                    <div className="mt-2 flex flex-wrap gap-1.5" role="radiogroup" aria-label="API type">
                      {PROVIDER_APIS.map((api) => {
                        const selected = form.api === api
                        return (
                          <button
                            key={api}
                            type="button"
                            role="radio"
                            aria-checked={selected}
                            onClick={() => setForm((f) => ({ ...f, api }))}
                            className={`cursor-pointer rounded-md border px-2 py-1 font-mono text-xs transition-colors ${
                              selected
                                ? 'border-primary/60 bg-primary/10 text-primary-fg'
                                : 'border-border text-muted-fg hover:bg-muted hover:text-fg'
                            }`}
                          >
                            {api}
                          </button>
                        )
                      })}
                    </div>
                  </div>
                  )}

                  <div>
                    <Label>Base URL</Label>
                    <Input
                      value={form.baseUrl}
                      placeholder="https://api.example.com/v1"
                      onChange={(e) =>
                        setForm((f) => ({
                          ...f,
                          baseUrl: e.currentTarget.value,
                        }))
                      }
                      className="mt-2 font-mono"
                    />
                  </div>

                  <div>
                    <Label>
                      API key{' '}
                      {editingProvider?.hasApiKey && !form.removeKey && (
                        <span className="text-muted-fg">(stored — leave blank to keep)</span>
                      )}
                    </Label>
                    <Input
                      type="password"
                      value={form.apiKey}
                      placeholder={editingProvider?.hasApiKey ? '••••••••' : 'sk-…'}
                      onChange={(e) =>
                        setForm((f) => ({
                          ...f,
                          apiKey: e.currentTarget.value,
                        }))
                      }
                      className="mt-2"
                    />
                    {editingProvider?.hasApiKey && (
                      <CheckboxField
                        isSelected={form.removeKey}
                        onChange={(selected) =>
                          setForm((f) => ({ ...f, removeKey: selected }))
                        }
                        className="mt-2"
                      >
                        <Checkbox>Remove the stored API key</Checkbox>
                      </CheckboxField>
                    )}
                  </div>

                  <div>
                    <Label>Models</Label>
                    {form.picked ? (
                      <p className="mt-1 text-muted-fg text-xs">
                        Leave empty to use pi's built-in catalog for{' '}
                        {form.picked.name} ({form.picked.modelCount}{' '}
                        {form.picked.modelCount === 1 ? 'model' : 'models'}).
                        Add rows only to extend or override it.
                      </p>
                    ) : (
                      <p className="mt-1 text-muted-fg text-xs">
                        At least one. ID is required; label and context window are optional.
                      </p>
                    )}
                    <div className="mt-2 space-y-2">
                      {form.models.map((model, i) => (
                        <div key={i} className="flex items-start gap-2">
                          <Input
                            value={model.id}
                            placeholder="model-id"
                            aria-label={`Model ${i + 1} ID`}
                            onChange={(e) =>
                              setForm((f) => ({
                                ...f,
                                models: f.models.map((m, j) =>
                                  j === i
                                    ? { ...m, id: e.currentTarget.value }
                                    : m,
                                ),
                              }))
                            }
                            className="flex-1 font-mono"
                          />
                          <Input
                            value={model.name}
                            placeholder="Label"
                            aria-label={`Model ${i + 1} label`}
                            onChange={(e) =>
                              setForm((f) => ({
                                ...f,
                                models: f.models.map((m, j) =>
                                  j === i
                                    ? { ...m, name: e.currentTarget.value }
                                    : m,
                                ),
                              }))
                            }
                            className="w-32 shrink-0"
                          />
                          <Input
                            value={model.contextWindow}
                            placeholder="Ctx"
                            aria-label={`Model ${i + 1} context window`}
                            inputMode="numeric"
                            onChange={(e) =>
                              setForm((f) => ({
                                ...f,
                                models: f.models.map((m, j) =>
                                  j === i
                                    ? {
                                        ...m,
                                        contextWindow:
                                          e.currentTarget.value.replace(/\D/g, ''),
                                      }
                                    : m,
                                ),
                              }))
                            }
                            className="w-20 shrink-0"
                          />
                          <Button
                            intent="outline"
                            size="xs"
                            onPress={() =>
                              setForm((f) => ({
                                ...f,
                                models:
                                  f.models.length > 1
                                    ? f.models.filter((_, j) => j !== i)
                                    : f.models,
                              }))
                            }
                            isDisabled={form.models.length === 1}
                            aria-label={`Remove model row ${i + 1}`}
                          >
                            <XMarkIcon data-slot="icon" />
                          </Button>
                        </div>
                      ))}
                    </div>
                    <Button
                      intent="outline"
                      size="xs"
                      className="mt-2"
                      onPress={() =>
                        setForm((f) => ({
                          ...f,
                          models: [...f.models, { id: '', name: '', contextWindow: '' }],
                        }))
                      }
                    >
                      <PlusIcon data-slot="icon" />
                      Add model
                    </Button>
                  </div>

                  {saveError && (
                    <Note intent="danger" indicator={false} role="alert">
                      {saveError}
                    </Note>
                  )}
                </div>
                )}
              </SheetBody>
              <SheetFooter>
                <SheetClose intent="outline">Cancel</SheetClose>
                {!(editing === 'add' && picking) && (
                  <Button
                    intent="primary"
                    isPending={saving}
                    isDisabled={formErrors.length > 0}
                    onPress={() => void save()}
                  >
                    {editing === 'add' ? 'Add provider' : 'Save changes'}
                  </Button>
                )}
              </SheetFooter>
            </SheetContent>
          </Sheet>
          <Button
            intent="outline"
            size="sm"
            isPending={report === undefined}
            onPress={() => void load()}
          >
            <ArrowPathIcon data-slot="icon" />
            Refresh
          </Button>
        </>
      }
    >
      {report === undefined || report === null ? (
        report === undefined ? (
          <div className="space-y-3">
            {Array.from({ length: 3 }).map((_, i) => (
              <Skeleton key={i} className="h-16" />
            ))}
          </div>
        ) : (
          <WorkbenchCard className="py-14 text-center">
            <ServerStackIcon className="mx-auto mb-3 size-8 text-muted-fg" />
            <p className="font-medium">Providers unavailable</p>
            <p className="mx-auto mt-1.5 max-w-sm text-muted-fg text-sm leading-6">
              The pi agent server isn&apos;t reachable. Start it with{' '}
              <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">
                pnpm agent:sse
              </code>{' '}
              to read and edit models.json.
            </p>
          </WorkbenchCard>
        )
      ) : (
        <>
          {report.parseError && (
            <Note intent="danger" className="mb-6">
              {report.parseError} — fix or remove the file before editing
              providers here, or a save would overwrite it.
            </Note>
          )}

          <Note
            indicator={false}
            role="status"
            aria-live="polite"
            className="mb-6 min-h-11 py-2.5"
          >
            {statusLine}
          </Note>

          <WorkbenchSection title="Custom providers">
            <WorkbenchCard className="p-0">
              {custom.length === 0 && !report.parseError ? (
                <div className="px-4 py-10 text-center">
                  <ServerStackIcon className="mx-auto mb-3 size-7 text-muted-fg" />
                  <p className="text-sm/6 font-medium">No custom providers</p>
                  <p className="mx-auto mt-1 max-w-sm text-muted-fg text-xs leading-5">
                  Pick from the providers pi supports to connect an
                  OpenAI-compatible endpoint, an Anthropic-style API, or any
                  gateway. Saved to models.json and shared with the pi CLI.
                </p>
                </div>
              ) : (
                <ul className="divide-border divide-y">
                  {custom.map((provider) =>
                    removingId === provider.id ? (
                      <li
                        key={provider.id}
                        className="flex min-w-0 items-center gap-1 bg-danger-subtle px-4 py-2.5"
                      >
                        <span className="min-w-0 flex-1 truncate text-danger-subtle-fg text-xs">
                          Remove “{provider.name ?? provider.id}” and its{' '}
                          {provider.models.length}{' '}
                          {provider.models.length === 1 ? 'model' : 'models'}?
                        </span>
                        <Button
                          intent="outline"
                          size="xs"
                          className="border-danger-subtle-fg/30 text-danger-subtle-fg hover:bg-danger-subtle-fg/10"
                          onPress={() => void remove(provider.id)}
                        >
                          Remove
                        </Button>
                        <Button
                          intent="outline"
                          size="xs"
                          onPress={() => setRemovingId(null)}
                        >
                          Cancel
                        </Button>
                      </li>
                    ) : (
                      <ProviderRow
                        key={provider.id}
                        provider={provider}
                        busy={saving || removingId !== null}
                        onEdit={() => openEdit(provider)}
                        onRemove={() => setRemovingId(provider.id)}
                      />
                    ),
                  )}
                </ul>
              )}
              {saveError && (
                <div className="border-border border-t px-4 py-2.5 text-danger-subtle-fg text-xs">
                  {saveError}
                </div>
              )}
              <div className="border-border border-t px-4 py-2.5 font-mono text-muted-fg text-[11px]">
                {report.modelsJsonPath}
              </div>
            </WorkbenchCard>
          </WorkbenchSection>

          <WorkbenchSection title="Built-in providers">
            <WorkbenchCard className="p-0">
              {catalog.length === 0 ? (
                <p className="px-4 py-6 text-muted-fg text-sm">
                  {report.catalogError
                    ? `The live catalog is unreachable: ${report.catalogError}`
                    : 'No providers are serving models right now — add credentials with pi /login.'}
                </p>
              ) : (
                <ul className="divide-border divide-y">
                  {catalog.map((entry) => (
                    <li
                      key={entry.provider}
                      className="flex min-w-0 items-center gap-3 px-4 py-2.5"
                    >
                      <ProviderIcon
                        provider={entry.provider}
                        className="size-4 shrink-0"
                      />
                      <span className="truncate font-mono text-sm/6">
                        {entry.provider}
                      </span>
                      {entry.isCustom && (
                        <Badge className="shrink-0">custom</Badge>
                      )}
                      <span className="ml-auto shrink-0 text-muted-fg text-xs tabular-nums">
                        {entry.modelCount}{' '}
                        {entry.modelCount === 1 ? 'model' : 'models'}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
              <div className="border-border border-t px-4 py-2.5 text-muted-fg text-xs">
                Live from the pi agent&apos;s catalog. Built-ins authenticate
                with <Code>pi /login</Code> — models appear under Scoped models
                once authenticated.
              </div>
            </WorkbenchCard>
          </WorkbenchSection>
        </>
      )}
    </WorkbenchPage>
  )
}