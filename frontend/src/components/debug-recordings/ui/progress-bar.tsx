// Progress Bar Component
// Animated horizontal progress bar with gradient

import { cn } from '@/lib/utils'
import { useEffect, useState } from 'react'

interface ProgressBarProps {
  value: number
  max?: number
  label?: string
  valueLabel?: string | number
  color?: 'teal' | 'emerald' | 'violet' | 'amber' | 'red' | 'blue'
  size?: 'sm' | 'md' | 'lg'
  animated?: boolean
  showPercentage?: boolean
  className?: string
}

const colorClasses = {
  teal: 'from-teal-500 to-cyan-500',
  emerald: 'from-emerald-500 to-teal-500',
  violet: 'from-violet-500 to-purple-500',
  amber: 'from-amber-500 to-orange-500',
  red: 'from-red-500 to-rose-500',
  blue: 'from-blue-500 to-indigo-500',
}

const sizeClasses = {
  sm: 'h-1.5',
  md: 'h-2.5',
  lg: 'h-4',
}

export function ProgressBar({
  value,
  max = 100,
  label,
  valueLabel,
  color = 'teal',
  size = 'md',
  animated = true,
  showPercentage = false,
  className,
}: ProgressBarProps) {
  const [width, setWidth] = useState(animated ? 0 : (value / max) * 100)

  useEffect(() => {
    if (animated) {
      const timer = setTimeout(() => {
        setWidth((value / max) * 100)
      }, 100)
      return () => clearTimeout(timer)
    } else {
      setWidth((value / max) * 100)
    }
  }, [value, max, animated])

  const percentage = Math.round((value / max) * 100)

  return (
    <div className={cn('space-y-1.5', className)}>
      {(label || valueLabel !== undefined || showPercentage) && (
        <div className="flex items-center justify-between">
          {label && (
            <span className="text-sm text-muted-foreground font-medium">
              {label}
            </span>
          )}
          <div className="flex items-center gap-2">
            {showPercentage && (
              <span className="text-xs text-muted-foreground">
                {percentage}%
              </span>
            )}
            {valueLabel !== undefined && (
              <span
                className={cn(
                  'text-sm font-semibold',
                  color === 'emerald' && 'text-emerald-400',
                  color === 'red' && 'text-red-400',
                  color === 'amber' && 'text-amber-400',
                  color === 'teal' && 'text-teal-400',
                  color === 'violet' && 'text-violet-400',
                  color === 'blue' && 'text-blue-400',
                )}
              >
                {valueLabel}
              </span>
            )}
          </div>
        </div>
      )}

      <div
        className={cn(
          'w-full rounded-full bg-secondary/50 overflow-hidden',
          sizeClasses[size],
        )}
      >
        <div
          className={cn(
            'h-full rounded-full bg-gradient-to-r transition-all duration-700 ease-out',
            colorClasses[color],
          )}
          style={{
            width: `${width}%`,
            boxShadow: width > 0 ? `0 0 12px rgba(20, 184, 166, 0.4)` : 'none',
          }}
        />
      </div>
    </div>
  )
}
