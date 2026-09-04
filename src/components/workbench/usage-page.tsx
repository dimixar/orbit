import { ArrowPathIcon } from '@heroicons/react/24/outline'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { Bar, BarChart, CartesianGrid, Label, Pie, PieChart } from 'recharts'
import { Button } from '@/components/ui/button'
import {
  Chart,
  type ChartConfig,
  ChartLegend,
  ChartLegendContent,
  ChartTooltip,
  ChartTooltipContent,
  XAxis,
  YAxis,
} from '@/components/ui/chart'
import { fetchUsageReport } from '@/lib/pi-client'
import type { PiUsageReport } from '@/lib/pi-agent'
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

/** Daily token series for the stacked usage chart. */
type UsageDayRow = PiUsageReport['byDay'][number]

const CHART_COLOR_CYCLE = ['var(--chart-1)', 'var(--chart-2)', 'var(--chart-3)', 'var(--chart-4)', 'var(--chart-5)']

/** CSS-safe key for a model (custom property names can't contain `/`). */
function modelKey(providerModel: string): string {
  return providerModel.replace(/[^a-zA-Z0-9_-]/g, '-')
}

const usageChartConfig: ChartConfig = {
  input: { label: 'Input', color: 'var(--chart-1)' },
  output: { label: 'Output', color: 'var(--chart-2)' },
  cacheRead: { label: 'Cache read', color: 'var(--chart-3)' },
  cacheWrite: { label: 'Cache write', color: 'var(--chart-4)' },
}

const dayLabel = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' })

/** Last-N-days token bars: stacked input/output/cache series with Intent UI chart chrome. */
function UsageChart({ days }: { days: PiUsageReport['byDay'] }) {
  const recent = days.slice(-30)
  if (recent.length === 0) return null

  return (
    <Chart config={usageChartConfig} containerHeight={280}>
      <BarChart accessibilityLayer data={recent} barCategoryGap={2}>
        <CartesianGrid vertical={false} />
        <XAxis
          dataKey="date"
          tickLine={false}
          tickMargin={10}
          axisLine={false}
          minTickGap={28}
          tickFormatter={(value: string) => dayLabel.format(new Date(`${value}T00:00:00`))}
        />
        <YAxis tickFormatter={(value: number) => fmt(value)} width={52} />
        <ChartTooltip
          content={
            <ChartTooltipContent
              labelFormatter={(_, payload) => {
                const row = payload?.[0]?.payload as UsageDayRow | undefined
                if (!row?.date) return null
                const total = row.input + row.output + row.cacheRead + row.cacheWrite
                return (
                  <span>
                    {dayLabel.format(new Date(`${row.date}T00:00:00`))}
                    <span className="text-muted-fg"> — {fmt(total)} tokens</span>
                  </span>
                )
              }}
              formatter={(value, name) => (
                <div className="flex w-full items-center justify-between gap-6">
                  <span className="text-muted-fg">
                    {(typeof name === 'string' ? usageChartConfig[name]?.label : undefined) ?? name}
                  </span>
                  <span className="font-mono font-medium tabular-nums">{fmt(Number(value))} tokens</span>
                </div>
              )}
            />
          }
        />
        <ChartLegend content={<ChartLegendContent />} />
        <Bar dataKey="input" stackId="tokens" fill="var(--color-input)" />
        <Bar dataKey="output" stackId="tokens" fill="var(--color-output)" />
        <Bar dataKey="cacheRead" stackId="tokens" fill="var(--color-cacheRead)" />
        <Bar dataKey="cacheWrite" stackId="tokens" fill="var(--color-cacheWrite)" radius={[3, 3, 0, 0]} />
      </BarChart>
    </Chart>
  )
}

/** Donut of token share per model over the last 30 days. */
function ModelUsageDonut({ models }: { models: PiUsageReport['recentByModel'] }) {
  const data = useMemo(
    () =>
      models.map((m, i) => {
        const name = `${m.provider}/${m.model}`
        const key = modelKey(name)
        return {
          key,
          name,
          tokens: m.input + m.output,
          calls: m.calls,
          cost: m.cost,
          fill: `var(--color-${key})`,
          colorIndex: i,
        }
      }),
    [models],
  )

  const config: ChartConfig = useMemo(
    () => ({
      tokens: { label: 'Tokens' },
      ...Object.fromEntries(
        data.map((d) => [d.key, { label: d.name, color: CHART_COLOR_CYCLE[d.colorIndex % CHART_COLOR_CYCLE.length] }]),
      ),
    }),
    [data],
  )

  const totalTokens = useMemo(() => data.reduce((sum, d) => sum + d.tokens, 0), [data])

  return (
    <div className="grid gap-6 lg:grid-cols-[auto_1fr]">
      <Chart config={config} className="mx-auto aspect-square max-h-[260px]">
        <PieChart>
          <ChartTooltip
            cursor={false}
            content={
              <ChartTooltipContent
                hideLabel
                formatter={(value, name, item) => {
                  const row = item?.payload as { calls?: number; cost?: number } | undefined
                  return (
                    <div className="flex w-full items-center justify-between gap-6">
                      <span className="text-muted-fg">{config[name as string]?.label ?? name}</span>
                      <span className="font-mono font-medium tabular-nums">
                        {fmt(Number(value))} tokens{row?.calls ? ` · ${fmt(row.calls)} calls` : ''}
                      </span>
                    </div>
                  )
                }}
              />
            }
          />
          <Pie data={data} dataKey="tokens" nameKey="name" innerRadius={62} strokeWidth={5}>
            <Label
              content={({ viewBox }) => {
                if (viewBox && 'cx' in viewBox && 'cy' in viewBox) {
                  return (
                    <text x={viewBox.cx} y={viewBox.cy} textAnchor="middle" dominantBaseline="middle">
                      <tspan x={viewBox.cx} y={viewBox.cy} className="fill-fg font-bold text-2xl">
                        {fmt(totalTokens)}
                      </tspan>
                      <tspan x={viewBox.cx} y={(viewBox.cy || 0) + 22} className="fill-muted-fg">
                        tokens
                      </tspan>
                    </text>
                  )
                }
              }}
            />
          </Pie>
        </PieChart>
      </Chart>

      <table className="self-center text-sm">
        <thead>
          <tr className="border-border border-b text-left text-muted-fg text-xs uppercase tracking-wider">
            <th className="py-2 pr-4 font-medium">Model</th>
            <th className="py-2 pr-4 text-right font-medium">Tokens</th>
            <th className="py-2 pr-4 text-right font-medium">Calls</th>
            <th className="py-2 text-right font-medium">Cost</th>
          </tr>
        </thead>
        <tbody>
          {models.map((m) => (
            <tr key={`${m.provider}/${m.model}`}>
              <td className="py-2 pr-4">
                <span className="flex items-center gap-2">
                  <span
                    className="size-2.5 shrink-0 rounded-full"
                    style={{ backgroundColor: CHART_COLOR_CYCLE[models.indexOf(m) % CHART_COLOR_CYCLE.length] }}
                  />
                  <span className="block truncate font-medium">{m.model}</span>
                </span>
                <span className="ml-[18px] text-muted-fg text-xs">{m.provider}</span>
              </td>
              <td className="py-2 pr-4 text-right font-mono tabular-nums">{fmt(m.input + m.output)}</td>
              <td className="py-2 pr-4 text-right font-mono tabular-nums">{fmt(m.calls)}</td>
              <td className="py-2 text-right font-mono tabular-nums">{fmtCost(m.cost)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

export function UsagePage() {
  const [usage, setUsage] = useState<PiUsageReport | null>(null)
  const [loading, setLoading] = useState(true)

  const load = useCallback(async () => {
    setLoading(true)
    const report = await fetchUsageReport()
    setUsage(report)
    setLoading(false)
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const refresh = () => void load()

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
      ) : usage === null ? (
        <WorkbenchCard className="py-14 text-center">
          <p className="font-medium">Couldn't load usage</p>
          <p className="mx-auto mt-1.5 max-w-sm text-muted-fg text-sm leading-6">
            The pi agent server isn't reachable. Start it with{' '}
            <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">pnpm agent:sse</code>{' '}
            and try again.
          </p>
        </WorkbenchCard>
      ) : usage.totalCalls === 0 ? (
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

          <WorkbenchSection title="By model · last 30 days">
            <WorkbenchCard className="p-4">
              {(usage.recentByModel?.length ?? 0) === 0 ? (
                <p className="text-muted-fg py-10 text-center text-sm">
                  No model usage in the last 30 days — older activity is below.
                </p>
              ) : (
                <ModelUsageDonut models={usage.recentByModel} />
              )}
            </WorkbenchCard>
          </WorkbenchSection>

          <WorkbenchSection title="By model · all time">
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
