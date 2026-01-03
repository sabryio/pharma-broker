// Recording Playback Component
// Beautiful timeline UI for debugging match reviews

import { useState, useCallback, useEffect } from 'react'
import { cn } from '@/lib/utils'
import {
  Play,
  Pause,
  SkipBack,
  SkipForward,
  ChevronLeft,
  ChevronRight,
  Download,
  Trash2,
  Clock,
  Layers,
  Sparkles,
  X,
  Maximize2,
  Minimize2,
} from 'lucide-react'
import type { MatchRecording, MatchRecordingSnapshot } from './types'
import { EVENT_COLORS, EVENT_ICONS } from './types'

interface RecordingPlaybackProps {
  recording: MatchRecording
  onClose?: () => void
  onExport?: () => void
  onDelete?: () => void
}

function formatTime(date: Date): string {
  return date.toLocaleTimeString('en-US', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  })
}

function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  if (hours > 0) return `${hours}h ${minutes % 60}m ${seconds % 60}s`
  if (minutes > 0) return `${minutes}m ${seconds % 60}s`
  return `${seconds}s`
}

function SnapshotCard({
  snapshot,
  isActive,
}: {
  snapshot: MatchRecordingSnapshot
  isActive: boolean
}) {
  const colors = EVENT_COLORS[snapshot.event.type]
  const icon = EVENT_ICONS[snapshot.event.type]

  return (
    <div
      className={cn(
        'p-5 rounded-2xl border transition-all duration-300',
        isActive
          ? 'bg-gradient-to-br from-teal-500/15 via-emerald-500/10 to-teal-500/15 border-teal-500/50 shadow-xl shadow-teal-500/20 scale-[1.01]'
          : 'bg-secondary/30 border-border/50 hover:border-border',
      )}
    >
      {/* Event Header */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <div
            className={cn(
              'w-10 h-10 rounded-xl flex items-center justify-center text-xl shadow-lg',
              colors.bg,
            )}
          >
            {icon}
          </div>
          <div>
            <span className={cn('text-sm font-semibold', colors.text)}>
              {snapshot.event.label}
            </span>
            <p className="text-xs text-muted-foreground">
              {formatTime(snapshot.timestamp)}
            </p>
          </div>
        </div>
        <div
          className={cn(
            'px-3 py-1.5 rounded-full text-xs font-bold tabular-nums',
            colors.bg,
            colors.text,
            colors.border,
            'border',
          )}
        >
          {snapshot.confidence.toFixed(1)}%
        </div>
      </div>

      {/* Match Details */}
      <div className="grid grid-cols-2 gap-3 mb-4">
        <div className="p-3 rounded-xl bg-background/50 border border-border/30">
          <p className="text-[10px] text-muted-foreground uppercase tracking-wider font-semibold mb-1.5">
            Offer
          </p>
          <p className="text-sm font-medium text-foreground truncate">
            {snapshot.offer.product}
          </p>
          {snapshot.offer.medicationRaw && (
            <p
              className="text-xs text-muted-foreground truncate mt-0.5"
              dir="auto"
            >
              {snapshot.offer.medicationRaw}
            </p>
          )}
        </div>
        <div className="p-3 rounded-xl bg-background/50 border border-border/30">
          <p className="text-[10px] text-muted-foreground uppercase tracking-wider font-semibold mb-1.5">
            Request
          </p>
          <p className="text-sm font-medium text-foreground truncate">
            {snapshot.request.product}
          </p>
          {snapshot.request.medicationRaw && (
            <p
              className="text-xs text-muted-foreground truncate mt-0.5"
              dir="auto"
            >
              {snapshot.request.medicationRaw}
            </p>
          )}
        </div>
      </div>

      {/* AI Status */}
      {snapshot.aiStatus && (
        <div className="flex items-center gap-2 p-3 rounded-xl bg-violet-500/10 border border-violet-500/20 mb-4">
          <Sparkles className="w-4 h-4 text-violet-400" />
          <span className="text-sm text-violet-300">
            AI: {snapshot.aiStatus} (
            {snapshot.aiConfidence
              ? `${(snapshot.aiConfidence * 100).toFixed(0)}%`
              : 'N/A'}
            )
          </span>
        </div>
      )}

      {/* Issues */}
      {snapshot.issues.length > 0 && (
        <div className="space-y-1.5 mb-4">
          {snapshot.issues.slice(0, 3).map((issue, idx) => (
            <div
              key={idx}
              className="text-xs text-amber-400 px-3 py-1.5 rounded-lg bg-amber-500/10 border border-amber-500/20"
            >
              ⚠️ {issue}
            </div>
          ))}
          {snapshot.issues.length > 3 && (
            <p className="text-xs text-muted-foreground pl-1">
              +{snapshot.issues.length - 3} more issues
            </p>
          )}
        </div>
      )}

      {/* Score Breakdown */}
      {snapshot.metadata.scoreBreakdown && (
        <div className="pt-4 border-t border-border/30">
          <p className="text-[10px] text-muted-foreground uppercase tracking-wider font-semibold mb-3">
            Score Breakdown
          </p>
          <div className="grid grid-cols-3 gap-3 text-xs">
            <div className="p-2 rounded-lg bg-background/30 text-center">
              <span className="text-muted-foreground block mb-0.5">
                Medication
              </span>
              <span className="text-foreground font-semibold">
                {(
                  snapshot.metadata.scoreBreakdown.medicationSimilarity * 100
                ).toFixed(0)}
                %
              </span>
            </div>
            <div className="p-2 rounded-lg bg-background/30 text-center">
              <span className="text-muted-foreground block mb-0.5">
                Raw Text
              </span>
              <span className="text-foreground font-semibold">
                {(snapshot.metadata.scoreBreakdown.rawSimilarity * 100).toFixed(
                  0,
                )}
                %
              </span>
            </div>
            {snapshot.metadata.scoreBreakdown.embeddingSimilarity !== null && (
              <div className="p-2 rounded-lg bg-background/30 text-center">
                <span className="text-muted-foreground block mb-0.5">
                  Embedding
                </span>
                <span className="text-foreground font-semibold">
                  {(
                    snapshot.metadata.scoreBreakdown.embeddingSimilarity * 100
                  ).toFixed(0)}
                  %
                </span>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}

function Timeline({
  snapshots,
  currentIndex,
  onSelect,
}: {
  snapshots: MatchRecordingSnapshot[]
  currentIndex: number
  onSelect: (index: number) => void
}) {
  return (
    <div className="relative py-6 px-4">
      {/* Timeline line */}
      <div className="absolute top-1/2 left-4 right-4 h-0.5 bg-gradient-to-r from-border via-teal-500/30 to-border -translate-y-1/2" />

      {/* Timeline nodes */}
      <div className="relative flex items-center justify-between">
        {snapshots.map((snapshot, idx) => {
          const colors = EVENT_COLORS[snapshot.event.type]
          const icon = EVENT_ICONS[snapshot.event.type]
          const isActive = idx === currentIndex
          const isPast = idx < currentIndex

          return (
            <button
              key={snapshot.id}
              onClick={() => onSelect(idx)}
              className="relative group flex flex-col items-center transition-all duration-300 hover:scale-110"
            >
              {/* Node */}
              <div
                className={cn(
                  'w-10 h-10 rounded-full flex items-center justify-center text-sm transition-all duration-300',
                  'border-2 shadow-lg',
                  isActive
                    ? 'bg-teal-500 border-teal-400 text-white scale-125 shadow-teal-500/40'
                    : isPast
                      ? cn(colors.bg, colors.border, colors.text)
                      : 'bg-secondary border-border text-muted-foreground',
                )}
              >
                {icon}
              </div>

              {/* Tooltip */}
              <div
                className={cn(
                  'absolute -bottom-14 left-1/2 -translate-x-1/2 px-3 py-1.5 rounded-lg text-[10px] whitespace-nowrap',
                  'bg-popover border border-border shadow-xl z-10',
                  'opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none',
                )}
              >
                <p className="font-semibold">{snapshot.event.label}</p>
                <p className="text-muted-foreground">
                  {formatTime(snapshot.timestamp)}
                </p>
              </div>

              {/* Active indicator */}
              {isActive && (
                <div className="absolute -bottom-2 w-2 h-2 rounded-full bg-teal-500 animate-pulse shadow-lg shadow-teal-500/50" />
              )}
            </button>
          )
        })}
      </div>
    </div>
  )
}

export function RecordingPlayback({
  recording,
  onClose,
  onExport,
  onDelete,
}: RecordingPlaybackProps) {
  const [currentIndex, setCurrentIndex] = useState(
    recording.snapshots.length - 1,
  )
  const [isPlaying, setIsPlaying] = useState(false)
  const [isExpanded, setIsExpanded] = useState(false)
  const [playbackSpeed, setPlaybackSpeed] = useState(1)

  const currentSnapshot = recording.snapshots[currentIndex]
  const totalSnapshots = recording.snapshots.length

  const goToStart = useCallback(() => setCurrentIndex(0), [])
  const goToEnd = useCallback(
    () => setCurrentIndex(totalSnapshots - 1),
    [totalSnapshots],
  )
  const goToPrev = useCallback(
    () => setCurrentIndex((i) => Math.max(0, i - 1)),
    [],
  )
  const goToNext = useCallback(
    () => setCurrentIndex((i) => Math.min(totalSnapshots - 1, i + 1)),
    [totalSnapshots],
  )

  // Auto-play effect
  useEffect(() => {
    if (!isPlaying) return
    if (currentIndex >= totalSnapshots - 1) {
      setIsPlaying(false)
      return
    }

    const timeout = setTimeout(() => {
      setCurrentIndex((i) => i + 1)
    }, 1500 / playbackSpeed)

    return () => clearTimeout(timeout)
  }, [isPlaying, currentIndex, totalSnapshots, playbackSpeed])

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement
      )
        return

      switch (e.key) {
        case 'ArrowLeft':
          e.preventDefault()
          goToPrev()
          break
        case 'ArrowRight':
          e.preventDefault()
          goToNext()
          break
        case ' ':
          e.preventDefault()
          setIsPlaying((p) => !p)
          break
        case 'Home':
          e.preventDefault()
          goToStart()
          break
        case 'End':
          e.preventDefault()
          goToEnd()
          break
        case 'Escape':
          e.preventDefault()
          onClose?.()
          break
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [goToPrev, goToNext, goToStart, goToEnd, onClose])

  if (!currentSnapshot) {
    return (
      <div className="p-8 text-center text-muted-foreground">
        <p>No snapshots recorded yet.</p>
      </div>
    )
  }

  return (
    <div
      className={cn(
        'rounded-2xl overflow-hidden transition-all duration-500 border border-border/50',
        'bg-gradient-to-br from-background via-background to-secondary/30 backdrop-blur-xl',
        isExpanded ? 'fixed inset-4 z-50' : 'relative',
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-border/50 bg-gradient-to-r from-slate-900/80 to-slate-800/80">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-teal-500/30 to-emerald-500/30 flex items-center justify-center shadow-lg">
            <Layers className="w-5 h-5 text-teal-400" />
          </div>
          <div>
            <h3 className="text-sm font-semibold text-foreground">
              Recording Playback
            </h3>
            <p className="text-xs text-muted-foreground">
              Match{' '}
              <span className="font-mono">
                #{recording.matchId.slice(0, 8)}
              </span>{' '}
              • {totalSnapshots} snapshots
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-secondary/50 text-xs text-muted-foreground">
            <Clock className="w-3.5 h-3.5" />
            {recording.duration
              ? formatDuration(recording.duration)
              : 'In progress'}
          </div>

          {recording.outcome && (
            <div
              className={cn(
                'px-3 py-1.5 rounded-lg text-xs font-semibold',
                recording.outcome === 'approved'
                  ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30'
                  : recording.outcome === 'rejected'
                    ? 'bg-red-500/20 text-red-400 border border-red-500/30'
                    : 'bg-amber-500/20 text-amber-400 border border-amber-500/30',
              )}
            >
              {recording.outcome.charAt(0).toUpperCase() +
                recording.outcome.slice(1)}
            </div>
          )}

          {onExport && (
            <button
              onClick={onExport}
              className="p-2 rounded-lg bg-secondary/50 hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
              title="Export JSON"
            >
              <Download className="w-4 h-4" />
            </button>
          )}
          {onDelete && (
            <button
              onClick={onDelete}
              className="p-2 rounded-lg bg-red-500/10 hover:bg-red-500/20 text-red-400 transition-colors"
              title="Delete recording"
            >
              <Trash2 className="w-4 h-4" />
            </button>
          )}
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="p-2 rounded-lg bg-secondary/50 hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
            title={isExpanded ? 'Minimize' : 'Maximize'}
          >
            {isExpanded ? (
              <Minimize2 className="w-4 h-4" />
            ) : (
              <Maximize2 className="w-4 h-4" />
            )}
          </button>
          {onClose && (
            <button
              onClick={onClose}
              className="p-2 rounded-lg bg-secondary/50 hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
              title="Close"
            >
              <X className="w-4 h-4" />
            </button>
          )}
        </div>
      </div>

      {/* Timeline */}
      <div className="px-6 py-4 border-b border-border/30 bg-secondary/20">
        <Timeline
          snapshots={recording.snapshots}
          currentIndex={currentIndex}
          onSelect={setCurrentIndex}
        />
      </div>

      {/* Playback Controls */}
      <div className="flex items-center justify-center gap-3 p-4 border-b border-border/30 bg-secondary/10">
        <button
          onClick={goToStart}
          className="p-2 rounded-lg bg-secondary/50 hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
          title="Go to start"
        >
          <SkipBack className="w-4 h-4" />
        </button>
        <button
          onClick={goToPrev}
          disabled={currentIndex === 0}
          className="p-2 rounded-lg bg-secondary/50 hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors disabled:opacity-30"
          title="Previous"
        >
          <ChevronLeft className="w-5 h-5" />
        </button>
        <button
          onClick={() => setIsPlaying(!isPlaying)}
          className={cn(
            'p-3 rounded-xl transition-all duration-300 shadow-lg',
            isPlaying
              ? 'bg-amber-500 text-white shadow-amber-500/30'
              : 'bg-teal-500 text-white shadow-teal-500/30',
          )}
          title={isPlaying ? 'Pause' : 'Play'}
        >
          {isPlaying ? (
            <Pause className="w-5 h-5" />
          ) : (
            <Play className="w-5 h-5" />
          )}
        </button>
        <button
          onClick={goToNext}
          disabled={currentIndex === totalSnapshots - 1}
          className="p-2 rounded-lg bg-secondary/50 hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors disabled:opacity-30"
          title="Next"
        >
          <ChevronRight className="w-5 h-5" />
        </button>
        <button
          onClick={goToEnd}
          className="p-2 rounded-lg bg-secondary/50 hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
          title="Go to end"
        >
          <SkipForward className="w-4 h-4" />
        </button>

        <div className="ml-4 flex items-center gap-2">
          <span className="text-xs text-muted-foreground">Speed:</span>
          {[0.5, 1, 2].map((speed) => (
            <button
              key={speed}
              onClick={() => setPlaybackSpeed(speed)}
              className={cn(
                'px-2.5 py-1 rounded-lg text-xs font-medium transition-colors',
                playbackSpeed === speed
                  ? 'bg-teal-500 text-white'
                  : 'bg-secondary/50 text-muted-foreground hover:text-foreground',
              )}
            >
              {speed}x
            </button>
          ))}
        </div>

        <div className="ml-4 text-sm text-muted-foreground tabular-nums">
          <span className="text-teal-400 font-bold">{currentIndex + 1}</span>
          <span> / {totalSnapshots}</span>
        </div>
      </div>

      {/* Snapshot Content */}
      <div
        className={cn(
          'p-6 overflow-auto',
          isExpanded ? 'max-h-[calc(100vh-280px)]' : 'max-h-[400px]',
        )}
      >
        <SnapshotCard snapshot={currentSnapshot} isActive={true} />
      </div>

      {/* Keyboard shortcuts hint */}
      <div className="px-4 py-2 border-t border-border/30 bg-secondary/10">
        <p className="text-[10px] text-muted-foreground text-center">
          ← → Navigate • Space Play/Pause • Home/End Jump • Esc Close
        </p>
      </div>
    </div>
  )
}
