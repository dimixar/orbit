'use client'

/**
 * App sidebar — SSE-backed.
 *
 * Session listing runs over the Pi × assistant-ui runtime on SSE
 * (agent/sse-server.ts). The server merges the supervisor's workspace catalog
 * with cold sessions from `SessionManager.listAll()`, so the sidebar groups
 * sessions across every project by workspace folder.
 *
 * Connection status and the active model come from `usePiRuntimeExtras`; the
 * session list is driven by the aui remote thread list, enriched with server
 * metadata (session name, first-message snippet, timestamps) refetched on a
 * short poll so renames, new messages, and relative times stay fresh.
 *
 * Each workspace folder header carries a hover-revealed new-chat button that
 * creates a session bound to that folder (piClient.createThread) and switches
 * to it, so a chat can be started inside a specific project without first
 * picking the folder from the composer's empty state.
 */

import {
  CheckIcon,
  ChevronUpDownIcon,
  PlusIcon,
} from '@heroicons/react/20/solid'
import {
  ArchiveBoxIcon,
  ArrowPathIcon,
  ChartBarIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  Cog6ToothIcon,
  CubeTransparentIcon,
  DocumentTextIcon,
  EllipsisHorizontalIcon,
  FolderIcon,
  FolderOpenIcon,
  LifebuoyIcon,
  MagnifyingGlassIcon,
  PencilSquareIcon,
  ServerStackIcon,
  SparklesIcon,
  Square3Stack3DIcon,
  TrashIcon,
} from '@heroicons/react/24/outline'
import { twMerge } from 'tailwind-merge'
import { Avatar } from '@/components/ui/avatar'
import {
  Menu,
  MenuContent,
  MenuHeader,
  MenuItem,
  MenuLabel,
  MenuSection,
  MenuSeparator,
  MenuTrigger,
} from '@/components/ui/menu'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarItem,
  SidebarLabel,
  SidebarRail,
  SidebarSection,
  SidebarSectionGroup,
} from '@/components/ui/sidebar'
import { Input, InputGroup } from '@/components/ui/input'
import { useAui, useAuiState } from '@assistant-ui/react'
import { usePiRuntimeExtras } from '@assistant-ui/react-pi'
import { useEffect, useMemo, useRef, useState } from 'react'
import { isMac } from '@/lib/platform'
import { piClient, reloadPiAgent } from '@/lib/pi-client'
import { isSessionRunning } from '@/lib/session-running'

export type WorkbenchView =
  | 'chat'
  | 'usage'
  | 'skills'
  | 'plugins'
  | 'models'
  | 'providers'
  | 'settings'

interface AppSidebarProps extends React.ComponentProps<typeof Sidebar> {
  view?: WorkbenchView
  onNavigate?: (view: WorkbenchView) => void
  onNewChat?: () => void
}

/** Thread item with the custom metadata fields we need for grouping. */
type ThreadItemWithMeta = {
  title?: string
  custom?: { workspacePath?: string; status?: string }
}

/** Server-enriched thread metadata from GET /threads. */
type ThreadMeta = {
  id: string
  title?: string
  firstMessage?: string
  sessionName?: string
  updatedAt?: string
  workspacePath?: string
  status?: 'idle' | 'running' | 'failed'
}

/**
 * Two display lines for a session row: the title, and below it a snippet of
 * the first message. For unnamed sessions the title IS the first message's
 * opening, so the snippet continues where the title left off instead of
 * repeating it. Falls back to the pi-derived title while metadata loads.
 */
/** Word-boundary split point for a max-width title, so it never cuts mid-word. */
function titleSplit(text: string, max: number): number {
  if (text.length <= max) return text.length
  const space = text.slice(0, max).lastIndexOf(' ')
  return space > max * 0.6 ? space : max
}

function sessionLines(
  meta: ThreadMeta | undefined,
  fallbackTitle?: string,
): { title: string; preview?: string } {
  const first = meta?.firstMessage ?? ''
  if (meta?.sessionName) {
    return { title: meta.sessionName, preview: first || undefined }
  }
  if (first) {
    const cut = titleSplit(first, 60)
    const title = (first.slice(0, cut).trimEnd() + (first.length > cut ? '…' : '')).trim()
    const rest = first.slice(cut).trim()
    return { title: title || 'Untitled task', preview: rest || undefined }
  }
  const title = (meta?.title ?? fallbackTitle ?? '').replace(/\s+/g, ' ').trim()
  return { title: title || 'Untitled task' }
}

/** Compact relative time: "now", "20m", "3h", "2d", then "Sep 3" */
function timeAgo(iso: string | Date | undefined): string {
  if (!iso) return ''
  const date = typeof iso === 'string' ? new Date(iso) : iso
  const diff = Date.now() - date.getTime()
  const mins = Math.floor(diff / 60000)
  if (mins < 1) return 'now'
  if (mins < 60) return `${mins}m`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}h`
  const days = Math.floor(hours / 24)
  if (days < 7) return `${days}d`
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

interface SessionRowProps {
  id: string
  item?: ThreadItemWithMeta
  meta?: ThreadMeta
  isActive: boolean
  /** Live extras status for the open thread (`running` while tokens arrive). */
  extrasStatus?: string
  onSwitch: () => void
  /** Called after a rename/archive/delete lands so the server meta refetches. */
  onChanged: () => void
  onHide: (id: string) => void
}

/**
 * One session row: title + preview lines, relative timestamp, live "running"
 * pulse, hover-revealed action menu (rename / archive / delete), inline
 * rename editor, and an inline delete confirmation.
 */
function SessionRow({
  id,
  item,
  meta,
  isActive,
  extrasStatus,
  onSwitch,
  onChanged,
  onHide,
}: SessionRowProps) {
  const aui = useAui()
  const { title, preview } = sessionLines(meta, item?.title)
  const stamp = meta?.updatedAt ? timeAgo(meta.updatedAt) : ''
  const isRunning = isSessionRunning({
    metaStatus: meta?.status,
    itemStatus: item?.custom?.status,
    isActive,
    extrasStatus,
  })

  const [isRenaming, setIsRenaming] = useState(false)
  const [draft, setDraft] = useState('')
  const cancelledRef = useRef(false)
  const [confirmingDelete, setConfirmingDelete] = useState(false)
  const rowRef = useRef<HTMLButtonElement>(null)

  // Repin: when this row becomes active and is scrolled out of view in the
  // sidebar, scroll the nearest scrollable parent so the active session is
  // always visible — without jumping if it's already on-screen.
  useEffect(() => {
    if (!isActive) return
    const el = rowRef.current
    if (!el) return
    const scroller = el.closest<HTMLElement>('[data-slot=sidebar-content]')
    if (!scroller) return
    const elRect = el.getBoundingClientRect()
    const scRect = scroller.getBoundingClientRect()
    if (elRect.top < scRect.top || elRect.bottom > scRect.bottom) {
      el.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
    }
  }, [isActive, id])

  // Escape dismisses the delete confirmation without touching the mouse.
  useEffect(() => {
    if (!confirmingDelete) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setConfirmingDelete(false)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [confirmingDelete])

  const startRename = () => {
    cancelledRef.current = false
    setDraft(title)
    setIsRenaming(true)
  }

  const commitRename = async () => {
    setIsRenaming(false)
    if (cancelledRef.current) return
    const next = draft.trim()
    if (!next || next === title) return
    await aui.threads.item({ id }).rename(next)
    onChanged()
  }

  const archive = async () => {
    await aui.threads.item({ id }).archive()
    onHide(id)
    onChanged()
  }

  const remove = async () => {
    setConfirmingDelete(false)
    await aui.threads.item({ id }).delete()
    onHide(id)
    onChanged()
  }

  if (confirmingDelete) {
    return (
      <div className="flex min-w-0 items-center gap-1 rounded-md bg-danger-subtle px-2 py-1.5">
        <span className="min-w-0 flex-1 truncate text-danger-subtle-fg text-xs">
          Delete “{title}”?
        </span>
        <button
          type="button"
          onClick={() => void remove()}
          className="shrink-0 rounded-md px-1.5 py-0.5 text-danger-subtle-fg text-xs font-medium hover:bg-danger-subtle-fg/10"
        >
          Delete
        </button>
        <button
          type="button"
          onClick={() => setConfirmingDelete(false)}
          className="shrink-0 rounded-md px-1.5 py-0.5 text-danger-subtle-fg/70 text-xs hover:bg-danger-subtle-fg/10"
        >
          Cancel
        </button>
      </div>
    )
  }

  if (isRenaming) {
    return (
      <div className="px-0.5 py-0.5">
        <Input
          autoFocus
          aria-label="Session name"
          value={draft}
          onChange={(e) => setDraft(e.currentTarget.value)}
          onFocus={(e) => e.currentTarget.select()}
          onBlur={() => void commitRename()}
          onKeyDown={(e) => {
            if (e.key === 'Enter') void commitRename()
            if (e.key === 'Escape') {
              cancelledRef.current = true
              setIsRenaming(false)
            }
          }}
          className="px-2 py-1 text-sm"
        />
      </div>
    )
  }

  return (
    <div className="group/row relative min-w-0">
      <button
        ref={rowRef}
        type="button"
        onClick={onSwitch}
        title={title}
        className={`flex w-full min-w-0 flex-col gap-0.5 rounded-md py-1.5 pe-7 ps-2.5 text-left transition-colors ${
          isActive
            ? 'bg-sidebar-primary text-sidebar-primary-fg'
            : 'text-sidebar-fg hover:bg-sidebar-accent hover:text-sidebar-accent-fg'
        }`}
      >
        <span className="flex w-full min-w-0 items-center gap-1.5">
          {isRunning && (
            <span
              className="relative flex size-1.5 shrink-0"
              role="status"
              aria-label="Session running"
            >
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-success-subtle-fg opacity-60" />
              <span className="relative inline-flex size-1.5 rounded-full bg-success-subtle-fg" />
            </span>
          )}
          <span className="min-w-0 flex-1 truncate text-sm">{title}</span>
          {stamp && !isRunning && (
            <span
              className={`shrink-0 text-xs tabular-nums ${
                isActive ? 'text-sidebar-primary-fg/60' : 'text-muted-fg'
              }`}
            >
              {stamp}
            </span>
          )}
        </span>
        {preview && (
          <span
            className={`w-full truncate text-xs ${
              isActive ? 'text-sidebar-primary-fg/60' : 'text-muted-fg'
            }`}
          >
            {preview}
          </span>
        )}
      </button>

      <Menu>
        <MenuTrigger
          aria-label={`Actions for “${title}”`}
          className="absolute end-1 top-1.5 z-10 size-6 items-center justify-center rounded-md text-muted-fg opacity-0 pointer-events-none transition-opacity hover:bg-sidebar-accent-fg/10 hover:text-sidebar-fg focus-visible:ring-2 focus-visible:ring-ring group-hover/row:opacity-100 group-hover/row:pointer-events-auto group-focus-within/row:opacity-100 group-focus-within/row:pointer-events-auto aria-expanded:opacity-100 aria-expanded:pointer-events-auto"
        >
          <EllipsisHorizontalIcon className="size-4" />
        </MenuTrigger>
        <MenuContent placement="bottom end" className="min-w-40">
          <MenuItem onAction={startRename}>
            <PencilSquareIcon />
            <MenuLabel>Rename</MenuLabel>
          </MenuItem>
          <MenuItem onAction={() => void archive()}>
            <ArchiveBoxIcon />
            <MenuLabel>Archive</MenuLabel>
          </MenuItem>
          <MenuSeparator />
          <MenuItem intent="danger" onAction={() => setConfirmingDelete(true)}>
            <TrashIcon />
            <MenuLabel>Delete…</MenuLabel>
          </MenuItem>
        </MenuContent>
      </Menu>
    </div>
  )
}

export default function AppSidebar({
  view = 'chat',
  onNavigate,
  onNewChat,
  ...props
}: AppSidebarProps) {
  const aui = useAui()
  const { status } = usePiRuntimeExtras()
  //
  // SSE connectivity is tracked from the metadata fetch itself — NOT from
  // `usePiRuntimeExtras().readiness`, which only arrives with a thread
  // snapshot. On first load the app sits on a brand-new thread
  // (useNewPiThreadStore → EMPTY_RUNTIME_EXTRAS), so readiness is undefined
  // until the user opens/creates a session; gating the list on it hid the
  // sessions behind “Connecting to agent…” even though /threads worked.
  const [conn, setConn] = useState<'connecting' | 'connected' | 'offline'>(
    'connecting',
  )
  const [reloading, setReloading] = useState(false)
  const connected = conn === 'connected'

  // Pi thread list (sessions) over SSE.
  const threads = useAuiState((s) => s.threads)
  const isLoading = threads?.isLoading ?? true
  const threadIds = (threads?.threadIds ?? []) as readonly string[]
  const mainThreadId = threads?.mainThreadId
  const threadItems = (threads?.threadItems ?? []) as unknown as
    | readonly ThreadItemWithMeta[]
    | Record<string, ThreadItemWithMeta>

  const getThreadItem = (id: string, index: number): ThreadItemWithMeta | undefined => {
    if (Array.isArray(threadItems)) return threadItems[index]
    return (threadItems as Record<string, ThreadItemWithMeta>)[id]
  }

  // Session metadata straight from the SSE server (session name, first-message
  // snippet, timestamps, live status). Aui thread items carry only a trimmed
  // subset, so the two-line session rows need this richer source.
  //
  // Freshness: refetch when the thread list changes (open/new/delete), when a
  // mutation lands (metaVersion), and on a 30s poll (tick — which also drives
  // relative-time re-rendering). The old key missed renames and message-activity
  // updates because threadIds alone don't change then.
  const [threadMeta, setThreadMeta] = useState<ReadonlyMap<string, ThreadMeta>>(new Map())
  const [metaVersion, setMetaVersion] = useState(0)
  const [tick, setTick] = useState(0)
  // Poll doubles as the connectivity probe: 3s while connecting/offline.
  // While the open thread is running, poll every 2s so other rows pick up
  // live status (and drop it) without waiting for the relaxed 30s cadence.
  const catalogRunning = [...threadMeta.values()].some(
    (meta) => meta.status === 'running',
  )
  useEffect(() => {
    const period =
      status === 'running' || catalogRunning ? 2_000 : connected ? 30_000 : 3_000
    const timer = setInterval(() => setTick((v) => v + 1), period)
    return () => clearInterval(timer)
  }, [catalogRunning, connected, status])
  const refreshMeta = () => setMetaVersion((v) => v + 1)

  // The chat panel auto-titles new sessions (first-message snippet → pi
  // session name) and nudges this event, so rows show the new sessionName
  // immediately instead of waiting for the 30s metadata poll.
  useEffect(() => {
    const onThreadsUpdated = () => setMetaVersion((v) => v + 1)
    window.addEventListener('orbit:threads-updated', onThreadsUpdated)
    window.addEventListener('orbit:agent-reloaded', onThreadsUpdated)
    return () => {
      window.removeEventListener('orbit:threads-updated', onThreadsUpdated)
      window.removeEventListener('orbit:agent-reloaded', onThreadsUpdated)
    }
  }, [])

  const reloadAgent = async () => {
    if (reloading) return
    setReloading(true)
    setConn('connecting')
    try {
      await reloadPiAgent()
      refreshMeta()
    } catch (error) {
      console.error('[orbit] failed to reload the pi agent:', error)
      setConn('offline')
    } finally {
      setReloading(false)
    }
  }

  const metaRefreshKey = `${status}:${tick}:${metaVersion}:${threadIds.join('\n')}`
  useEffect(() => {
    let cancelled = false
    piClient
      .listThreads()
      .then((list) => {
        if (cancelled) return
        setConn('connected')
        setThreadMeta(new Map(list.map((meta) => [meta.id, meta as ThreadMeta])))
      })
      .catch(() => {
        // SSE server unreachable — rows fall back to the trimmed thread items.
        if (!cancelled) setConn('offline')
      })
    return () => {
      cancelled = true
    }
  }, [metaRefreshKey])

  // Rows hidden optimistically after archive/delete, until the refetched list
  // (which no longer contains them) confirms.
  const [hiddenIds, setHiddenIds] = useState<ReadonlySet<string>>(new Set())
  const hideRow = (id: string) => {
    setHiddenIds((prev) => new Set(prev).add(id))
  }

  const getMeta = (id: string | undefined): ThreadMeta | undefined =>
    id === undefined ? undefined : threadMeta.get(id)

  // Free-text filter across every visible session (title + preview). While
  // filtering, the per-folder row cap is lifted so all matches are reachable.
  const [filter, setFilter] = useState('')
  const filterQuery = filter.trim().toLowerCase()

  // Group threads by their workspace folder path. Threads without a
  // workspacePath go into an "Other" bucket.
  const { folderGroups, folderOrder } = useMemo(() => {
    const groups = new Map<
      string,
      { id: string; item: ThreadItemWithMeta; meta?: ThreadMeta }[]
    >()
    const order: string[] = []

    for (let i = 0; i < threadIds.length; i++) {
      const id = threadIds[i]!
      if (hiddenIds.has(id)) continue
      const item = getThreadItem(id, i)
      const meta = getMeta(id)
      if (filterQuery) {
        const { title, preview } = sessionLines(meta, item?.title)
        if (!`${title} ${preview ?? ''}`.toLowerCase().includes(filterQuery)) continue
      }
      const folder = meta?.workspacePath ?? item?.custom?.workspacePath ?? 'Other'
      if (!groups.has(folder)) {
        groups.set(folder, [])
        order.push(folder)
      }
      groups.get(folder)!.push({ id, item: item ?? {}, meta })
    }

    return { folderGroups: groups, folderOrder: order }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [threadIds, threadItems, threadMeta, hiddenIds, filterQuery])

  // Track which folders are expanded. Default to all expanded.
  const [collapsedFolders, setCollapsedFolders] = useState<Set<string>>(new Set())
  const toggleFolder = (folder: string) => {
    setCollapsedFolders((prev) => {
      const next = new Set(prev)
      if (next.has(folder)) next.delete(folder)
      else next.add(folder)
      return next
    })
  }

  // Per-folder row cap: big projects render their most recent sessions and
  // hand the tail to a “Show N more” expander, so a 130-session folder can't
  // wall out the rest of the list. Filtering lifts the cap.
  const SESSION_ROW_CAP = 8
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set())
  const toggleShowAll = (folder: string) => {
    setExpandedFolders((prev) => {
      const next = new Set(prev)
      if (next.has(folder)) next.delete(folder)
      else next.add(folder)
      return next
    })
  }

  /** Shorten a full path to just its last segment for display. */
  const folderLabel = (path: string) => {
    if (path === 'Other') return 'Other'
    const parts = path.replace(/\/+$/, '').split('/')
    return parts[parts.length - 1] || path
  }

  // Footer lockup: static "Local" plus the active session's workspace folder
  // (pi thread metadata) — the model name no longer lives here.
  const activeWorkspace =
    getMeta(mainThreadId)?.workspacePath ??
    (mainThreadId
      ? getThreadItem(mainThreadId, threadIds.indexOf(mainThreadId))?.custom
          ?.workspacePath
      : undefined)
  const workspaceName =
    activeWorkspace && activeWorkspace !== 'Other'
      ? folderLabel(activeWorkspace)
      : undefined

  // Switching sessions must also land the user on the chat page — otherwise
  // clicking a session from the sidebar while on Settings (or another page)
  // swaps the thread in the background but leaves the old page on screen.
  const switchToThread = (id: string) => {
    void aui.threads.switchToThread(id)
    onNavigate?.('chat')
  }
  const newChat = () => {
    void aui.threads.switchToNewThread()
    onNewChat?.()
  }

  // Per-workspace new chat: create the session bound to that folder up front
  // (same flow as the chat panel's folder picker) and switch to it, so the
  // empty state shows the right workspace. While the request is in flight the
  // folder's button spins; other folders' buttons are disabled to keep one
  // pending creation at a time.
  const [creatingFolder, setCreatingFolder] = useState<string | null>(null)
  const newChatInWorkspace = async (folder: string) => {
    if (creatingFolder) return
    setCreatingFolder(folder)
    try {
      const snapshot = await piClient.createThread({ workspacePath: folder })
      await aui.threads.switchToThread(snapshot.metadata.id)
      onNewChat?.()
      refreshMeta()
    } catch (error) {
      console.error('[orbit] failed to open a new session in workspace:', error)
      refreshMeta()
    } finally {
      setCreatingFolder(null)
    }
  }

  return (
    <Sidebar {...props}>
      <SidebarHeader
        data-tauri-drag-region
        className={
          // Top strip: no top padding, so the lockup row's center lands on the
          // bar's content line — the traffic lights' line on macOS (Rust
          // centers the lights at y=20 in the 40px row), the TopBar's center
          // (y=24 in its 48px row) elsewhere.
          'p-0 pb-2'
        }
      >
        {/* The row's height always matches the TopBar's so the avatar shares
            one horizontal line with the bar's icons. On macOS the native
            traffic lights overlay this header — pad right of them (they end
            ≈x60; 16px clearance, same as the collapsed TopBar's pl-[76px]).
            pe-3 keeps the status label off the sidebar's right border. */}
        <div
          data-tauri-drag-region
          className={twMerge(
            'flex w-full items-center gap-x-2 pe-3',
            isMac
              ? 'h-10 ps-[76px] group-data-[state=collapsed]:ps-0'
              : 'h-12 ps-3',
          )}
        >
          <Avatar
            isSquare
            size="sm"
            initials="O"
            alt="Orbit"
            className="bg-primary text-primary-fg outline-hidden"
          />
          {/* pe-0: the shared label reserves 24px for hover controls; the
              lockup has none, and dropping it keeps "Orbit Pi" untruncated
              now that the row reserves pe-3 for the status label. */}
          <SidebarLabel className="font-medium pe-0">
            Orbit <span className="text-muted-fg">Pi</span>
          </SidebarLabel>
          <span className="ms-auto flex items-center gap-x-1.5 text-muted-fg text-xs">
            <span
              className={`size-1.5 rounded-full ${
                conn === 'connected'
                  ? 'bg-success-subtle-fg'
                  : conn === 'offline'
                    ? 'bg-danger-subtle-fg'
                    : 'bg-warning-subtle-fg'
              }`}
            />
            {conn === 'connected' ? 'Connected' : conn === 'offline' ? 'Offline' : 'Connecting…'}
          </span>
        </div>
      </SidebarHeader>

      <SidebarContent>
        <SidebarSectionGroup>
          <SidebarSection className="pb-1.5">
            <SidebarItem
              tooltip="New chat"
              isCurrent={view === 'chat' && !mainThreadId}
              onPress={newChat}
            >
              <PlusIcon />
              <SidebarLabel>New chat</SidebarLabel>
            </SidebarItem>
          </SidebarSection>

          <SidebarSection label="Sessions" className="pt-1.5 pb-2">
            {!connected ? (
              <p className="col-span-full px-6 py-2 text-muted-fg text-sm group-data-[state=collapsed]:hidden">
                {conn === 'offline'
                  ? 'Agent offline — start the SSE server'
                  : 'Connecting to agent…'}
              </p>
            ) : isLoading ? (
              <p className="col-span-full px-6 py-2 text-muted-fg text-sm group-data-[state=collapsed]:hidden">
                Loading sessions…
              </p>
            ) : threadIds.length === 0 ? (
              <p className="col-span-full px-6 py-2 text-muted-fg text-sm group-data-[state=collapsed]:hidden">
                No sessions yet
              </p>
            ) : (
              <>
                {/* Session search — filters title + first-message across all projects.
                    col-span-full: the section inner is a 2-col grid (icon+label rows). */}
                <div className="col-span-full px-3 pb-1.5 group-data-[state=collapsed]:hidden">
                  <InputGroup>
                    <MagnifyingGlassIcon data-slot="icon" />
                    <Input
                      aria-label="Search sessions"
                      placeholder="Search sessions"
                      value={filter}
                      onChange={(e) => setFilter(e.currentTarget.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Escape' && filter) {
                          e.stopPropagation()
                          setFilter('')
                        }
                      }}
                      className="h-7 py-1 text-xs"
                    />
                  </InputGroup>
                </div>
                {folderOrder.length === 0 ? (
                  <p className="col-span-full px-6 py-2 text-muted-fg text-sm group-data-[state=collapsed]:hidden">
                    No sessions match “{filter.trim()}”
                  </p>
                ) : (
                // Folders separate generously (space-y-3) while rows inside a
                // folder stay tightly packed (space-y-px), so each workspace
                // reads as one visual group in a long session list.
                <div className="col-span-full space-y-3">
                {folderOrder.map((folder) => {
                  const threadsInFolder = folderGroups.get(folder) ?? []
                  const isCollapsed = collapsedFolders.has(folder)
                  const hasActiveThread = threadsInFolder.some(
                    ({ id }) => id === mainThreadId,
                  )
                  const isFiltering = filterQuery.length > 0
                  const showAll = isFiltering || expandedFolders.has(folder)
                  let visibleThreads = showAll
                    ? threadsInFolder
                    : threadsInFolder.slice(0, SESSION_ROW_CAP)
                  // The active session is never cut off by the cap — swap it
                  // into the visible window if recency pushed it past the end.
                  if (
                    !showAll &&
                    mainThreadId !== undefined &&
                    !visibleThreads.some(({ id }) => id === mainThreadId) &&
                    threadsInFolder.some(({ id }) => id === mainThreadId)
                  ) {
                    const active = threadsInFolder.find(
                      ({ id }) => id === mainThreadId,
                    )!
                    visibleThreads = [...visibleThreads.slice(0, -1), active]
                  }
                  const overflowCount =
                    threadsInFolder.length - visibleThreads.length

                  return (
                    <div key={folder} className="group/header">
                      {/* Folder header row — its own relative wrapper so the
                          hover-revealed plus centers on this row only. Anchoring
                          it to the outer group would center it on the entire
                          (expanded) folder group, i.e. mid-chat-list. */}
                      <div className="relative">
                      <button
                        type="button"
                        onClick={() => toggleFolder(folder)}
                        aria-expanded={!isCollapsed}
                        className="flex w-full items-center gap-1.5 rounded-md px-2 py-1.5 text-left text-xs font-medium text-muted-fg transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-fg group-data-[state=collapsed]:hidden"
                        title={folder === 'Other' ? 'Other' : folder}
                      >
                        {isCollapsed ? (
                          <ChevronRightIcon className="size-3 shrink-0" strokeWidth={2} />
                        ) : (
                          <ChevronDownIcon className="size-3 shrink-0" strokeWidth={2} />
                        )}
                        {isCollapsed ? (
                          <FolderIcon className="size-3.5 shrink-0" strokeWidth={1.8} />
                        ) : (
                          <FolderOpenIcon className="size-3.5 shrink-0" strokeWidth={1.8} />
                        )}
                        <span className="min-w-0 flex-1 truncate">{folderLabel(folder)}</span>
                        {/* Count yields to the new-chat button while hovered or
                            keyboard-focused, so the two never fight for space. */}
                        <span
                          className={`shrink-0 tabular-nums text-muted-fg/70 transition-opacity ${
                            folder === 'Other'
                              ? ''
                              : 'group-hover/header:opacity-0 group-focus-within/header:opacity-0'
                          }`}
                        >
                          {threadsInFolder.length}
                        </span>
                      </button>

                      {/* New chat in this workspace — hover/focus-revealed in the
                          count's spot. Not rendered for “Other” (no path to bind). */}
                      {folder !== 'Other' && (
                        <button
                          type="button"
                          disabled={creatingFolder !== null}
                          onClick={() => void newChatInWorkspace(folder)}
                          title={`New chat in ${folderLabel(folder)}`}
                          aria-label={`New chat in ${folderLabel(folder)}`}
                          className="absolute end-1 top-1/2 z-10 flex size-5 -translate-y-1/2 items-center justify-center rounded-md text-muted-fg opacity-0 pointer-events-none transition-opacity hover:bg-sidebar-accent-fg/10 hover:text-sidebar-fg focus-visible:ring-2 focus-visible:ring-ring focus-visible:opacity-100 focus-visible:pointer-events-auto group-hover/header:opacity-100 group-hover/header:pointer-events-auto group-focus-within/header:opacity-100 group-focus-within/header:pointer-events-auto group-data-[state=collapsed]:hidden disabled:cursor-wait disabled:opacity-100"
                        >
                          {creatingFolder === folder ? (
                            <ArrowPathIcon className="size-3.5 animate-spin" />
                          ) : (
                            <PlusIcon className="size-3.5" strokeWidth={2} />
                          )}
                        </button>
                      )}
                      </div>

                      {/* Thread items under this folder */}
                      {!isCollapsed && visibleThreads.length > 0 && (
                        <div className="ml-3 space-y-px border-l border-border/60 pl-1">
                          {visibleThreads.map(({ id, item, meta }) => (
                            <SessionRow
                              key={id}
                              id={id}
                              item={item}
                              meta={meta}
                              isActive={id === mainThreadId}
                              extrasStatus={status}
                              onSwitch={() => switchToThread(id)}
                              onChanged={refreshMeta}
                              onHide={hideRow}
                            />
                          ))}
                          {overflowCount > 0 && (
                            <button
                              type="button"
                              onClick={() => toggleShowAll(folder)}
                              className="flex w-full items-center gap-1.5 rounded-md px-2.5 py-1 text-left text-muted-fg text-xs transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-fg"
                            >
                              {showAll ? 'Show fewer' : `Show ${overflowCount} more`}
                            </button>
                          )}
                        </div>
                      )}

                      {/* When collapsed, show a single compact row if one is active */}
                      {isCollapsed && hasActiveThread && (
                        <div className="ml-3 space-y-px border-l border-border/60 pl-1">
                          {threadsInFolder
                            .filter(({ id }) => id === mainThreadId)
                            .map(({ id, item }) => (
                              <button
                                key={id}
                                type="button"
                                onClick={() => switchToThread(id)}
                                className="flex w-full min-w-0 items-center gap-2 rounded-md bg-sidebar-primary px-2.5 py-1.5 text-left text-sidebar-primary-fg transition-colors"
                              >
                                <span className="min-w-0 flex-1 truncate text-sm">
                                  {sessionLines(getMeta(id), item?.title).title}
                                </span>
                              </button>
                            ))}
                        </div>
                      )}
                    </div>
                  )
                })}
              </div>
                )}
              </>
            )}
          </SidebarSection>
        </SidebarSectionGroup>
      </SidebarContent>

      <SidebarFooter className="flex flex-row justify-between gap-4 group-data-[state=collapsed]:flex-col">
        <Menu>
          <MenuTrigger className="flex w-full items-center justify-between" aria-label="Agent">
            <div className="flex items-center gap-x-2">
              <Avatar
                isSquare
                size="sm"
                initials="P"
                alt="Pi"
                className="bg-primary text-primary-fg outline-hidden"
              />
              <div className="in-data-[collapsible=dock]:hidden text-sm">
                <SidebarLabel className="max-w-40 truncate">
                  Local
                  {workspaceName && (
                    <span className="font-normal text-muted-fg">
                      {' · '}
                      {workspaceName}
                    </span>
                  )}
                </SidebarLabel>
                <span className="-mt-0.5 block text-muted-fg">
                  {connected
                    ? 'Connected'
                    : conn === 'offline'
                      ? 'Offline'
                      : 'Connecting…'}
                </span>
              </div>
            </div>
            <ChevronUpDownIcon data-slot="chevron" />
          </MenuTrigger>
          <MenuContent
            className="in-data-[sidebar-collapsible=collapsed]:min-w-56 min-w-(--trigger-width)"
            placement="bottom right"
          >
            <MenuSection>
              <MenuHeader separator>
                <span className="block text-xs">
                  Local{workspaceName ? ` \u00b7 ${workspaceName}` : ''}
                </span>
                <span className="font-normal text-muted-fg">
                  {connected
                    ? 'Connected to the pi agent'
                    : conn === 'offline'
                      ? 'SSE server unreachable'
                      : 'Waiting for the pi agent…'}
                </span>
              </MenuHeader>
            </MenuSection>

            <MenuItem
              textValue="Reload agent"
              isDisabled={reloading}
              onAction={() => {
                void reloadAgent()
              }}
            >
              <ArrowPathIcon className={reloading ? 'animate-spin' : undefined} />
              <MenuLabel>{reloading ? 'Reloading…' : 'Reload agent'}</MenuLabel>
            </MenuItem>

            <MenuSeparator />

            {/* Workbench pages */}
            <MenuSection label="Workbench">
              <MenuItem onAction={() => onNavigate?.('usage')}>
                <ChartBarIcon />
                <MenuLabel>Usage</MenuLabel>
                {view === 'usage' && <CheckIcon />}
              </MenuItem>
              <MenuItem onAction={() => onNavigate?.('skills')}>
                <SparklesIcon />
                <MenuLabel>Skills</MenuLabel>
                {view === 'skills' && <CheckIcon />}
              </MenuItem>
              <MenuItem onAction={() => onNavigate?.('plugins')}>
                <Square3Stack3DIcon />
                <MenuLabel>Plugins</MenuLabel>
                {view === 'plugins' && <CheckIcon />}
              </MenuItem>
              <MenuItem onAction={() => onNavigate?.('models')}>
                <CubeTransparentIcon />
                <MenuLabel>Scoped models</MenuLabel>
                {view === 'models' && <CheckIcon />}
              </MenuItem>
              <MenuItem onAction={() => onNavigate?.('providers')}>
                <ServerStackIcon />
                <MenuLabel>Providers</MenuLabel>
                {view === 'providers' && <CheckIcon />}
              </MenuItem>
              <MenuItem onAction={() => onNavigate?.('settings')}>
                <Cog6ToothIcon />
                <MenuLabel>Settings</MenuLabel>
                {view === 'settings' && <CheckIcon />}
              </MenuItem>
            </MenuSection>

            <MenuSeparator />

            <MenuItem
              onAction={() => window.open('https://pi.dev/docs/latest/sdk', '_blank')}
            >
              <DocumentTextIcon />
              Pi SDK docs
            </MenuItem>
            <MenuItem
              onAction={() =>
                window.open('https://github.com/earendil-works/pi/issues', '_blank')
              }
            >
              <LifebuoyIcon />
              Report an issue
            </MenuItem>
          </MenuContent>
        </Menu>
      </SidebarFooter>

      <SidebarRail />
    </Sidebar>
  )
}