// Circular Progress Component
// Animated circular progress indicator with labels

import { cn } from '@/lib/utils'
import { useEffect, useState } from 'react'

interface CircularProgressProps {
  value: number
  max?: number
  size?: number
  strokeWidth?: number
  color?: 'teal' | 'emerald' | 'violet' | 'amber' | 'red' | 'blue'
  label?: string
  sublabel?: string
  showValue?: boolean
  animated?: boolean
  className?: string
}

const colorMap = {
  teal: '#14b8a6',
  emerald: '#10b981',
  violet: '#8b5cf6',
  amber: '#f59e0b',
  red: '#ef4444',
  blue: '#3b82f6',
}

export function CircularProgress({
  value,
  max = 100,
  size = 120,
  strokeWidth = 8,
  color = 'teal',
  label,
  sublabel,
  showValue = true,
  animated = true,
  className,
}: CircularProgressProps) {
  const [animatedValue, setAnimatedValue] = useState(animated ? 0 : value)

  useEffect(() => {
    if (!animated) {
      setAnimatedValue(value)
      return
    }

    const duration = 1000
    const startTime = Date.now()
    const startValue = animatedValue

    const animate = () => {
      const elapsed = Date.now() - startTime
      const progress = Math.min(elapsed / duration, 1)
      const eased = 1 - Math.pow(1 - progress, 3) // ease-out cubic
      setAnimatedValue(startValue + (value - startValue) * eased)

      if (progress < 1) {
        requestAnimationFrame(animate)
      }
    }

    requestAnimationFrame(animate)
  }, [value, animated])

  const radius = (size - strokeWidth) / 2
  const circumference = radius * 2 * Math.PI
  const percent = Math.min(animatedValue / max, 1)
  const offset = circumference - percent * circumference
  const strokeColor = colorMap[color]

  return (
    <div
      className={cn(
        'relative inline-flex items-center justify-center',
        className,
      )}
    >
      <svg width={size} height={size} className="-rotate-90">
        <defs>
          <linearGradient
            id={`progress-gradient-${color}`}
            x1="0%"
            y1="0%"
            x2="100%"
            y2="0%"
          >
            <stop offset="0%" stopColor={strokeColor} stopOpacity="0.8" />
            <stop offset="100%" stopColor={strokeColor} stopOpacity="1" />
          </linearGradient>
          <filter id="progress-glow">
            <feGaussianBlur stdDeviation="2" result="coloredBlur" />
            <feMerge>
              <feMergeNode in="coloredBlur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        {/* Background circle */}
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          stroke="currentColor"
          strokeWidth={strokeWidth}
          className="text-secondary/40"
        />

        {/* Progress circle */}
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          stroke={`url(#progress-gradient-${color})`}
          strokeWidth={strokeWidth}
          strokeDasharray={circumference}
          strokeDashoffset={offset}
          strokeLinecap="round"
          filter="url(#progress-glow)"
          className="transition-all duration-300 ease-out"
        />

        {/* Decorative dots */}
        {[0, 90, 180, 270].map((angle) => (
          <circle
            key={angle}
            cx={size / 2 + radius * Math.cos(((angle - 90) * Math.PI) / 180)}
            cy={size / 2 + radius * Math.sin(((angle - 90) * Math.PI) / 180)}
            r="2"
            fill="currentColor"
            className="text-muted-foreground/30"
          />
        ))}
      </svg>

      <div className="absolute inset-0 flex flex-col items-center justify-center">
        {showValue && (
          <span
            className="text-2xl font-bold text-foreground tabular-nums"
            style={{ color: strokeColor }}
          >
            {Math.round(percent * 100)}%
          </span>
        )}
        {label && (
          <span className="text-xs text-muted-foreground font-medium mt-0.5">
            {label}
          </span>
        )}
        {sublabel && (
          <span className="text-[10px] text-muted-foreground/70">
            {sublabel}
          </span>
        )}
      </div>
    </div>
  )
}
