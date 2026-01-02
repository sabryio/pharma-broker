// Uncertainty Panel Component
// Visualizes uncertainty estimation for match confidence

import { useState } from 'react'
import { cn } from '@/lib/utils'
import {
  useMatchUncertainty,
  useUncertaintyStatus,
  useEstimateUncertainty,
  getUncertaintyColor,
  formatUncertainty,
  formatConfidenceInterval,
  type UncertaintyResult,
} from '@/hooks/use-uncertainty'
import {
  Activity,
  AlertTriangle,
  CheckCircle,
  ChevronDown,
  ChevronUp,
  Gauge,
  HelpCircle,
  RefreshCw,
  Settings,
  TrendingDown,
  TrendingUp,
  Zap,
} from 'lucide-react'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'

// =============================================================================
// Uncertainty Level Badge
// =============================================================================

interface UncertaintyBadgeProps {
  level: string
  className?: string
}

function UncertaintyBadge({ level, className }: UncertaintyBadgeProps) {
  const colorClass = getUncertaintyColor(level)
  const bgClass = {
    very_low: 'bg-green-500/20 border-green-500/30',
    low: 'bg-green-500/20 border-green-500/30',
    moderate: 'bg-yellow-500/20 border-yellow-500/30',
    high: 'bg-orange-500/20 border-orange-500/30',
    very_high: 'bg-red-500/20 border-red-500/30',
  }[level] || 'bg-gray-500/20 border-gray-500/30'

  const label = level.replace('_', ' ').replace(/\b\w/g, (c) => c.toUpperCase())

  return (
    <span
      className={cn(
        'px-2.5 py-1 rounded-full text-xs font-medium border',
        bgClass,
        colorClass,
        className,
      )}
    >
      {label}
    </span>
  )
}

// =============================================================================
// Confidence Interval Visualization
// =============================================================================

interface ConfidenceIntervalBarProps {
  result: UncertaintyResult
  className?: string
}

function ConfidenceIntervalBar({
  result,
  className,
}: ConfidenceIntervalBarProps) {
  const { meanScore, ciLower, ciUpper, originalScore } = result

  // Scale to percentage
  const lowerPct = ciLower * 100
  const upperPct = ciUpper * 100
  const meanPct = meanScore * 100
  const originalPct = originalScore * 100

  return (
    <div className={cn('space-y-2', className)}>
      <div className="flex items-center justify-between text-xs text-muted-foreground">
        <span>0%</span>
        <span>50%</span>
        <span>100%</span>
      </div>
      <div className="relative h-8 bg-secondary/50 rounded-lg overflow-hidden">
        {/* Confidence interval range */}
        <div
          className="absolute top-0 bottom-0 bg-teal-500/30 border-l-2 border-r-2 border-teal-500/50"
          style={{
            left: `${lowerPct}%`,
            width: `${upperPct - lowerPct}%`,
          }}
        />

        {/* Mean score marker */}
        <div
          className="absolute top-0 bottom-0 w-1 bg-teal-500"
          style={{ left: `${meanPct}%` }}
        />

        {/* Original score marker */}
        <div
          className="absolute top-1 bottom-1 w-0.5 bg-amber-500"
          style={{ left: `${originalPct}%` }}
        />

        {/* Labels */}
        <div
          className="absolute -top-5 text-[10px] font-medium text-teal-400 transform -translate-x-1/2"
          style={{ left: `${meanPct}%` }}
        >
          μ={meanPct.toFixed(1)}%
        </div>
      </div>
      <div className="flex items-center justify-between text-xs">
        <span className="text-muted-foreground">
          CI: {formatConfidenceInterval(ciLower, ciUpper)}
        </span>
        <span className="text-amber-400">Original: {originalPct.toFixed(1)}%</span>
      </div>
    </div>
  )
}

// =============================================================================
// Uncertainty Stats Grid
// =============================================================================

interface UncertaintyStatsProps {
  result: UncertaintyResult
}

function UncertaintyStats({ result }: UncertaintyStatsProps) {
  return (
    <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
      <div className="p-3 rounded-lg bg-background/50 border border-border/30">
        <div className="flex items-center gap-1.5 text-xs text-muted-foreground mb-1">
          <Gauge className="w-3 h-3" />
          Mean Score
        </div>
        <p className="text-lg font-bold text-teal-400">
          {(result.meanScore * 100).toFixed(1)}%
        </p>
      </div>

      <div className="p-3 rounded-lg bg-background/50 border border-border/30">
        <div className="flex items-center gap-1.5 text-xs text-muted-foreground mb-1">
          <Activity className="w-3 h-3" />
          Std Dev
        </div>
        <p className="text-lg font-bold text-foreground">
          {formatUncertainty(result.stdDev)}
        </p>
      </div>

      <div className="p-3 rounded-lg bg-background/50 border border-border/30">
        <div className="flex items-center gap-1.5 text-xs text-muted-foreground mb-1">
          <TrendingUp className="w-3 h-3" />
          CV
        </div>
        <p className="text-lg font-bold text-foreground">
          {(result.coefficientOfVariation * 100).toFixed(1)}%
        </p>
      </div>

      <div className="p-3 rounded-lg bg-background/50 border border-border/30">
        <div className="flex items-center gap-1.5 text-xs text-muted-foreground mb-1">
          <Zap className="w-3 h-3" />
          Samples
        </div>
        <p className="text-lg font-bold text-foreground">{result.numSamples}</p>
      </div>
    </div>
  )
}

// =============================================================================
// Uncertainty Indicator (Compact)
// =============================================================================

export interface UncertaintyIndicatorProps {
  matchId: string
  className?: string
}

export function UncertaintyIndicator({
  matchId,
  className,
}: UncertaintyIndicatorProps) {
  const { data, isLoading, error } = useMatchUncertainty(matchId)

  if (isLoading) {
    return (
      <div className={cn('flex items-center gap-1.5', className)}>
        <RefreshCw className="w-3 h-3 animate-spin text-muted-foreground" />
        <span className="text-xs text-muted-foreground">Loading...</span>
      </div>
    )
  }

  if (error || !data) {
    return null
  }

  const { result } = data

  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>
          <div
            className={cn(
              'flex items-center gap-1.5 cursor-help',
              className,
            )}
          >
            {result.isCertain ? (
              <CheckCircle className="w-3.5 h-3.5 text-emerald-400" />
            ) : (
              <AlertTriangle className="w-3.5 h-3.5 text-amber-400" />
            )}
            <span className="text-xs text-muted-foreground">
              {formatUncertainty(result.stdDev)}
            </span>
          </div>
        </TooltipTrigger>
        <TooltipContent side="top" className="max-w-[250px]">
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-xs font-medium">Uncertainty</span>
              <UncertaintyBadge level={result.uncertaintyLevel} />
            </div>
            <p className="text-xs text-muted-foreground">
              Mean: {(result.meanScore * 100).toFixed(1)}% ±{' '}
              {(result.stdDev * 100).toFixed(1)}%
            </p>
            <p className="text-xs text-muted-foreground">
              CI: {formatConfidenceInterval(result.ciLower, result.ciUpper)}
            </p>
          </div>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}

// =============================================================================
// Main Uncertainty Panel
// =============================================================================

export interface UncertaintyPanelProps {
  matchId?: string
  offerId?: string
  requestId?: string
  className?: string
  defaultExpanded?: boolean
}

export function UncertaintyPanel({
  matchId,
  offerId,
  requestId,
  className,
  defaultExpanded = false,
}: UncertaintyPanelProps) {
  const [isExpanded, setIsExpanded] = useState(defaultExpanded)

  // Use match uncertainty if matchId provided
  const {
    data: matchData,
    isLoading: matchLoading,
    refetch: refetchMatch,
  } = useMatchUncertainty(matchId)

  // Use estimate mutation for manual estimation
  const estimateMutation = useEstimateUncertainty()

  // Get status for config info
  const { data: statusData } = useUncertaintyStatus()

  const result = matchData?.result
  const isLoading = matchLoading || estimateMutation.isPending

  const handleEstimate = () => {
    if (offerId && requestId) {
      estimateMutation.mutate({ offerId, requestId })
    } else if (matchId) {
      refetchMatch()
    }
  }

  return (
    <div
      className={cn(
        'rounded-xl border overflow-hidden transition-all duration-300',
        'bg-gradient-to-br from-cyan-500/5 to-blue-500/5',
        'border-cyan-500/20',
        className,
      )}
    >
      {/* Header */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center justify-between p-4 hover:bg-white/5 transition-colors"
      >
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 rounded-lg bg-cyan-500/20 flex items-center justify-center">
            <Activity className="w-4 h-4 text-cyan-400" />
          </div>
          <div className="text-left">
            <h4 className="text-sm font-semibold text-white">
              Uncertainty Estimation
            </h4>
            <p className="text-[10px] text-slate-400">
              Monte Carlo weight perturbation analysis
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {result && <UncertaintyBadge level={result.uncertaintyLevel} />}
          {isExpanded ? (
            <ChevronUp className="w-4 h-4 text-muted-foreground" />
          ) : (
            <ChevronDown className="w-4 h-4 text-muted-foreground" />
          )}
        </div>
      </button>

      {/* Expanded Content */}
      {isExpanded && (
        <div className="px-4 pb-4 space-y-4 animate-fade-in">
          {isLoading ? (
            <div className="flex items-center justify-center py-8">
              <RefreshCw className="w-6 h-6 text-cyan-400 animate-spin" />
            </div>
          ) : result ? (
            <>
              {/* Certainty Status */}
              <div
                className={cn(
                  'p-3 rounded-lg border',
                  result.isCertain
                    ? 'bg-emerald-500/10 border-emerald-500/20'
                    : 'bg-amber-500/10 border-amber-500/20',
                )}
              >
                <div className="flex items-center gap-2">
                  {result.isCertain ? (
                    <CheckCircle className="w-5 h-5 text-emerald-400" />
                  ) : (
                    <AlertTriangle className="w-5 h-5 text-amber-400" />
                  )}
                  <div>
                    <p
                      className={cn(
                        'text-sm font-medium',
                        result.isCertain ? 'text-emerald-400' : 'text-amber-400',
                      )}
                    >
                      {result.isCertain
                        ? 'High Certainty Match'
                        : 'Uncertain Match'}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {result.isCertain
                        ? 'Score is stable under weight perturbation'
                        : 'Score varies significantly with weight changes'}
                    </p>
                  </div>
                </div>
              </div>

              {/* Stats Grid */}
              <UncertaintyStats result={result} />

              {/* Confidence Interval Visualization */}
              <div className="pt-4">
                <h5 className="text-xs font-medium text-foreground mb-4 flex items-center gap-2">
                  <TrendingDown className="w-3.5 h-3.5 text-cyan-400" />
                  Confidence Interval (95%)
                </h5>
                <ConfidenceIntervalBar result={result} />
              </div>

              {/* Robustness Indicator */}
              <div className="flex items-center justify-between p-3 rounded-lg bg-background/50 border border-border/30">
                <div className="flex items-center gap-2">
                  <Settings className="w-4 h-4 text-muted-foreground" />
                  <span className="text-sm text-foreground">Robustness</span>
                </div>
                <span
                  className={cn(
                    'text-sm font-medium',
                    result.isRobust ? 'text-emerald-400' : 'text-amber-400',
                  )}
                >
                  {result.isRobust ? 'Robust' : 'Sensitive'}
                </span>
              </div>
            </>
          ) : (
            <div className="text-center py-8">
              <HelpCircle className="w-10 h-10 text-muted-foreground/30 mx-auto mb-3" />
              <p className="text-sm text-muted-foreground mb-4">
                No uncertainty data available
              </p>
              {(offerId && requestId) || matchId ? (
                <button
                  onClick={handleEstimate}
                  disabled={isLoading}
                  className="px-4 py-2 rounded-lg bg-cyan-500/20 text-cyan-400 hover:bg-cyan-500/30 transition-colors text-sm font-medium"
                >
                  Estimate Uncertainty
                </button>
              ) : null}
            </div>
          )}

          {/* Config Info */}
          {statusData && (
            <div className="pt-3 border-t border-border/30">
              <p className="text-[10px] text-muted-foreground">
                Config: {statusData.defaultConfig.numSamples} samples,{' '}
                {(statusData.defaultConfig.perturbationStd * 100).toFixed(0)}%
                perturbation,{' '}
                {(statusData.defaultConfig.confidenceLevel * 100).toFixed(0)}%
                CI
              </p>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
