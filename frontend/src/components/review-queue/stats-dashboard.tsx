// Stats Dashboard Component
// Enhanced statistics with trends and charts

import { useMemo } from 'react'
import { cn } from '@/lib/utils'
import {
  Clock,
  CheckCircle,
  XCircle,
  TrendingUp,
  TrendingDown,
  Activity,
  Zap,
  Target,
  BarChart3,
} from 'lucide-react'

interface StatsData {
  pending: number
  approved: number
  rejected: number
  avgConfidence: number
  // Optional trend data
  approvedYesterday?: number
  rejectedYesterday?: number
  avgConfidenceYesterday?: number
  // Time-based stats
  avgReviewTimeMs?: number
  reviewsPerHour?: number
  // Distribution
  highConfidenceCount?: number
  mediumConfidenceCount?: number
  lowConfidenceCount?: number
}

interface StatsDashboardProps extends StatsData {
  className?: string
  compact?: boolean
}

// Mini sparkline component
function MiniSparkline({ data, color }: { data: number[]; color: string }) {
  if (data.length < 2) return null

  const max = Math.max(...data)
  const min = Math.min(...data)
  const range = max - min || 1

  const points = data
    .map((value, index) => {
      const x = (index / (data.length - 1)) * 60
      const y = 20 - ((value - min) / range) * 16
      return `${x},${y}`
    })
    .join(' ')

  return (
    <svg width="60" height="24" className="overflow-visible">
      <polyline
        points={points}
        fill="none"
        stroke={color}
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

// Trend indicator
function TrendIndicator({
  current,
  previous,
  inverted = false,
}: {
  current: number
  previous?: number
  inverted?: boolean
}) {
  if (previous === undefined) return null

  const diff = current - previous
  const percentage = previous > 0 ? ((diff / previous) * 100).toFixed(0) : '0'
  const isPositive = inverted ? diff < 0 : diff > 0
  const isNeutral = diff === 0

  return (
    <div
      className={cn(
        'flex items-center gap-1 text-xs font-medium',
        isNeutral && 'text-muted-foreground',
        !isNeutral && isPositive && 'text-emerald-400',
        !isNeutral && !isPositive && 'text-red-400',
      )}
    >
      {!isNeutral &&
        (isPositive ? (
          <TrendingUp className="w-3 h-3" />
        ) : (
          <TrendingDown className="w-3 h-3" />
        ))}
      <span>{isNeutral ? '—' : `${diff > 0 ? '+' : ''}${percentage}%`}</span>
    </div>
  )
}

// Progress ring
function ProgressRing({
  value,
  max,
  size = 48,
  strokeWidth = 4,
  color,
}: {
  value: number
  max: number
  size?: number
  strokeWidth?: number
  color: string
}) {
  const radius = (size - strokeWidth) / 2
  const circumference = radius * 2 * Math.PI
  const percentage = max > 0 ? value / max : 0
  const offset = circumference - percentage * circumference

  return (
    <svg width={size} height={size} className="-rotate-90">
      <circle
        cx={size / 2}
        cy={size / 2}
        r={radius}
        fill="none"
        stroke="currentColor"
        strokeWidth={strokeWidth}
        className="text-secondary"
      />
      <circle
        cx={size / 2}
        cy={size / 2}
        r={radius}
        fill="none"
        stroke={color}
        strokeWidth={strokeWidth}
        strokeLinecap="round"
        strokeDasharray={circumference}
        strokeDashoffset={offset}
        className="transition-all duration-500"
      />
    </svg>
  )
}

// Distribution bar
function DistributionBar({
  high,
  medium,
  low,
}: {
  high: number
  medium: number
  low: number
}) {
  const total = high + medium + low
  if (total === 0) return null

  const highPct = (high / total) * 100
  const mediumPct = (medium / total) * 100
  const lowPct = (low / total) * 100

  return (
    <div className="space-y-2">
      <div className="flex h-3 rounded-full overflow-hidden bg-secondary/50">
        <div
          className="bg-emerald-500 transition-all duration-500"
          style={{ width: `${highPct}%` }}
          title={`High: ${high}`}
        />
        <div
          className="bg-amber-500 transition-all duration-500"
          style={{ width: `${mediumPct}%` }}
          title={`Medium: ${medium}`}
        />
        <div
          className="bg-red-500 transition-all duration-500"
          style={{ width: `${lowPct}%` }}
          title={`Low: ${low}`}
        />
      </div>
      <div className="flex items-center justify-between text-xs text-muted-foreground">
        <span className="flex items-center gap-1">
          <span className="w-2 h-2 rounded-full bg-emerald-500" />
          High {highPct.toFixed(0)}%
        </span>
        <span className="flex items-center gap-1">
          <span className="w-2 h-2 rounded-full bg-amber-500" />
          Med {mediumPct.toFixed(0)}%
        </span>
        <span className="flex items-center gap-1">
          <span className="w-2 h-2 rounded-full bg-red-500" />
          Low {lowPct.toFixed(0)}%
        </span>
      </div>
    </div>
  )
}

export function StatsDashboard({
  pending,
  approved,
  rejected,
  avgConfidence,
  approvedYesterday,
  rejectedYesterday,
  avgConfidenceYesterday,
  avgReviewTimeMs,
  reviewsPerHour,
  highConfidenceCount = 0,
  mediumConfidenceCount = 0,
  lowConfidenceCount = 0,
  className,
  compact = false,
}: StatsDashboardProps) {
  const total = approved + rejected
  const approvalRate = total > 0 ? (approved / total) * 100 : 0

  // Mock sparkline data (in production, this would come from API)
  const approvalTrend = useMemo(
    () => [65, 72, 68, 75, 80, 78, approvalRate],
    [approvalRate],
  )

  if (compact) {
    return (
      <div className={cn('grid grid-cols-2 md:grid-cols-4 gap-3', className)}>
        {/* Pending */}
        <div className="p-3 rounded-xl bg-gradient-to-br from-teal/10 to-teal/5 border border-teal/20">
          <div className="flex items-center gap-2 mb-1">
            <Clock className="w-4 h-4 text-teal" />
            <span className="text-xs text-muted-foreground">Pending</span>
          </div>
          <p className="text-2xl font-bold text-teal">{pending}</p>
        </div>

        {/* Approved */}
        <div className="p-3 rounded-xl bg-gradient-to-br from-emerald/10 to-emerald/5 border border-emerald/20">
          <div className="flex items-center gap-2 mb-1">
            <CheckCircle className="w-4 h-4 text-emerald" />
            <span className="text-xs text-muted-foreground">Approved</span>
          </div>
          <div className="flex items-center justify-between">
            <p className="text-2xl font-bold text-emerald">{approved}</p>
            <TrendIndicator current={approved} previous={approvedYesterday} />
          </div>
        </div>

        {/* Rejected */}
        <div className="p-3 rounded-xl bg-gradient-to-br from-red-500/10 to-red-500/5 border border-red-500/20">
          <div className="flex items-center gap-2 mb-1">
            <XCircle className="w-4 h-4 text-red-400" />
            <span className="text-xs text-muted-foreground">Rejected</span>
          </div>
          <div className="flex items-center justify-between">
            <p className="text-2xl font-bold text-red-400">{rejected}</p>
            <TrendIndicator
              current={rejected}
              previous={rejectedYesterday}
              inverted
            />
          </div>
        </div>

        {/* Avg Confidence */}
        <div className="p-3 rounded-xl bg-gradient-to-br from-amber/10 to-amber/5 border border-amber/20">
          <div className="flex items-center gap-2 mb-1">
            <TrendingUp className="w-4 h-4 text-amber" />
            <span className="text-xs text-muted-foreground">Avg Score</span>
          </div>
          <div className="flex items-center justify-between">
            <p className="text-2xl font-bold text-amber">{avgConfidence}%</p>
            <TrendIndicator
              current={avgConfidence}
              previous={avgConfidenceYesterday}
            />
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className={cn('space-y-4', className)}>
      {/* Main stats row */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
        {/* Pending */}
        <div className="p-4 rounded-xl bg-gradient-to-br from-teal/10 to-teal/5 border border-teal/20 shadow-lg shadow-teal/5">
          <div className="flex items-center justify-between mb-3">
            <div className="w-10 h-10 rounded-xl bg-teal/20 flex items-center justify-center">
              <Clock className="w-5 h-5 text-teal" />
            </div>
            <MiniSparkline data={[pending, pending]} color="#14b8a6" />
          </div>
          <p className="text-xs text-muted-foreground mb-1">Pending Review</p>
          <p className="text-3xl font-bold text-teal">{pending}</p>
        </div>

        {/* Approved Today */}
        <div className="p-4 rounded-xl bg-gradient-to-br from-emerald/10 to-emerald/5 border border-emerald/20 shadow-lg shadow-emerald/5">
          <div className="flex items-center justify-between mb-3">
            <div className="w-10 h-10 rounded-xl bg-emerald/20 flex items-center justify-center">
              <CheckCircle className="w-5 h-5 text-emerald" />
            </div>
            <TrendIndicator current={approved} previous={approvedYesterday} />
          </div>
          <p className="text-xs text-muted-foreground mb-1">Approved Today</p>
          <p className="text-3xl font-bold text-emerald">{approved}</p>
          {approvedYesterday !== undefined && (
            <p className="text-xs text-muted-foreground mt-1">
              vs {approvedYesterday} yesterday
            </p>
          )}
        </div>

        {/* Rejected Today */}
        <div className="p-4 rounded-xl bg-gradient-to-br from-red-500/10 to-red-500/5 border border-red-500/20 shadow-lg shadow-red-500/5">
          <div className="flex items-center justify-between mb-3">
            <div className="w-10 h-10 rounded-xl bg-red-500/20 flex items-center justify-center">
              <XCircle className="w-5 h-5 text-red-400" />
            </div>
            <TrendIndicator
              current={rejected}
              previous={rejectedYesterday}
              inverted
            />
          </div>
          <p className="text-xs text-muted-foreground mb-1">Rejected Today</p>
          <p className="text-3xl font-bold text-red-400">{rejected}</p>
          {rejectedYesterday !== undefined && (
            <p className="text-xs text-muted-foreground mt-1">
              vs {rejectedYesterday} yesterday
            </p>
          )}
        </div>

        {/* Approval Rate */}
        <div className="p-4 rounded-xl bg-gradient-to-br from-violet-500/10 to-violet-500/5 border border-violet-500/20 shadow-lg shadow-violet-500/5">
          <div className="flex items-center justify-between mb-3">
            <div className="w-10 h-10 rounded-xl bg-violet-500/20 flex items-center justify-center">
              <Target className="w-5 h-5 text-violet-400" />
            </div>
            <div className="relative">
              <ProgressRing value={approvalRate} max={100} color="#8b5cf6" />
              <span className="absolute inset-0 flex items-center justify-center text-xs font-bold text-violet-400">
                {approvalRate.toFixed(0)}%
              </span>
            </div>
          </div>
          <p className="text-xs text-muted-foreground mb-1">Approval Rate</p>
          <p className="text-3xl font-bold text-violet-400">
            {approvalRate.toFixed(1)}%
          </p>
        </div>
      </div>

      {/* Secondary stats row */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {/* Confidence Distribution */}
        <div className="p-4 rounded-xl bg-gradient-to-br from-secondary/50 to-secondary/20 border border-border/50">
          <div className="flex items-center gap-2 mb-4">
            <BarChart3 className="w-4 h-4 text-muted-foreground" />
            <span className="text-sm font-medium text-foreground">
              Confidence Distribution
            </span>
          </div>
          <DistributionBar
            high={highConfidenceCount}
            medium={mediumConfidenceCount}
            low={lowConfidenceCount}
          />
        </div>

        {/* Average Confidence */}
        <div className="p-4 rounded-xl bg-gradient-to-br from-amber/10 to-amber/5 border border-amber/20">
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-2">
              <Activity className="w-4 h-4 text-amber" />
              <span className="text-sm font-medium text-foreground">
                Avg Confidence
              </span>
            </div>
            <TrendIndicator
              current={avgConfidence}
              previous={avgConfidenceYesterday}
            />
          </div>
          <div className="flex items-center gap-4">
            <p className="text-4xl font-bold text-amber">{avgConfidence}%</p>
            <MiniSparkline data={approvalTrend} color="#f59e0b" />
          </div>
        </div>

        {/* Performance */}
        <div className="p-4 rounded-xl bg-gradient-to-br from-cyan-500/10 to-cyan-500/5 border border-cyan-500/20">
          <div className="flex items-center gap-2 mb-3">
            <Zap className="w-4 h-4 text-cyan-400" />
            <span className="text-sm font-medium text-foreground">
              Performance
            </span>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <p className="text-xs text-muted-foreground">Avg Review Time</p>
              <p className="text-lg font-bold text-cyan-400">
                {avgReviewTimeMs
                  ? `${(avgReviewTimeMs / 1000).toFixed(1)}s`
                  : '—'}
              </p>
            </div>
            <div>
              <p className="text-xs text-muted-foreground">Reviews/Hour</p>
              <p className="text-lg font-bold text-cyan-400">
                {reviewsPerHour ?? '—'}
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
