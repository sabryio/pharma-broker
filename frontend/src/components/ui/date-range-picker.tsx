// Date Range Picker Component
// Reusable date range picker using Shadcn Calendar and Popover

import { useState } from 'react'
import { format } from 'date-fns'
import { Calendar as CalendarIcon, X } from 'lucide-react'
import { Calendar } from '@/components/ui/calendar'
import { Button } from '@/components/ui/button'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { cn } from '@/lib/utils'

interface DateRangePickerProps {
  startDate: Date | undefined
  endDate: Date | undefined
  onStartDateChange: (date: Date | undefined) => void
  onEndDateChange: (date: Date | undefined) => void
  className?: string
  compact?: boolean
}

export function DateRangePicker({
  startDate,
  endDate,
  onStartDateChange,
  onEndDateChange,
  className,
  compact = false,
}: DateRangePickerProps) {
  const [startOpen, setStartOpen] = useState(false)
  const [endOpen, setEndOpen] = useState(false)

  const hasValue = startDate || endDate

  return (
    <div className={cn('flex items-center gap-1', className)}>
      {/* Start Date */}
      <Popover open={startOpen} onOpenChange={setStartOpen}>
        <PopoverTrigger asChild>
          <Button
            variant="outline"
            size={compact ? 'sm' : 'default'}
            className={cn(
              'justify-start text-left font-normal',
              compact ? 'h-8 px-2 text-xs' : 'h-9 px-3 text-sm',
              !startDate && 'text-muted-foreground',
              startDate && 'border-primary/50',
            )}
          >
            <CalendarIcon className={cn('mr-1.5', compact ? 'h-3 w-3' : 'h-3.5 w-3.5')} />
            {startDate ? format(startDate, 'MMM d') : 'From'}
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-auto p-0" align="start">
          <Calendar
            mode="single"
            selected={startDate}
            onSelect={(date) => {
              onStartDateChange(date)
              setStartOpen(false)
            }}
            disabled={(date) => (endDate ? date > endDate : false)}
            initialFocus
          />
        </PopoverContent>
      </Popover>

      <span className="text-muted-foreground text-xs">→</span>

      {/* End Date */}
      <Popover open={endOpen} onOpenChange={setEndOpen}>
        <PopoverTrigger asChild>
          <Button
            variant="outline"
            size={compact ? 'sm' : 'default'}
            className={cn(
              'justify-start text-left font-normal',
              compact ? 'h-8 px-2 text-xs' : 'h-9 px-3 text-sm',
              !endDate && 'text-muted-foreground',
              endDate && 'border-primary/50',
            )}
          >
            <CalendarIcon className={cn('mr-1.5', compact ? 'h-3 w-3' : 'h-3.5 w-3.5')} />
            {endDate ? format(endDate, 'MMM d') : 'To'}
          </Button>
        </PopoverTrigger>
        <PopoverContent className="w-auto p-0" align="start">
          <Calendar
            mode="single"
            selected={endDate}
            onSelect={(date) => {
              onEndDateChange(date)
              setEndOpen(false)
            }}
            disabled={(date) => (startDate ? date < startDate : false)}
            initialFocus
          />
        </PopoverContent>
      </Popover>

      {/* Clear Button */}
      {hasValue && (
        <Button
          variant="ghost"
          size="icon"
          className={cn(compact ? 'h-8 w-8' : 'h-9 w-9')}
          onClick={() => {
            onStartDateChange(undefined)
            onEndDateChange(undefined)
          }}
        >
          <X className={cn(compact ? 'h-3 w-3' : 'h-3.5 w-3.5')} />
        </Button>
      )}
    </div>
  )
}
