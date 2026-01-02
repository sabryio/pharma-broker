// Recording Card Component
// Beautiful card for displaying match recording summary

import { cn } from '@/lib/utils'
import {
  Calendar,
  CheckCircle,
  XCircle,
  AlertCircle,
  GitBranch,
  Play,
  Download,
  Trash2,
  Clock,
  Layers,
  Sparkles,
} from 'lucide-react'
import type { MatchRecording } from './types'
import { EVENT_COLORS, EVENT_ICONS } from './types'

interface RecordingCardProps {
  recording: MatchRecording
  isSelected?: boolean
  onSelect?: () => void
  onPlay?: () => void
  onExport?: () => void
  onDelete?: () => void
  onViewPipeline?: () => void
  compact?: boolean
}

function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  if (hours > 0) return `${hours}h ${minutes % 60}m`
  if (minutes > 0) return `${minutes}m ${seconds % 60}s`
  return `${seconds}s`
}

function formatDate(date: Date): string {
  return date.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function formatRelativeTime(date: Date): string {
  const now = new Date()
  const diff = now.getTime() - date.getTime()
  const minutes = Math.floor(diff / 60000)
  const hours = Math.floor(minutes / 60)
  const days = Math.floor(hours / 24)
  
  if (days > 0) return `${days}d ago`
  if (hours > 0) return `${hours}h ago`
  if (minutes > 0) return `${minutes}m ago`
  return 'Just now'
}

export function RecordingCard({
  recording,
  isSelected = false,
  onSelect,
  onPlay,
  onExport,
  onDelete,
  onViewPipeline,
  compact = false,
}: RecordingCardProps) {
  const lastSnapshot = recording.snapshots[recording.snapshots.length - 1]
  const lastEvent = lastSnapshot?.event
  const firstSnapshot = recording.snapshots[0]
  const avgConfidence = recording.snapshots.length > 0
    ? recording.snapshots.reduce((acc, s) => acc + s.confidence, 0) / recording.snapshots.length
    : 0

  const outcomeConfig = {
    approved: {
      icon: CheckCircle,
      gradient: 'from-emerald-500/30 to-teal-500/30',
      text: 'text-emerald-400',
      shadow: 'shadow-emerald-500/20',
      badge: 'bg-emerald-500/20 text-emerald-400',
      line: 'from-emerald-500 to-teal-500',
    },
    rejected: {
      icon: XCircle,
      gradient: 'from-red-500/30 to-rose-500/30',
      text: 'text-red-400',
      shadow: 'shadow-red-500/20',
      badge: 'bg-red-500/20 text-red-400',
      line: 'from-red-500 to-rose-500',
    },
    pending: {
      icon: AlertCircle,
      gradient: 'from-amber-500/30 to-orange-500/30',
      text: 'text-amber-400',
      shadow: 'shadow-amber-500/20',
      badge: 'bg-amber-500/20 text-amber-400',
      line: 'from-amber-500 to-orange-500',
    },
  }

  const config = outcomeConfig[recording.outcome || 'pending']
  const OutcomeIcon = config.icon

  return (
    <div
      onClick={onSelect}
      className={cn(
        'relative p-4 rounded-2xl border transition-all duration-300 cursor-pointer group overflow-hidden',
        isSelected
          ? 'bg-gradient-to-br from-teal-500/10 via-emerald-500/5 to-teal-500/10 border-teal-500/50 shadow-xl shadow-teal-500/10 scale-[1.02]'
          : 'bg-gradient-to-br from-secondary/40 to-secondary/20 border-border/50 hover:border-border hover:bg-secondary/50 hover:shadow-lg',
      )}
    >
      {/* Animated background gradient */}
      <div className={cn(
        'absolute inset-0 opacity-0 transition-opacity duration-500',
        'bg-gradient-to-br from-teal-500/5 via-transparent to-emerald-500/5',
        isSelected && 'opacity-100',
      )} />

      {/* Status indicator line */}
      <div className={cn(
        'absolute top-0 left-0 right-0 h-1 rounded-t-2xl bg-gradient-to-r',
        config.line,
      )} />

      <div className="relative">
        {/* Header */}
        <div className="flex items-start justify-between mb-4">
          <div className="flex items-center gap-3">
            <div className={cn(
              'w-12 h-12 rounded-xl flex items-center justify-center shadow-lg transition-transform group-hover:scale-110 bg-gradient-to-br',
              config.gradient,
              config.text,
              config.shadow,
            )}>
              <OutcomeIcon className="w-6 h-6" />
            </div>
            <div>
              <p className="text-sm font-bold text-foreground flex items-center gap-2">
                <span className="font-mono">#{recording.matchId.slice(0, 8)}</span>
                <span className={cn(
                  'px-2 py-0.5 rounded-full text-[10px] font-semibold uppercase tracking-wide',
                  config.badge,
                )}>
                  {recording.outcome || 'pending'}
                </span>
              </p>
              <p className="text-xs text-muted-foreground flex items-center gap-2 mt-0.5">
                <Calendar className="w-3 h-3" />
                {formatDate(recording.startedAt)}
                <span className="text-muted-foreground/40">•</span>
                <span className="text-muted-foreground/70">{formatRelativeTime(recording.startedAt)}</span>
              </p>
            </div>
          </div>

          {/* Quick actions */}
          <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-all duration-200 translate-x-2 group-hover:translate-x-0">
            {onViewPipeline && (
              <button
                onClick={(e) => { e.stopPropagation(); onViewPipeline() }}
                className="p-2 rounded-xl bg-violet-500/20 hover:bg-violet-500/30 text-violet-400 transition-all hover:scale-110"
                title="View pipeline"
              >
                <GitBranch className="w-4 h-4" />
              </button>
            )}
            {onPlay && (
              <button
                onClick={(e) => { e.stopPropagation(); onPlay() }}
                className="p-2 rounded-xl bg-teal-500/20 hover:bg-teal-500/30 text-teal-400 transition-all hover:scale-110"
                title="Play recording"
              >
                <Play className="w-4 h-4" />
              </button>
            )}
            {onExport && (
              <button
                onClick={(e) => { e.stopPropagation(); onExport() }}
                className="p-2 rounded-xl bg-secondary hover:bg-secondary/80 text-muted-foreground hover:text-foreground transition-all hover:scale-110"
                title="Export JSON"
              >
                <Download className="w-4 h-4" />
              </button>
            )}
            {onDelete && (
              <button
                onClick={(e) => { e.stopPropagation(); onDelete() }}
                className="p-2 rounded-xl bg-red-500/10 hover:bg-red-500/20 text-red-400 transition-all hover:scale-110"
                title="Delete"
              >
                <Trash2 className="w-4 h-4" />
              </button>
            )}
          </div>
        </div>

        {!compact && (
          <>
            {/* Stats row */}
            <div className="grid grid-cols-4 gap-2 mb-4">
              <div className="p-2.5 rounded-xl bg-background/50 backdrop-blur-sm text-center border border-border/20">
                <div className="flex items-center justify-center gap-1 mb-0.5">
                  <Layers className="w-3 h-3 text-muted-foreground" />
                  <p className="text-lg font-bold text-foreground">{recording.snapshots.length}</p>
                </div>
                <p className="text-[10px] text-muted-foreground font-medium">Snapshots</p>
              </div>
              <div className="p-2.5 rounded-xl bg-background/50 backdrop-blur-sm text-center border border-border/20">
                <div className="flex items-center justify-center gap-1 mb-0.5">
                  <Clock className="w-3 h-3 text-muted-foreground" />
                  <p className="text-lg font-bold text-foreground">
                    {recording.duration ? formatDuration(recording.duration) : '—'}
                  </p>
                </div>
                <p className="text-[10px] text-muted-foreground font-medium">Duration</p>
              </div>
              <div className="p-2.5 rounded-xl bg-background/50 backdrop-blur-sm text-center border border-border/20">
                <div className="flex items-center justify-center gap-1 mb-0.5">
                  <Sparkles className="w-3 h-3 text-muted-foreground" />
                  <p className={cn(
                    'text-lg font-bold',
                    avgConfidence >= 80 ? 'text-emerald-400' : avgConfidence >= 60 ? 'text-amber-400' : 'text-red-400',
                  )}>
                    {avgConfidence.toFixed(0)}%
                  </p>
                </div>
                <p className="text-[10px] text-muted-foreground font-medium">Confidence</p>
              </div>
              <div className="p-2.5 rounded-xl bg-background/50 backdrop-blur-sm text-center border border-border/20">
                {lastEvent ? (
                  <>
                    <p className={cn('text-lg mb-0.5', EVENT_COLORS[lastEvent.type].text)}>
                      {EVENT_ICONS[lastEvent.type]}
                    </p>
                    <p className="text-[10px] text-muted-foreground font-medium truncate">{lastEvent.label}</p>
                  </>
                ) : (
                  <p className="text-muted-foreground/50 text-sm">—</p>
                )}
              </div>
            </div>

            {/* Timeline visualization */}
            <div className="mb-4">
              <div className="flex items-center justify-between mb-2">
                <span className="text-[10px] text-muted-foreground uppercase tracking-wider font-semibold">
                  Event Timeline
                </span>
                <span className="text-[10px] text-muted-foreground">
                  {recording.snapshots.length} events
                </span>
              </div>
              <div className="flex items-center gap-0.5 h-7 rounded-lg overflow-hidden bg-secondary/40 p-0.5">
                {recording.snapshots.slice(0, 20).map((snapshot) => {
                  const colors = EVENT_COLORS[snapshot.event.type]
                  const width = `${100 / Math.min(recording.snapshots.length, 20)}%`
                  return (
                    <div
                      key={snapshot.id}
                      className={cn(
                        'h-full rounded transition-all hover:opacity-80 hover:scale-y-110',
                        colors.bg,
                      )}
                      style={{ width }}
                      title={`${snapshot.event.label} - ${snapshot.confidence.toFixed(1)}%`}
                    />
                  )
                })}
              </div>
            </div>

            {/* Offer/Request preview */}
            {firstSnapshot && (
              <div className="grid grid-cols-2 gap-2">
                <div className="p-3 rounded-xl bg-background/50 backdrop-blur-sm border border-border/30 hover:border-teal-500/30 transition-colors">
                  <div className="flex items-center gap-2 mb-1.5">
                    <div className="w-5 h-5 rounded-md bg-teal-500/20 flex items-center justify-center">
                      <span className="text-[10px] text-teal-400 font-bold">O</span>
                    </div>
                    <span className="text-[10px] text-muted-foreground uppercase font-semibold tracking-wide">Offer</span>
                  </div>
                  <p className="text-xs text-foreground font-medium truncate">{firstSnapshot.offer.product}</p>
                  {firstSnapshot.offer.medicationRaw && (
                    <p className="text-[10px] text-muted-foreground truncate mt-0.5" dir="auto">
                      {firstSnapshot.offer.medicationRaw}
                    </p>
                  )}
                </div>
                <div className="p-3 rounded-xl bg-background/50 backdrop-blur-sm border border-border/30 hover:border-violet-500/30 transition-colors">
                  <div className="flex items-center gap-2 mb-1.5">
                    <div className="w-5 h-5 rounded-md bg-violet-500/20 flex items-center justify-center">
                      <span className="text-[10px] text-violet-400 font-bold">R</span>
                    </div>
                    <span className="text-[10px] text-muted-foreground uppercase font-semibold tracking-wide">Request</span>
                  </div>
                  <p className="text-xs text-foreground font-medium truncate">{firstSnapshot.request.product}</p>
                  {firstSnapshot.request.medicationRaw && (
                    <p className="text-[10px] text-muted-foreground truncate mt-0.5" dir="auto">
                      {firstSnapshot.request.medicationRaw}
                    </p>
                  )}
                </div>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  )
}
