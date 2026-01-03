import { useMemo } from 'react'
import { cn } from '@/lib/utils'
import {
  Clock,
  CheckCircle,
  XCircle,
  Activity,
  BarChart3,
} from 'lucide-react'
import type { MatchReviewItem, MatchReviewStats } from '@/schema/match-review'

// Confidence band thresholds
const CONFIDENCE_BANDS = {
  high: { min: 80, max: 100 },
  medium: { min: 50, max: 79 },
  low: { min: 0, max: 49 },
} as const

interface StatsDashboardProps {
  stats: MatchReviewStats | undefined
  matches: MatchReviewItem[]
  isLoading?: boolean
  className?: string
}

/**
 * Calculate average confidence from an array of matches
 * Returns 0 for empty arrays
 */
export function calculateAverageConfidence(matches: MatchReviewItem[]): number {
  if (matches.length === 0) return 0
  const sum = matches.reduce((acc, match) => acc + match.confidence, 0)
  return sum / matches.length
}

/**
 * Calculate confidence band distribution from an array of matches
 * Each match is counted in exactly one band based on its confidence score
 */
export function calculateConfidenceBands(matches: MatchReviewItem[]): {
  high: number
  medium: number
  low: number
} {
  return {
    high: matches.filter(
      (m) => m.confidence >= CONFIDENCE_BANDS.high.min && m.confidence <= CONFIDENCE_BANDS.high.max
    ).length,
    medium: matches.filter(
      (m) => m.confidence >= CONFIDENCE_BANDS.medium.min && m.confidence <= CONFIDENCE_BANDS.medium.max
    ).length,
    low: matches.filter(
      (m) => m.confidence >= CONFIDENCE_BANDS.low.min && m.confidence <= CONFIDENCE_BANDS.low.max
    ).length,
  }
}

// Progress ring component
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

// Distribution bar component
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
  if (total === 0) {
    return (
      <div className="space-y-2">
        <div className="flex h-3 rounded-full overflow-hidden bg-secondary/50" />
        <div className="flex items-center justify-between text-xs text-muted-foreground">
          <span>No data</span>
        </div>
      </div>
    )
  }

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
  stats,
  matches,
  isLoading = false,
  className,
}: StatsDashboardProps) {
  // Calculate confidence bands from matches
  const confidenceBands = useMemo(() => calculateConfidenceBands(matches), [matches])

  // Calculate average confidence from pending matches
  const pendingMatches = useMemo(
    () => matches.filter((m) => m.status === 'PENDING'),
    [matches]
  )
  const avgConfidence = useMemo(
    () => Math.round(calculateAverageConfidence(pendingMatches)),
    [pendingMatches]
  )

  if (isLoading) {
    return (
      <div className={cn('space-y-4', className)}>
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
          {[...Array(4)].map((_, i) => (
            <div
              key={i}
              className="p-4 rounded-xl bg-secondary/20 border border-border/50 animate-pulse h-28"
            />
          ))}
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
          </div>
          <p className="text-xs text-muted-foreground mb-1">Pending Matches</p>
          <p className="text-3xl font-bold text-teal">{stats?.pending ?? 0}</p>
        </div>

        {/* Confirmed Today */}
        <div className="p-4 rounded-xl bg-gradient-to-br from-emerald/10 to-emerald/5 border border-emerald/20 shadow-lg shadow-emerald/5">
          <div className="flex items-center justify-between mb-3">
            <div className="w-10 h-10 rounded-xl bg-emerald/20 flex items-center justify-center">
              <CheckCircle className="w-5 h-5 text-emerald" />
            </div>
          </div>
          <p className="text-xs text-muted-foreground mb-1">Confirmed Today</p>
          <p className="text-3xl font-bold text-emerald">{stats?.confirmedToday ?? 0}</p>
        </div>

        {/* Rejected Today */}
        <div className="p-4 rounded-xl bg-gradient-to-br from-red-500/10 to-red-500/5 border border-red-500/20 shadow-lg shadow-red-500/5">
          <div className="flex items-center justify-between mb-3">
            <div className="w-10 h-10 rounded-xl bg-red-500/20 flex items-center justify-center">
              <XCircle className="w-5 h-5 text-red-400" />
            </div>
          </div>
          <p className="text-xs text-muted-foreground mb-1">Rejected Today</p>
          <p className="text-3xl font-bold text-red-400">{stats?.rejectedToday ?? 0}</p>
        </div>

        {/* Average Confidence */}
        <div className="p-4 rounded-xl bg-gradient-to-br from-amber/10 to-amber/5 border border-amber/20 shadow-lg shadow-amber/5">
          <div className="flex items-center justify-between mb-3">
            <div className="w-10 h-10 rounded-xl bg-amber/20 flex items-center justify-center">
              <Activity className="w-5 h-5 text-amber" />
            </div>
            <div className="relative">
              <ProgressRing value={avgConfidence} max={100} color="#f59e0b" />
              <span className="absolute inset-0 flex items-center justify-center text-xs font-bold text-amber">
                {avgConfidence}%
              </span>
            </div>
          </div>
          <p className="text-xs text-muted-foreground mb-1">Avg Confidence</p>
          <p className="text-3xl font-bold text-amber">{avgConfidence}%</p>
        </div>
      </div>

      {/* Confidence Distribution */}
      <div className="p-4 rounded-xl bg-gradient-to-br from-secondary/50 to-secondary/20 border border-border/50">
        <div className="flex items-center gap-2 mb-4">
          <BarChart3 className="w-4 h-4 text-muted-foreground" />
          <span className="text-sm font-medium text-foreground">
            Confidence Distribution
          </span>
          <span className="text-xs text-muted-foreground ml-auto">
            {matches.length} total matches
          </span>
        </div>
        <DistributionBar
          high={confidenceBands.high}
          medium={confidenceBands.medium}
          low={confidenceBands.low}
        />
        <div className="grid grid-cols-3 gap-4 mt-4">
          <div className="text-center">
            <p className="text-2xl font-bold text-emerald">{confidenceBands.high}</p>
            <p className="text-xs text-muted-foreground">High (≥80%)</p>
          </div>
          <div className="text-center">
            <p className="text-2xl font-bold text-amber">{confidenceBands.medium}</p>
            <p className="text-xs text-muted-foreground">Medium (50-79%)</p>
          </div>
          <div className="text-center">
            <p className="text-2xl font-bold text-red-400">{confidenceBands.low}</p>
            <p className="text-xs text-muted-foreground">Low (&lt;50%)</p>
          </div>
        </div>
      </div>
    </div>
  )
}
