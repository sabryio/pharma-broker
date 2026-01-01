import { createFileRoute } from '@tanstack/react-router'
import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import {
  Brain,
  ChevronLeft,
  ChevronRight,
  Keyboard,
  Loader2,
  RefreshCw,
  Sparkles,
  Undo2,
  Zap,
} from 'lucide-react'

import { DashboardLayout } from '@/components/layout/dashboard-layout'
import {
  type ParsingReviewItem,
  MessageBubble,
  ParsedResultCard,
  AIConfidenceMeter,
  ParsingReviewActions,
  ParsingStatsCards,
} from '@/components/parsing-review'
import { cn } from '@/lib/utils'
import {
  useReviewQueueItems,
  useReviewQueueStats,
  useUpdateReviewStatus,
} from '@/hooks/use-review-queue'

export const Route = createFileRoute('/parsing-review')({
  component: ParsingReview,
})

export default function ParsingReview() {
  // API hooks
  const {
    data: reviewData,
    isLoading,
    error,
    refetch,
  } = useReviewQueueItems({ limit: 50 })
  const { data: stats, isLoading: statsLoading } = useReviewQueueStats()
  const updateMutation = useUpdateReviewStatus()

  // Local state for navigation and undo
  const [currentIndex, setCurrentIndex] = useState(0)
  const [history, setHistory] = useState<
    Array<{ item: ParsingReviewItem; action: string }>
  >([])
  const [optimisticallyRemoved, setOptimisticallyRemoved] = useState<
    Set<string>
  >(new Set())

  // Filter out optimistically removed items
  const reviewItems =
    reviewData?.items.filter((item) => !optimisticallyRemoved.has(item.id)) ??
    []
  const current = reviewItems[currentIndex]

  // Reset index when items change
  useEffect(() => {
    if (currentIndex >= reviewItems.length && reviewItems.length > 0) {
      setCurrentIndex(Math.max(0, reviewItems.length - 1))
    }
  }, [reviewItems.length, currentIndex])

  // Navigation
  const nextItem = useCallback(() => {
    if (reviewItems.length > 0) {
      setCurrentIndex((i) => (i + 1) % reviewItems.length)
    }
  }, [reviewItems.length])

  const prevItem = useCallback(() => {
    if (reviewItems.length > 0) {
      setCurrentIndex((i) => (i - 1 + reviewItems.length) % reviewItems.length)
    }
  }, [reviewItems.length])

  // Handle action (approve/reject/skip)
  const handleAction = useCallback(
    (action: 'approved' | 'rejected' | 'skipped') => {
      if (!current) return

      // Add to history for undo
      setHistory((prev) => [{ item: current, action }, ...prev])

      // Optimistically remove from local view
      setOptimisticallyRemoved((prev) => new Set(prev).add(current.id))

      // Call API
      updateMutation.mutate(
        { id: current.id, status: action },
        {
          onError: () => {
            // Restore on error
            setOptimisticallyRemoved((prev) => {
              const next = new Set(prev)
              next.delete(current.id)
              return next
            })
            setHistory((prev) => prev.slice(1))
            toast.error('Failed to update status')
          },
        },
      )

      // Adjust index if needed
      if (currentIndex >= reviewItems.length - 1) {
        setCurrentIndex(Math.max(0, reviewItems.length - 2))
      }

      const messages = {
        approved: '✓ Approved - Will create offer/request',
        rejected: '✗ Rejected - Discarded',
        skipped: '→ Skipped for later',
      }

      toast.success(messages[action], {
        description: current.aiResult.medication,
        action: {
          label: 'Undo',
          onClick: undoLast,
        },
      })
    },
    [current, currentIndex, reviewItems.length, updateMutation],
  )

  // Undo last action (frontend only - restores to view)
  const undoLast = useCallback(() => {
    if (history.length === 0) return
    const last = history[0]

    // Remove from optimistically removed set
    setOptimisticallyRemoved((prev) => {
      const next = new Set(prev)
      next.delete(last.item.id)
      return next
    })

    setHistory((prev) => prev.slice(1))
    toast.success('Restored to queue')
  }, [history])

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Don't trigger if user is typing in an input
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement
      ) {
        return
      }

      switch (e.key) {
        case 'ArrowLeft':
          e.preventDefault()
          prevItem()
          break
        case 'ArrowRight':
          e.preventDefault()
          nextItem()
          break
        case 'Enter':
          if (current) {
            e.preventDefault()
            handleAction('approved')
          }
          break
        case 'Backspace':
        case 'Delete':
          if (current) {
            e.preventDefault()
            handleAction('rejected')
          }
          break
        case 'z':
          if ((e.ctrlKey || e.metaKey) && history.length > 0) {
            e.preventDefault()
            undoLast()
          }
          break
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [nextItem, prevItem, current, history, handleAction, undoLast])

  // Loading state
  if (isLoading) {
    return (
      <DashboardLayout>
        <div className="flex items-center justify-center min-h-[400px]">
          <div className="flex flex-col items-center gap-4">
            <Loader2 className="w-8 h-8 text-purple-400 animate-spin" />
            <p className="text-muted-foreground">Loading review queue...</p>
          </div>
        </div>
      </DashboardLayout>
    )
  }

  // Error state
  if (error) {
    return (
      <DashboardLayout>
        <div className="flex items-center justify-center min-h-[400px]">
          <div className="flex flex-col items-center gap-4 text-center">
            <div className="p-4 rounded-full bg-red-500/10">
              <Brain className="w-8 h-8 text-red-400" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-foreground mb-1">
                Failed to load review queue
              </h2>
              <p className="text-muted-foreground text-sm mb-4">
                {error.message}
              </p>
              <button
                onClick={() => refetch()}
                className="flex items-center gap-2 px-4 py-2 rounded-lg bg-purple-500 hover:bg-purple-600 text-white transition-colors mx-auto"
              >
                <RefreshCw className="w-4 h-4" />
                Retry
              </button>
            </div>
          </div>
        </div>
      </DashboardLayout>
    )
  }

  // Empty state
  if (reviewItems.length === 0) {
    return (
      <DashboardLayout>
        <div className="space-y-6">
          <div className="flex items-center justify-between">
            <div>
              <h1 className="text-2xl font-bold text-foreground flex items-center gap-3">
                <Brain className="w-7 h-7 text-purple-400" />
                AI Parsing Review
              </h1>
              <p className="text-muted-foreground">
                All parsing results have been reviewed
              </p>
            </div>
          </div>

          {stats && <ParsingStatsCards stats={stats} />}

          <div className="glass-card-enhanced p-12 rounded-2xl text-center">
            <Sparkles className="w-16 h-16 text-purple-400 mx-auto mb-4" />
            <h2 className="text-xl font-semibold text-foreground mb-2">
              Queue Empty
            </h2>
            <p className="text-muted-foreground">
              No pending AI parsing results require review at this time.
            </p>
            <button
              onClick={() => refetch()}
              className="mt-4 flex items-center gap-2 px-4 py-2 rounded-lg bg-secondary hover:bg-secondary/80 text-foreground transition-colors mx-auto"
            >
              <RefreshCw className="w-4 h-4" />
              Refresh
            </button>
          </div>
        </div>
      </DashboardLayout>
    )
  }

  return (
    <DashboardLayout>
      <div className="space-y-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold text-foreground flex items-center gap-3">
              <Brain className="w-7 h-7 text-purple-400" />
              AI Parsing Review
            </h1>
            <p className="text-muted-foreground">
              Verify low-confidence AI message parsing
            </p>
          </div>
          <div className="flex items-center gap-3">
            <div className="hidden lg:flex items-center gap-2 px-3 py-1.5 rounded-lg bg-secondary/50 text-xs text-muted-foreground">
              <Keyboard className="w-3.5 h-3.5" />
              <span>←→ Nav</span>
              <span className="mx-1">|</span>
              <span>Enter Approve</span>
              <span className="mx-1">|</span>
              <span>⌘Z Undo</span>
            </div>
            {history.length > 0 && (
              <button
                onClick={undoLast}
                className="flex items-center gap-2 px-4 py-2 rounded-lg bg-amber/20 text-amber border border-amber/30 hover:bg-amber/30 transition-colors"
              >
                <Undo2 className="w-4 h-4" />
                Undo ({history.length})
              </button>
            )}
          </div>
        </div>

        {/* Stats */}
        {stats && !statsLoading && <ParsingStatsCards stats={stats} />}

        {/* Navigation */}
        <div className="flex items-center justify-between">
          <button
            onClick={prevItem}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-secondary hover:bg-secondary/80 text-foreground transition-all hover-lift"
          >
            <ChevronLeft className="w-4 h-4" /> Previous
          </button>
          <div className="flex items-center gap-2">
            {reviewItems.slice(0, 10).map((_, idx) => (
              <button
                key={idx}
                onClick={() => setCurrentIndex(idx)}
                className={cn(
                  'w-2.5 h-2.5 rounded-full transition-all duration-300',
                  idx === currentIndex
                    ? 'bg-purple-500 w-6'
                    : 'bg-muted hover:bg-muted-foreground',
                )}
              />
            ))}
            {reviewItems.length > 10 && (
              <span className="text-xs text-muted-foreground ml-2">
                +{reviewItems.length - 10} more
              </span>
            )}
          </div>
          <button
            onClick={nextItem}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-secondary hover:bg-secondary/80 text-foreground transition-all hover-lift"
          >
            Next <ChevronRight className="w-4 h-4" />
          </button>
        </div>

        {/* Main Comparison View */}
        {current && (
          <div className="glass-card-enhanced p-8 rounded-2xl animate-scale-in">
            <div className="grid grid-cols-1 lg:grid-cols-7 gap-8 items-start">
              {/* Original Message */}
              <div className="lg:col-span-2">
                <MessageBubble
                  text={current.originalText}
                  senderName={current.senderName}
                  groupName={current.groupName}
                  timestamp={current.timestamp}
                />
              </div>

              {/* Center - Confidence + Actions */}
              <div className="lg:col-span-3 flex flex-col items-center gap-6">
                <AIConfidenceMeter confidence={current.confidence} />

                {/* Reason tag */}
                <div className="flex items-center gap-2 px-4 py-2 rounded-full bg-purple-500/10 border border-purple-500/30">
                  <Zap className="w-4 h-4 text-purple-400" />
                  <span className="text-sm text-purple-300">
                    {current.reason}
                  </span>
                </div>

                {/* Actions */}
                <div className="w-full max-w-sm">
                  <ParsingReviewActions
                    onApprove={() => handleAction('approved')}
                    onReject={() => handleAction('rejected')}
                    onSkip={() => handleAction('skipped')}
                    loading={updateMutation.isPending}
                  />
                </div>
              </div>

              {/* Parsed Result */}
              <div className="lg:col-span-2">
                <ParsedResultCard
                  result={current.aiResult}
                  confidence={current.confidence}
                />
              </div>
            </div>
          </div>
        )}

        {/* Queue Progress */}
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <span className="text-sm text-muted-foreground">Queue:</span>
            <span className="text-lg font-bold text-purple-400">
              {reviewItems.length}
            </span>
            <span className="text-sm text-muted-foreground">pending</span>
          </div>
          <div className="flex-1 h-2 bg-secondary rounded-full overflow-hidden">
            <div
              className="h-full bg-linear-to-r from-purple-500 to-purple-400 transition-all duration-500"
              style={{
                width: `${(((reviewData?.total ?? 0) - reviewItems.length) / Math.max(1, reviewData?.total ?? 1)) * 100}%`,
              }}
            />
          </div>
          <span className="text-sm text-muted-foreground">
            {(reviewData?.total ?? 0) - reviewItems.length} reviewed
          </span>
        </div>
      </div>
    </DashboardLayout>
  )
}
