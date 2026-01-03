import { useState } from 'react'
import {
  CheckCircle,
  Clock,
  ShieldX,
  AlertTriangle,
  Undo2,
  Wifi,
  WifiOff,
  Trash2,
  ChevronDown,
  ChevronUp,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import type { LiveFeedItem } from '@/schema/supervision'
import { formatDistanceToNow } from 'date-fns'

interface LiveFeedPanelProps {
  items: LiveFeedItem[]
  isConnected: boolean
  onOverride?: (matchId: string, reason: string) => void
  onClear?: () => void
  className?: string
}

/**
 * Real-time feed of AI auto-approval activity
 * Requirements: 3.1, 3.4, 3.5
 */
export function LiveFeedPanel({
  items,
  isConnected,
  onOverride,
  onClear,
  className,
}: LiveFeedPanelProps) {
  const [expandedId, setExpandedId] = useState<string | null>(null)
  const [overrideReason, setOverrideReason] = useState('')

  const actionConfig = {
    approved: {
      icon: CheckCircle,
      color: 'text-emerald',
      bgColor: 'bg-emerald/10',
      label: 'Auto-Approved',
    },
    queued: {
      icon: Clock,
      color: 'text-amber',
      bgColor: 'bg-amber/10',
      label: 'Queued for Review',
    },
    blocked: {
      icon: ShieldX,
      color: 'text-red-400',
      bgColor: 'bg-red-400/10',
      label: 'Blocked',
    },
  }

  const handleOverride = (matchId: string) => {
    if (overrideReason.trim() && onOverride) {
      onOverride(matchId, overrideReason.trim())
      setOverrideReason('')
      setExpandedId(null)
    }
  }

  return (
    <div className={cn('glass-card rounded-xl overflow-hidden', className)}>
      {/* Header */}
      <div className="p-4 border-b border-border flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h3 className="font-semibold text-foreground">Live Activity Feed</h3>
          <div
            className={cn(
              'flex items-center gap-1.5 px-2 py-1 rounded-full text-xs font-medium',
              isConnected
                ? 'bg-emerald/20 text-emerald'
                : 'bg-red-400/20 text-red-400',
            )}
          >
            {isConnected ? (
              <>
                <Wifi className="w-3 h-3" />
                Connected
              </>
            ) : (
              <>
                <WifiOff className="w-3 h-3" />
                Disconnected
              </>
            )}
          </div>
        </div>

        {items.length > 0 && onClear && (
          <button
            onClick={onClear}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm text-muted-foreground hover:text-foreground hover:bg-secondary/50 transition-colors"
          >
            <Trash2 className="w-3.5 h-3.5" />
            Clear
          </button>
        )}
      </div>

      {/* Feed Items */}
      <div className="max-h-[500px] overflow-y-auto">
        {items.length === 0 ? (
          <div className="p-8 text-center text-muted-foreground">
            <Clock className="w-8 h-8 mx-auto mb-2 opacity-50" />
            <p>No activity yet</p>
            <p className="text-sm">
              AI decisions will appear here in real-time
            </p>
          </div>
        ) : (
          <div className="divide-y divide-border">
            {items.map((item) => {
              const config = actionConfig[item.action]
              const ActionIcon = config.icon
              const isExpanded = expandedId === item.id

              return (
                <div
                  key={item.id}
                  className={cn(
                    'p-4 transition-colors',
                    item.isBorderline && 'bg-amber/5',
                  )}
                >
                  <div className="flex items-start gap-3">
                    {/* Icon */}
                    <div className={cn('p-2 rounded-lg', config.bgColor)}>
                      <ActionIcon className={cn('w-4 h-4', config.color)} />
                    </div>

                    {/* Content */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 flex-wrap">
                        <span
                          className={cn('text-sm font-medium', config.color)}
                        >
                          {config.label}
                        </span>
                        {item.isBorderline && (
                          <span className="flex items-center gap-1 px-2 py-0.5 rounded-full bg-amber/20 text-amber text-xs font-medium">
                            <AlertTriangle className="w-3 h-3" />
                            Borderline
                          </span>
                        )}
                        <span className="text-xs text-muted-foreground">
                          {formatDistanceToNow(new Date(item.timestamp), {
                            addSuffix: true,
                          })}
                        </span>
                      </div>

                      {/* Medications */}
                      <div className="mt-1.5 flex items-center gap-2 text-sm">
                        <span className="font-medium text-foreground truncate max-w-[150px]">
                          {item.offerMedication}
                        </span>
                        <span className="text-muted-foreground">→</span>
                        <span className="font-medium text-foreground truncate max-w-[150px]">
                          {item.requestMedication}
                        </span>
                      </div>

                      {/* Confidence */}
                      {item.action !== 'blocked' && (
                        <div className="mt-1.5 flex items-center gap-2">
                          <div className="flex-1 h-1.5 bg-secondary rounded-full overflow-hidden max-w-[100px]">
                            <div
                              className={cn(
                                'h-full rounded-full transition-all',
                                item.aiConfidence >= 0.85
                                  ? 'bg-emerald'
                                  : item.aiConfidence >= 0.7
                                    ? 'bg-amber'
                                    : 'bg-red-400',
                              )}
                              style={{ width: `${item.aiConfidence * 100}%` }}
                            />
                          </div>
                          <span className="text-xs text-muted-foreground">
                            {(item.aiConfidence * 100).toFixed(0)}%
                          </span>
                        </div>
                      )}

                      {/* Block Reason */}
                      {item.blockReason && (
                        <p className="mt-1.5 text-xs text-red-400">
                          {item.blockReason}
                        </p>
                      )}

                      {/* AI Explanation (expandable) */}
                      {item.aiExplanation && !item.blockReason && (
                        <button
                          onClick={() =>
                            setExpandedId(isExpanded ? null : item.id)
                          }
                          className="mt-1.5 flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
                        >
                          {isExpanded ? (
                            <ChevronUp className="w-3 h-3" />
                          ) : (
                            <ChevronDown className="w-3 h-3" />
                          )}
                          {isExpanded ? 'Hide' : 'Show'} AI reasoning
                        </button>
                      )}

                      {isExpanded && (
                        <div className="mt-2 p-3 rounded-lg bg-secondary/50 text-sm text-muted-foreground">
                          {item.aiExplanation}

                          {/* Override Form */}
                          {item.action === 'approved' && onOverride && (
                            <div className="mt-3 pt-3 border-t border-border">
                              <label className="block text-xs font-medium text-foreground mb-1.5">
                                Override this decision
                              </label>
                              <div className="flex gap-2">
                                <input
                                  type="text"
                                  value={overrideReason}
                                  onChange={(e) =>
                                    setOverrideReason(e.target.value)
                                  }
                                  placeholder="Enter reason for override..."
                                  className="flex-1 px-3 py-1.5 rounded-lg bg-background border border-border text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
                                />
                                <button
                                  onClick={() => handleOverride(item.matchId)}
                                  disabled={!overrideReason.trim()}
                                  className={cn(
                                    'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm font-medium transition-colors',
                                    overrideReason.trim()
                                      ? 'bg-amber/20 text-amber hover:bg-amber/30'
                                      : 'bg-secondary text-muted-foreground cursor-not-allowed',
                                  )}
                                >
                                  <Undo2 className="w-3.5 h-3.5" />
                                  Override
                                </button>
                              </div>
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                  </div>
                </div>
              )
            })}
          </div>
        )}
      </div>
    </div>
  )
}
