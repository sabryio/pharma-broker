import { cn } from '@/lib/utils'

interface MatchConfidenceMeterProps {
  confidence: number
  size?: number
}

export function MatchConfidenceMeter({
  confidence,
  size = 180,
}: MatchConfidenceMeterProps) {
  const strokeWidth = 12
  const radius = (size - strokeWidth) / 2
  const circumference = radius * 2 * Math.PI
  const offset = circumference - (confidence / 100) * circumference

  // Color based on confidence
  const getColors = () => {
    if (confidence >= 80)
      return { stroke: '#00E676', glow: 'rgba(0, 230, 118, 0.5)' }
    if (confidence >= 60)
      return { stroke: '#F59E0B', glow: 'rgba(245, 158, 11, 0.5)' }
    return { stroke: '#EF4444', glow: 'rgba(239, 68, 68, 0.5)' }
  }

  const colors = getColors()

  const getLabel = () => {
    if (confidence >= 80) return 'Strong Match'
    if (confidence >= 60) return 'Partial Match'
    return 'Weak Match'
  }

  return (
    <div className="flex flex-col items-center">
      <div className="relative">
        {/* Glow effect */}
        <div
          className="absolute inset-0 rounded-full blur-2xl animate-pulse-slow"
          style={{
            background: `radial-gradient(circle, ${colors.glow} 0%, transparent 70%)`,
          }}
        />

        {/* SVG Ring */}
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

          {/* Progress arc with gradient */}
          <defs>
            <linearGradient
              id="matchGradient"
              x1="0%"
              y1="0%"
              x2="100%"
              y2="100%"
            >
              <stop offset="0%" stopColor="#00F2FF" />
              <stop offset="100%" stopColor={colors.stroke} />
            </linearGradient>
          </defs>
          <circle
            cx={size / 2}
            cy={size / 2}
            r={radius}
            fill="none"
            stroke="url(#matchGradient)"
            strokeWidth={strokeWidth}
            strokeLinecap="round"
            strokeDasharray={circumference}
            strokeDashoffset={offset}
            className="transition-all duration-1000 ease-out"
            style={{
              filter: `drop-shadow(0 0 10px ${colors.glow})`,
            }}
          />

          {/* Decorative particles */}
          {[...Array(12)].map((_, i) => {
            const angle = (i / 12) * 2 * Math.PI - Math.PI / 2
            const x = size / 2 + (radius + 18) * Math.cos(angle)
            const y = size / 2 + (radius + 18) * Math.sin(angle)
            const isActive = (i / 12) * 100 <= confidence
            return (
              <circle
                key={i}
                cx={x}
                cy={y}
                r={isActive ? 3 : 2}
                fill={isActive ? colors.stroke : 'hsl(var(--muted))'}
                className={cn(
                  'transition-all duration-500',
                  isActive && 'animate-pulse',
                )}
                style={{
                  animationDelay: `${i * 80}ms`,
                  opacity: isActive ? 1 : 0.3,
                }}
              />
            )
          })}
        </svg>

        {/* Center content */}
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <span className="text-4xl font-bold text-foreground">
            {confidence.toFixed(2)}%
          </span>
          <span className="text-xs text-muted-foreground mt-1">Match</span>
          <span className="text-xs text-muted-foreground">Confidence</span>
        </div>
      </div>

      {/* Label */}
      <div className="mt-4">
        <span
          className={cn(
            'text-sm font-medium px-4 py-1.5 rounded-full',
            confidence >= 80 &&
              'bg-emerald/20 text-emerald border border-emerald/30',
            confidence >= 60 &&
              confidence < 80 &&
              'bg-amber/20 text-amber border border-amber/30',
            confidence < 60 &&
              'bg-destructive/20 text-destructive border border-destructive/30',
          )}
        >
          {getLabel()}
        </span>
      </div>
    </div>
  )
}
