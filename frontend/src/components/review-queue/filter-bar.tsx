// Filter Bar Component
// Advanced filtering and sorting for match reviews

import { useState, useCallback } from 'react'
import { cn } from '@/lib/utils'
import {
  Filter,
  SortAsc,
  SortDesc,
  X,
  Search,
  ChevronDown,
  Sparkles,
  Clock,
  TrendingUp,
  Package,
} from 'lucide-react'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { Input } from '@/components/ui/input'
import { Slider } from '@/components/ui/slider'

export interface FilterState {
  confidenceBand: 'all' | 'high' | 'medium' | 'low'
  medicationSearch: string
  sortBy: 'confidence' | 'age' | 'medication'
  sortOrder: 'asc' | 'desc'
  minConfidence: number
  maxConfidence: number
  aiStatusFilter: 'all' | 'approved' | 'flagged' | 'rejected' | 'pending'
}

export const defaultFilterState: FilterState = {
  confidenceBand: 'all',
  medicationSearch: '',
  sortBy: 'confidence',
  sortOrder: 'desc',
  minConfidence: 0,
  maxConfidence: 100,
  aiStatusFilter: 'all',
}

interface FilterBarProps {
  filters: FilterState
  onFiltersChange: (filters: FilterState) => void
  totalCount: number
  filteredCount: number
}

const confidenceBands = [
  { value: 'all', label: 'All', color: 'bg-secondary' },
  {
    value: 'high',
    label: 'High (80%+)',
    color: 'bg-emerald/20 text-emerald border-emerald/30',
  },
  {
    value: 'medium',
    label: 'Medium (50-80%)',
    color: 'bg-amber/20 text-amber border-amber/30',
  },
  {
    value: 'low',
    label: 'Low (<50%)',
    color: 'bg-red-400/20 text-red-400 border-red-400/30',
  },
] as const

const aiStatusOptions = [
  { value: 'all', label: 'All AI Status', icon: Sparkles },
  { value: 'approved', label: 'AI Approved', color: 'text-emerald' },
  { value: 'flagged', label: 'AI Flagged', color: 'text-amber' },
  { value: 'rejected', label: 'AI Rejected', color: 'text-red-400' },
  { value: 'pending', label: 'Pending Review', color: 'text-muted-foreground' },
] as const

const sortOptions = [
  { value: 'confidence', label: 'Confidence', icon: TrendingUp },
  { value: 'age', label: 'Age', icon: Clock },
  { value: 'medication', label: 'Medication', icon: Package },
] as const

export function FilterBar({
  filters,
  onFiltersChange,
  totalCount,
  filteredCount,
}: FilterBarProps) {
  const [isAdvancedOpen, setIsAdvancedOpen] = useState(false)

  const updateFilter = useCallback(
    <K extends keyof FilterState>(key: K, value: FilterState[K]) => {
      onFiltersChange({ ...filters, [key]: value })
    },
    [filters, onFiltersChange],
  )

  const resetFilters = useCallback(() => {
    onFiltersChange(defaultFilterState)
  }, [onFiltersChange])

  const hasActiveFilters =
    filters.confidenceBand !== 'all' ||
    filters.medicationSearch !== '' ||
    filters.aiStatusFilter !== 'all' ||
    filters.minConfidence > 0 ||
    filters.maxConfidence < 100

  const activeFilterCount = [
    filters.confidenceBand !== 'all',
    filters.medicationSearch !== '',
    filters.aiStatusFilter !== 'all',
    filters.minConfidence > 0 || filters.maxConfidence < 100,
  ].filter(Boolean).length

  return (
    <div className="space-y-3">
      {/* Main Filter Row */}
      <div className="flex items-center gap-3 flex-wrap">
        {/* Search */}
        <div className="relative flex-1 min-w-[200px] max-w-[300px]">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
          <Input
            placeholder="Search medication..."
            value={filters.medicationSearch}
            onChange={(e) => updateFilter('medicationSearch', e.target.value)}
            className="pl-9 h-9 bg-secondary/50 border-border/50"
          />
          {filters.medicationSearch && (
            <button
              onClick={() => updateFilter('medicationSearch', '')}
              className="absolute right-2 top-1/2 -translate-y-1/2 p-1 rounded-full hover:bg-secondary"
            >
              <X className="w-3 h-3 text-muted-foreground" />
            </button>
          )}
        </div>

        {/* Confidence Band Chips */}
        <div className="flex items-center gap-1.5">
          {confidenceBands.map((band) => (
            <button
              key={band.value}
              onClick={() =>
                updateFilter(
                  'confidenceBand',
                  band.value as FilterState['confidenceBand'],
                )
              }
              className={cn(
                'px-3 py-1.5 rounded-full text-xs font-medium transition-all duration-200',
                'border hover:scale-105 active:scale-95',
                filters.confidenceBand === band.value
                  ? band.value === 'all'
                    ? 'bg-teal/20 text-teal border-teal/30'
                    : band.color
                  : 'bg-secondary/50 text-muted-foreground border-border/50 hover:border-border',
              )}
            >
              {band.label}
            </button>
          ))}
        </div>

        {/* Sort Dropdown */}
        <Popover>
          <PopoverTrigger asChild>
            <button
              className={cn(
                'flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium',
                'bg-secondary/50 border border-border/50 hover:border-border',
                'transition-all duration-200',
              )}
            >
              {filters.sortOrder === 'desc' ? (
                <SortDesc className="w-3.5 h-3.5" />
              ) : (
                <SortAsc className="w-3.5 h-3.5" />
              )}
              <span>
                {sortOptions.find((o) => o.value === filters.sortBy)?.label}
              </span>
              <ChevronDown className="w-3 h-3 text-muted-foreground" />
            </button>
          </PopoverTrigger>
          <PopoverContent className="w-48 p-2" align="end">
            <div className="space-y-1">
              {sortOptions.map((option) => (
                <button
                  key={option.value}
                  onClick={() => {
                    if (filters.sortBy === option.value) {
                      updateFilter(
                        'sortOrder',
                        filters.sortOrder === 'desc' ? 'asc' : 'desc',
                      )
                    } else {
                      updateFilter(
                        'sortBy',
                        option.value as FilterState['sortBy'],
                      )
                    }
                  }}
                  className={cn(
                    'w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm',
                    'transition-colors',
                    filters.sortBy === option.value
                      ? 'bg-teal/10 text-teal'
                      : 'hover:bg-secondary',
                  )}
                >
                  <option.icon className="w-4 h-4" />
                  <span className="flex-1 text-left">{option.label}</span>
                  {filters.sortBy === option.value && (
                    <span className="text-xs text-muted-foreground">
                      {filters.sortOrder === 'desc' ? '↓' : '↑'}
                    </span>
                  )}
                </button>
              ))}
            </div>
          </PopoverContent>
        </Popover>

        {/* Advanced Filters Toggle */}
        <button
          onClick={() => setIsAdvancedOpen(!isAdvancedOpen)}
          className={cn(
            'flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium',
            'transition-all duration-200',
            isAdvancedOpen || hasActiveFilters
              ? 'bg-violet-500/20 text-violet-400 border border-violet-500/30'
              : 'bg-secondary/50 text-muted-foreground border border-border/50 hover:border-border',
          )}
        >
          <Filter className="w-3.5 h-3.5" />
          <span>Filters</span>
          {activeFilterCount > 0 && (
            <span className="w-4 h-4 rounded-full bg-violet-500 text-white text-[10px] flex items-center justify-center">
              {activeFilterCount}
            </span>
          )}
        </button>

        {/* Reset Button */}
        {hasActiveFilters && (
          <button
            onClick={resetFilters}
            className="flex items-center gap-1.5 px-2 py-1.5 rounded-lg text-xs text-muted-foreground hover:text-foreground transition-colors"
          >
            <X className="w-3 h-3" />
            Reset
          </button>
        )}

        {/* Results Count */}
        <div className="ml-auto text-xs text-muted-foreground">
          Showing{' '}
          <span className="font-medium text-foreground">{filteredCount}</span>{' '}
          of <span className="font-medium text-foreground">{totalCount}</span>
        </div>
      </div>

      {/* Advanced Filters Panel */}
      {isAdvancedOpen && (
        <div className="p-4 rounded-xl bg-secondary/30 border border-border/50 animate-fade-in">
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
            {/* Confidence Range */}
            <div className="space-y-3">
              <label className="text-xs font-medium text-foreground flex items-center gap-2">
                <TrendingUp className="w-3.5 h-3.5 text-teal" />
                Confidence Range
              </label>
              <div className="px-2">
                <Slider
                  value={[
                    filters.minConfidence ?? 0,
                    filters.maxConfidence ?? 100,
                  ]}
                  onValueChange={([min, max]) => {
                    onFiltersChange({
                      ...filters,
                      minConfidence: min ?? 0,
                      maxConfidence: max ?? 100,
                    })
                  }}
                  min={0}
                  max={100}
                  step={5}
                  className="w-full"
                />
                <div className="flex justify-between mt-2 text-xs text-muted-foreground">
                  <span>{filters.minConfidence}%</span>
                  <span>{filters.maxConfidence}%</span>
                </div>
              </div>
            </div>

            {/* AI Status Filter */}
            <div className="space-y-3">
              <label className="text-xs font-medium text-foreground flex items-center gap-2">
                <Sparkles className="w-3.5 h-3.5 text-violet-400" />
                AI Review Status
              </label>
              <div className="flex flex-wrap gap-1.5">
                {aiStatusOptions.map((option) => (
                  <button
                    key={option.value}
                    onClick={() =>
                      updateFilter(
                        'aiStatusFilter',
                        option.value as FilterState['aiStatusFilter'],
                      )
                    }
                    className={cn(
                      'px-2.5 py-1 rounded-full text-xs transition-all duration-200',
                      'border',
                      filters.aiStatusFilter === option.value
                        ? 'bg-violet-500/20 text-violet-400 border-violet-500/30'
                        : 'bg-secondary/50 text-muted-foreground border-border/50 hover:border-border',
                    )}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            </div>

            {/* Quick Presets */}
            <div className="space-y-3">
              <label className="text-xs font-medium text-foreground">
                Quick Presets
              </label>
              <div className="flex flex-wrap gap-1.5">
                <button
                  onClick={() =>
                    onFiltersChange({
                      ...defaultFilterState,
                      confidenceBand: 'low',
                      sortBy: 'confidence',
                      sortOrder: 'asc',
                    })
                  }
                  className="px-2.5 py-1 rounded-full text-xs bg-red-400/10 text-red-400 border border-red-400/30 hover:bg-red-400/20 transition-colors"
                >
                  🔥 Needs Review
                </button>
                <button
                  onClick={() =>
                    onFiltersChange({
                      ...defaultFilterState,
                      confidenceBand: 'high',
                      aiStatusFilter: 'approved',
                    })
                  }
                  className="px-2.5 py-1 rounded-full text-xs bg-emerald/10 text-emerald border border-emerald/30 hover:bg-emerald/20 transition-colors"
                >
                  ✓ Quick Approve
                </button>
                <button
                  onClick={() =>
                    onFiltersChange({
                      ...defaultFilterState,
                      aiStatusFilter: 'flagged',
                    })
                  }
                  className="px-2.5 py-1 rounded-full text-xs bg-amber/10 text-amber border border-amber/30 hover:bg-amber/20 transition-colors"
                >
                  ⚠️ AI Flagged
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
