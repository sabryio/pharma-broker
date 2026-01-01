import { cn } from '@/lib/utils'
import type { ParsingStats } from './types'
import { Clock, CheckCircle, XCircle, TrendingUp } from 'lucide-react'

interface ParsingStatsCardsProps {
  stats: ParsingStats
}

export function ParsingStatsCards({ stats }: ParsingStatsCardsProps) {
  const cards = [
    {
      label: 'Pending',
      value: stats.pending,
      icon: Clock,
      color: 'purple',
      glowClass: 'shadow-purple-500/20',
    },
    {
      label: 'Approved Today',
      value: stats.todayReviewed,
      icon: CheckCircle,
      color: 'emerald',
      glowClass: 'shadow-emerald/20',
    },
    {
      label: 'Rejected',
      value: stats.rejected,
      icon: XCircle,
      color: 'destructive',
      glowClass: 'shadow-destructive/20',
    },
    {
      label: 'Avg Confidence',
      value: `${Math.round(stats.avgConfidence * 100)}%`,
      icon: TrendingUp,
      color: 'amber',
      glowClass: 'shadow-amber/20',
    },
  ]

  return (
    <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
      {cards.map((card) => (
        <div
          key={card.label}
          className={cn(
            'glass-card p-4 rounded-xl border transition-all duration-300 hover:scale-[1.02]',
            card.color === 'purple' &&
              'border-purple-500/30 hover:border-purple-500/50',
            card.color === 'emerald' &&
              'border-emerald/30 hover:border-emerald/50',
            card.color === 'destructive' &&
              'border-destructive/30 hover:border-destructive/50',
            card.color === 'amber' && 'border-amber/30 hover:border-amber/50',
            `shadow-lg ${card.glowClass}`,
          )}
        >
          <div className="flex items-center gap-3">
            <div
              className={cn(
                'w-10 h-10 rounded-lg flex items-center justify-center',
                card.color === 'purple' && 'bg-purple-500/20',
                card.color === 'emerald' && 'bg-emerald/20',
                card.color === 'destructive' && 'bg-destructive/20',
                card.color === 'amber' && 'bg-amber/20',
              )}
            >
              <card.icon
                className={cn(
                  'w-5 h-5',
                  card.color === 'purple' && 'text-purple-400',
                  card.color === 'emerald' && 'text-emerald',
                  card.color === 'destructive' && 'text-destructive',
                  card.color === 'amber' && 'text-amber',
                )}
              />
            </div>
            <div>
              <p className="text-xs text-muted-foreground">{card.label}</p>
              <p
                className={cn(
                  'text-xl font-bold',
                  card.color === 'purple' && 'text-purple-400',
                  card.color === 'emerald' && 'text-emerald',
                  card.color === 'destructive' && 'text-destructive',
                  card.color === 'amber' && 'text-amber',
                )}
              >
                {card.value}
              </p>
            </div>
          </div>
        </div>
      ))}
    </div>
  )
}
