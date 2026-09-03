'use client'

/**
 * App sidebar — SSE-backed.
 *
 * The chat now runs on the Pi × assistant-ui runtime over SSE
 * (agent/sse-server.ts), so the WebSocket daemon state (sessions, model,
 * thinking) is gone. Connection status and the active model come from
 * `usePiRuntimeExtras`; the thread list can be added from the Pi runtime's
 * thread list when needed.
 */

import {
  CheckIcon,
  ChevronUpDownIcon,
  PlusIcon,
} from '@heroicons/react/20/solid'
import {
  ChartBarIcon,
  ChatBubbleLeftRightIcon,
  ChevronDownIcon,
  ChevronRightIcon,
  Cog6ToothIcon,
  DocumentTextIcon,
  FolderIcon,
  FolderOpenIcon,
  LifebuoyIcon,
  SparklesIcon,
  Square3Stack3DIcon,
} from '@heroicons/react/24/outline'
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
import { useAui, useAuiState } from '@assistant-ui/react'
import { usePiRuntimeExtras } from '@assistant-ui/react-pi'
import { useMemo, useState } from 'react'

export type WorkbenchView = 'chat' | 'chat-demo' | 'usage' | 'skills' | 'plugins' | 'settings'

interface AppSidebarProps extends React.ComponentProps<typeof Sidebar> {
  view?: WorkbenchView
  onNavigate?: (view: WorkbenchView) => void
  onNewChat?: () => void
}

/** Thread item with the custom metadata fields we need for grouping. */
type ThreadItemWithMeta = {
  title?: string
  lastMessageAt?: Date
  custom?: { workspacePath?: string; status?: string }
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

export default function AppSidebar({
  view = 'chat',
  onNavigate,
  onNewChat,
  ...props
}: AppSidebarProps) {
  const aui = useAui()
  const { status, readiness } = usePiRuntimeExtras()
  const connected = readiness?.state === 'ready'
  const model = readiness?.state === 'ready'
    ? `${readiness.selection.provider}/${readiness.selection.modelId}`
    : status

  // Pi thread list (sessions) over SSE.
  const threads = useAuiState((s) => s.threads)
  const threadIds = (threads?.threadIds ?? []) as readonly string[]
  const mainThreadId = threads?.mainThreadId
  const threadItems = (threads?.threadItems ?? []) as unknown as
    | readonly ThreadItemWithMeta[]
    | Record<string, ThreadItemWithMeta>

  const getThreadItem = (id: string, index: number): ThreadItemWithMeta | undefined => {
    if (Array.isArray(threadItems)) return threadItems[index]
    return (threadItems as Record<string, ThreadItemWithMeta>)[id]
  }

  // Group threads by their workspace folder path. Threads without a
  // workspacePath go into an "Other" bucket.
  const { folderGroups, folderOrder } = useMemo(() => {
    const groups = new Map<string, { id: string; item: ThreadItemWithMeta; index: number }[]>()
    const order: string[] = []

    for (let i = 0; i < threadIds.length; i++) {
      const id = threadIds[i]!
      const item = getThreadItem(id, i)
      const folder = item?.custom?.workspacePath ?? 'Other'
      if (!groups.has(folder)) {
        groups.set(folder, [])
        order.push(folder)
      }
      groups.get(folder)!.push({ id, item: item ?? {}, index: i })
    }

    return { folderGroups: groups, folderOrder: order }
  }, [threadIds, threadItems])

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

  /** Shorten a full path to just its last segment for display. */
  const folderLabel = (path: string) => {
    if (path === 'Other') return 'Other'
    const parts = path.replace(/\/+$/, '').split('/')
    return parts[parts.length - 1] || path
  }

  const switchToThread = (id: string) => {
    void aui.threads.switchToThread(id)
  }
  const newChat = () => {
    void aui.threads.switchToNewThread()
    onNewChat?.()
  }

  return (
    <Sidebar {...props}>
      <SidebarHeader>
        <div className="flex w-full items-center gap-x-2">
          <Avatar
            isSquare
            size="sm"
            initials="O"
            alt="Orbit"
            className="bg-primary text-primary-fg outline-hidden"
          />
          <SidebarLabel className="font-medium">
            Orbit <span className="text-muted-fg">Pi</span>
          </SidebarLabel>
          <span className="ms-auto flex items-center gap-x-1.5 text-muted-fg text-xs">
            <span
              className={`size-1.5 rounded-full ${
                connected ? 'bg-success-subtle-fg' : 'bg-warning-subtle-fg'
              }`}
            />
            {connected ? 'SSE' : 'connecting…'}
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
            {threadIds.length === 0 ? (
              <p className="px-6 py-2 text-muted-fg text-sm group-data-[state=collapsed]:hidden">
                No sessions yet
              </p>
            ) : (
              <div className="space-y-px">
                {folderOrder.map((folder) => {
                  const threadsInFolder = folderGroups.get(folder) ?? []
                  const isCollapsed = collapsedFolders.has(folder)
                  const hasActiveThread = threadsInFolder.some(
                    ({ id }) => id === mainThreadId,
                  )

                  return (
                    <div key={folder}>
                      {/* Folder header */}
                      <button
                        type="button"
                        onClick={() => toggleFolder(folder)}
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
                        <span className="shrink-0 tabular-nums text-muted-fg/70">
                          {threadsInFolder.length}
                        </span>
                      </button>

                      {/* Thread items under this folder */}
                      {!isCollapsed && (
                        <div className="ml-3 space-y-px border-l border-border/60 pl-1">
                          {threadsInFolder.map(({ id, item }) => {
                            const title = item?.title ?? 'Untitled task'
                            const isActive = id === mainThreadId
                            return (
                              <button
                                key={id}
                                type="button"
                                onClick={() => switchToThread(id)}
                                className={`flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-sm transition-colors ${
                                  isActive
                                    ? 'bg-sidebar-primary text-sidebar-primary-fg'
                                    : 'text-sidebar-fg hover:bg-sidebar-accent hover:text-sidebar-accent-fg'
                                }`}
                              >
                                <span className="min-w-0 flex-1 truncate">{title}</span>
                                {item?.lastMessageAt && (
                                  <span className="shrink-0 text-xs text-muted-fg">
                                    {timeAgo(item.lastMessageAt)}
                                  </span>
                                )}
                              </button>
                            )
                          })}
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
                                className="flex w-full items-center gap-2 rounded-md bg-sidebar-primary px-2.5 py-1.5 text-left text-sm text-sidebar-primary-fg transition-colors"
                              >
                                <span className="min-w-0 flex-1 truncate">
                                  {item?.title ?? 'Untitled task'}
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
                <SidebarLabel className="max-w-40 truncate font-mono text-xs">
                  {model}
                </SidebarLabel>
                <span className="-mt-0.5 block text-muted-fg">
                  {connected ? 'pi · sse' : 'connecting…'}
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
                <span className="block font-mono text-xs">{model}</span>
                <span className="font-normal text-muted-fg">
                  {connected ? 'connected over SSE' : 'waiting for SSE server…'}
                </span>
              </MenuHeader>
            </MenuSection>

            <MenuSeparator />

            {/* Workbench pages */}
            <MenuSection label="Workbench">
              <MenuItem onAction={() => onNavigate?.('chat-demo')}>
                <ChatBubbleLeftRightIcon />
                <MenuLabel>Chat demo</MenuLabel>
                {view === 'chat-demo' && <CheckIcon />}
              </MenuItem>
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
