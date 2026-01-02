import { cn } from '@/lib/utils'
import { RefreshCw } from 'lucide-react'

interface MatchConfidenceMeterProps {
  confidence: number
  size?: number
  onClick?: () => void
  isPending?: boolean
}

export function MatchConfidenceMeter({
  confidence,
  size = 180,
  onClick,
  isPending = false,
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
      <button
        onClick={onClick}
        disabled={!onClick || isPending}
        className={cn(
          'relative group transition-all duration-500',
          onClick &&
            !isPending &&
            'cursor-pointer hover:scale-105 active:scale-95',
          isPending && 'opacity-70 cursor-wait',
        )}
        title={onClick ? 'Click to find new matches for the anchored item (left card)' : undefined}
      >
        {/* Glow effect */}
        <div
          className={cn(
            'absolute inset-0 rounded-full blur-2xl transition-all duration-500',
            isPending
              ? 'animate-pulse scale-110'
              : 'animate-pulse-slow group-hover:blur-3xl group-hover:scale-110',
          )}
          style={{
            background: `radial-gradient(circle, ${colors.glow} 0%, transparent 70%)`,
          }}
        />

        {/* SVG Ring */}
        <svg
          width={size}
          height={size}
          className={cn(
            'transform -rotate-90 transition-all duration-700',
            isPending && 'animate-spin-slow',
          )}
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
                  isActive && (isPending ? 'animate-bounce' : 'animate-pulse'),
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
            {isPending ? (
              <RefreshCw className="w-8 h-8 animate-spin text-teal" />
            ) : (
              `${confidence.toFixed(2)}%`
            )}
          </span>
          <span className="text-[10px] text-muted-foreground mt-1 uppercase tracking-tighter">
            {isPending ? 'Rematching...' : 'Match Confidence'}
          </span>
          {onClick && !isPending && (
            <div className="absolute -bottom-2 opacity-0 group-hover:opacity-100 transition-opacity flex items-center gap-1 bg-black/60 backdrop-blur-sm px-2 py-0.5 rounded-full border border-white/10">
              <RefreshCw className="w-2 h-2 text-teal" />
              <span className="text-[8px] font-bold text-teal">FIND NEW MATCHES</span>
            </div>
          )}
        </div>
      </button>

      {/* Label */}
      <div className="mt-4">
        <span
          className={cn(
            'text-sm font-medium px-4 py-1.5 rounded-full transition-all duration-300',
            confidence >= 80 &&
              'bg-emerald/20 text-emerald border border-emerald/30 shadow-lg shadow-emerald/10',
            confidence >= 60 &&
              confidence < 80 &&
              'bg-amber/20 text-amber border border-amber/30 shadow-lg shadow-amber/10',
            confidence < 60 &&
              'bg-destructive/20 text-destructive border border-destructive/30 shadow-lg shadow-destructive/10',
            isPending && 'animate-pulse',
          )}
        >
          {isPending ? 'Refreshing Results' : getLabel()}
        </span>
      </div>
    </div>
  )
}
