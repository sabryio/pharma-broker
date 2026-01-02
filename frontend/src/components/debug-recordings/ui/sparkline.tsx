// Sparkline Chart Component
// Minimal animated line chart for trend visualization

import { useMemo } from 'react'

interface SparklineProps {
  data: number[]
  color?: 'teal' | 'emerald' | 'violet' | 'amber' | 'red' | 'blue' | 'pink'
  height?: number
  showDots?: boolean
  animated?: boolean
}

const colorMap = {
  teal: { stroke: '#14b8a6', fill: 'rgba(20, 184, 166, 0.2)' },
  emerald: { stroke: '#10b981', fill: 'rgba(16, 185, 129, 0.2)' },
  violet: { stroke: '#8b5cf6', fill: 'rgba(139, 92, 246, 0.2)' },
  amber: { stroke: '#f59e0b', fill: 'rgba(245, 158, 11, 0.2)' },
  red: { stroke: '#ef4444', fill: 'rgba(239, 68, 68, 0.2)' },
  blue: { stroke: '#3b82f6', fill: 'rgba(59, 130, 246, 0.2)' },
  pink: { stroke: '#ec4899', fill: 'rgba(236, 72, 153, 0.2)' },
}

export function Sparkline({ 
  data, 
  color = 'teal', 
  height = 40,
  showDots = false,
  animated = true,
}: SparklineProps) {
  const { points, areaPoints, lastPoint } = useMemo(() => {
    if (data.length < 2) return { points: '', areaPoints: '', lastPoint: null }

    const max = Math.max(...data)
    const min = Math.min(...data)
    const range = max - min || 1
    const padding = 2

    const pts = data.map((value, index) => {
      const x = padding + (index / (data.length - 1)) * (100 - padding * 2)
      const y = padding + (1 - (value - min) / range) * (height - padding * 2)
      return { x, y, value }
    })

    const linePoints = pts.map(p => `${p.x},${p.y}`).join(' ')
    const area = `0,${height} ${linePoints} 100,${height}`
    
    return { 
      points: linePoints, 
      areaPoints: area,
      lastPoint: pts[pts.length - 1],
    }
  }, [data, height])

  if (data.length < 2) return null

  const colors = colorMap[color]

  return (
    <svg 
      viewBox={`0 0 100 ${height}`} 
      className="w-full h-full" 
      preserveAspectRatio="none"
    >
      <defs>
        <linearGradient id={`sparkline-gradient-${color}`} x1="0%" y1="0%" x2="0%" y2="100%">
          <stop offset="0%" stopColor={colors.stroke} stopOpacity="0.4" />
          <stop offset="100%" stopColor={colors.stroke} stopOpacity="0" />
        </linearGradient>
        <filter id="glow">
          <feGaussianBlur stdDeviation="1" result="coloredBlur"/>
          <feMerge>
            <feMergeNode in="coloredBlur"/>
            <feMergeNode in="SourceGraphic"/>
          </feMerge>
        </filter>
      </defs>
      
      {/* Area fill */}
      <polygon
        fill={`url(#sparkline-gradient-${color})`}
        points={areaPoints}
        className={animated ? 'animate-fade-in' : ''}
      />
      
      {/* Line */}
      <polyline
        fill="none"
        stroke={colors.stroke}
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        points={points}
        filter="url(#glow)"
        className={animated ? 'animate-draw-line' : ''}
        style={animated ? {
          strokeDasharray: 1000,
          strokeDashoffset: 1000,
          animation: 'draw-line 1.5s ease-out forwards',
        } : undefined}
      />
      
      {/* End dot */}
      {showDots && lastPoint && (
        <g>
          <circle
            cx={lastPoint.x}
            cy={lastPoint.y}
            r="4"
            fill={colors.stroke}
            className="animate-pulse"
          />
          <circle
            cx={lastPoint.x}
            cy={lastPoint.y}
            r="6"
            fill={colors.stroke}
            opacity="0.3"
            className="animate-ping"
          />
        </g>
      )}
      
      <style>{`
        @keyframes draw-line {
          to {
            stroke-dashoffset: 0;
          }
        }
      `}</style>
    </svg>
  )
}
