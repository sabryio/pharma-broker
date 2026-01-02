import { useEffect, useState, useRef } from 'react'
import { cn } from '@/lib/utils'
import { RefreshCw } from 'lucide-react'

interface MatchConfidenceMeterProps {
  confidence: number
  size?: number
  onClick?: () => void
  isPending?: boolean
}

// Animated counter hook
function useAnimatedCounter(target: number, duration: number = 1000) {
  const [count, setCount] = useState(0)
  const startTimeRef = useRef<number | null>(null)
  const startValueRef = useRef(0)

  useEffect(() => {
    startValueRef.current = count
    startTimeRef.current = null

    const animate = (timestamp: number) => {
      if (!startTimeRef.current) startTimeRef.current = timestamp
      const progress = Math.min((timestamp - startTimeRef.current) / duration, 1)

      // Easing function (ease-out cubic)
      const eased = 1 - Math.pow(1 - progress, 3)
      const current =
        startValueRef.current + (target - startValueRef.current) * eased

      setCount(current)

      if (progress < 1) {
        requestAnimationFrame(animate)
      }
    }

    requestAnimationFrame(animate)
  }, [target, duration])

  return count
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
  const animatedConfidence = useAnimatedCounter(confidence, 800)

  // Color based on confidence with gradient stops
  const getColors = () => {
    if (confidence >= 80)
      return {
        stroke: '#00E676',
        glow: 'rgba(0, 230, 118, 0.5)',
        gradient: ['#00E676', '#00F2FF'],
      }
    if (confidence >= 60)
      return {
        stroke: '#F59E0B',
        glow: 'rgba(245, 158, 11, 0.5)',
        gradient: ['#F59E0B', '#FBBF24'],
      }
    return {
      stroke: '#EF4444',
      glow: 'rgba(239, 68, 68, 0.5)',
      gradient: ['#EF4444', '#F87171'],
    }
  }

  const colors = getColors()

  const getLabel = () => {
    if (confidence >= 80) return 'Strong Match'
    if (confidence >= 60) return 'Partial Match'
    return 'Weak Match'
  }

  // Generate tick marks
  const tickMarks = Array.from({ length: 20 }, (_, i) => {
    const angle = (i / 20) * 360 - 90
    const isActive = (i / 20) * 100 <= confidence
    const isMajor = i % 5 === 0
    return { angle, isActive, isMajor, index: i }
  })

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
          {/* Gradient definitions */}
          <defs>
            <linearGradient
              id="matchGradient"
              x1="0%"
              y1="0%"
              x2="100%"
              y2="100%"
            >
              <stop offset="0%" stopColor={colors.gradient[0]} />
              <stop offset="100%" stopColor={colors.gradient[1]} />
            </linearGradient>
            <filter id="glow" x="-50%" y="-50%" width="200%" height="200%">
              <feGaussianBlur stdDeviation="3" result="coloredBlur" />
              <feMerge>
                <feMergeNode in="coloredBlur" />
                <feMergeNode in="SourceGraphic" />
              </feMerge>
            </filter>
          </defs>

          {/* Outer tick marks ring */}
          {tickMarks.map(({ angle, isActive, isMajor, index }) => {
            const rad = (angle * Math.PI) / 180
            const innerR = radius + 14
            const outerR = radius + (isMajor ? 22 : 18)
            const x1 = size / 2 + innerR * Math.cos(rad)
            const y1 = size / 2 + innerR * Math.sin(rad)
            const x2 = size / 2 + outerR * Math.cos(rad)
            const y2 = size / 2 + outerR * Math.sin(rad)
            return (
              <line
                key={index}
                x1={x1}
                y1={y1}
                x2={x2}
                y2={y2}
                stroke={isActive ? colors.stroke : 'hsl(var(--muted))'}
                strokeWidth={isMajor ? 2 : 1}
                strokeLinecap="round"
                className="transition-all duration-300"
                style={{
                  opacity: isActive ? 1 : 0.3,
                  transitionDelay: `${index * 20}ms`,
                }}
              />
            )
          })}

          {/* Background circle */}
          <circle
            cx={size / 2}
            cy={size / 2}
            r={radius}
            fill="none"
            stroke="hsl(var(--muted))"
            strokeWidth={strokeWidth}
            className="opacity-20"
          />

          {/* Progress arc with gradient */}
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
            filter="url(#glow)"
            style={{
              filter: `drop-shadow(0 0 12px ${colors.glow})`,
            }}
          />

          {/* Decorative particles */}
          {[...Array(12)].map((_, i) => {
            const angle = (i / 12) * 2 * Math.PI - Math.PI / 2
            const x = size / 2 + (radius + 28) * Math.cos(angle)
            const y = size / 2 + (radius + 28) * Math.sin(angle)
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

          {/* High confidence pulsing glow - circles expanding from center continuously */}
          {confidence >= 85 && !isPending && (
            <>
              <circle
                cx={size / 2}
                cy={size / 2}
                r={radius}
                fill="none"
                stroke={colors.stroke}
                strokeWidth={3}
                className="animate-pulse-from-center-1"
                style={{ 
                  transformOrigin: `${size / 2}px ${size / 2}px`,
                }}
              />
              <circle
                cx={size / 2}
                cy={size / 2}
                r={radius}
                fill="none"
                stroke={colors.stroke}
                strokeWidth={3}
                className="animate-pulse-from-center-2"
                style={{ 
                  transformOrigin: `${size / 2}px ${size / 2}px`,
                }}
              />
            </>
          )}
        </svg>

        {/* Center content */}
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          <span className="text-4xl font-bold text-foreground tabular-nums">
            {isPending ? (
              <RefreshCw className="w-8 h-8 animate-spin text-teal" />
            ) : (
              `${animatedConfidence.toFixed(1)}%`
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
