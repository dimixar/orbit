'use client'

import {
  ChevronUpDownIcon,
  PlusIcon,
} from '@heroicons/react/20/solid'
import {
  ChatBubbleLeftRightIcon,
  Cog6ToothIcon,
  DocumentTextIcon,
  FolderIcon,
  LifebuoyIcon,
} from '@heroicons/react/24/outline'
import { Avatar } from '@/components/ui/avatar'
import {
  Menu,
  MenuContent,
  MenuHeader,
  MenuItem,
  MenuSection,
  MenuSeparator,
  MenuTrigger,
} from '@/components/ui/menu'
import {
  Sidebar,
  SidebarBadge,
  SidebarContent,
  SidebarDisclosure,
  SidebarDisclosureGroup,
  SidebarDisclosurePanel,
  SidebarDisclosureTrigger,
  SidebarFooter,
  SidebarHeader,
  SidebarItem,
  SidebarLabel,
  SidebarRail,
  SidebarSection,
  SidebarSectionGroup,
} from '@/components/ui/sidebar'
import type { PiAgentState, PiSessionGroup } from '@/lib/pi-agent'

interface AppSidebarProps extends React.ComponentProps<typeof Sidebar> {
  agentState: PiAgentState
  sessionGroups: PiSessionGroup[]
  activeSessionPath?: string
  onNewChat?: () => void
  onOpenSession: (path: string) => void
}

/** Relative time like "20m", "3h", "2d" */
function timeAgo(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime()
  const mins = Math.floor(diff / 60_000)
  if (mins < 1) return 'now'
  if (mins < 60) return `${mins}m`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}h`
  return `${Math.floor(hours / 24)}d`
}

function SessionGroup({
  group,
  index,
  activeSessionPath,
  onOpenSession,
}: {
  group: PiSessionGroup
  index: number
  activeSessionPath?: string
  onOpenSession: (path: string) => void
}) {
  return (
    <SidebarDisclosure id={index} defaultExpanded={index === 0}>
      <SidebarDisclosureTrigger>
        <FolderIcon />
        <SidebarLabel>{group.project}</SidebarLabel>
        <SidebarBadge>{group.sessions.length}</SidebarBadge>
      </SidebarDisclosureTrigger>
      <SidebarDisclosurePanel>
        {group.sessions.map((s) => (
          <SidebarItem
            key={s.path}
            tooltip={s.name ?? s.firstMessage}
            isCurrent={s.path === activeSessionPath}
            onPress={() => onOpenSession(s.path)}
          >
            <SidebarLabel>
              <span className="block truncate">
                {s.name ?? s.firstMessage ?? 'Untitled session'}
              </span>
              <span className="-mt-0.5 flex items-center gap-x-1 text-xs font-normal text-muted-fg">
                <ChatBubbleLeftRightIcon className="size-3" />
                {s.messageCount} messages · {timeAgo(s.modified)}
              </span>
            </SidebarLabel>
          </SidebarItem>
        ))}
      </SidebarDisclosurePanel>
    </SidebarDisclosure>
  )
}

export default function AppSidebar({
  agentState,
  sessionGroups,
  activeSessionPath,
  onNewChat,
  onOpenSession,
  ...props
}: AppSidebarProps) {
  const model = agentState.model ?? 'no model'

  return (
    <Sidebar {...props}>
      <SidebarHeader>
        <div className="flex items-center gap-x-2">
          <Avatar
            isSquare
            size="sm"
            className="outline-hidden"
            src="https://design.intentui.com/logo"
          />
          <SidebarLabel className="font-medium">
            Orbit <span className="text-muted-fg">Pi</span>
          </SidebarLabel>
          <SidebarBadge
            className={agentState.connected ? 'text-success-fg' : 'text-muted-fg'}
          >
            {agentState.connected ? 'online' : 'offline'}
          </SidebarBadge>
        </div>
      </SidebarHeader>
      <SidebarContent>
        <SidebarSectionGroup>
          <SidebarSection label="Chat">
            <SidebarItem tooltip="New chat" onPress={onNewChat}>
              <PlusIcon />
              <SidebarLabel>New chat</SidebarLabel>
            </SidebarItem>

            <SidebarItem tooltip="Current chat" isCurrent={!activeSessionPath}>
              <ChatBubbleLeftRightIcon />
              <SidebarLabel>Current chat</SidebarLabel>
              {agentState.isStreaming && (
                <SidebarBadge>streaming</SidebarBadge>
              )}
            </SidebarItem>
          </SidebarSection>

          <SidebarDisclosureGroup defaultExpandedKeys={[0]}>
            {sessionGroups.map((group, i) => (
              <SessionGroup
                key={group.cwd}
                group={group}
                index={i}
                activeSessionPath={activeSessionPath}
                onOpenSession={onOpenSession}
              />
            ))}
            {sessionGroups.length === 0 && (
              <SidebarDisclosure id={0}>
                <SidebarDisclosureTrigger>
                  <FolderIcon />
                  <SidebarLabel>Projects</SidebarLabel>
                </SidebarDisclosureTrigger>
                <SidebarDisclosurePanel>
                  <SidebarItem tooltip="No sessions yet">
                    <DocumentTextIcon />
                    <SidebarLabel className="font-normal text-muted-fg">
                      No sessions yet
                    </SidebarLabel>
                  </SidebarItem>
                </SidebarDisclosurePanel>
              </SidebarDisclosure>
            )}
          </SidebarDisclosureGroup>
        </SidebarSectionGroup>
      </SidebarContent>

      <SidebarFooter className="flex flex-row justify-between gap-4 group-data-[state=collapsed]:flex-col">
        <Menu>
          <MenuTrigger className="flex w-full items-center justify-between" aria-label="Agent">
            <div className="flex items-center gap-x-2">
              <Avatar
                className="size-8 *:size-8 group-data-[state=collapsed]:size-6 group-data-[state=collapsed]:*:size-6"
                isSquare
                src="https://intentui.com/images/avatar/cobain.jpg"
              />
              <div className="in-data-[collapsible=dock]:hidden text-sm">
                <SidebarLabel className="max-w-40 truncate font-mono text-xs">
                  {model}
                </SidebarLabel>
                <span className="-mt-0.5 block text-muted-fg">pi agent</span>
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
                  session {agentState.sessionId?.slice(0, 8) ?? '—'}
                </span>
              </MenuHeader>
            </MenuSection>

            <MenuItem isDisabled>
              <Cog6ToothIcon />
              Agent settings
            </MenuItem>
            <MenuItem
              onAction={() => window.open('https://pi.dev/docs/latest/sdk', '_blank')}
            >
              <DocumentTextIcon />
              Pi SDK docs
            </MenuItem>
            <MenuSeparator />
            <MenuItem>
              <LifebuoyIcon />
              Support
            </MenuItem>
          </MenuContent>
        </Menu>
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  )
}