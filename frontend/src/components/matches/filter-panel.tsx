// Filter Panel Component
// Advanced filtering and sorting for matches

import { useState, useCallback } from 'react'
import { cn } from '@/lib/utils'
import {
  Filter,
  SortAsc,
  SortDesc,
  X,
  Search,
  ChevronDown,
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
import type { MatchStatus } from '@/schema/match-review'

export interface FilterState {
  status: 'all' | MatchStatus
  minConfidence: number
  maxConfidence: number
  confidenceBand: 'all' | 'high' | 'medium' | 'low'
  medicationSearch: string
  sortBy: 'confidence' | 'date' | 'medication'
  sortOrder: 'asc' | 'desc'
}

export const defaultFilterState: FilterState = {
  status: 'all',
  minConfidence: 0,
  maxConfidence: 100,
  confidenceBand: 'all',
  medicationSearch: '',
  sortBy: 'confidence',
  sortOrder: 'desc',
}

// Confidence band thresholds
export const CONFIDENCE_BANDS = {
  high: { min: 80, max: 100 },
  medium: { min: 50, max: 79 },
  low: { min: 0, max: 49 },
} as const

interface FilterPanelProps {
  filters: FilterState
  onFiltersChange: (filters: FilterState) => void
  totalCount: number
  filteredCount: number
}

const statusOptions = [
  { value: 'all', label: 'All Status' },
  { value: 'PENDING', label: 'Pending' },
  { value: 'CONFIRMED', label: 'Confirmed' },
  { value: 'REJECTED', label: 'Rejected' },
  { value: 'EXPIRED', label: 'Expired' },
] as const

const confidenceBands = [
  { value: 'all', label: 'All', color: 'bg-secondary' },
  {
    value: 'high',
    label: 'High (≥80%)',
    color: 'bg-emerald/20 text-emerald border-emerald/30',
  },
  {
    value: 'medium',
    label: 'Medium (50-79%)',
    color: 'bg-amber/20 text-amber border-amber/30',
  },
  {
    value: 'low',
    label: 'Low (<50%)',
    color: 'bg-red-400/20 text-red-400 border-red-400/30',
  },
] as const

const sortOptions = [
  { value: 'confidence', label: 'Confidence', icon: TrendingUp },
  { value: 'date', label: 'Date', icon: Clock },
  { value: 'medication', label: 'Medication', icon: Package },
] as const

export function FilterPanel({
  filters,
  onFiltersChange,
  totalCount,
  filteredCount,
}: FilterPanelProps) {
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
    filters.status !== 'all' ||
    filters.confidenceBand !== 'all' ||
    filters.medicationSearch !== '' ||
    filters.minConfidence > 0 ||
    filters.maxConfidence < 100

  const activeFilterCount = [
    filters.status !== 'all',
    filters.confidenceBand !== 'all',
    filters.medicationSearch !== '',
    filters.minConfidence > 0 || filters.maxConfidence < 100,
  ].filter(Boolean).length

  return (
    <div className="space-y-3">
      {/* Main Filter Row */}
      <div className="glass-card p-4 rounded-xl">
        <div className="flex items-center gap-3 flex-wrap">
          {/* Filter Icon */}
          <div className="flex items-center gap-2">
            <Filter className="w-4 h-4 text-muted-foreground" />
            <span className="text-sm font-medium text-foreground">Filters</span>
          </div>

          {/* Status Filter */}
          <select
            value={filters.status}
            onChange={(e) =>
              updateFilter('status', e.target.value as FilterState['status'])
            }
            className="h-9 px-3 rounded-lg bg-secondary/50 border border-border text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-teal/30"
          >
            {statusOptions.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>

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

          {/* Search */}
          <div className="relative flex-1 min-w-[200px]">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
            <Input
              placeholder="Search medications..."
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
            <TrendingUp className="w-3.5 h-3.5" />
            <span>Advanced</span>
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
          <div className="ml-auto text-sm text-muted-foreground">
            <span className="font-medium text-foreground">{filteredCount}</span>{' '}
            of <span className="font-medium text-foreground">{totalCount}</span>{' '}
            matches
          </div>
        </div>
      </div>

      {/* Advanced Filters Panel */}
      {isAdvancedOpen && (
        <div className="p-4 rounded-xl bg-secondary/30 border border-border/50 animate-fade-in">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
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
                      status: 'PENDING',
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
                      status: 'PENDING',
                      confidenceBand: 'high',
                    })
                  }
                  className="px-2.5 py-1 rounded-full text-xs bg-emerald/10 text-emerald border border-emerald/30 hover:bg-emerald/20 transition-colors"
                >
                  ✓ High Confidence
                </button>
                <button
                  onClick={() =>
                    onFiltersChange({
                      ...defaultFilterState,
                      status: 'CONFIRMED',
                      sortBy: 'date',
                      sortOrder: 'desc',
                    })
                  }
                  className="px-2.5 py-1 rounded-full text-xs bg-teal/10 text-teal border border-teal/30 hover:bg-teal/20 transition-colors"
                >
                  📋 Recent Confirmed
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
