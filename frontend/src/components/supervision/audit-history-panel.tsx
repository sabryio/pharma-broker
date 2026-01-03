import { useState } from 'react'
import {
  History,
  Filter,
  ChevronDown,
  ChevronUp,
  CheckCircle,
  Clock,
  ShieldX,
  Undo2,
  Settings,
  Pause,
  Play,
  RefreshCw,
  Search,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import type { AuditEntry, AuditQueryParams } from '@/schema/supervision'
import { format } from 'date-fns'

interface AuditHistoryPanelProps {
  entries: AuditEntry[]
  total: number
  isLoading?: boolean
  filters: AuditQueryParams
  onFiltersChange: (filters: AuditQueryParams) => void
  onRefresh?: () => void
  className?: string
}

/**
 * Audit history panel with filtering
 * Requirements: 2.3, 2.5
 */
export function AuditHistoryPanel({
  entries,
  total,
  isLoading,
  filters,
  onFiltersChange,
  onRefresh,
  className,
}: AuditHistoryPanelProps) {
  const [showFilters, setShowFilters] = useState(false)
  const [expandedId, setExpandedId] = useState<string | null>(null)

  const eventTypeConfig: Record<
    string,
    { icon: typeof CheckCircle; color: string; label: string }
  > = {
    AutoApproved: {
      icon: CheckCircle,
      color: 'text-emerald',
      label: 'Auto-Approved',
    },
    QueuedForReview: {
      icon: Clock,
      color: 'text-amber',
      label: 'Queued for Review',
    },
    Blocked: { icon: ShieldX, color: 'text-red-400', label: 'Blocked' },
    Overridden: { icon: Undo2, color: 'text-violet-400', label: 'Overridden' },
    UndoApproval: { icon: Undo2, color: 'text-amber', label: 'Undo Approval' },
    ConfigChanged: {
      icon: Settings,
      color: 'text-teal',
      label: 'Config Changed',
    },
    SystemPaused: { icon: Pause, color: 'text-amber', label: 'System Paused' },
    SystemResumed: {
      icon: Play,
      color: 'text-emerald',
      label: 'System Resumed',
    },
  }

  const getEventConfig = (eventType: string) => {
    return (
      eventTypeConfig[eventType] || {
        icon: History,
        color: 'text-muted-foreground',
        label: eventType,
      }
    )
  }

  const handleFilterChange = (key: keyof AuditQueryParams, value: unknown) => {
    onFiltersChange({
      ...filters,
      [key]: value === '' ? undefined : value,
    })
  }

  return (
    <div className={cn('glass-card rounded-xl overflow-hidden', className)}>
      {/* Header */}
      <div className="p-4 border-b border-border">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <History className="w-5 h-5 text-teal" />
            <h3 className="font-semibold text-foreground">Audit History</h3>
            <span className="px-2 py-0.5 rounded-full bg-secondary text-xs text-muted-foreground">
              {total} entries
            </span>
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={() => setShowFilters(!showFilters)}
              className={cn(
                'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm transition-colors',
                showFilters
                  ? 'bg-teal/20 text-teal'
                  : 'text-muted-foreground hover:text-foreground hover:bg-secondary/50',
              )}
            >
              <Filter className="w-3.5 h-3.5" />
              Filters
            </button>
            {onRefresh && (
              <button
                onClick={onRefresh}
                disabled={isLoading}
                className="p-1.5 rounded-lg text-muted-foreground hover:text-foreground hover:bg-secondary/50 transition-colors"
              >
                <RefreshCw
                  className={cn('w-4 h-4', isLoading && 'animate-spin')}
                />
              </button>
            )}
          </div>
        </div>

        {/* Filters */}
        {showFilters && (
          <div className="mt-4 pt-4 border-t border-border grid grid-cols-2 lg:grid-cols-4 gap-4">
            {/* Event Type Filter */}
            <div>
              <label className="block text-xs text-muted-foreground mb-1.5">
                Event Type
              </label>
              <select
                value={filters.eventType || ''}
                onChange={(e) =>
                  handleFilterChange('eventType', e.target.value)
                }
                className="w-full px-3 py-2 rounded-lg bg-secondary border border-border text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
              >
                <option value="">All Types</option>
                <option value="auto_approved">Auto-Approved</option>
                <option value="queued_for_review">Queued for Review</option>
                <option value="blocked">Blocked</option>
                <option value="overridden">Overridden</option>
                <option value="undo_approval">Undo Approval</option>
                <option value="config_changed">Config Changed</option>
                <option value="system_paused">System Paused</option>
                <option value="system_resumed">System Resumed</option>
              </select>
            </div>

            {/* Override Status Filter */}
            <div>
              <label className="block text-xs text-muted-foreground mb-1.5">
                Override Status
              </label>
              <select
                value={
                  filters.overridden === undefined
                    ? ''
                    : filters.overridden
                      ? 'true'
                      : 'false'
                }
                onChange={(e) =>
                  handleFilterChange(
                    'overridden',
                    e.target.value === ''
                      ? undefined
                      : e.target.value === 'true',
                  )
                }
                className="w-full px-3 py-2 rounded-lg bg-secondary border border-border text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
              >
                <option value="">All</option>
                <option value="true">Overridden</option>
                <option value="false">Not Overridden</option>
              </select>
            </div>

            {/* Min Confidence */}
            <div>
              <label className="block text-xs text-muted-foreground mb-1.5">
                Min Confidence
              </label>
              <input
                type="number"
                min="0"
                max="1"
                step="0.05"
                value={filters.minConfidence || ''}
                onChange={(e) =>
                  handleFilterChange(
                    'minConfidence',
                    e.target.value ? parseFloat(e.target.value) : undefined,
                  )
                }
                placeholder="0.00"
                className="w-full px-3 py-2 rounded-lg bg-secondary border border-border text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
              />
            </div>

            {/* Max Confidence */}
            <div>
              <label className="block text-xs text-muted-foreground mb-1.5">
                Max Confidence
              </label>
              <input
                type="number"
                min="0"
                max="1"
                step="0.05"
                value={filters.maxConfidence || ''}
                onChange={(e) =>
                  handleFilterChange(
                    'maxConfidence',
                    e.target.value ? parseFloat(e.target.value) : undefined,
                  )
                }
                placeholder="1.00"
                className="w-full px-3 py-2 rounded-lg bg-secondary border border-border text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
              />
            </div>

            {/* Match ID Search */}
            <div className="col-span-2">
              <label className="block text-xs text-muted-foreground mb-1.5">
                Match ID
              </label>
              <div className="relative">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                <input
                  type="text"
                  value={filters.matchId || ''}
                  onChange={(e) =>
                    handleFilterChange('matchId', e.target.value)
                  }
                  placeholder="Search by match ID..."
                  className="w-full pl-10 pr-3 py-2 rounded-lg bg-secondary border border-border text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
                />
              </div>
            </div>

            {/* Date Range */}
            <div>
              <label className="block text-xs text-muted-foreground mb-1.5">
                Start Date
              </label>
              <input
                type="date"
                value={filters.startDate?.split('T')[0] || ''}
                onChange={(e) =>
                  handleFilterChange(
                    'startDate',
                    e.target.value ? `${e.target.value}T00:00:00Z` : undefined,
                  )
                }
                className="w-full px-3 py-2 rounded-lg bg-secondary border border-border text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
              />
            </div>

            <div>
              <label className="block text-xs text-muted-foreground mb-1.5">
                End Date
              </label>
              <input
                type="date"
                value={filters.endDate?.split('T')[0] || ''}
                onChange={(e) =>
                  handleFilterChange(
                    'endDate',
                    e.target.value ? `${e.target.value}T23:59:59Z` : undefined,
                  )
                }
                className="w-full px-3 py-2 rounded-lg bg-secondary border border-border text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
              />
            </div>
          </div>
        )}
      </div>

      {/* Entries List */}
      <div className="max-h-[600px] overflow-y-auto">
        {isLoading ? (
          <div className="p-8 text-center">
            <RefreshCw className="w-6 h-6 mx-auto mb-2 text-teal animate-spin" />
            <p className="text-muted-foreground">Loading audit history...</p>
          </div>
        ) : entries.length === 0 ? (
          <div className="p-8 text-center text-muted-foreground">
            <History className="w-8 h-8 mx-auto mb-2 opacity-50" />
            <p>No audit entries found</p>
            <p className="text-sm">Try adjusting your filters</p>
          </div>
        ) : (
          <div className="divide-y divide-border">
            {entries.map((entry) => {
              const config = getEventConfig(entry.eventType)
              const EventIcon = config.icon
              const isExpanded = expandedId === entry.id

              return (
                <div key={entry.id} className="p-4">
                  <div className="flex items-start gap-3">
                    {/* Icon */}
                    <div
                      className={cn(
                        'p-2 rounded-lg bg-secondary/50',
                        config.color,
                      )}
                    >
                      <EventIcon className="w-4 h-4" />
                    </div>

                    {/* Content */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 flex-wrap">
                        <span
                          className={cn('text-sm font-medium', config.color)}
                        >
                          {config.label}
                        </span>
                        {entry.overridden && (
                          <span className="px-2 py-0.5 rounded-full bg-violet-400/20 text-violet-400 text-xs font-medium">
                            Overridden
                          </span>
                        )}
                        <span className="text-xs text-muted-foreground">
                          {format(
                            new Date(entry.timestamp),
                            'MMM d, yyyy HH:mm',
                          )}
                        </span>
                      </div>

                      {/* Match ID */}
                      {entry.matchId && (
                        <p className="mt-1 text-xs text-muted-foreground font-mono">
                          Match: {entry.matchId}
                        </p>
                      )}

                      {/* Confidence */}
                      {entry.aiConfidence !== null &&
                        entry.aiConfidence !== undefined && (
                          <div className="mt-1.5 flex items-center gap-2">
                            <span className="text-xs text-muted-foreground">
                              Confidence:
                            </span>
                            <div className="flex-1 h-1.5 bg-secondary rounded-full overflow-hidden max-w-[80px]">
                              <div
                                className={cn(
                                  'h-full rounded-full',
                                  entry.aiConfidence >= 0.85
                                    ? 'bg-emerald'
                                    : entry.aiConfidence >= 0.7
                                      ? 'bg-amber'
                                      : 'bg-red-400',
                                )}
                                style={{
                                  width: `${entry.aiConfidence * 100}%`,
                                }}
                              />
                            </div>
                            <span className="text-xs font-medium text-foreground">
                              {(entry.aiConfidence * 100).toFixed(0)}%
                            </span>
                          </div>
                        )}

                      {/* Override Info */}
                      {entry.overridden && entry.overrideReason && (
                        <div className="mt-2 p-2 rounded-lg bg-violet-400/10 text-sm">
                          <p className="text-violet-400 font-medium">
                            Override Reason:
                          </p>
                          <p className="text-muted-foreground">
                            {entry.overrideReason}
                          </p>
                          {entry.overrideAt && (
                            <p className="text-xs text-muted-foreground mt-1">
                              Overridden at:{' '}
                              {format(
                                new Date(entry.overrideAt),
                                'MMM d, yyyy HH:mm',
                              )}
                            </p>
                          )}
                        </div>
                      )}

                      {/* Expandable AI Explanation */}
                      {entry.aiExplanation && (
                        <>
                          <button
                            onClick={() =>
                              setExpandedId(isExpanded ? null : entry.id)
                            }
                            className="mt-2 flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
                          >
                            {isExpanded ? (
                              <ChevronUp className="w-3 h-3" />
                            ) : (
                              <ChevronDown className="w-3 h-3" />
                            )}
                            {isExpanded ? 'Hide' : 'Show'} AI reasoning
                          </button>

                          {isExpanded && (
                            <div className="mt-2 p-3 rounded-lg bg-secondary/50 text-sm text-muted-foreground">
                              {entry.aiExplanation}
                            </div>
                          )}
                        </>
                      )}
                    </div>
                  </div>
                </div>
              )
            })}
          </div>
        )}
      </div>

      {/* Pagination */}
      {total > (filters.limit || 20) && (
        <div className="p-4 border-t border-border flex items-center justify-between">
          <span className="text-sm text-muted-foreground">
            Showing {entries.length} of {total} entries
          </span>
          <div className="flex items-center gap-2">
            <button
              onClick={() =>
                handleFilterChange(
                  'offset',
                  Math.max(0, (filters.offset || 0) - (filters.limit || 20)),
                )
              }
              disabled={(filters.offset || 0) === 0}
              className="px-3 py-1.5 rounded-lg text-sm text-muted-foreground hover:text-foreground hover:bg-secondary/50 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Previous
            </button>
            <button
              onClick={() =>
                handleFilterChange(
                  'offset',
                  (filters.offset || 0) + (filters.limit || 20),
                )
              }
              disabled={(filters.offset || 0) + (filters.limit || 20) >= total}
              className="px-3 py-1.5 rounded-lg text-sm text-muted-foreground hover:text-foreground hover:bg-secondary/50 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Next
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
