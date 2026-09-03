import { MagnifyingGlassIcon, SparklesIcon } from '@heroicons/react/20/solid'
import { useEffect, useMemo, useState } from 'react'
import { Input, InputGroup } from '@/components/ui/input'
import { piAgent, type PiSkillInfo } from '@/lib/pi-agent'
import { Skeleton, WorkbenchCard, WorkbenchPage } from './page'

function SkillBadge({ children }: { children: React.ReactNode }) {
  return (
    <span className="inline-flex items-center rounded-md bg-primary-subtle px-2 py-0.5 font-mono text-primary-subtle-fg text-xs">
      {children}
    </span>
  )
}

export function SkillsPage() {
  const [skills, setSkills] = useState<PiSkillInfo[] | null>(null)
  const [query, setQuery] = useState('')

  useEffect(() => {
    const off = piAgent.on('skills', ({ skills }) => setSkills(skills))
    piAgent.requestSkills()
    return off
  }, [])

  const filtered = useMemo(() => {
    if (!skills) return []
    const q = query.trim().toLowerCase()
    if (!q) return skills
    return skills.filter(
      (s) =>
        s.name.toLowerCase().includes(q) ||
        s.description.toLowerCase().includes(q) ||
        s.triggers.some((t) => t.toLowerCase().includes(q)),
    )
  }, [skills, query])

  const loading = skills === null

  return (
    <WorkbenchPage
      title="Skills"
      description="Capability packages pi loads on demand — instructions and tools that extend what the agent can do. Read from ~/.pi/agent/skills."
      actions={
        <div className="w-52">
          <InputGroup>
            <MagnifyingGlassIcon data-slot="icon" />
            <Input
              aria-label="Filter skills"
              placeholder="Filter skills…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </InputGroup>
        </div>
      }
    >
      {loading ? (
        <div className="space-y-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <Skeleton key={i} className="h-28" />
          ))}
        </div>
      ) : filtered.length === 0 ? (
        <WorkbenchCard className="py-14 text-center">
          <SparklesIcon className="mx-auto mb-3 size-8 text-muted-fg" />
          <p className="font-medium">
            {skills.length === 0 ? 'No skills installed' : `No skills match “${query.trim()}”`}
          </p>
          <p className="mx-auto mt-1.5 max-w-sm text-muted-fg text-sm leading-6">
            {skills.length === 0
              ? 'Install pi skills to extend the agent with specialized capabilities. They appear here automatically.'
              : 'Try a different search term.'}
          </p>
        </WorkbenchCard>
      ) : (
        <div className="space-y-3">
          {filtered.map((skill) => (
            <WorkbenchCard key={skill.name} className="transition-colors hover:border-muted-fg/30">
              <div className="flex items-start justify-between gap-4">
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-x-2.5 gap-y-1.5">
                    <h3 className="font-medium font-mono text-sm">{skill.name}</h3>
                    {skill.userInvocable && <SkillBadge>/{skill.name}</SkillBadge>}
                  </div>
                  {skill.description && (
                    <p className="mt-2 text-muted-fg text-sm leading-6">{skill.description}</p>
                  )}
                  {skill.triggers.length > 0 && (
                    <div className="mt-3 flex flex-wrap gap-1.5">
                      {skill.triggers.slice(0, 6).map((t) => (
                        <span
                          key={t}
                          className="rounded border border-border bg-muted px-1.5 py-0.5 text-muted-fg text-xs"
                        >
                          {t}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
              </div>
            </WorkbenchCard>
          ))}
        </div>
      )}
    </WorkbenchPage>
  )
}
