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
}: {
  title: string
  description?: string
  actions?: React.ReactNode
  children: React.ReactNode
  className?: string
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
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
}: {
  title: string
  children: React.ReactNode
  className?: string
}) {
  return (
    <section className={twMerge('mb-10', className)}>
      <Heading level={3} className="mb-4 text-muted-fg text-sm font-medium uppercase tracking-wider">
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
