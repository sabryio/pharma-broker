// Filter Toolbar Component for Raw Messages
import { useCallback } from 'react'
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
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { Search, X, Calendar, RefreshCw } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { RawMessageFilters, ProcessingStatus } from './types'
import { defaultFilters } from './types'

interface FilterToolbarProps {
  filters: RawMessageFilters
  onFiltersChange: (filters: RawMessageFilters) => void
  onRefresh: () => void
  isRefreshing: boolean
  totalCount: number
}

export function FilterToolbar({
  filters,
  onFiltersChange,
  onRefresh,
  isRefreshing,
  totalCount,
}: FilterToolbarProps) {
  const updateFilter = useCallback(
    <K extends keyof RawMessageFilters>(key: K, value: RawMessageFilters[K]) => {
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

      {/* Date Range */}
      <div className="flex items-center gap-1">
        <Tooltip>
          <TooltipTrigger asChild>
            <div className="relative">
              <Calendar className="absolute left-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground pointer-events-none" />
              <Input
                type="date"
                value={filters.startDate}
                onChange={(e) => updateFilter('startDate', e.target.value)}
                className="h-8 w-[130px] pl-7 text-sm bg-background"
              />
            </div>
          </TooltipTrigger>
          <TooltipContent>Start date</TooltipContent>
        </Tooltip>
        <span className="text-muted-foreground text-xs">→</span>
        <Input
          type="date"
          value={filters.endDate}
          onChange={(e) => updateFilter('endDate', e.target.value)}
          className="h-8 w-[130px] text-sm bg-background"
        />
      </div>

      {/* Clear Filters */}
      {hasActiveFilters && (
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              onClick={clearFilters}
              className="h-8 px-2"
            >
              <X className="w-3.5 h-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Clear filters</TooltipContent>
        </Tooltip>
      )}

      {/* Spacer */}
      <div className="flex-1" />

      {/* Count */}
      <span className="text-xs text-muted-foreground tabular-nums">
        {totalCount.toLocaleString()} messages
      </span>

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
