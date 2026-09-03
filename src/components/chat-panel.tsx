'use client'

import { useEffect, useRef, useState } from 'react'
import {
  ArrowUpIcon,
  ChevronDownIcon,
  ComputerDesktopIcon,
  FolderIcon,
  InformationCircleIcon,
  LockOpenIcon,
  StopIcon,
} from '@heroicons/react/24/outline'
import { twMerge } from 'tailwind-merge'
import type { PiAgentState, PiSessionGroup } from '@/lib/pi-agent'

export interface ChatMessage {
  role: 'user' | 'assistant'
  text: string
}

interface ChatPanelProps {
  agentState: PiAgentState
  messages: ChatMessage[]
  sessionGroups: PiSessionGroup[]
  activeSessionPath?: string
  onSend: (text: string) => void
  onAbort: () => void
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
      className={twMerge('text-[#e0785a]', className)}
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
function PiTile({ className, tint = '#1a1a1a' }: { className?: string; tint?: string }) {
  return (
    <span
      aria-hidden="true"
      className={twMerge(
        'flex size-3.5 shrink-0 items-center justify-center rounded-[3px] bg-[#e8e8e8]',
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
        'flex size-7 cursor-default items-center justify-center rounded-lg text-muted-fg transition-colors duration-100 hover:bg-[#2a2a2a] hover:text-fg',
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

function ChatHeader({ title }: { title: string }) {
  return (
    <header className="flex h-12 shrink-0 items-center justify-between pl-4 pr-3.5">
      <h1 className="truncate text-[13px] font-medium tracking-[-0.01em] text-fg">
        {title}
      </h1>

      <div className="flex shrink-0 items-center gap-4">
        <span className="flex items-center gap-2 text-xs font-medium tabular-nums">
          <span className="text-[#62c987]">+3040</span>
          <span className="text-[#e2726a]">-2225</span>
        </span>

        <div className="flex items-center gap-2">
          <button
            type="button"
            aria-label="Preview"
            title="Preview"
            className="flex h-7 w-12 cursor-default items-stretch overflow-hidden rounded-lg border border-[#303030] transition-colors duration-100 hover:bg-[#2a2a2a]"
          >
            <span className="flex flex-1 items-center justify-center">
              <PiTile tint="#4d9fec" className="size-[15px] rounded-[4px]" />
            </span>
            <span className="my-auto h-4 w-px bg-[#303030]" />
            <span className="flex flex-1 items-center justify-center text-muted-fg">
              <ChevronDownIcon className="size-3" strokeWidth={1.8} />
            </span>
          </button>

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

function Composer({
  model,
  connected,
  isStreaming,
  onSend,
  onAbort,
}: {
  model: string
  connected: boolean
  isStreaming: boolean
  onSend: (text: string) => void
  onAbort: () => void
}) {
  const [draft, setDraft] = useState('')
  const taRef = useRef<HTMLTextAreaElement>(null)
  const canSend = connected && draft.trim().length > 0

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
    if (!text || !connected) return
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
        <div className="rounded-[10px] border border-[#2f2f2f] bg-[#212121] px-4 pt-3.5 pb-2.5 shadow-[0_8px_24px_rgba(0,0,0,0.35)]">
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
            className="block w-full resize-none bg-transparent text-[13px] leading-[18px] text-fg outline-none placeholder:text-[#6b6b6b]"
          />

          <div className="mt-3 flex items-center gap-x-5">
            <span className="flex min-w-0 items-center gap-1.5">
              <PiTile />
              <span className="truncate text-xs text-[#b3b3b3]">{model}</span>
            </span>
            <span className="shrink-0 text-xs text-muted-fg">Medium</span>
            <span className="flex shrink-0 items-center gap-1.5 text-xs text-muted-fg">
              <LockOpenIcon className="size-3.5" strokeWidth={1.6} />
              Full access
            </span>

            <button
              type="submit"
              aria-label={isStreaming ? 'Stop' : 'Send'}
              className={twMerge(
                'ml-auto flex size-6.5 shrink-0 items-center justify-center rounded-full transition-colors duration-150',
                isStreaming
                  ? 'bg-[#323232] text-fg hover:bg-[#3d3d3d]'
                  : canSend
                    ? 'bg-fg text-bg hover:bg-[#c9c9c9]'
                    : 'cursor-default bg-[#323232] text-[#545454]',
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
            imeichcek
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
              connected ? 'border-[#62c987]' : 'border-[#3f3f3f]',
            )}
          />
        </div>
      </form>
    </div>
  )
}

/* ------------------------------------------------------------------ */
/* Messages / empty state                                              */
/* ------------------------------------------------------------------ */

function EmptyState() {
  return (
    <div className="flex flex-1 flex-col items-center justify-center pb-12">
      <Asterisk className="size-4" />
      <h2 className="mt-5 text-xl font-semibold tracking-[-0.015em] text-fg">
        What should we build in imeichcek?
      </h2>
    </div>
  )
}

function MessageList({
  messages,
  isStreaming,
}: {
  messages: ChatMessage[]
  isStreaming: boolean
}) {
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    ref.current?.scrollTo({ top: ref.current.scrollHeight })
  }, [messages, isStreaming])

  const waiting = isStreaming && messages[messages.length - 1]?.role === 'user'

  return (
    <div ref={ref} className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
      <div className="mx-auto flex max-w-[720px] flex-col gap-4">
        {messages.map((msg, i) =>
          msg.role === 'user' ? (
            <div
              key={i}
              className="ml-auto max-w-[80%] whitespace-pre-wrap rounded-lg bg-[#2a2a2a] px-3.5 py-2.5 text-[13px] leading-relaxed text-fg"
            >
              {msg.text}
            </div>
          ) : (
            <div
              key={i}
              className="max-w-[85%] whitespace-pre-wrap text-[13px] leading-relaxed text-fg"
            >
              {msg.text}
            </div>
          ),
        )}
        {waiting && (
          <div className="flex items-center gap-1.5 pl-0.5 text-xs text-muted-fg">
            <span className="size-1.5 animate-pulse rounded-full bg-muted-fg" />
            working…
          </div>
        )}
      </div>
    </div>
  )
}

/* ------------------------------------------------------------------ */
/* Panel                                                               */
/* ------------------------------------------------------------------ */

export default function ChatPanel({
  agentState,
  messages,
  sessionGroups,
  activeSessionPath,
  onSend,
  onAbort,
}: ChatPanelProps) {
  const activeSession = activeSessionPath
    ? sessionGroups
        .flatMap((g) => g.sessions)
        .find((s) => s.path === activeSessionPath)
    : undefined

  const title = activeSession ? (activeSession.name ?? 'Untitled task') : 'New task'
  const model = agentState.model ?? 'glm-5.3-flash:cloud'

  return (
    <div className="flex min-h-0 flex-1 flex-col bg-bg">
      <ChatHeader title={title} />

      {messages.length === 0 ? <EmptyState /> : <MessageList messages={messages} isStreaming={agentState.isStreaming} />}

      <Composer
        model={model}
        connected={agentState.connected}
        isStreaming={agentState.isStreaming}
        onSend={onSend}
        onAbort={onAbort}
      />
    </div>
  )
}
