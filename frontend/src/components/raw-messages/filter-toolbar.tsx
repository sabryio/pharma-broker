// Filter Toolbar Component for Raw Messages
import { useCallback, useState } from 'react'
import { format } from 'date-fns'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { Calendar } from '@/components/ui/calendar'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { Search, X, CalendarIcon, RefreshCw, Zap } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { RawMessageFilters, ProcessingStatus } from './types'
import { defaultFilters } from './types'

interface FilterToolbarProps {
  filters: RawMessageFilters
  onFiltersChange: (filters: RawMessageFilters) => void
  onRefresh: () => void
  isRefreshing: boolean
  totalCount: number
  onAutoReprocess?: () => void
  isAutoReprocessing?: boolean
}

export function FilterToolbar({
  filters,
  onFiltersChange,
  onRefresh,
  isRefreshing,
  totalCount,
  onAutoReprocess,
  isAutoReprocessing = false,
}: FilterToolbarProps) {
  const [startDateOpen, setStartDateOpen] = useState(false)
  const [endDateOpen, setEndDateOpen] = useState(false)

  const updateFilter = useCallback(
    <K extends keyof RawMessageFilters>(
      key: K,
      value: RawMessageFilters[K],
    ) => {
      onFiltersChange({ ...filters, [key]: value })
    },
    [filters, onFiltersChange],
  )

  const clearFilters = useCallback(() => {
    onFiltersChange(defaultFilters)
  }, [onFiltersChange])

  const hasActiveFilters =
    filters.search ||
    filters.status !== 'all' ||
    filters.startDate ||
    filters.endDate

  // Parse dates for calendar
  const startDate = filters.startDate ? new Date(filters.startDate) : undefined
  const endDate = filters.endDate ? new Date(filters.endDate) : undefined

  const handleStartDateSelect = (date: Date | undefined) => {
    updateFilter('startDate', date ? format(date, 'yyyy-MM-dd') : '')
    setStartDateOpen(false)
  }

  const handleEndDateSelect = (date: Date | undefined) => {
    updateFilter('endDate', date ? format(date, 'yyyy-MM-dd') : '')
    setEndDateOpen(false)
  }

  return (
    <div className="flex items-center gap-2 p-2 bg-muted/30 rounded-lg border">
      {/* Search */}
      <div className="relative flex-1 max-w-xs">
        <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground" />
        <Input
          placeholder="Search messages..."
          value={filters.search}
          onChange={(e) => updateFilter('search', e.target.value)}
          className="h-8 pl-8 text-sm bg-background"
        />
      </div>

      {/* Status Filter */}
      <Select
        value={filters.status}
        onValueChange={(v) => updateFilter('status', v as ProcessingStatus)}
      >
        <SelectTrigger className="h-8 w-[120px] text-sm">
          <SelectValue placeholder="Status" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="all">All</SelectItem>
          <SelectItem value="processed">Processed</SelectItem>
          <SelectItem value="unprocessed">Pending</SelectItem>
          <SelectItem value="error">Error</SelectItem>
        </SelectContent>
      </Select>

      {/* Date Range with Calendar Picker */}
      <div className="flex items-center gap-1">
        {/* Start Date */}
        <Popover open={startDateOpen} onOpenChange={setStartDateOpen}>
          <PopoverTrigger asChild>
            <Button
              variant="outline"
              size="sm"
              className={cn(
                'h-8 px-2 text-xs justify-start font-normal',
                !startDate && 'text-muted-foreground',
                startDate && 'border-primary/50',
              )}
            >
              <CalendarIcon className="mr-1.5 h-3 w-3" />
              {startDate ? format(startDate, 'MMM d') : 'From'}
            </Button>
          </PopoverTrigger>
          <PopoverContent className="w-auto p-0" align="start">
            <Calendar
              mode="single"
              selected={startDate}
              onSelect={handleStartDateSelect}
              disabled={(date) => (endDate ? date > endDate : false)}
              initialFocus
            />
          </PopoverContent>
        </Popover>

        <span className="text-muted-foreground text-xs">→</span>

        {/* End Date */}
        <Popover open={endDateOpen} onOpenChange={setEndDateOpen}>
          <PopoverTrigger asChild>
            <Button
              variant="outline"
              size="sm"
              className={cn(
                'h-8 px-2 text-xs justify-start font-normal',
                !endDate && 'text-muted-foreground',
                endDate && 'border-primary/50',
              )}
            >
              <CalendarIcon className="mr-1.5 h-3 w-3" />
              {endDate ? format(endDate, 'MMM d') : 'To'}
            </Button>
          </PopoverTrigger>
          <PopoverContent className="w-auto p-0" align="start">
            <Calendar
              mode="single"
              selected={endDate}
              onSelect={handleEndDateSelect}
              disabled={(date) => (startDate ? date < startDate : false)}
              initialFocus
            />
          </PopoverContent>
        </Popover>

        {/* Clear Date Range */}
        {(startDate || endDate) && (
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8"
                onClick={() => {
                  updateFilter('startDate', '')
                  updateFilter('endDate', '')
                }}
              >
                <X className="h-3 w-3" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Clear dates</TooltipContent>
          </Tooltip>
        )}
      </div>

      {/* Clear All Filters */}
      {hasActiveFilters && (
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              onClick={clearFilters}
              className="h-8 px-2 text-xs text-muted-foreground hover:text-foreground"
            >
              <X className="w-3.5 h-3.5 mr-1" />
              Clear
            </Button>
          </TooltipTrigger>
          <TooltipContent>Clear all filters</TooltipContent>
        </Tooltip>
      )}

      {/* Spacer */}
      <div className="flex-1" />

      {/* Count */}
      <span className="text-xs text-muted-foreground tabular-nums">
        {totalCount.toLocaleString()} messages
      </span>

      {/* Auto Reprocess Failed Messages */}
      {onAutoReprocess && (
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="outline"
              size="sm"
              onClick={onAutoReprocess}
              disabled={isAutoReprocessing}
              className={cn(
                'h-8 px-3 gap-1.5',
                'bg-linear-to-r from-amber-500/10 to-orange-500/10',
                'border-amber-500/30 hover:border-amber-500/50',
                'text-amber-600 dark:text-amber-400',
                'hover:from-amber-500/20 hover:to-orange-500/20',
                'transition-all duration-200',
                isAutoReprocessing && 'animate-pulse',
              )}
            >
              <Zap
                className={cn(
                  'w-3.5 h-3.5',
                  isAutoReprocessing && 'animate-spin',
                )}
              />
              <span className="text-xs font-medium">
                {isAutoReprocessing ? 'Processing...' : 'Fix Failed'}
              </span>
            </Button>
          </TooltipTrigger>
          <TooltipContent>
            Automatically reprocess all failed messages
          </TooltipContent>
        </Tooltip>
      )}

      {/* Refresh */}
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="sm"
            onClick={onRefresh}
            disabled={isRefreshing}
            className="h-8 px-2"
          >
            <RefreshCw
              className={cn('w-3.5 h-3.5', isRefreshing && 'animate-spin')}
            />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Refresh</TooltipContent>
      </Tooltip>
    </div>
  )
}
