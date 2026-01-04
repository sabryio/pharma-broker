// Performance Analytics View Component
// Displays real performance data from the backend API
// Requirements: 8.4, 8.5

import { useMemo, useState } from 'react'
import {
  Activity,
  AlertTriangle,
  BarChart3,
  Clock,
  Cpu,
  Database,
  RefreshCw,
  Sparkles,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import {
  usePerformanceAnalytics,
  type LatencyStats,
  type SlowStageAlert,
} from '@/hooks/use-audit-records'
import { ProgressBar, Sparkline } from './ui'

// =============================================================================
// Helper Functions
// =============================================================================

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms.toFixed(0)}ms`
  const seconds = ms / 1000
  if (seconds < 60) return `${seconds.toFixed(1)}s`
  const minutes = Math.floor(seconds / 60)
  return `${minutes}m ${Math.floor(seconds % 60)}s`
}

// =============================================================================
// Sub-Components
// =============================================================================

function LatencyStatsCard({
  title,
  stats,
  icon: Icon,
  color = 'teal',
}: {
  title: string
  stats: LatencyStats
  icon: React.ElementType
  color?: 'teal' | 'violet' | 'emerald' | 'amber' | 'red'
}) {
  const colorClasses = {
    teal: 'from-teal-500/20 to-teal-500/5 text-teal-400',
    violet: 'from-violet-500/20 to-violet-500/5 text-violet-400',
    emerald: 'from-emerald-500/20 to-emerald-500/5 text-emerald-400',
    amber: 'from-amber-500/20 to-amber-500/5 text-amber-400',
    red: 'from-red-500/20 to-red-500/5 text-red-400',
  }

  return (
    <div className="p-4 rounded-xl bg-gradient-to-br from-secondary/50 to-secondary/20 border border-border/50">
      <div className="flex items-center gap-2 mb-3">
        <div
          className={cn(
            'w-8 h-8 rounded-lg bg-gradient-to-br flex items-center justify-center',
            colorClasses[color],
          )}
        >
          <Icon className="w-4 h-4" />
        </div>
        <span className="text-sm font-medium text-foreground">{title}</span>
      </div>
      <div className="grid grid-cols-2 gap-2 text-xs">
        <div className="p-2 rounded-lg bg-background/50">
          <p className="text-muted-foreground">Avg</p>
          <p className="text-foreground font-semibold">
            {formatDuration(stats.avgMs)}
          </p>
        </div>
        <div className="p-2 rounded-lg bg-background/50">
          <p className="text-muted-foreground">Median</p>
          <p className="text-foreground font-semibold">
            {formatDuration(stats.medianMs)}
          </p>
        </div>
        <div className="p-2 rounded-lg bg-background/50">
          <p className="text-muted-foreground">P95</p>
          <p className="text-amber-400 font-semibold">
            {formatDuration(stats.p95Ms)}
          </p>
        </div>
        <div className="p-2 rounded-lg bg-background/50">
          <p className="text-muted-foreground">P99</p>
          <p className="text-red-400 font-semibold">
            {formatDuration(stats.p99Ms)}
          </p>
        </div>
      </div>
      <div className="mt-2 text-xs text-muted-foreground">
        {stats.count} samples • Min: {formatDuration(stats.minMs)} • Max:{' '}
        {formatDuration(stats.maxMs)}
      </div>
    </div>
  )
}

function SlowStageAlertCard({ alert }: { alert: SlowStageAlert }) {
  const severityRatio = alert.p95Ms / alert.thresholdMs
  const severity =
    severityRatio > 2 ? 'critical' : severityRatio > 1.5 ? 'warning' : 'info'

  const severityColors = {
    critical: 'border-red-500/50 bg-red-500/10',
    warning: 'border-amber-500/50 bg-amber-500/10',
    info: 'border-blue-500/50 bg-blue-500/10',
  }

  const severityTextColors = {
    critical: 'text-red-400',
    warning: 'text-amber-400',
    info: 'text-blue-400',
  }

  return (
    <div className={cn('p-3 rounded-lg border', severityColors[severity])}>
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-2">
          <AlertTriangle
            className={cn('w-4 h-4', severityTextColors[severity])}
          />
          <span className="text-sm font-medium text-foreground">
            {alert.stage}
          </span>
        </div>
        <span
          className={cn('text-xs font-semibold', severityTextColors[severity])}
        >
          {((alert.p95Ms / alert.thresholdMs) * 100).toFixed(0)}% of threshold
        </span>
      </div>
      <div className="grid grid-cols-3 gap-2 text-xs">
        <div>
          <p className="text-muted-foreground">Avg</p>
          <p className="text-foreground">{formatDuration(alert.avgMs)}</p>
        </div>
        <div>
          <p className="text-muted-foreground">P95</p>
          <p className={severityTextColors[severity]}>
            {formatDuration(alert.p95Ms)}
          </p>
        </div>
        <div>
          <p className="text-muted-foreground">Threshold</p>
          <p className="text-foreground">{formatDuration(alert.thresholdMs)}</p>
        </div>
      </div>
      <div className="mt-2">
        <ProgressBar
          value={alert.p95Ms}
          max={alert.thresholdMs * 2}
          color={
            severity === 'critical'
              ? 'red'
              : severity === 'warning'
                ? 'amber'
                : 'blue'
          }
        />
      </div>
    </div>
  )
}

function StageLatencyTable({
  stageLatencies,
}: {
  stageLatencies: Record<string, LatencyStats>
}) {
  const sortedStages = useMemo(() => {
    return Object.entries(stageLatencies)
      .sort(([, a], [, b]) => b.avgMs - a.avgMs)
      .slice(0, 10)
  }, [stageLatencies])

  if (sortedStages.length === 0) {
    return (
      <div className="text-center py-8 text-muted-foreground">
        No stage latency data available
      </div>
    )
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-border/50">
            <th className="text-left py-2 px-3 text-muted-foreground font-medium">
              Stage
            </th>
            <th className="text-right py-2 px-3 text-muted-foreground font-medium">
              Count
            </th>
            <th className="text-right py-2 px-3 text-muted-foreground font-medium">
              Avg
            </th>
            <th className="text-right py-2 px-3 text-muted-foreground font-medium">
              P95
            </th>
            <th className="text-right py-2 px-3 text-muted-foreground font-medium">
              P99
            </th>
          </tr>
        </thead>
        <tbody>
          {sortedStages.map(([stage, stats]) => (
            <tr
              key={stage}
              className="border-b border-border/30 hover:bg-secondary/30"
            >
              <td className="py-2 px-3 font-medium text-foreground">{stage}</td>
              <td className="py-2 px-3 text-right text-muted-foreground">
                {stats.count}
              </td>
              <td className="py-2 px-3 text-right text-foreground">
                {formatDuration(stats.avgMs)}
              </td>
              <td className="py-2 px-3 text-right text-amber-400">
                {formatDuration(stats.p95Ms)}
              </td>
              <td className="py-2 px-3 text-right text-red-400">
                {formatDuration(stats.p99Ms)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

// =============================================================================
// Main Component
// =============================================================================

export function PerformanceAnalyticsView() {
  const [limit, setLimit] = useState(1000)
  const { data, isLoading, error, refetch, isFetching } =
    usePerformanceAnalytics({ limit })

  // Generate sparkline data from stage latencies
  const sparklineData = useMemo(() => {
    if (!data?.stageLatencies) return []
    return Object.values(data.stageLatencies)
      .map((s) => s.avgMs)
      .slice(0, 20)
  }, [data])

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-20">
        <div className="flex items-center gap-3 text-muted-foreground">
          <RefreshCw className="w-5 h-5 animate-spin" />
          <span>Loading performance analytics...</span>
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className="text-center py-20">
        <AlertTriangle className="w-16 h-16 text-red-400/50 mx-auto mb-4" />
        <h3 className="text-lg font-semibold text-foreground mb-2">
          Failed to Load Analytics
        </h3>
        <p className="text-sm text-muted-foreground mb-4">
          {error instanceof Error ? error.message : 'Unknown error occurred'}
        </p>
        <button
          onClick={() => refetch()}
          className="px-4 py-2 rounded-lg bg-teal-500/20 text-teal-400 hover:bg-teal-500/30 transition-colors"
        >
          Retry
        </button>
      </div>
    )
  }

  if (!data || data.recordsAnalyzed === 0 || !data.overallLatency) {
    return (
      <div className="text-center py-20">
        <BarChart3 className="w-16 h-16 text-muted-foreground/20 mx-auto mb-4" />
        <h3 className="text-lg font-semibold text-foreground mb-2">
          No Analytics Data
        </h3>
        <p className="text-sm text-muted-foreground">
          Performance analytics will appear here once audit records are
          available.
        </p>
      </div>
    )
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-lg font-semibold text-foreground">
            Performance Analytics
          </h3>
          <p className="text-sm text-muted-foreground">
            Analyzing {data.recordsAnalyzed} audit records
          </p>
        </div>
        <div className="flex items-center gap-2">
          <select
            value={limit}
            onChange={(e) => setLimit(Number(e.target.value))}
            className="px-3 py-2 rounded-lg bg-secondary/50 border border-border/50 text-sm text-foreground"
          >
            <option value={100}>Last 100</option>
            <option value={500}>Last 500</option>
            <option value={1000}>Last 1000</option>
          </select>
          <button
            onClick={() => refetch()}
            disabled={isFetching}
            className="p-2 rounded-lg bg-secondary/50 hover:bg-secondary text-muted-foreground hover:text-foreground transition-all disabled:opacity-50"
          >
            <RefreshCw
              className={cn('w-4 h-4', isFetching && 'animate-spin')}
            />
          </button>
        </div>
      </div>

      {/* Overall Latency Stats */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="p-6 rounded-2xl bg-gradient-to-br from-secondary/50 to-secondary/20 border border-border/50">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h4 className="text-sm font-semibold text-foreground">
                Overall Latency
              </h4>
              <p className="text-xs text-muted-foreground">
                End-to-end match processing time
              </p>
            </div>
            <Clock className="w-5 h-5 text-muted-foreground" />
          </div>
          <div className="grid grid-cols-4 gap-3">
            <div className="p-3 rounded-xl bg-background/50 text-center">
              <p className="text-2xl font-bold text-foreground">
                {formatDuration(data.overallLatency.avgMs)}
              </p>
              <p className="text-xs text-muted-foreground">Average</p>
            </div>
            <div className="p-3 rounded-xl bg-background/50 text-center">
              <p className="text-2xl font-bold text-foreground">
                {formatDuration(data.overallLatency.medianMs)}
              </p>
              <p className="text-xs text-muted-foreground">Median</p>
            </div>
            <div className="p-3 rounded-xl bg-background/50 text-center">
              <p className="text-2xl font-bold text-amber-400">
                {formatDuration(data.overallLatency.p95Ms)}
              </p>
              <p className="text-xs text-muted-foreground">P95</p>
            </div>
            <div className="p-3 rounded-xl bg-background/50 text-center">
              <p className="text-2xl font-bold text-red-400">
                {formatDuration(data.overallLatency.p99Ms)}
              </p>
              <p className="text-xs text-muted-foreground">P99</p>
            </div>
          </div>
          {sparklineData.length > 1 && (
            <div className="mt-4 h-16">
              <Sparkline data={sparklineData} color="teal" height={64} />
            </div>
          )}
        </div>

        {/* AI Metrics */}
        {data.aiMetrics && (
          <div className="p-6 rounded-2xl bg-gradient-to-br from-secondary/50 to-secondary/20 border border-border/50">
            <div className="flex items-center justify-between mb-4">
              <div>
                <h4 className="text-sm font-semibold text-foreground">
                  AI Processing
                </h4>
                <p className="text-xs text-muted-foreground">
                  {data.aiMetrics.invocationCount} AI invocations
                </p>
              </div>
              <Sparkles className="w-5 h-5 text-muted-foreground" />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <LatencyStatsCard
                title="Queue Wait"
                stats={data.aiMetrics.queueWait}
                icon={Clock}
                color="amber"
              />
              <LatencyStatsCard
                title="Processing"
                stats={data.aiMetrics.processingTime}
                icon={Cpu}
                color="violet"
              />
            </div>
            {data.aiMetrics.avgTokens && (
              <div className="mt-3 p-3 rounded-lg bg-background/50 flex items-center justify-between">
                <span className="text-sm text-muted-foreground">
                  Avg Tokens/Request
                </span>
                <span className="text-sm font-semibold text-foreground">
                  {data.aiMetrics.avgTokens.toFixed(0)}
                </span>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Slow Stage Alerts */}
      {data.slowStages.length > 0 && (
        <div className="p-6 rounded-2xl bg-gradient-to-br from-secondary/50 to-secondary/20 border border-border/50">
          <div className="flex items-center justify-between mb-4">
            <div>
              <h4 className="text-sm font-semibold text-foreground flex items-center gap-2">
                <AlertTriangle className="w-4 h-4 text-amber-400" />
                Slow Stages Detected
              </h4>
              <p className="text-xs text-muted-foreground">
                Stages exceeding performance thresholds
              </p>
            </div>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
            {data.slowStages.map((alert) => (
              <SlowStageAlertCard key={alert.stage} alert={alert} />
            ))}
          </div>
        </div>
      )}

      {/* Stage Latency Table */}
      <div className="p-6 rounded-2xl bg-gradient-to-br from-secondary/50 to-secondary/20 border border-border/50">
        <div className="flex items-center justify-between mb-4">
          <div>
            <h4 className="text-sm font-semibold text-foreground">
              Stage Latencies
            </h4>
            <p className="text-xs text-muted-foreground">
              Per-stage performance breakdown
            </p>
          </div>
          <Activity className="w-5 h-5 text-muted-foreground" />
        </div>
        <StageLatencyTable stageLatencies={data.stageLatencies} />
      </div>

      {/* Database Metrics */}
      <div className="p-6 rounded-2xl bg-gradient-to-br from-secondary/50 to-secondary/20 border border-border/50">
        <div className="flex items-center justify-between mb-4">
          <div>
            <h4 className="text-sm font-semibold text-foreground">
              Database Metrics
            </h4>
            <p className="text-xs text-muted-foreground">
              Query performance statistics
            </p>
          </div>
          <Database className="w-5 h-5 text-muted-foreground" />
        </div>
        <div className="grid grid-cols-3 gap-4">
          <div className="p-4 rounded-xl bg-background/50 text-center">
            <p className="text-2xl font-bold text-foreground">
              {data.dbMetrics.totalQueries}
            </p>
            <p className="text-xs text-muted-foreground">Total Queries</p>
          </div>
          <div className="p-4 rounded-xl bg-background/50 text-center">
            <p className="text-2xl font-bold text-foreground">
              {data.dbMetrics.avgQueriesPerRecord.toFixed(1)}
            </p>
            <p className="text-xs text-muted-foreground">Avg/Record</p>
          </div>
          <div className="p-4 rounded-xl bg-background/50 text-center">
            <p className="text-2xl font-bold text-foreground">
              {data.dbMetrics.queryLatency.count > 0
                ? formatDuration(data.dbMetrics.queryLatency.avgMs)
                : 'N/A'}
            </p>
            <p className="text-xs text-muted-foreground">Avg Query Time</p>
          </div>
        </div>
      </div>
    </div>
  )
}
