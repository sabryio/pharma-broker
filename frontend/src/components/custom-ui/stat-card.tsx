import { cn } from '@/lib/utils'
import type { LucideIcon } from 'lucide-react'

interface StatCardProps {
  title: string
  value: string | number
  subtitle?: string
  icon?: LucideIcon
  trend?: {
    value: number
    isPositive: boolean
  }
  variant?: 'teal' | 'amber' | 'emerald' | 'default'
  className?: string
}

export function StatCard({
  title,
  value,
  subtitle,
  icon: Icon,
  trend,
  variant = 'default',
  className,
}: StatCardProps) {
  const variantStyles = {
    teal: 'border-teal/30 glow-teal',
    amber: 'border-amber/30 glow-amber',
    emerald: 'border-emerald/30 glow-emerald',
    default: 'border-border',
  }

  const accentColors = {
    teal: 'text-teal',
    amber: 'text-amber',
    emerald: 'text-emerald',
    default: 'text-foreground',
  }

  return (
    <div
      className={cn(
        'glass-card p-5 rounded-xl transition-all duration-300 hover:scale-[1.02]',
        variantStyles[variant],
        className,
      )}
    >
      <div className="flex items-start justify-between mb-3">
        <span className="text-sm font-medium text-muted-foreground">
          {title}
        </span>
        {Icon && (
          <div
            className={cn(
              'p-2 rounded-lg bg-secondary/50',
              accentColors[variant],
            )}
          >
            <Icon className="w-4 h-4" />
          </div>
        )}
      </div>

      <div className="flex items-end gap-2">
        <span
          className={cn(
            'text-3xl font-bold tracking-tight',
            accentColors[variant],
          )}
        >
          {value}
        </span>
        {trend && (
          <span
            className={cn(
              'text-sm font-medium mb-1',
              trend.isPositive ? 'text-emerald' : 'text-destructive',
            )}
          >
            {trend.isPositive ? '+' : ''}
            {trend.value}%
          </span>
        )}
      </div>

      {subtitle && (
        <p className="mt-2 text-xs text-muted-foreground">{subtitle}</p>
      )}
    </div>
  )
}
