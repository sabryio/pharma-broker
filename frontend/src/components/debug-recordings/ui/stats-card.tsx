// Stats Card Component
// Beautiful animated statistics card with sparkline support

import { cn } from '@/lib/utils'
import { TrendingUp } from 'lucide-react'
import { Sparkline } from './sparkline'

interface StatsCardProps {
  icon: React.ElementType
  label: string
  value: number | string
  trend?: number
  trendLabel?: string
  color?: 'teal' | 'emerald' | 'violet' | 'amber' | 'red' | 'blue' | 'pink'
  sparklineData?: number[]
  className?: string
}

const colorClasses = {
  teal: { bg: 'bg-teal-500/20', text: 'text-teal-400', border: 'border-teal-500/30', glow: 'shadow-teal-500/10' },
  emerald: { bg: 'bg-emerald-500/20', text: 'text-emerald-400', border: 'border-emerald-500/30', glow: 'shadow-emerald-500/10' },
  violet: { bg: 'bg-violet-500/20', text: 'text-violet-400', border: 'border-violet-500/30', glow: 'shadow-violet-500/10' },
  amber: { bg: 'bg-amber-500/20', text: 'text-amber-400', border: 'border-amber-500/30', glow: 'shadow-amber-500/10' },
  red: { bg: 'bg-red-500/20', text: 'text-red-400', border: 'border-red-500/30', glow: 'shadow-red-500/10' },
  blue: { bg: 'bg-blue-500/20', text: 'text-blue-400', border: 'border-blue-500/30', glow: 'shadow-blue-500/10' },
  pink: { bg: 'bg-pink-500/20', text: 'text-pink-400', border: 'border-pink-500/30', glow: 'shadow-pink-500/10' },
}

export function StatsCard({
  icon: Icon,
  label,
  value,
  trend,
  trendLabel,
  color = 'teal',
  sparklineData,
  className,
}: StatsCardProps) {
  const colors = colorClasses[color]

  return (
    <div className={cn(
      'relative p-5 rounded-2xl border overflow-hidden transition-all duration-300',
      'bg-gradient-to-br from-secondary/50 to-secondary/20 backdrop-blur-sm',
      'hover:shadow-xl hover:scale-[1.02] hover:border-opacity-60',
      colors.border,
      colors.glow,
      className,
    )}>
      {/* Background decoration */}
      <div className={cn(
        'absolute -right-6 -top-6 w-28 h-28 rounded-full opacity-20 blur-2xl',
        colors.bg,
      )} />
      <div className={cn(
        'absolute -left-4 -bottom-4 w-20 h-20 rounded-full opacity-10 blur-xl',
        colors.bg,
      )} />
      
      <div className="relative">
        <div className="flex items-start justify-between mb-4">
          <div className={cn(
            'w-12 h-12 rounded-xl flex items-center justify-center',
            'shadow-lg transition-transform hover:scale-110',
            colors.bg,
          )}>
            <Icon className={cn('w-6 h-6', colors.text)} />
          </div>
          {trend !== undefined && (
            <div className={cn(
              'flex items-center gap-1 px-2.5 py-1 rounded-lg text-xs font-semibold',
              trend >= 0 ? 'bg-emerald-500/20 text-emerald-400' : 'bg-red-500/20 text-red-400',
            )}>
              <TrendingUp className={cn('w-3 h-3', trend < 0 && 'rotate-180')} />
              {Math.abs(trend)}%
            </div>
          )}
        </div>
        
        <p className={cn('text-3xl font-bold text-foreground mb-1 tracking-tight', colors.text)}>
          {value}
        </p>
        <p className="text-sm text-muted-foreground font-medium">{label}</p>
        {trendLabel && (
          <p className="text-xs text-muted-foreground/70 mt-1">{trendLabel}</p>
        )}
        
        {sparklineData && sparklineData.length > 1 && (
          <div className="mt-4 h-10 -mx-1">
            <Sparkline data={sparklineData} color={color} height={40} />
          </div>
        )}
      </div>
    </div>
  )
}
