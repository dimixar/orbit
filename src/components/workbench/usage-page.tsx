import {
  ArrowPathIcon,
  BoltIcon,
  ChatBubbleLeftRightIcon,
  ChartBarIcon,
  CurrencyDollarIcon,
  InboxIcon,
  SparklesIcon,
} from '@heroicons/react/24/outline'
import { useCallback, useEffect, useId, useMemo, useState } from 'react'
import { Area, AreaChart, Bar, BarChart, CartesianGrid, Label, Line, LineChart, Pie, PieChart } from 'recharts'
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

/** Tooltip row with a color swatch — ties each popover entry to its series/model color. */
function TooltipRow({
  color,
  label,
  value,
}: {
  color?: string
  label: React.ReactNode
  value: React.ReactNode
}) {
  return (
    <div className="flex w-full items-center justify-between gap-6">
      <span className="flex items-center gap-2">
        {color && <span className="size-2.5 shrink-0 rounded-full" style={{ backgroundColor: color }} />}
        <span className="text-muted-fg">{label}</span>
      </span>
      <span className="font-mono font-medium tabular-nums">{value}</span>
    </div>
  )
}

/** One-line caption under a section card's top edge — explains how to read the chart. */
function SectionHint({ children }: { children: React.ReactNode }) {
  return <p className="text-muted-fg mb-3 text-xs leading-5">{children}</p>
}

const statIcon =
  'text-muted-fg size-4 shrink-0 [&>svg]:size-4 [&>svg]:stroke-[1.5]'

function Stat({
  label,
  value,
  sub,
  icon,
  active,
}: {
  label: string
  value: string
  sub?: string
  icon: React.ReactNode
  /** Gives the card a primary-tint accent — used for "Today" when there's activity. */
  active?: boolean
}) {
  return (
    <WorkbenchCard
      className={active ? 'border-primary/40 bg-primary/[0.04] shadow-primary/5' : undefined}
    >
      <p className="flex items-center gap-1.5 text-muted-fg text-xs font-medium uppercase tracking-wider">
        <span className={statIcon}>{icon}</span>
        {label}
      </p>
      <p className="mt-2 font-mono text-2xl font-medium tabular-nums tracking-tight">{value}</p>
      {sub && <p className="mt-1 text-muted-fg text-xs">{sub}</p>}
    </WorkbenchCard>
  )
}

/** Daily token series for the stacked usage chart. */
type UsageDayRow = PiUsageReport['byDay'][number]

/** Static model colors — Tailwind palette literals, independent of light/dark theme. */
const MODEL_COLORS = [
  '#3b82f6', // blue-500
  '#10b981', // emerald-500
  '#f59e0b', // amber-500
  '#f43f5e', // rose-500
  '#8b5cf6', // violet-500
  '#06b6d4', // cyan-500
  '#f97316', // orange-500
  '#ec4899', // pink-500
  '#84cc16', // lime-500
  '#6366f1', // indigo-500
  '#14b8a6', // teal-500
  '#d946ef', // fuchsia-500
] as const

/** CSS-safe key for a model (custom property names can't contain `/`). */
function modelKey(providerModel: string): string {
  return providerModel.replace(/[^a-zA-Z0-9_-]/g, '-')
}

/** Stable per-model color, matching the donut's assignment (recentByModel order). */
function modelColor(providerModel: string, recent: PiUsageReport['recentByModel']): string {
  const idx = recent.findIndex((m) => `${m.provider}/${m.model}` === providerModel)
  return MODEL_COLORS[(idx === -1 ? 0 : idx) % MODEL_COLORS.length]
}

/** Static series colors for the token-type split — Tailwind palette literals, theme-independent. */
const usageChartConfig: ChartConfig = {
  input: { label: 'Input', color: '#3b82f6' }, // blue-500
  output: { label: 'Output', color: '#10b981' }, // emerald-500
  cacheRead: { label: 'Cache read', color: '#8b5cf6' }, // violet-500
  cacheWrite: { label: 'Cache write', color: '#f59e0b' }, // amber-500
}

const dayLabel = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' })

/** Last-N-days token bars: stacked input/output/cache series with Intent UI chart chrome. */
function UsageChart({ days }: { days: PiUsageReport['byDay'] }) {
  const recent = days.slice(-30)
  if (recent.length === 0) return null

  const windowTotal = recent.reduce(
    (s, d) => s + d.input + d.output + d.cacheRead + d.cacheWrite,
    0,
  )
  const busiest = recent.reduce((a, b) =>
    a.input + a.output + a.cacheRead + a.cacheWrite >= b.input + b.output + b.cacheRead + b.cacheWrite ? a : b,
  )

  return (
    <>
      <SectionHint>
        Total tokens per day, split by token type. Busiest day:{' '}
        <span className="text-fg font-medium">{dayLabel.format(new Date(`${busiest.date}T00:00:00`))}</span> with{' '}
        <span className="text-fg font-medium font-mono">
          {fmt(busiest.input + busiest.output + busiest.cacheRead + busiest.cacheWrite)}
        </span>{' '}
        of {fmt(windowTotal)} tokens.
      </SectionHint>
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
                formatter={(value, name, item) => (
                  <TooltipRow
                    color={item?.color}
                    label={(typeof name === 'string' ? usageChartConfig[name]?.label : undefined) ?? name}
                    value={`${fmt(Number(value))} tokens`}
                  />
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
    </>
  )
}

/** One row per model in the today-vs-average comparison chart. */
type ModelCompareRow = {
  label: string // short model name for the axis
  provider: string
  model: string
  today: number
  avg: number // 30-day daily average (total / 30)
  calls: number
}

/** Static series colors — today stands out, the average stays muted. */
const modelCompareConfig: ChartConfig = {
  today: { label: 'Today', color: '#3b82f6' }, // blue-500
  avg: { label: '30-day daily avg', color: '#94a3b8' }, // slate-400
}

/** Area chart in the shadcn "area-chart-05" style — gradient-filled smooth areas per model:
 *  today's tokens against each model's 30-day daily average, on the same single-day scale. */
function ModelCompareArea({ today, recent }: { today: PiUsageReport['todayByModel']; recent: PiUsageReport['recentByModel'] }) {
  const gradientId = useId().replace(/:/g, '')
  const rows = useMemo(() => {
    const map = new Map<string, ModelCompareRow>()
    for (const m of recent) {
      map.set(`${m.provider}/${m.model}`, {
        label: m.model,
        provider: m.provider,
        model: m.model,
        today: 0,
        avg: (m.input + m.output) / 30,
        calls: m.calls,
      })
    }
    for (const m of today) {
      const key = `${m.provider}/${m.model}`
      const row = map.get(key) ?? {
        label: m.model, provider: m.provider, model: m.model, today: 0, avg: 0, calls: 0,
      }
      row.today = m.input + m.output
      map.set(key, row)
    }
    return [...map.values()].sort((a, b) => b.today - a.today || b.avg - a.avg)
  }, [today, recent])

  if (rows.length === 0) {
    return <p className="text-muted-fg py-10 text-center text-sm">No model usage in the last 30 days.</p>
  }

  return (
    <Chart config={modelCompareConfig} data={rows} dataKey="label" containerHeight={320}>
      <AreaChart data={rows} margin={{ left: 4, right: 12, top: 8 }}>
        <defs>
          <linearGradient id={`fillToday-${gradientId}`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="5%" stopColor="var(--color-today)" stopOpacity={0.65} />
            <stop offset="95%" stopColor="var(--color-today)" stopOpacity={0.06} />
          </linearGradient>
          <linearGradient id={`fillAvg-${gradientId}`} x1="0" y1="0" x2="0" y2="1">
            <stop offset="5%" stopColor="var(--color-avg)" stopOpacity={0.4} />
            <stop offset="95%" stopColor="var(--color-avg)" stopOpacity={0.05} />
          </linearGradient>
        </defs>
        <CartesianGrid vertical={false} />
        <XAxis
          dataKey="label"
          tickLine={false}
          axisLine={false}
          tickMargin={8}
          height={58}
          angle={-35}
          textAnchor="end"
          interval={0}
          tickFormatter={(value: string) => (value.length > 15 ? `${value.slice(0, 14)}…` : value)}
        />
        <YAxis tickFormatter={(value: number) => fmt(value)} width={52} />
        <ChartTooltip
          cursor={{ stroke: 'var(--border)', strokeWidth: 1 }}
          content={
            <ChartTooltipContent
              labelFormatter={(_, payload) => {
                const row = payload?.[0]?.payload as ModelCompareRow | undefined
                if (!row) return null
                return (
                  <span>
                    {row.model}
                    <span className="text-muted-fg"> · {row.provider}</span>
                  </span>
                )
              }}
              formatter={(value, name, item) => {
                const row = item?.payload as ModelCompareRow | undefined
                const delta =
                  row && row.avg > 0 && typeof name === 'string' && name === 'today'
                    ? ` · ${row.today >= row.avg ? '+' : ''}${(((row.today - row.avg) / row.avg) * 100).toFixed(0)}% vs avg`
                    : ''
                return (
                  <TooltipRow
                    color={item?.color}
                    label={(typeof name === 'string' ? modelCompareConfig[name]?.label : undefined) ?? name}
                    value={`${fmt(Number(value))} tokens${delta}`}
                  />
                )
              }}
            />
          }
        />
        <ChartLegend content={<ChartLegendContent />} />
        <Area
          dataKey="avg"
          type="monotone"
          stroke="var(--color-avg)"
          fill={`url(#fillAvg-${gradientId})`}
          strokeWidth={2}
          activeDot={{ r: 3 }}
        />
        <Area
          dataKey="today"
          type="monotone"
          stroke="var(--color-today)"
          fill={`url(#fillToday-${gradientId})`}
          strokeWidth={2}
          activeDot={{ r: 4 }}
        />
      </AreaChart>
    </Chart>
  )
}

/** Stacked bars per day, one series per model — which model burned tokens on which day. */
type ModelDayRow = { date: string; [modelKey: string]: string | number }

function ModelDayChart({
  slices,
  recent,
}: {
  slices: PiUsageReport['byDayByModel']
  recent: PiUsageReport['recentByModel']
}) {
  const { rows, config, keys } = useMemo(() => {
    // Model display names + stable color order (matches the donut).
    const names = new Map<string, string>()
    for (const m of recent) names.set(`${m.provider}/${m.model}`, m.model)
    for (const md of slices) names.set(`${md.provider}/${md.model}`, md.model)

    // One row per day with a column per model.
    const dayMap = new Map<string, ModelDayRow>()
    for (const md of slices) {
      const row = dayMap.get(md.date) ?? { date: md.date }
      const key = modelKey(`${md.provider}/${md.model}`)
      row[key] = (Number(row[key]) || 0) + md.input + md.output
      dayMap.set(md.date, row)
    }
    const rows = [...dayMap.values()].sort((a, b) => a.date.localeCompare(b.date))
    const config: ChartConfig = Object.fromEntries(
      [...names.entries()].map(([pm, name]) => [modelKey(pm), { label: name, color: modelColor(pm, recent) }]),
    )
    return { rows, config, keys: [...names.keys()].map(modelKey) }
  }, [slices, recent])

  if (rows.length === 0) {
    return <p className="text-muted-fg py-10 text-center text-sm">No model usage in the last 30 days.</p>
  }

  return (
    <Chart config={config} data={rows} dataKey="date" containerHeight={300}>
      <BarChart accessibilityLayer data={rows} barCategoryGap={2}>
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
                const row = payload?.[0]?.payload as ModelDayRow | undefined
                if (!row?.date) return null
                const total = keys.reduce((s, k) => s + (Number(row[k]) || 0), 0)
                const modelCount = keys.filter((k) => (Number(row[k]) || 0) > 0).length
                return (
                  <span>
                    {dayLabel.format(new Date(`${row.date}T00:00:00`))}
                    <span className="text-muted-fg">
                      {' '}
                      — {fmt(total)} tokens · {modelCount} model{modelCount === 1 ? '' : 's'}
                    </span>
                  </span>
                )
              }}
              formatter={(value, name, item) => (
                <TooltipRow
                  color={item?.color}
                  label={(typeof name === 'string' ? config[name]?.label : undefined) ?? name}
                  value={`${fmt(Number(value))} tokens`}
                />
              )}
            />
          }
        />
        <ChartLegend content={<ChartLegendContent />} />
        {keys.map((k) => (
          <Bar
            key={k}
            dataKey={k}
            stackId="models"
            fill={`var(--color-${k})`}
            radius={k === keys[keys.length - 1] ? [3, 3, 0, 0] : undefined}
          />
        ))}
      </BarChart>
    </Chart>
  )
}

/** line-chart-02 style: smooth multi-series lines — daily tokens for the top models. */
function ModelTrendLines({
  slices,
  recent,
}: {
  slices: PiUsageReport['byDayByModel']
  recent: PiUsageReport['recentByModel']
}) {
  const { rows, config, keys } = useMemo(() => {
    // Cap lines at the top 6 models so the chart stays readable.
    const top = recent.slice(0, 6)
    const keys = top.map((m) => modelKey(`${m.provider}/${m.model}`))
    const config: ChartConfig = Object.fromEntries(
      top.map((m) => {
        const pm = `${m.provider}/${m.model}`
        return [modelKey(pm), { label: m.model, color: modelColor(pm, recent) }]
      }),
    )

    // One row per day, zero-filled for every tracked model so lines don't gap.
    const dayMap = new Map<string, ModelDayRow>()
    for (const md of slices) {
      const row = dayMap.get(md.date) ?? { date: md.date, ...Object.fromEntries(keys.map((k) => [k, 0])) }
      const key = modelKey(`${md.provider}/${md.model}`)
      if (key in row) row[key] = (Number(row[key]) || 0) + md.input + md.output
      dayMap.set(md.date, row)
    }
    const rows = [...dayMap.values()].sort((a, b) => a.date.localeCompare(b.date))
    return { rows, config, keys }
  }, [slices, recent])

  if (rows.length === 0) {
    return <p className="text-muted-fg py-10 text-center text-sm">No model usage in the last 30 days.</p>
  }

  return (
    <Chart config={config} data={rows} dataKey="date" containerHeight={300}>
      <LineChart data={rows} margin={{ left: 4, right: 12, top: 8 }}>
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
                const row = payload?.[0]?.payload as ModelDayRow | undefined
                if (!row?.date) return null
                const total = keys.reduce((s, k) => s + (Number(row[k]) || 0), 0)
                return (
                  <span>
                    {dayLabel.format(new Date(`${row.date}T00:00:00`))}
                    <span className="text-muted-fg"> — {fmt(total)} tokens</span>
                  </span>
                )
              }}
              formatter={(value, name, item) => (
                <TooltipRow
                  color={item?.color}
                  label={(typeof name === 'string' ? config[name]?.label : undefined) ?? name}
                  value={`${fmt(Number(value))} tokens`}
                />
              )}
            />
          }
        />
        <ChartLegend content={<ChartLegendContent />} />
        {keys.map((k) => (
          <Line
            key={k}
            dataKey={k}
            type="monotone"
            stroke={`var(--color-${k})`}
            strokeWidth={2}
            dot={false}
            activeDot={{ r: 4 }}
          />
        ))}
      </LineChart>
    </Chart>
  )
}

/** Donut + share list: token share per model over the last 30 days. */
function ModelUsageDonut({ models }: { models: PiUsageReport['recentByModel'] }) {
  const data = useMemo(
    () =>
      models.map((m, i) => {
        const name = `${m.provider}/${m.model}`
        const key = modelKey(name)
        return {
          key,
          name,
          provider: m.provider,
          tokens: m.input + m.output,
          calls: m.calls,
          cost: m.cost,
          fill: `var(--color-${key})`,
          color: MODEL_COLORS[i % MODEL_COLORS.length],
          colorIndex: i,
        }
      }),
    [models],
  )

  const config: ChartConfig = useMemo(
    () => ({
      tokens: { label: 'Tokens' },
      ...Object.fromEntries(
        data.map((d) => [d.key, { label: d.name, color: d.color }]),
      ),
    }),
    [data],
  )

  const totalTokens = useMemo(() => data.reduce((sum, d) => sum + d.tokens, 0), [data])

  return (
    <div className="grid gap-8 lg:grid-cols-[auto_1fr]">
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
                    <TooltipRow
                      color={item?.color}
                      label={config[name as string]?.label ?? name}
                      value={`${fmt(Number(value))} tokens${row?.calls ? ` · ${fmt(row.calls)} calls` : ''}`}
                    />
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

      <ul className="space-y-4 self-center">
        {data.map((d) => {
          const share = totalTokens > 0 ? d.tokens / totalTokens : 0
          return (
            <li key={d.key} className="flex items-center gap-3">
              <span
                className="mt-0.5 size-2.5 shrink-0 rounded-full"
                style={{ backgroundColor: d.color }}
              />
              <div className="min-w-0 flex-1">
                <div className="flex items-baseline justify-between gap-3">
                  <span className="truncate font-medium">{d.name}</span>
                  <span className="font-mono text-sm font-medium tabular-nums">
                    {fmt(d.tokens)}
                    <span className="text-muted-fg ml-1.5 text-xs font-normal">
                      {Math.round(share * 100)}%
                    </span>
                  </span>
                </div>
                <div className="bg-muted mt-1.5 h-1.5 w-full overflow-hidden rounded-full">
                  <div
                    className="h-full rounded-full transition-[width] duration-500"
                    style={{ width: `${Math.max(share * 100, d.tokens > 0 ? 1.5 : 0)}%`, backgroundColor: d.color }}
                  />
                </div>
                <p className="text-muted-fg mt-1 text-xs">
                  {d.provider} · {fmt(d.calls)} calls · {fmtCost(d.cost)}
                </p>
              </div>
            </li>
          )
        })}
      </ul>
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

  const todayByModel = usage?.todayByModel ?? []
  const recentByModel = usage?.recentByModel ?? []
  const todayTokens = todayByModel.reduce((sum, m) => sum + m.input + m.output, 0)
  const todayCalls = todayByModel.reduce((sum, m) => sum + m.calls, 0)
  const last30Tokens = recentByModel.reduce((sum, m) => sum + m.input + m.output, 0)
  const modelCount = new Set([
    ...recentByModel.map((m) => `${m.provider}/${m.model}`),
    ...todayByModel.map((m) => `${m.provider}/${m.model}`),
  ]).size

  const description =
    usage && usage.totalCalls > 0
      ? `${fmt(usage.totalInput + usage.totalOutput)} tokens from ${usage.byModel.length} models across ${fmt(usage.totalSessions)} sessions — aggregated from every pi session on this machine.`
      : 'Token consumption and cost across every pi session on this machine, aggregated from the session record.'

  return (
    <WorkbenchPage
      title="Usage"
      description={description}
      className="max-w-none sm:px-12"
      actions={
        <Button intent="outline" size="sm" onPress={refresh} isDisabled={loading}>
          <ArrowPathIcon data-slot="icon" />
          Refresh
        </Button>
      }
    >
      {loading ? (
        <div>
          <div className="grid grid-cols-2 gap-4 lg:grid-cols-5">
            {Array.from({ length: 5 }).map((_, i) => (
              <Skeleton key={i} className="h-24" />
            ))}
          </div>
          <div className="mt-8 space-y-4">
            <Skeleton className="h-4 w-2/3" />
            <Skeleton className="h-72" />
          </div>
        </div>
      ) : usage === null ? (
        <WorkbenchCard className="py-14 text-center">
          <InboxIcon className="text-muted-fg mx-auto size-8" />
          <p className="mt-3 font-medium">Couldn't load usage</p>
          <p className="mx-auto mt-1.5 max-w-sm text-muted-fg text-sm leading-6">
            The pi agent server isn't reachable. Start it with{' '}
            <code className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">pnpm agent:sse</code>{' '}
            and try again.
          </p>
        </WorkbenchCard>
      ) : usage.totalCalls === 0 ? (
        <WorkbenchCard className="py-14 text-center">
          <ChartBarIcon className="text-muted-fg mx-auto size-8" />
          <p className="mt-3 font-medium">No usage yet</p>
          <p className="mx-auto mt-1.5 max-w-sm text-muted-fg text-sm leading-6">
            Run a few prompts and your token and cost history will appear here.
          </p>
        </WorkbenchCard>
      ) : (
        <>
          <div className="mb-8 grid grid-cols-2 gap-4 lg:grid-cols-5">
            <Stat
              label="Today"
              value={fmt(todayTokens)}
              sub={todayTokens > 0 ? `${fmt(todayCalls)} calls today` : 'no activity yet'}
              icon={<SparklesIcon />}
              active={todayTokens > 0}
            />
            <Stat
              label="Total tokens"
              value={fmt(usage.totalInput + usage.totalOutput)}
              sub={`${fmt(usage.totalInput)} in · ${fmt(usage.totalOutput)} out`}
              icon={<ChartBarIcon />}
            />
            <Stat
              label="Cached"
              value={fmt(usage.totalCacheRead + usage.totalCacheWrite)}
              sub={`${fmt(usage.totalCacheRead)} read · ${fmt(usage.totalCacheWrite)} write`}
              icon={<BoltIcon />}
            />
            <Stat
              label="Sessions"
              value={fmt(usage.totalSessions)}
              sub={`${fmt(usage.totalCalls)} model calls`}
              icon={<ChatBubbleLeftRightIcon />}
            />
            <Stat
              label="Cost"
              value={fmtCost(usage.totalCost)}
              sub={usage.totalCost === 0 ? 'free / local models' : undefined}
              icon={<CurrencyDollarIcon />}
            />
          </div>

          <WorkbenchSection title="Last 30 days">
            <WorkbenchCard className="p-4">
              <UsageChart days={usage.byDay} />
            </WorkbenchCard>
          </WorkbenchSection>

          <WorkbenchSection title="Day by day · last 30 days">
            <WorkbenchCard className="p-4">
              {(usage.byDayByModel?.length ?? 0) === 0 ? (
                <p className="text-muted-fg py-10 text-center text-sm">
                  No model usage in the last 30 days — older activity is below.
                </p>
              ) : (
                <>
                  <SectionHint>
                    The same daily totals split by model — each color is one model. Hover a bar to see
                    which models ran that day and how many tokens each used.
                  </SectionHint>
                  <ModelDayChart slices={usage.byDayByModel} recent={recentByModel} />
                </>
              )}
            </WorkbenchCard>
          </WorkbenchSection>

          <WorkbenchSection title="By model · today vs last 30 days">
            <WorkbenchCard className="p-4">
              {modelCount === 0 ? (
                <p className="text-muted-fg py-10 text-center text-sm">
                  No model usage in the last 30 days — older activity is below.
                </p>
              ) : (
                <>
                  <SectionHint>
                    <span className="text-fg font-medium font-mono">{fmt(last30Tokens)}</span> tokens from{' '}
                    <span className="text-fg font-medium font-mono">{modelCount}</span> model
                    {modelCount === 1 ? '' : 's'} in the last 30 days
                    {todayTokens > 0 && (
                      <>
                        {' '}· <span className="text-fg font-medium font-mono">{fmt(todayTokens)}</span> today
                      </>
                    )}
                    . Blue is today, grey is each model's 30-day daily average — points above the grey
                    line mean a busier-than-usual day for that model.
                  </SectionHint>
                  <ModelCompareArea today={todayByModel} recent={recentByModel} />
                </>
              )}
            </WorkbenchCard>
          </WorkbenchSection>

          <WorkbenchSection title="Token share by model · last 30 days">
            <WorkbenchCard className="p-4">
              {recentByModel.length === 0 ? (
                <p className="text-muted-fg py-10 text-center text-sm">
                  No model usage in the last 30 days — older activity is below.
                </p>
              ) : (
                <>
                  <SectionHint>
                    Daily trend for your top {Math.min(recentByModel.length, 6)} models — each line is one
                    model in its own color. Hover a line for that day's breakdown.
                  </SectionHint>
                  {(usage.byDayByModel?.length ?? 0) > 0 && (
                    <ModelTrendLines slices={usage.byDayByModel} recent={recentByModel} />
                  )}
                  <div className="border-border my-8 border-t" />
                  <SectionHint>
                    Share of the 30-day token total per model — hover the donut for exact figures.
                  </SectionHint>
                  <ModelUsageDonut models={recentByModel} />
                </>
              )}
            </WorkbenchCard>
          </WorkbenchSection>

          <WorkbenchSection title="By model · all time">
            <WorkbenchCard className="p-0">
              {(() => {
                const rows = [...usage.byModel].sort(
                  (a, b) => b.input + b.output - (a.input + a.output),
                )
                const max = Math.max(...rows.map((m) => m.input + m.output), 1)
                return (
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="border-border border-b text-left text-muted-fg text-xs uppercase tracking-wider">
                        <th className="px-5 py-3 font-medium">Model</th>
                        <th className="px-3 py-3 text-right font-medium">Tokens</th>
                        <th className="px-3 py-3 text-right font-medium">In</th>
                        <th className="px-3 py-3 text-right font-medium">Out</th>
                        <th className="px-3 py-3 text-right font-medium">Calls</th>
                        <th className="px-5 py-3 text-right font-medium">Cost</th>
                      </tr>
                    </thead>
                    <tbody>
                      {rows.map((m) => {
                        const key = `${m.provider}/${m.model}`
                        const tokens = m.input + m.output
                        return (
                          <tr key={key} className="border-border border-b last:border-0 hover:bg-muted/50">
                            <td className="px-5 py-3">
                              <span className="flex items-center gap-2">
                                <span
                                  className="size-2.5 shrink-0 rounded-full"
                                  style={{ backgroundColor: modelColor(key, recentByModel) }}
                                />
                                <span className="block truncate font-medium">{m.model}</span>
                              </span>
                              <span className="ml-[18px] text-muted-fg text-xs">{m.provider}</span>
                            </td>
                            <td className="px-3 py-3 text-right font-mono tabular-nums">
                              <span className="block">{fmt(tokens)}</span>
                              <span className="bg-muted mt-1 ml-auto block h-1 w-16 max-w-full overflow-hidden rounded-full">
                                <span
                                  className="block h-full rounded-full bg-primary/60"
                                  style={{ width: `${(tokens / max) * 100}%` }}
                                />
                              </span>
                            </td>
                            <td className="px-3 py-3 text-right font-mono tabular-nums">{fmt(m.input)}</td>
                            <td className="px-3 py-3 text-right font-mono tabular-nums">{fmt(m.output)}</td>
                            <td className="px-3 py-3 text-right font-mono tabular-nums">{fmt(m.calls)}</td>
                            <td className="px-5 py-3 text-right font-mono tabular-nums">{fmtCost(m.cost)}</td>
                          </tr>
                        )
                      })}
                    </tbody>
                  </table>
                )
              })()}
            </WorkbenchCard>
          </WorkbenchSection>
        </>
      )}
    </WorkbenchPage>
  )
}