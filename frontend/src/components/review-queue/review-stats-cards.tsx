import { cn } from '@/lib/utils'
import { Clock, CheckCircle, XCircle, TrendingUp } from 'lucide-react'

interface ReviewStatsProps {
  pending: number
  approved: number
  rejected: number
  avgConfidence: number
}

export function ReviewStatsCards({
  pending,
  approved,
  rejected,
  avgConfidence,
}: ReviewStatsProps) {
  const cards = [
    {
      label: 'Pending',
      value: pending,
      icon: Clock,
      color: 'teal',
      glowClass: 'shadow-teal/20',
    },
    {
      label: 'Approved',
      value: approved,
      icon: CheckCircle,
      color: 'emerald',
      glowClass: 'shadow-emerald/20',
    },
    {
      label: 'Rejected',
      value: rejected,
      icon: XCircle,
      color: 'destructive',
      glowClass: 'shadow-destructive/20',
    },
    {
      label: 'Avg Score',
      value: `${avgConfidence.toFixed(2)}%`,
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
            card.color === 'teal' && 'border-teal/30 hover:border-teal/50',
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
                card.color === 'teal' && 'bg-teal/20',
                card.color === 'emerald' && 'bg-emerald/20',
                card.color === 'destructive' && 'bg-destructive/20',
                card.color === 'amber' && 'bg-amber/20',
              )}
            >
              <card.icon
                className={cn(
                  'w-5 h-5',
                  card.color === 'teal' && 'text-teal',
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
                  card.color === 'teal' && 'text-teal',
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
