import { useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Calendar as CalendarComponent } from '@/components/ui/calendar'
import { Input } from '@/components/ui/input'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { cn } from '@/lib/utils'
import { format } from 'date-fns'
import {
  Calendar,
  CheckCircle,
  Filter,
  Search,
  Undo2,
  X,
  XCircle,
} from 'lucide-react'
import type { HistoryEntry } from './types'

interface HistoryLogProps {
  history: HistoryEntry[]
  onRestore: (id: string) => void
}

function formatTime(date: Date): string {
  return date.toLocaleTimeString('en-US', {
    hour: '2-digit',
    minute: '2-digit',
    hour12: true,
  })
}

function formatDate(date: Date): string {
  return date.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  })
}

export function HistoryLog({ history, onRestore }: HistoryLogProps) {
  const [searchQuery, setSearchQuery] = useState('')
  const [actionFilter, setActionFilter] = useState<
    'all' | 'approved' | 'rejected'
  >('all')
  const [dateFrom, setDateFrom] = useState<Date | undefined>(undefined)
  const [dateTo, setDateTo] = useState<Date | undefined>(undefined)
  const [showFilters, setShowFilters] = useState(false)

  const filteredHistory = history.filter((entry) => {
    const matchesSearch =
      searchQuery === '' ||
      entry.product.toLowerCase().includes(searchQuery.toLowerCase()) ||
      entry.originalReview.offer.source
        .toLowerCase()
        .includes(searchQuery.toLowerCase()) ||
      entry.originalReview.request.source
        .toLowerCase()
        .includes(searchQuery.toLowerCase())

    const matchesAction =
      actionFilter === 'all' || entry.action === actionFilter

    const entryDate = new Date(entry.timestamp)
    entryDate.setHours(0, 0, 0, 0)

    const matchesDateFrom = !dateFrom || entryDate >= dateFrom
    const matchesDateTo = !dateTo || entryDate <= dateTo

    return matchesSearch && matchesAction && matchesDateFrom && matchesDateTo
  })

  const clearFilters = () => {
    setSearchQuery('')
    setActionFilter('all')
    setDateFrom(undefined)
    setDateTo(undefined)
  }

  const hasActiveFilters =
    searchQuery !== '' || actionFilter !== 'all' || dateFrom || dateTo

  if (history.length === 0) {
    return (
      <div className="glass-card p-6 rounded-xl animate-fade-in">
        <h3 className="text-lg font-semibold text-foreground mb-4">
          Review History
        </h3>
        <p className="text-sm text-muted-foreground text-center py-8">
          No review decisions yet.
        </p>
      </div>
    )
  }

  return (
    <div className="glass-card p-6 rounded-xl animate-fade-in">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold text-foreground">
          Review History
        </h3>
        <div className="flex items-center gap-4 text-sm">
          <span className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-emerald" />
            Approved: {history.filter((h) => h.action === 'approved').length}
          </span>
          <span className="flex items-center gap-2">
            <div className="w-3 h-3 rounded-full bg-destructive" />
            Rejected: {history.filter((h) => h.action === 'rejected').length}
          </span>
        </div>
      </div>

      {/* Search and Filter Controls */}
      <div className="space-y-3 mb-4">
        <div className="flex items-center gap-3">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
            <Input
              type="text"
              placeholder="Search by product or source..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-10 bg-secondary/50 border-border"
            />
            {searchQuery && (
              <button
                onClick={() => setSearchQuery('')}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
              >
                <XCircle className="w-4 h-4" />
              </button>
            )}
          </div>

          <button
            onClick={() => setShowFilters(!showFilters)}
            className={cn(
              'flex items-center gap-2 px-4 py-2 rounded-lg border transition-colors',
              showFilters || hasActiveFilters
                ? 'bg-teal/20 border-teal/50 text-teal'
                : 'bg-secondary/50 border-border text-muted-foreground hover:text-foreground',
            )}
          >
            <Filter className="w-4 h-4" />
            Filters
            {hasActiveFilters && (
              <span className="w-2 h-2 rounded-full bg-teal" />
            )}
          </button>
        </div>

        {showFilters && (
          <div className="flex flex-wrap items-center gap-3 p-4 rounded-lg bg-secondary/30 border border-border animate-fade-in">
            <div className="flex items-center gap-2">
              <span className="text-sm text-muted-foreground">Action:</span>
              <Select
                value={actionFilter}
                onValueChange={(v) =>
                  setActionFilter(v as 'all' | 'approved' | 'rejected')
                }
              >
                <SelectTrigger className="w-[130px] bg-background border-border">
                  <SelectValue placeholder="All actions" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">All Actions</SelectItem>
                  <SelectItem value="approved">Approved</SelectItem>
                  <SelectItem value="rejected">Rejected</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="flex items-center gap-2">
              <span className="text-sm text-muted-foreground">From:</span>
              <Popover>
                <PopoverTrigger asChild>
                  <button
                    className={cn(
                      'flex items-center gap-2 px-3 py-2 rounded-lg border text-sm transition-colors',
                      dateFrom
                        ? 'bg-background border-teal/50 text-foreground'
                        : 'bg-background border-border text-muted-foreground',
                    )}
                  >
                    <Calendar className="w-4 h-4" />
                    {dateFrom ? format(dateFrom, 'MMM d, yyyy') : 'Start date'}
                  </button>
                </PopoverTrigger>
                <PopoverContent className="w-auto p-0" align="start">
                  <CalendarComponent
                    mode="single"
                    selected={dateFrom}
                    onSelect={setDateFrom}
                    initialFocus
                    className="pointer-events-auto"
                  />
                </PopoverContent>
              </Popover>
              {dateFrom && (
                <button
                  onClick={() => setDateFrom(undefined)}
                  className="text-muted-foreground hover:text-foreground"
                >
                  <XCircle className="w-4 h-4" />
                </button>
              )}
            </div>

            <div className="flex items-center gap-2">
              <span className="text-sm text-muted-foreground">To:</span>
              <Popover>
                <PopoverTrigger asChild>
                  <button
                    className={cn(
                      'flex items-center gap-2 px-3 py-2 rounded-lg border text-sm transition-colors',
                      dateTo
                        ? 'bg-background border-teal/50 text-foreground'
                        : 'bg-background border-border text-muted-foreground',
                    )}
                  >
                    <Calendar className="w-4 h-4" />
                    {dateTo ? format(dateTo, 'MMM d, yyyy') : 'End date'}
                  </button>
                </PopoverTrigger>
                <PopoverContent className="w-auto p-0" align="start">
                  <CalendarComponent
                    mode="single"
                    selected={dateTo}
                    onSelect={setDateTo}
                    initialFocus
                    className="pointer-events-auto"
                  />
                </PopoverContent>
              </Popover>
              {dateTo && (
                <button
                  onClick={() => setDateTo(undefined)}
                  className="text-muted-foreground hover:text-foreground"
                >
                  <XCircle className="w-4 h-4" />
                </button>
              )}
            </div>

            {hasActiveFilters && (
              <button
                onClick={clearFilters}
                className="flex items-center gap-2 px-3 py-2 rounded-lg bg-destructive/10 text-destructive text-sm hover:bg-destructive/20 transition-colors ml-auto"
              >
                <XCircle className="w-4 h-4" />
                Clear All
              </button>
            )}
          </div>
        )}
      </div>

      {hasActiveFilters && (
        <div className="text-sm text-muted-foreground mb-3">
          Showing {filteredHistory.length} of {history.length} results
        </div>
      )}

      <div className="space-y-3 max-h-80 overflow-y-auto">
        {filteredHistory.length === 0 ? (
          <div className="text-center py-8">
            <Search className="w-8 h-8 text-muted-foreground mx-auto mb-2" />
            <p className="text-sm text-muted-foreground">
              No matching results found.
            </p>
            <button
              onClick={clearFilters}
              className="text-sm text-teal hover:underline mt-2"
            >
              Clear filters
            </button>
          </div>
        ) : (
          filteredHistory.map((entry) => (
            <div
              key={entry.id}
              className={cn(
                'flex items-center gap-4 p-4 rounded-lg border transition-colors group',
                entry.action === 'approved'
                  ? 'bg-emerald/5 border-emerald/20'
                  : 'bg-destructive/5 border-destructive/20',
              )}
            >
              <div
                className={cn(
                  'w-10 h-10 rounded-full flex items-center justify-center shrink-0',
                  entry.action === 'approved'
                    ? 'bg-emerald/20 text-emerald'
                    : 'bg-destructive/20 text-destructive',
                )}
              >
                {entry.action === 'approved' ? (
                  <CheckCircle className="w-5 h-5" />
                ) : (
                  <X className="w-5 h-5" />
                )}
              </div>

              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-sm font-medium text-foreground truncate">
                    {entry.product}
                  </span>
                  <Badge
                    variant="outline"
                    className={cn(
                      'text-xs capitalize',
                      entry.action === 'approved'
                        ? 'border-emerald/50 text-emerald'
                        : 'border-destructive/50 text-destructive',
                    )}
                  >
                    {entry.action}
                  </Badge>
                </div>
                <p className="text-xs text-muted-foreground">
                  Confidence: {entry.confidence}% | Adjustments: P
                  {entry.adjustments.priceFlexibility}% Q
                  {entry.adjustments.quantityTolerance}% D
                  {entry.adjustments.dosageStrictness}%
                </p>
              </div>

              <div className="text-right shrink-0">
                <p className="text-sm text-foreground">
                  {formatTime(entry.timestamp)}
                </p>
                <p className="text-xs text-muted-foreground">
                  {formatDate(entry.timestamp)}
                </p>
              </div>

              <button
                onClick={() => onRestore(entry.id)}
                className="opacity-0 group-hover:opacity-100 flex items-center gap-1 px-3 py-1.5 rounded-lg bg-amber/20 text-amber border border-amber/30 text-xs font-medium hover:bg-amber/30 transition-all"
              >
                <Undo2 className="w-3 h-3" />
                Restore
              </button>
            </div>
          ))
        )}
      </div>
    </div>
  )
}
