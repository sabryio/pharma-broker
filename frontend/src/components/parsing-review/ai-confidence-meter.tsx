import { cn } from '@/lib/utils'
import { getConfidenceLabel } from './types'

interface AIConfidenceMeterProps {
  confidence: number
  size?: number
  showLabel?: boolean
}

export function AIConfidenceMeter({
  confidence,
  size = 160,
  showLabel = true,
}: AIConfidenceMeterProps) {
  const percentage = Math.round(confidence * 100)
  const strokeWidth = 10
  const radius = (size - strokeWidth) / 2
  const circumference = radius * 2 * Math.PI
  const offset = circumference - confidence * circumference

  // Color based on confidence
  const getColor = () => {
    if (confidence >= 0.8)
      return { stroke: '#A855F7', glow: 'rgba(168, 85, 247, 0.5)' }
    if (confidence >= 0.6)
      return { stroke: '#F59E0B', glow: 'rgba(245, 158, 11, 0.5)' }
    return { stroke: '#EF4444', glow: 'rgba(239, 68, 68, 0.5)' }
  }

  const colors = getColor()

  return (
    <div className="flex flex-col items-center">
      <div className="relative">
        {/* Glow effect */}
        <div
          className="absolute inset-0 rounded-full blur-xl animate-pulse-slow"
          style={{
            background: `radial-gradient(circle, ${colors.glow} 0%, transparent 70%)`,
          }}
        />

        {/* SVG Meter */}
        <svg
          width={size}
          height={size}
          className="transform -rotate-90"
          style={{ overflow: 'visible' }}
        >
          {/* Background circle */}
          <circle
            cx={size / 2}
            cy={size / 2}
            r={radius}
            fill="none"
            stroke="hsl(var(--muted))"
            strokeWidth={strokeWidth}
            className="opacity-30"
          />

          {/* Progress arc */}
          <circle
            cx={size / 2}
            cy={size / 2}
            r={radius}
            fill="none"
            stroke={colors.stroke}
            strokeWidth={strokeWidth}
            strokeLinecap="round"
            strokeDasharray={circumference}
            strokeDashoffset={offset}
            className="transition-all duration-1000 ease-out"
            style={{
              filter: `drop-shadow(0 0 8px ${colors.glow})`,
            }}
          />

          {/* Decorative particles */}
          {[...Array(8)].map((_, i) => {
            const angle = (i / 8) * 2 * Math.PI - Math.PI / 2
            const x = size / 2 + (radius + 15) * Math.cos(angle)
            const y = size / 2 + (radius + 15) * Math.sin(angle)
            const isActive = i / 8 <= confidence
            return (
              <circle
                key={i}
                cx={x}
                cy={y}
                r={2}
                fill={isActive ? colors.stroke : 'hsl(var(--muted))'}
                className={cn(
                  'transition-all duration-500',
                  isActive && 'animate-pulse',
                )}
                style={{ animationDelay: `${i * 100}ms` }}
              />
            )
          })}
        </svg>

        {/* Center content */}
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <span className="text-3xl font-bold text-foreground">
            {percentage}%
          </span>
          <span className="text-xs text-muted-foreground">AI Confidence</span>
        </div>
      </div>

      {/* Label */}
      {showLabel && (
        <div className="mt-4 text-center">
          <span
            className={cn(
              'text-sm font-medium px-3 py-1 rounded-full',
              confidence >= 0.8 && 'bg-purple-500/20 text-purple-400',
              confidence >= 0.6 && confidence < 0.8 && 'bg-amber/20 text-amber',
              confidence < 0.6 && 'bg-destructive/20 text-destructive',
            )}
          >
            {getConfidenceLabel(confidence)}
          </span>
        </div>
      )}
    </div>
  )
}
