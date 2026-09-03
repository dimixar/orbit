'use client'

import { CommandLineIcon, Squares2X2Icon } from '@heroicons/react/24/outline'
import { Avatar } from '@/components/ui/avatar'
import { Breadcrumbs, BreadcrumbsItem } from '@/components/ui/breadcrumbs'
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
import { SidebarNav, SidebarTrigger } from '@/components/ui/sidebar'

export default function AppSidebarNav() {
  return (
    <SidebarNav>
      <span className="flex items-center gap-x-4">
        <SidebarTrigger className="-ml-2.5 lg:ml-0" />
        <Breadcrumbs className="hidden md:flex">
          <BreadcrumbsItem href="#">Orbit</BreadcrumbsItem>
          <BreadcrumbsItem>Pi Agent</BreadcrumbsItem>
        </Breadcrumbs>
      </span>
      <UserMenu />
    </SidebarNav>
  )
}

function UserMenu() {
  return (
    <Menu>
      <MenuTrigger className="ml-auto md:hidden" aria-label="Open Menu">
        <Avatar isSquare alt="kurt cobain" src="https://intentui.com/images/avatar/cobain.jpg" />
      </MenuTrigger>
      <MenuContent popover={{ placement: 'bottom end' }} className="min-w-64">
        <MenuSection>
          <MenuHeader separator>
            <span className="block">Orbit</span>
            <span className="font-normal text-muted-fg">pi agent</span>
          </MenuHeader>
        </MenuSection>
        <MenuItem href="#dashboard">
          <Squares2X2Icon />
          <MenuLabel>Dashboard</MenuLabel>
        </MenuItem>
        <MenuItem
          onAction={() => window.open('https://pi.dev/docs/latest/sdk', '_blank')}
        >
          <CommandLineIcon />
          <MenuLabel>Pi SDK Docs</MenuLabel>
        </MenuItem>
        <MenuSeparator />
        <MenuItem>
          <MenuLabel>Contact Support</MenuLabel>
        </MenuItem>
      </MenuContent>
    </Menu>
  )
}