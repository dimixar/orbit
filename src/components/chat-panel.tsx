'use client'

/**
 * The main chat — the original Orbit chat UI, now powered by the Pi runtime
 * over SSE (agent/sse-server.ts) instead of the WebSocket daemon.
 *
 *   browser (this webview)                    Node server
 *   ─────────────────                         ───────────
 *   usePiRuntime (SSE)  ──HTTP/SSE──▶        agent/sse-server.ts
 *   usePiSseChat                              └ createPiNodeClient → Pi SDK
 *
 * Start the SSE server first:  pnpm agent:sse
 */

import { useEffect, useMemo, useRef, useState } from 'react'
import {
  ArrowUpIcon,
  ChevronDownIcon,
  ComputerDesktopIcon,
  EyeIcon,
  FolderIcon,
  InformationCircleIcon,
  LockClosedIcon,
  LockOpenIcon,
  StopIcon,
} from '@heroicons/react/24/outline'
import { twMerge } from 'tailwind-merge'
import type { ChatStatus, UIMessage } from 'ai'
import { useAui } from '@assistant-ui/react'
import { usePiRuntimeExtras } from '@assistant-ui/react-pi'
import { MessageList as AgentMessageList } from '@/components/agent-elements/message-list'
import {
  Menu,
  MenuContent,
  MenuDescription,
  MenuItem,
  MenuLabel,
  MenuSection,
  MenuTrigger,
} from '@/components/ui/menu'
import { usePiSseChat, usePiModels } from '@/lib/pi-sse'
import type { ChatPart } from '@/lib/pi-agent'

/** toolName → tool-card part type: bash → tool-Bash, web_fetch → tool-WebFetch. */
function pascalToolName(name: string): string {
  return name.replace(/(?:^|_)([a-z])/g, (_, c: string) => c.toUpperCase())
}

/** Map one daemon ChatPart onto the Agent Elements UIMessage part(s). */
function toUIPart(part: ChatPart, key: string): UIMessage['parts'] {
  if (part.type === 'text') {
    return part.text ? [{ type: 'text', text: part.text }] : []
  }
  if (part.type === 'thinking') {
    const done = part.done === true
    return [
      {
        type: 'tool-Thinking' as const,
        toolCallId: `${key}-think`,
        state: done ? 'output-available' : 'input-streaming',
        input: { thought: part.text },
        ...(done ? { output: part.text } : {}),
      },
    ] as unknown as UIMessage['parts']
  }
  return [
    {
      type: `tool-${pascalToolName(part.toolName)}` as const,
      toolCallId: part.toolCallId || `${key}-tool`,
      state: part.isError ? 'output-error' : part.output !== undefined ? 'output-available' : 'call',
      input: part.input ?? {},
      ...(part.output !== undefined ? { output: part.output } : {}),
    },
  ] as unknown as UIMessage['parts']
}

/* ------------------------------------------------------------------ */
/* Marks                                                               */
/* ------------------------------------------------------------------ */

/** Six-spoke coral asterisk used above the empty-state prompt. */
function Asterisk({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden="true"
      className={twMerge('text-warning', className)}
    >
      <g stroke="currentColor" strokeWidth="1.9" strokeLinecap="round">
        <line x1="8" y1="1.6" x2="8" y2="14.4" />
        <line x1="2.8" y1="4.8" x2="13.2" y2="11.2" />
        <line x1="2.8" y1="11.2" x2="13.2" y2="4.8" />
      </g>
    </svg>
  )
}

/** White "Pᵢ" tile: the agent mark, optionally tinted (header variant). */
function PiTile({ className, tint = 'oklch(0.228 0.013 107.4)' }: { className?: string; tint?: string }) {
  return (
    <span
      aria-hidden="true"
      className={twMerge(
        'flex size-3.5 shrink-0 items-center justify-center rounded-[3px] bg-fg',
        className,
      )}
    >
      <svg viewBox="0 0 10 10" className="size-2.5" fill="none">
        <path
          d="M2.6 8.4V1.6"
          stroke={tint}
          strokeWidth="2"
          strokeLinecap="round"
        />
        <path
          d="M2.6 1.6h2a1.9 1.9 0 0 1 0 3.8h-2"
          stroke={tint}
          strokeWidth="2"
          strokeLinecap="round"
        />
        <rect x="6.4" y="6.3" width="2.1" height="2.1" rx="0.4" fill={tint} />
      </svg>
    </span>
  )
}

/** Git-branch glyph (heroicons has none). */
function BranchIcon({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      aria-hidden="true"
      className={className}
    >
      <circle cx="4.5" cy="3.6" r="1.8" />
      <circle cx="4.5" cy="12.4" r="1.8" />
      <circle cx="11.8" cy="5.6" r="1.8" />
      <path d="M4.5 5.4v5.2" />
      <path d="M11.8 7.4c0 2.2-2.2 3.3-5 3.5" />
    </svg>
  )
}

/** Right-hand panel glyph (heroicons only ships the left variant). */
function RightPanelIcon({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 20 20"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      aria-hidden="true"
      className={className}
    >
      <rect x="2.8" y="4" width="14.4" height="12" rx="2.2" />
      <path d="M13.2 4v12" />
    </svg>
  )
}

/** Small square icon button, muted until hover. */
function IconButton({
  label,
  className,
  children,
}: {
  label: string
  className?: string
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={twMerge(
        'flex size-7 cursor-pointer items-center justify-center rounded-lg text-muted-fg transition-colors duration-100 hover:bg-muted hover:text-fg',
        className,
      )}
    >
      {children}
    </button>
  )
}

/* ------------------------------------------------------------------ */
/* Header                                                              */
/* ------------------------------------------------------------------ */

function ChatHeader({ title, connected }: { title: string; connected: boolean }) {
  return (
    <header className="flex h-12 shrink-0 items-center justify-between pl-4 pr-3.5">
      <h1 className="truncate text-[13px] font-medium tracking-[-0.01em] text-fg">
        {title}
      </h1>

      <div className="flex shrink-0 items-center gap-4">
        <span className="flex items-center gap-2 text-xs font-medium tabular-nums">
          <span className="text-success-subtle-fg">+3040</span>
          <span className="text-danger-subtle-fg">-2225</span>
        </span>

        <div className="flex items-center gap-2">
          <span
            aria-label={connected ? 'Agent connected' : 'Agent disconnected'}
            title={connected ? 'Agent connected' : 'Agent disconnected'}
            className={twMerge(
              'size-2.5 rounded-full border-[1.5px] transition-colors',
              connected ? 'border-success-subtle-fg' : 'border-input',
            )}
          />
          <IconButton label="Session info">
            <InformationCircleIcon className="size-4" strokeWidth={1.6} />
          </IconButton>

          <IconButton label="Toggle panel">
            <RightPanelIcon className="size-4" />
          </IconButton>
        </div>
      </div>
    </header>
  )
}

/* ------------------------------------------------------------------ */
/* Composer                                                            */
/* ------------------------------------------------------------------ */

const THINKING_LEVEL_LABELS: Record<string, string> = {
  off: 'Off',
  minimal: 'Minimal',
  low: 'Low',
  medium: 'Medium',
  high: 'High',
  xhigh: 'Extra high',
  max: 'Max',
}

/** Shared look for the composer's selector chips: muted text, hover wash, chevron. */
const chipTriggerClass =
  'flex shrink-0 cursor-pointer items-center gap-1.5 rounded-md px-1.5 py-0.5 text-xs text-muted-fg transition-colors duration-100 hover:bg-muted hover:text-fg data-[pressed]:bg-muted disabled:opacity-50'

/** First string key of a react-aria selection, if any. */
function firstKey(keys: 'all' | Set<React.Key>): string | undefined {
  if (keys === 'all') return undefined
  const [first] = keys
  return typeof first === 'string' ? first : undefined
}

/** Signal bars used for thinking-effort levels; `lit` of 4 bars filled. */
function EffortGlyph({ lit, className }: { lit: 0 | 1 | 2 | 3 | 4; className?: string }) {
  const heights = [4, 6.5, 9, 11.5]
  return (
    <svg viewBox="0 0 14 14" aria-hidden="true" className={className}>
      {heights.map((h, i) => (
        <rect
          key={i}
          x={1.4 + i * 3.3}
          y={12.6 - h}
          width={2.1}
          height={h}
          rx={0.8}
          className={i < lit ? 'fill-current' : 'fill-current opacity-25'}
        />
      ))}
    </svg>
  )
}

/** Dot-in-circle used for the "off" effort level. */
function EffortOffGlyph({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth={1.4} aria-hidden="true" className={className}>
      <circle cx="7" cy="7" r="4.6" />
      <path d="M3.9 3.9l6.2 6.2" />
    </svg>
  )
}

const EFFORT_ICONS: Record<string, (className: string) => React.ReactNode> = {
  off: (c) => <EffortOffGlyph className={c} />,
  minimal: (c) => <EffortGlyph lit={1} className={c} />,
  low: (c) => <EffortGlyph lit={2} className={c} />,
  medium: (c) => <EffortGlyph lit={3} className={c} />,
  high: (c) => <EffortGlyph lit={4} className={c} />,
  xhigh: (c) => <EffortGlyph lit={4} className={c} />,
  max: (c) => <EffortGlyph lit={4} className={c} />,
}

function Composer({
  modelLabel,
  models,
  thinkingLevel,
  thinkingLevels,
  connected,
  isStreaming,
  onSend,
  onAbort,
  onSelectModel,
  onSelectThinkingLevel,
}: {
  modelLabel: string
  models: { provider: string; modelId: string; name?: string }[]
  thinkingLevel: string
  thinkingLevels: string[]
  connected: boolean
  isStreaming: boolean
  onSend: (text: string) => void
  onAbort: () => void
  onSelectModel: (provider: string, modelId: string) => void
  onSelectThinkingLevel: (level: string) => void
}) {
  const [draft, setDraft] = useState('')
  const taRef = useRef<HTMLTextAreaElement>(null)
  const canSend = draft.trim().length > 0

  useEffect(() => {
    const el = taRef.current
    if (!el) return
    el.style.height = 'auto'
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`
  }, [draft])

  const submit = () => {
    if (isStreaming) {
      onAbort()
      return
    }
    const text = draft.trim()
    if (!text) return
    setDraft('')
    if (taRef.current) taRef.current.style.height = 'auto'
    onSend(text)
  }

  return (
    <div className="shrink-0 px-4 pb-4">
      <form
        className="mx-auto w-full max-w-[720px]"
        onSubmit={(e) => {
          e.preventDefault()
          submit()
        }}
      >
        <div className="rounded-[10px] border border-border bg-card px-4 pt-3.5 pb-2.5 shadow-[0_8px_24px_rgba(0,0,0,0.35)]">
          <textarea
            ref={taRef}
            rows={1}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault()
                submit()
              }
            }}
            placeholder="Do anything…"
            aria-label="Prompt"
            spellCheck={false}
            className="block w-full resize-none bg-transparent text-[13px] leading-[18px] text-fg outline-none placeholder:text-muted-fg"
          />

          <div className="mt-3 flex items-center gap-x-2">
            <Menu>
              <MenuTrigger
                aria-label="Model"
                isDisabled={!connected}
                className={chipTriggerClass}
              >
                <PiTile />
                <span className="truncate">{modelLabel}</span>
                <ChevronDownIcon className="size-3 shrink-0 text-muted-fg" strokeWidth={1.8} />
              </MenuTrigger>
              <MenuContent
                placement="top start"
                selectionMode="single"
                selectedKeys={[modelLabel]}
                onSelectionChange={(keys) => {
                  const key = firstKey(keys)
                  if (!key) return
                  const [provider, modelId] = key.split('/')
                  if (provider && modelId) onSelectModel(provider, modelId)
                }}
                className="max-h-80"
              >
                <MenuSection label="Available models">
                  {models.map((m) => {
                    const id = `${m.provider}/${m.modelId}`
                    return (
                      <MenuItem key={id} id={id} textValue={m.name ?? m.modelId}>
                        <MenuLabel>{m.name ?? m.modelId}</MenuLabel>
                        <MenuDescription>{m.provider}</MenuDescription>
                      </MenuItem>
                    )
                  })}
                  {models.length === 0 && (
                    <MenuItem isDisabled>No models — is the SSE server running?</MenuItem>
                  )}
                </MenuSection>
              </MenuContent>
            </Menu>

            <Menu>
              <MenuTrigger
                aria-label="Thinking effort"
                isDisabled={!connected}
                className={chipTriggerClass}
              >
                {THINKING_LEVEL_LABELS[thinkingLevel] ?? thinkingLevel}
                <ChevronDownIcon className="size-3 shrink-0 text-muted-fg" strokeWidth={1.8} />
              </MenuTrigger>
              <MenuContent
                placement="top start"
                selectionMode="single"
                selectedKeys={[thinkingLevel]}
                onSelectionChange={(keys) => {
                  const key = firstKey(keys)
                  if (key) onSelectThinkingLevel(key)
                }}
              >
                {thinkingLevels.map((level) => (
                  <MenuItem
                    key={level}
                    id={level}
                    textValue={THINKING_LEVEL_LABELS[level] ?? level}
                  >
                    {EFFORT_ICONS[level]?.('size-4') ?? <EffortGlyph lit={3} className="size-4" />}
                    <MenuLabel>{THINKING_LEVEL_LABELS[level] ?? level}</MenuLabel>
                    <MenuDescription>{level}</MenuDescription>
                  </MenuItem>
                ))}
              </MenuContent>
            </Menu>

            <Menu>
              <MenuTrigger aria-label="Access" className={chipTriggerClass}>
                <LockOpenIcon className="size-3.5 shrink-0" strokeWidth={1.6} />
                Full access
                <ChevronDownIcon className="size-3 shrink-0 text-muted-fg" strokeWidth={1.8} />
              </MenuTrigger>
              <MenuContent
                placement="top start"
                selectionMode="single"
                selectedKeys={['full']}
                popover={{ className: 'min-w-56' }}
              >
                <MenuSection label="Access">
                  <MenuItem id="full" textValue="Full access">
                    <LockOpenIcon data-slot="icon" />
                    <MenuLabel>Full access</MenuLabel>
                    <MenuDescription>Edit files and run commands</MenuDescription>
                  </MenuItem>
                  <MenuItem id="ask" isDisabled textValue="Ask first">
                    <LockClosedIcon data-slot="icon" />
                    <MenuLabel>Ask first</MenuLabel>
                    <MenuDescription>Not available yet</MenuDescription>
                  </MenuItem>
                  <MenuItem id="read" isDisabled textValue="Read only">
                    <EyeIcon data-slot="icon" />
                    <MenuLabel>Read only</MenuLabel>
                    <MenuDescription>Not available yet</MenuDescription>
                  </MenuItem>
                </MenuSection>
              </MenuContent>
            </Menu>

            <button
              type="submit"
              aria-label={isStreaming ? 'Stop' : 'Send'}
              className={twMerge(
                'ml-auto flex size-6.5 shrink-0 cursor-pointer items-center justify-center rounded-full transition-colors duration-150',
                isStreaming
                  ? 'bg-secondary text-fg hover:bg-muted'
                  : canSend
                    ? 'bg-fg text-bg hover:bg-fg/80'
                    : 'cursor-default bg-secondary text-muted-fg/60',
              )}
            >
              {isStreaming ? (
                <StopIcon className="size-2.5 fill-current" />
              ) : (
                <ArrowUpIcon className="size-3.5" strokeWidth={2} />
              )}
            </button>
          </div>
        </div>

        <div className="mt-3 flex items-center gap-5 px-4 text-[11px] text-muted-fg">
          <span className="flex items-center gap-1.5">
            <FolderIcon className="size-3.5" strokeWidth={1.6} />
            {projectName}
          </span>
          <span className="flex items-center gap-1.5">
            <ComputerDesktopIcon className="size-3.5" strokeWidth={1.6} />
            Local
          </span>
          <span className="flex items-center gap-1.5">
            <BranchIcon className="size-3.5" />
            main
          </span>
          <span
            aria-label={connected ? 'Agent connected' : 'Agent disconnected'}
            title={connected ? 'Agent connected' : 'Agent disconnected'}
            className={twMerge(
              'ml-auto size-2.5 rounded-full border-[1.5px] transition-colors',
              connected ? 'border-success-subtle-fg' : 'border-input',
            )}
          />
        </div>
      </form>
    </div>
  )
}

/* ------------------------------------------------------------------ */
/* Empty state                                                         */
/* ------------------------------------------------------------------ */

function EmptyState({ projectName }: { projectName: string }) {
  return (
    <div className="relative flex flex-1 flex-col items-center justify-center overflow-hidden pb-12">
      <Asterisk className="relative size-4 motion-safe:animate-rise" />
      <h2 className="relative mt-6 font-display text-2xl font-semibold tracking-[-0.02em] text-balance text-fg motion-safe:animate-rise motion-safe:[animation-delay:120ms]">
        What should we build in{' '}
        <span className="inline-flex translate-y-[3px] items-center gap-1.5 rounded-lg bg-secondary px-2.5 py-1 text-[20px] text-fg">
          <FolderIcon className="size-4 text-muted-fg" strokeWidth={1.7} />
          {projectName}
        </span>
        ?
      </h2>
    </div>
  )
}

/* ------------------------------------------------------------------ */
/* Panel                                                               */
/* ------------------------------------------------------------------ */

const projectName = 'orbit'

export default function ChatPanel() {
  const aui = useAui()
  const { messages, isStreaming, connected, model, provider, cancel, setModel, setThinkingLevel } =
    usePiSseChat()
  const models = usePiModels()
  const { readiness } = usePiRuntimeExtras()
  const thinkingLevel = readiness?.state === 'ready' ? 'medium' : 'medium'
  const thinkingLevels = ['off', 'minimal', 'low', 'medium', 'high', 'xhigh', 'max']

  const modelLabel = model ?? provider ?? 'pi'

  // Adapter: daemon turns + parts → AI SDK UIMessage[] (ids stable; list only appends).
  const uiMessages = useMemo<UIMessage[]>(
    () =>
      messages.map((m, i) => {
        const key = `${m.role}-${i}`
        return {
          id: key,
          role: m.role,
          parts: m.parts.flatMap((p, pi) => toUIPart(p, `${key}-${pi}`)),
        }
      }),
    [messages],
  )
  const status: ChatStatus = isStreaming ? 'streaming' : 'ready'

  const handleSend = (text: string) => {
    aui.composer.setText(text)
    aui.composer.send()
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col bg-bg">
      <ChatHeader title="New task" connected={connected} />

      {messages.length === 0 ? (
        <EmptyState projectName={projectName} />
      ) : (
        <AgentMessageList messages={uiMessages} status={status} />
      )}

      <Composer
        modelLabel={modelLabel}
        models={models}
        thinkingLevel={thinkingLevel}
        thinkingLevels={thinkingLevels}
        connected={connected}
        isStreaming={isStreaming}
        onSend={handleSend}
        onAbort={() => void cancel()}
        onSelectModel={(provider, modelId) => void setModel({ provider, modelId })}
        onSelectThinkingLevel={(level) => void setThinkingLevel(level as never)}
      />
    </div>
  )
}
