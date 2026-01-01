'use client'

import { Link2, Sparkles, TrendingUp } from 'lucide-react'
import { cn } from '@/lib/utils'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'

interface MatchCountBadgeProps {
  count: number
  variant?: 'offer' | 'request'
  size?: 'sm' | 'md' | 'lg'
  showLabel?: boolean
  className?: string
}

/**
 * A beautiful badge component to display confirmed match counts
 * with animated effects and tooltips
 */
export function MatchCountBadge({
  count,
  variant = 'offer',
  size = 'md',
  showLabel = false,
  className,
}: MatchCountBadgeProps) {
  const isHot = count >= 5
  const isWarm = count >= 3 && count < 5
  const hasMatches = count > 0

  const sizeClasses = {
    sm: 'h-5 min-w-5 text-xs gap-0.5 px-1.5',
    md: 'h-6 min-w-6 text-xs gap-1 px-2',
    lg: 'h-7 min-w-7 text-sm gap-1.5 px-2.5',
  }

  const iconSizes = {
    sm: 'w-3 h-3',
    md: 'w-3.5 h-3.5',
    lg: 'w-4 h-4',
  }

  // Color schemes based on variant and count
  const getColorClasses = () => {
    if (!hasMatches) {
      return 'bg-muted/50 text-muted-foreground border-muted'
    }

    if (variant === 'offer') {
      if (isHot) {
        return 'bg-gradient-to-r from-emerald-500/20 to-teal-500/20 text-emerald-400 border-emerald-500/50 shadow-emerald-500/20'
      }
      if (isWarm) {
        return 'bg-emerald-500/15 text-emerald-400 border-emerald-500/40'
      }
      return 'bg-emerald-500/10 text-emerald-500 border-emerald-500/30'
    }

    // Request variant
    if (isHot) {
      return 'bg-gradient-to-r from-amber-500/20 to-orange-500/20 text-amber-400 border-amber-500/50 shadow-amber-500/20'
    }
    if (isWarm) {
      return 'bg-amber-500/15 text-amber-400 border-amber-500/40'
    }
    return 'bg-amber-500/10 text-amber-500 border-amber-500/30'
  }

  const Icon = isHot ? Sparkles : hasMatches ? TrendingUp : Link2

  const tooltipText = hasMatches
    ? `${count} confirmed match${count !== 1 ? 'es' : ''}`
    : 'No matches yet'

  const badge = (
    <div
      className={cn(
        'inline-flex items-center justify-center rounded-full border font-medium transition-all duration-300',
        sizeClasses[size],
        getColorClasses(),
        hasMatches && 'shadow-sm',
        isHot && 'animate-pulse shadow-md',
        className,
      )}
    >
      <Icon
        className={cn(
          iconSizes[size],
          isHot && 'animate-bounce',
          'transition-transform',
        )}
      />
      <span className="tabular-nums">{count}</span>
      {showLabel && (
        <span className="ml-0.5 hidden sm:inline">
          {count === 1 ? 'match' : 'matches'}
        </span>
      )}
    </div>
  )

  return (
    <Tooltip>
      <TooltipTrigger asChild>{badge}</TooltipTrigger>
      <TooltipContent side="top" className="font-medium">
        {tooltipText}
        {isHot && <span className="ml-1 text-amber-400">🔥 Hot item!</span>}
      </TooltipContent>
    </Tooltip>
  )
}

/**
 * A compact inline version for table cells
 */
export function MatchCountInline({
  count,
  variant = 'offer',
}: {
  count: number
  variant?: 'offer' | 'request'
}) {
  if (count === 0) {
    return <span className="text-muted-foreground/50">—</span>
  }

  const colorClass = variant === 'offer' ? 'text-emerald-500' : 'text-amber-500'
  const bgClass = variant === 'offer' ? 'bg-emerald-500/10' : 'bg-amber-500/10'

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          className={cn(
            'inline-flex items-center gap-1 px-2 py-0.5 rounded-md font-medium tabular-nums cursor-default transition-colors',
            colorClass,
            bgClass,
            'hover:opacity-80',
          )}
        >
          <Link2 className="w-3 h-3" />
          {count}
        </span>
      </TooltipTrigger>
      <TooltipContent>
        {count} confirmed match{count !== 1 ? 'es' : ''}
      </TooltipContent>
    </Tooltip>
  )
}
