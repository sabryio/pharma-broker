// Heat Map Component
// Activity visualization by day and hour

import { cn } from '@/lib/utils'
import { useMemo } from 'react'

interface HeatMapData {
  day: string
  hour: number
  value: number
}

interface HeatMapProps {
  data: HeatMapData[]
  className?: string
}

const DAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']
const HOURS = Array.from({ length: 24 }, (_, i) => i)

export function HeatMap({ data, className }: HeatMapProps) {
  const { maxValue, getValue } = useMemo(() => {
    const max = Math.max(...data.map((d) => d.value), 1)
    const dataMap = new Map(data.map((d) => [`${d.day}-${d.hour}`, d.value]))

    return {
      maxValue: max,
      getValue: (day: string, hour: number) =>
        dataMap.get(`${day}-${hour}`) ?? 0,
    }
  }, [data])

  return (
    <div className={cn('space-y-2', className)}>
      {/* Hour labels */}
      <div className="flex gap-1 ml-10">
        {HOURS.filter((_, i) => i % 4 === 0).map((hour) => (
          <div
            key={hour}
            className="text-[10px] text-muted-foreground font-medium"
            style={{ width: '48px', textAlign: 'center' }}
          >
            {hour.toString().padStart(2, '0')}:00
          </div>
        ))}
      </div>

      {/* Grid */}
      {DAYS.map((day) => (
        <div key={day} className="flex items-center gap-2">
          <span className="w-8 text-xs text-muted-foreground font-medium">
            {day}
          </span>
          <div className="flex gap-0.5">
            {HOURS.map((hour) => {
              const value = getValue(day, hour)
              const intensity = value / maxValue

              return (
                <div
                  key={hour}
                  className={cn(
                    'w-3 h-3 rounded-sm transition-all duration-200',
                    'hover:scale-150 hover:z-10 cursor-pointer',
                    'group relative',
                  )}
                  style={{
                    backgroundColor:
                      intensity > 0
                        ? `rgba(20, 184, 166, ${0.15 + intensity * 0.85})`
                        : 'rgba(255, 255, 255, 0.03)',
                    boxShadow:
                      intensity > 0.5
                        ? `0 0 ${intensity * 8}px rgba(20, 184, 166, ${intensity * 0.5})`
                        : 'none',
                  }}
                >
                  {/* Tooltip */}
                  <div
                    className={cn(
                      'absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2 py-1',
                      'bg-popover border border-border rounded-lg shadow-xl',
                      'text-[10px] whitespace-nowrap z-50',
                      'opacity-0 group-hover:opacity-100 pointer-events-none',
                      'transition-opacity duration-150',
                    )}
                  >
                    <p className="font-medium text-foreground">
                      {day} {hour}:00
                    </p>
                    <p className="text-muted-foreground">{value} recordings</p>
                  </div>
                </div>
              )
            })}
          </div>
        </div>
      ))}

      {/* Legend */}
      <div className="flex items-center justify-end gap-2 mt-4">
        <span className="text-[10px] text-muted-foreground">Less</span>
        <div className="flex gap-0.5">
          {[0, 0.25, 0.5, 0.75, 1].map((intensity, i) => (
            <div
              key={i}
              className="w-3 h-3 rounded-sm"
              style={{
                backgroundColor:
                  intensity > 0
                    ? `rgba(20, 184, 166, ${0.15 + intensity * 0.85})`
                    : 'rgba(255, 255, 255, 0.03)',
              }}
            />
          ))}
        </div>
        <span className="text-[10px] text-muted-foreground">More</span>
      </div>
    </div>
  )
}

// Generate mock heatmap data for demo
export function generateMockHeatmapData(): HeatMapData[] {
  const data: HeatMapData[] = []
  DAYS.forEach((day) => {
    HOURS.forEach((hour) => {
      // Simulate higher activity during work hours
      const isWorkHour = hour >= 9 && hour <= 18
      const isWeekday = !['Sat', 'Sun'].includes(day)
      const baseValue = isWorkHour && isWeekday ? 5 : 1
      data.push({
        day,
        hour,
        value: Math.floor(Math.random() * baseValue * 3),
      })
    })
  })
  return data
}
