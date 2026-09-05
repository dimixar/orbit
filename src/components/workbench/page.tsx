import type { Ref } from 'react'
import { twMerge } from 'tailwind-merge'
import { Heading } from '@/components/ui/heading'

/**
 * Shared workbench page scaffold: a consistent max-width content column with
 * a title block, optional actions slot, and generous header rhythm. All four
 * workbench pages (Usage, Skills, Plugins, Settings) render inside it, so
 * hierarchy and spacing stay uniform.
 */
export function WorkbenchPage({
  title,
  description,
  actions,
  children,
  className,
  scrollerRef,
}: {
  title: string
  description?: string
  actions?: React.ReactNode
  children: React.ReactNode
  className?: string
  /** The overflow column — pin scrollTop here so in-page layout shifts
   *  (status copy, appended sections) don't jump the viewport. */
  scrollerRef?: Ref<HTMLDivElement>
}) {
  return (
    <div
      ref={scrollerRef}
      className="flex min-h-0 flex-1 flex-col overflow-y-auto overflow-anchor-none"
    >
      <div className={twMerge('mx-auto w-full max-w-3xl px-6 py-8 sm:px-10', className)}>
        <header className="mb-8 flex flex-wrap items-start justify-between gap-x-4 gap-y-3">
          <div className="min-w-0">
            <Heading level={1}>{title}</Heading>
            {description && (
              <p className="mt-1.5 max-w-prose text-muted-fg text-sm leading-6">{description}</p>
            )}
          </div>
          {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
        </header>
        {children}
      </div>
    </div>
  )
}

/** Section heading inside a workbench page. */
export function WorkbenchSection({
  title,
  children,
  className,
  level = 2,
}: {
  title: string
  children: React.ReactNode
  className?: string
  level?: 2 | 3 | 4
}) {
  return (
    <section className={twMerge('mb-10', className)}>
      <Heading level={level} className="mb-4 text-muted-fg text-sm font-medium uppercase tracking-wider">
        {title}
      </Heading>
      {children}
    </section>
  )
}

/** A card-ish container that follows the design system's border/radius tokens. */
export function WorkbenchCard({
  children,
  className,
}: {
  children: React.ReactNode
  className?: string
}) {
  return (
    <div
      className={twMerge(
        'rounded-xl border border-border bg-card p-5 shadow-xs',
        className,
      )}
    >
      {children}
    </div>
  )
}

/** Skeleton line for loading states — content-sized, not a spinner. */
export function Skeleton({ className }: { className?: string }) {
  return <div className={twMerge('animate-pulse rounded-md bg-muted', className)} />
}
