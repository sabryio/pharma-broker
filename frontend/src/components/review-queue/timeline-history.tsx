// Timeline History Component
// Enhanced vertical timeline with visual age indicators and date grouping

import { useState, useEffect, useMemo } from 'react'
import { cn } from '@/lib/utils'
import {
  CheckCircle,
  X,
  Undo2,
  Clock,
  ChevronDown,
  ChevronRight,
  Calendar,
  Sparkles,
  Filter,
  Search,
  TrendingUp,
  TrendingDown,
  Minus,
} from 'lucide-react'
import type { HistoryEntry } from './types'

interface TimelineHistoryProps {
  history: HistoryEntry[]
  onRestore: (id: string) => void
  maxHeight?: string
}

type FilterType = 'all' | 'approved' | 'rejected'

// Time helpers
function getRelativeTime(date: Date): string {
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffSecs = Math.floor(diffMs / 1000)
  const diffMins = Math.floor(diffSecs / 60)
  const diffHours = Math.floor(diffMins / 60)
  const diffDays = Math.floor(diffHours / 24)

  if (diffSecs < 60) return 'just now'
  if (diffMins < 60) return `${diffMins}m ago`
  if (diffHours < 24) return `${diffHours}h ago`
  if (diffDays === 1) return 'yesterday'
  if (diffDays < 7) return `${diffDays}d ago`
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

function getAgeIndicator(date: Date): { color: string; label: string } {
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMins = Math.floor(diffMs / 1000 / 60)

  if (diffMins < 5) return { color: 'bg-emerald-500', label: 'Fresh' }
  if (diffMins < 30) return { color: 'bg-teal', label: 'Recent' }
  if (diffMins < 60) return { color: 'bg-amber-500', label: 'Aging' }
  return { color: 'bg-muted-foreground', label: 'Stale' }
}

function getDateGroup(date: Date): string {
  const now = new Date()
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())
  const yesterday = new Date(today.getTime() - 24 * 60 * 60 * 1000)
  const weekAgo = new Date(today.getTime() - 7 * 24 * 60 * 60 * 1000)

  const entryDate = new Date(date.getFullYear(), date.getMonth(), date.getDate())

  if (entryDate.getTime() === today.getTime()) return 'Today'
  if (entryDate.getTime() === yesterday.getTime()) return 'Yesterday'
  if (entryDate.getTime() > weekAgo.getTime()) return 'This Week'
  return 'Earlier'
}

// Timeline Entry Component
function TimelineEntry({
  entry,
  onRestore,
  isLast,
}: {
  entry: HistoryEntry
  onRestore: (id: string) => void
  isLast: boolean
}) {
  const [expanded, setExpanded] = useState(false)
  const [relativeTime, setRelativeTime] = useState(getRelativeTime(entry.timestamp))
  const age = getAgeIndicator(entry.timestamp)

  // Update relative time every minute
  useEffect(() => {
    const interval = setInterval(() => {
      setRelativeTime(getRelativeTime(entry.timestamp))
    }, 60000)
    return () => clearInterval(interval)
  }, [entry.timestamp])

  const isApproved = entry.action === 'approved'

  return (
    <div className="relative flex gap-4">
      {/* Timeline line */}
      {!isLast && (
        <div className="absolute left-[19px] top-10 bottom-0 w-0.5 bg-gradient-to-b from-border to-transparent" />
      )}

      {/* Timeline dot with age indicator */}
      <div className="relative z-10 shrink-0">
        <div
          className={cn(
            'w-10 h-10 rounded-full flex items-center justify-center',
            'border-2 transition-all duration-300',
            isApproved
              ? 'bg-emerald/20 border-emerald/50 text-emerald'
              : 'bg-red-500/20 border-red-500/50 text-red-400',
          )}
        >
          {isApproved ? (
            <CheckCircle className="w-5 h-5" />
          ) : (
            <X className="w-5 h-5" />
          )}
        </div>
        {/* Age indicator dot */}
        <div
          className={cn(
            'absolute -top-0.5 -right-0.5 w-3 h-3 rounded-full border-2 border-background',
            age.color,
          )}
          title={age.label}
        />
      </div>

      {/* Content */}
      <div className="flex-1 pb-6">
        <button
          onClick={() => setExpanded(!expanded)}
          className={cn(
            'w-full text-left p-3 rounded-xl transition-all duration-200',
            'bg-secondary/30 hover:bg-secondary/50 border border-border/30',
            expanded && 'bg-secondary/50 border-border/50',
          )}
        >
          {/* Header */}
          <div className="flex items-center justify-between gap-2">
            <div className="flex items-center gap-2 min-w-0">
              <span className="font-medium text-foreground truncate">
                {entry.product}
              </span>
              <span
                className={cn(
                  'px-2 py-0.5 rounded-full text-[10px] font-medium uppercase',
                  isApproved
                    ? 'bg-emerald/20 text-emerald'
                    : 'bg-red-500/20 text-red-400',
                )}
              >
                {entry.action}
              </span>
            </div>
            <div className="flex items-center gap-2 shrink-0">
              <span className="text-xs text-muted-foreground flex items-center gap-1">
                <Clock className="w-3 h-3" />
                {relativeTime}
              </span>
              {expanded ? (
                <ChevronDown className="w-4 h-4 text-muted-foreground" />
              ) : (
                <ChevronRight className="w-4 h-4 text-muted-foreground" />
              )}
            </div>
          </div>

          {/* Summary */}
          <div className="flex items-center gap-3 mt-2 text-xs text-muted-foreground">
            <span className="flex items-center gap-1">
              <Sparkles className="w-3 h-3" />
              {entry.confidence}% confidence
            </span>
            <span>•</span>
            <span>{entry.originalReview.offer.source}</span>
            <span>↔</span>
            <span>{entry.originalReview.request.source}</span>
          </div>
        </button>

        {/* Expanded details */}
        {expanded && (
          <div className="mt-2 p-3 rounded-xl bg-secondary/20 border border-border/20 space-y-3 animate-in slide-in-from-top-2 duration-200">
            {/* Match details */}
            <div className="grid grid-cols-2 gap-3">
              <div className="p-2 rounded-lg bg-teal/10 border border-teal/20">
                <p className="text-[10px] text-teal uppercase font-medium mb-1">Offer</p>
                <p className="text-sm text-foreground truncate">
                  {entry.originalReview.offer.product}
                </p>
                <p className="text-xs text-muted-foreground">
                  {entry.originalReview.offer.quantity} • {entry.originalReview.offer.price}
                </p>
              </div>
              <div className="p-2 rounded-lg bg-amber/10 border border-amber/20">
                <p className="text-[10px] text-amber uppercase font-medium mb-1">Request</p>
                <p className="text-sm text-foreground truncate">
                  {entry.originalReview.request.product}
                </p>
                <p className="text-xs text-muted-foreground">
                  {entry.originalReview.request.quantity} • {entry.originalReview.request.maxPrice}
                </p>
              </div>
            </div>

            {/* Adjustments */}
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <span>Adjustments:</span>
              <span className="px-1.5 py-0.5 rounded bg-secondary">
                Price ±{entry.adjustments.priceFlexibility}%
              </span>
              <span className="px-1.5 py-0.5 rounded bg-secondary">
                Qty ±{entry.adjustments.quantityTolerance}%
              </span>
              <span className="px-1.5 py-0.5 rounded bg-secondary">
                Dosage {entry.adjustments.dosageStrictness}%
              </span>
            </div>

            {/* Timestamp */}
            <div className="flex items-center justify-between pt-2 border-t border-border/30">
              <span className="text-xs text-muted-foreground">
                {entry.timestamp.toLocaleString()}
              </span>
              <button
                onClick={(e) => {
                  e.stopPropagation()
                  onRestore(entry.id)
                }}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-amber/20 text-amber border border-amber/30 text-xs font-medium hover:bg-amber/30 transition-colors"
              >
                <Undo2 className="w-3 h-3" />
                Restore to Queue
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

// Date Group Header
function DateGroupHeader({
  label,
  count,
  isExpanded,
  onToggle,
}: {
  label: string
  count: number
  isExpanded: boolean
  onToggle: () => void
}) {
  return (
    <button
      onClick={onToggle}
      className="w-full flex items-center gap-3 py-2 px-3 rounded-lg bg-secondary/20 hover:bg-secondary/30 transition-colors mb-3"
    >
      <Calendar className="w-4 h-4 text-muted-foreground" />
      <span className="text-sm font-medium text-foreground">{label}</span>
      <span className="px-2 py-0.5 rounded-full bg-secondary text-xs text-muted-foreground">
        {count}
      </span>
      <div className="flex-1" />
      {isExpanded ? (
        <ChevronDown className="w-4 h-4 text-muted-foreground" />
      ) : (
        <ChevronRight className="w-4 h-4 text-muted-foreground" />
      )}
    </button>
  )
}

export function TimelineHistory({
  history,
  onRestore,
  maxHeight = '500px',
}: TimelineHistoryProps) {
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(
    new Set(['Today', 'Yesterday']),
  )
  const [filter, setFilter] = useState<FilterType>('all')
  const [searchQuery, setSearchQuery] = useState('')

  // Filter history based on action type and search
  const filteredHistory = useMemo(() => {
    let result = history

    // Filter by action type
    if (filter !== 'all') {
      result = result.filter((h) => h.action === filter)
    }

    // Filter by search query
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase()
      result = result.filter(
        (h) =>
          h.product.toLowerCase().includes(query) ||
          h.originalReview.offer.source.toLowerCase().includes(query) ||
          h.originalReview.request.source.toLowerCase().includes(query)
      )
    }

    return result
  }, [history, filter, searchQuery])

  // Group entries by date
  const groupedHistory = useMemo(() => {
    const groups = new Map<string, HistoryEntry[]>()
    const order = ['Today', 'Yesterday', 'This Week', 'Earlier']

    for (const entry of filteredHistory) {
      const group = getDateGroup(entry.timestamp)
      if (!groups.has(group)) {
        groups.set(group, [])
      }
      groups.get(group)!.push(entry)
    }

    // Return in order
    return order
      .filter((g) => groups.has(g))
      .map((g) => ({ label: g, entries: groups.get(g)! }))
  }, [filteredHistory])

  // Calculate approval rate trend
  const approvalTrend = useMemo(() => {
    if (history.length < 5) return { trend: 'neutral' as const, rate: 0 }
    
    const recent = history.slice(0, 5)
    const older = history.slice(5, 10)
    
    const recentRate = recent.filter((h) => h.action === 'approved').length / recent.length
    const olderRate = older.length > 0 
      ? older.filter((h) => h.action === 'approved').length / older.length 
      : recentRate
    
    const diff = recentRate - olderRate
    const trend = diff > 0.1 ? 'up' : diff < -0.1 ? 'down' : 'neutral'
    
    return { trend, rate: Math.round(recentRate * 100) }
  }, [history])

  const toggleGroup = (label: string) => {
    setExpandedGroups((prev) => {
      const next = new Set(prev)
      if (next.has(label)) {
        next.delete(label)
      } else {
        next.add(label)
      }
      return next
    })
  }

  if (history.length === 0) {
    return (
      <div className="glass-card p-6 rounded-xl">
        <div className="flex items-center gap-2 mb-4">
          <Clock className="w-5 h-5 text-muted-foreground" />
          <h3 className="text-lg font-semibold text-foreground">Timeline</h3>
        </div>
        <div className="text-center py-8">
          <Clock className="w-12 h-12 text-muted-foreground/30 mx-auto mb-3" />
          <p className="text-sm text-muted-foreground">
            No review decisions yet. Your activity will appear here.
          </p>
        </div>
      </div>
    )
  }

  const approvedCount = history.filter((h) => h.action === 'approved').length
  const rejectedCount = history.filter((h) => h.action === 'rejected').length

  return (
    <div className="glass-card p-6 rounded-xl">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <Clock className="w-5 h-5 text-teal" />
          <h3 className="text-lg font-semibold text-foreground">Timeline</h3>
          <span className="px-2 py-0.5 rounded-full bg-secondary text-xs text-muted-foreground">
            {history.length} actions
          </span>
        </div>
        <div className="flex items-center gap-3 text-xs">
          <span className="flex items-center gap-1.5">
            <div className="w-2 h-2 rounded-full bg-emerald" />
            <span className="text-muted-foreground">{approvedCount} approved</span>
          </span>
          <span className="flex items-center gap-1.5">
            <div className="w-2 h-2 rounded-full bg-red-500" />
            <span className="text-muted-foreground">{rejectedCount} rejected</span>
          </span>
        </div>
      </div>

      {/* Approval Rate Trend */}
      {history.length >= 5 && (
        <div className="flex items-center gap-3 mb-4 p-3 rounded-xl bg-secondary/30 border border-border/30">
          <div className="flex items-center gap-2">
            {approvalTrend.trend === 'up' ? (
              <TrendingUp className="w-4 h-4 text-emerald" />
            ) : approvalTrend.trend === 'down' ? (
              <TrendingDown className="w-4 h-4 text-red-400" />
            ) : (
              <Minus className="w-4 h-4 text-muted-foreground" />
            )}
            <span className="text-sm font-medium text-foreground">
              {approvalTrend.rate}% approval rate
            </span>
          </div>
          <span className="text-xs text-muted-foreground">
            {approvalTrend.trend === 'up' && '↑ Trending up'}
            {approvalTrend.trend === 'down' && '↓ Trending down'}
            {approvalTrend.trend === 'neutral' && '→ Stable'}
          </span>
        </div>
      )}

      {/* Search & Filter Bar */}
      <div className="flex items-center gap-3 mb-4">
        {/* Search */}
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
          <input
            type="text"
            placeholder="Search history..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className={cn(
              'w-full pl-9 pr-3 py-2 rounded-lg text-sm',
              'bg-secondary/30 border border-border/50',
              'text-foreground placeholder:text-muted-foreground',
              'focus:outline-none focus:ring-2 focus:ring-teal/50 focus:border-teal/50',
            )}
          />
        </div>

        {/* Filter buttons */}
        <div className="flex items-center gap-1 p-1 rounded-lg bg-secondary/30 border border-border/30">
          <button
            onClick={() => setFilter('all')}
            className={cn(
              'px-3 py-1.5 rounded-md text-xs font-medium transition-colors',
              filter === 'all'
                ? 'bg-teal text-white'
                : 'text-muted-foreground hover:text-foreground',
            )}
          >
            All
          </button>
          <button
            onClick={() => setFilter('approved')}
            className={cn(
              'px-3 py-1.5 rounded-md text-xs font-medium transition-colors flex items-center gap-1',
              filter === 'approved'
                ? 'bg-emerald text-white'
                : 'text-muted-foreground hover:text-foreground',
            )}
          >
            <CheckCircle className="w-3 h-3" />
            Approved
          </button>
          <button
            onClick={() => setFilter('rejected')}
            className={cn(
              'px-3 py-1.5 rounded-md text-xs font-medium transition-colors flex items-center gap-1',
              filter === 'rejected'
                ? 'bg-red-500 text-white'
                : 'text-muted-foreground hover:text-foreground',
            )}
          >
            <X className="w-3 h-3" />
            Rejected
          </button>
        </div>
      </div>

      {/* Age indicator legend */}
      <div className="flex items-center gap-4 mb-4 p-2 rounded-lg bg-secondary/20 text-xs">
        <span className="text-muted-foreground">Age:</span>
        <span className="flex items-center gap-1">
          <div className="w-2 h-2 rounded-full bg-emerald-500" />
          Fresh (&lt;5m)
        </span>
        <span className="flex items-center gap-1">
          <div className="w-2 h-2 rounded-full bg-teal" />
          Recent (&lt;30m)
        </span>
        <span className="flex items-center gap-1">
          <div className="w-2 h-2 rounded-full bg-amber-500" />
          Aging (&lt;1h)
        </span>
        <span className="flex items-center gap-1">
          <div className="w-2 h-2 rounded-full bg-muted-foreground" />
          Stale
        </span>
      </div>

      {/* No results message */}
      {filteredHistory.length === 0 && (
        <div className="text-center py-8">
          <Filter className="w-10 h-10 text-muted-foreground/30 mx-auto mb-3" />
          <p className="text-sm text-muted-foreground">
            No matching entries found.
          </p>
          <button
            onClick={() => {
              setFilter('all')
              setSearchQuery('')
            }}
            className="mt-2 text-xs text-teal hover:underline"
          >
            Clear filters
          </button>
        </div>
      )}

      {/* Timeline content */}
      {filteredHistory.length > 0 && (
        <div
          className="overflow-y-auto pr-2"
          style={{ maxHeight }}
        >
          {groupedHistory.map(({ label, entries }) => (
            <div key={label} className="mb-4">
              <DateGroupHeader
                label={label}
                count={entries.length}
                isExpanded={expandedGroups.has(label)}
                onToggle={() => toggleGroup(label)}
              />
              {expandedGroups.has(label) && (
                <div className="pl-2">
                  {entries.map((entry, idx) => (
                    <TimelineEntry
                      key={entry.id}
                      entry={entry}
                      onRestore={onRestore}
                      isLast={idx === entries.length - 1}
                    />
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
