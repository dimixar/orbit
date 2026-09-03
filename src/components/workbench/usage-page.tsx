import { ArrowPathIcon } from '@heroicons/react/24/outline'
import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { piAgent, type PiUsageReport } from '@/lib/pi-agent'
import { Skeleton, WorkbenchCard, WorkbenchPage, WorkbenchSection } from './page'

/** Compact number: 1.2k, 3.4M */
function fmt(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
  return String(Math.round(n))
}

function fmtCost(usd: number): string {
  if (usd === 0) return '$0.00'
  if (usd < 0.01) return '<$0.01'
  return `$${usd.toFixed(2)}`
}

function Stat({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <WorkbenchCard className="p-4">
      <p className="text-muted-fg text-xs font-medium uppercase tracking-wider">{label}</p>
      <p className="mt-2 font-mono text-2xl font-medium tabular-nums tracking-tight">{value}</p>
      {sub && <p className="mt-1 text-muted-fg text-xs">{sub}</p>}
    </WorkbenchCard>
  )
}

/** Last-N-days token bar series. Pure SVG, animates on mount. */
function UsageChart({ days }: { days: PiUsageReport['byDay'] }) {
  const recent = days.slice(-30)
  if (recent.length === 0) return null
  const max = Math.max(...recent.map((d) => d.input + d.output + d.cacheRead + d.cacheWrite), 1)
  const W = 720
  const H = 140
  const gap = 3
  const barW = Math.max((W - gap * (recent.length - 1)) / recent.length, 2)

  return (
    <div className="overflow-x-auto">
      <svg
        viewBox={`0 0 ${W} ${H}`}
        className="h-36 w-full min-w-[420px]"
        role="img"
        aria-label="Tokens used per day for the last 30 days"
      >
        {recent.map((d, i) => {
          const total = d.input + d.output + d.cacheRead + d.cacheWrite
          const h = Math.max((total / max) * (H - 24), 1)
          const x = i * (barW + gap)
          const y = H - 16 - h
          return (
            <g key={d.date}>
              <rect
                x={x}
                y={y}
                width={barW}
                height={h}
                rx={1.5}
                className="fill-primary/80 transition-all duration-300 hover:fill-primary"
              >
                <title>{`${d.date} — ${fmt(total)} tokens`}</title>
              </rect>
              {recent.length <= 15 && (
                <text
                  x={x + barW / 2}
                  y={H - 3}
                  textAnchor="middle"
                  className="fill-muted-fg text-[9px]"
                >
                  {d.date.slice(8)}
                </text>
              )}
            </g>
          )
        })}
      </svg>
    </div>
  )
}

export function UsagePage() {
  const [usage, setUsage] = useState<PiUsageReport | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    const off = piAgent.on('usage', ({ usage }) => {
      setUsage(usage)
      setLoading(false)
    })
    piAgent.requestUsage()
    return off
  }, [])

  const refresh = () => {
    setLoading(true)
    piAgent.requestUsage()
  }

  return (
    <WorkbenchPage
      title="Usage"
      description="Token consumption and cost across every pi session on this machine, aggregated from the session record."
      actions={
        <Button intent="outline" size="sm" onPress={refresh} isDisabled={loading}>
          <ArrowPathIcon data-slot="icon" />
          Refresh
        </Button>
      }
    >
      {loading ? (
        <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <Skeleton key={i} className="h-24" />
          ))}
          <Skeleton className="col-span-full h-44" />
        </div>
      ) : !usage || usage.totalCalls === 0 ? (
        <WorkbenchCard className="py-14 text-center">
          <p className="font-medium">No usage yet</p>
          <p className="mx-auto mt-1.5 max-w-sm text-muted-fg text-sm leading-6">
            Run a few prompts and your token and cost history will appear here.
          </p>
        </WorkbenchCard>
      ) : (
        <>
          <div className="mb-8 grid grid-cols-2 gap-4 lg:grid-cols-4">
            <Stat label="Total tokens" value={fmt(usage.totalInput + usage.totalOutput)} sub={`${fmt(usage.totalInput)} in · ${fmt(usage.totalOutput)} out`} />
            <Stat label="Cached" value={fmt(usage.totalCacheRead + usage.totalCacheWrite)} sub={`${fmt(usage.totalCacheRead)} read · ${fmt(usage.totalCacheWrite)} write`} />
            <Stat label="Sessions" value={fmt(usage.totalSessions)} sub={`${fmt(usage.totalCalls)} model calls`} />
            <Stat label="Cost" value={fmtCost(usage.totalCost)} sub={usage.totalCost === 0 ? 'free / local models' : undefined} />
          </div>

          <WorkbenchSection title="Last 30 days">
            <WorkbenchCard className="p-4">
              <UsageChart days={usage.byDay} />
            </WorkbenchCard>
          </WorkbenchSection>

          <WorkbenchSection title="By model">
            <WorkbenchCard className="p-0">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-border border-b text-left text-muted-fg text-xs uppercase tracking-wider">
                    <th className="px-5 py-3 font-medium">Model</th>
                    <th className="px-3 py-3 text-right font-medium">Calls</th>
                    <th className="px-3 py-3 text-right font-medium">Tokens in</th>
                    <th className="px-3 py-3 text-right font-medium">Tokens out</th>
                    <th className="px-5 py-3 text-right font-medium">Cost</th>
                  </tr>
                </thead>
                <tbody>
                  {usage.byModel.map((m) => (
                    <tr key={`${m.provider}/${m.model}`} className="border-border border-b last:border-0 hover:bg-muted/50">
                      <td className="px-5 py-3">
                        <span className="block truncate font-medium">{m.model}</span>
                        <span className="text-muted-fg text-xs">{m.provider}</span>
                      </td>
                      <td className="px-3 py-3 text-right font-mono tabular-nums">{fmt(m.calls)}</td>
                      <td className="px-3 py-3 text-right font-mono tabular-nums">{fmt(m.input)}</td>
                      <td className="px-3 py-3 text-right font-mono tabular-nums">{fmt(m.output)}</td>
                      <td className="px-5 py-3 text-right font-mono tabular-nums">{fmtCost(m.cost)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </WorkbenchCard>
          </WorkbenchSection>
        </>
      )}
    </WorkbenchPage>
  )
}
