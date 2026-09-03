import { Square3Stack3DIcon } from '@heroicons/react/24/outline'
import { useEffect, useState } from 'react'
import { piAgent, type PiPluginInfo } from '@/lib/pi-agent'
import { Skeleton, WorkbenchCard, WorkbenchPage, WorkbenchSection } from './page'

/** Split "npm:@ollama/pi-web-search" into { registry, name } */
function parsePackage(pkg: string): { registry: string; name: string } {
  const idx = pkg.indexOf(':')
  if (idx === -1) return { registry: 'npm', name: pkg }
  return { registry: pkg.slice(0, idx), name: pkg.slice(idx + 1) }
}

const REGISTRY_STYLES: Record<string, string> = {
  npm: 'bg-danger-subtle text-danger-subtle-fg',
  git: 'bg-info-subtle text-info-subtle-fg',
  local: 'bg-success-subtle text-success-subtle-fg',
}

export function PluginsPage() {
  const [plugins, setPlugins] = useState<PiPluginInfo | null>(null)

  useEffect(() => {
    const off = piAgent.on('plugins', ({ plugins }) => setPlugins(plugins))
    piAgent.requestPlugins()
    return off
  }, [])

  const loading = plugins === null

  return (
    <WorkbenchPage
      title="Plugins"
      description="Pi packages and local extensions installed on this machine. Packages are loaded from settings.json and run inside the agent."
    >
      {loading ? (
        <div className="space-y-4">
          {Array.from({ length: 3 }).map((_, i) => (
            <Skeleton key={i} className="h-24" />
          ))}
        </div>
      ) : (
        <>
          <WorkbenchSection title={`Packages (${plugins.packages.length})`}>
            {plugins.packages.length === 0 ? (
              <WorkbenchCard className="py-14 text-center">
                <Square3Stack3DIcon className="mx-auto mb-3 size-8 text-muted-fg" />
                <p className="font-medium">No packages installed</p>
                <p className="mx-auto mt-1.5 max-w-sm text-muted-fg text-sm leading-6">
                  Install pi packages with{' '}
                  <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">
                    pi install npm:@scope/pkg
                  </code>{' '}
                  to add providers, tools, and skills.
                </p>
              </WorkbenchCard>
            ) : (
              <div className="space-y-2">
                {plugins.packages.map((pkg) => {
                  const { registry, name } = parsePackage(pkg)
                  return (
                    <WorkbenchCard
                      key={pkg}
                      className="flex items-center gap-3 p-4 transition-colors hover:border-muted-fg/30"
                    >
                      <span
                        className={`inline-flex w-14 shrink-0 items-center justify-center rounded-md px-2 py-1 font-mono text-xs font-medium ${
                          REGISTRY_STYLES[registry] ?? 'bg-muted text-muted-fg'
                        }`}
                      >
                        {registry}
                      </span>
                      <span className="truncate font-mono text-sm">{name}</span>
                    </WorkbenchCard>
                  )
                })}
              </div>
            )}
          </WorkbenchSection>

          <WorkbenchSection title="Local extensions">
            <WorkbenchCard>
              {plugins.extensionCount === 0 ? (
                <p className="text-muted-fg text-sm leading-6">
                  No local extensions. Drop TypeScript files into{' '}
                  <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">
                    {plugins.extensionsDir}
                  </code>{' '}
                  and they'll load on the next session.
                </p>
              ) : (
                <p className="text-sm leading-6">
                  <span className="font-mono font-medium">{plugins.extensionCount}</span>{' '}
                  extension{plugins.extensionCount === 1 ? '' : 's'} in{' '}
                  <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">
                    {plugins.extensionsDir}
                  </code>
                </p>
              )}
            </WorkbenchCard>
          </WorkbenchSection>

          {plugins.enabledModels.length > 0 && (
            <WorkbenchSection title={`Enabled models (${plugins.enabledModels.length})`}>
              <WorkbenchCard className="p-0">
                <ul className="divide-y divide-border">
                  {plugins.enabledModels.map((m) => (
                    <li key={m} className="px-5 py-2.5 font-mono text-sm">
                      {m}
                    </li>
                  ))}
                </ul>
              </WorkbenchCard>
            </WorkbenchSection>
          )}
        </>
      )}
    </WorkbenchPage>
  )
}
