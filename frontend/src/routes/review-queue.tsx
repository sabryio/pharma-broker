import { createFileRoute } from '@tanstack/react-router'
import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import {
  CheckCircle,
  Download,
  FileSpreadsheet,
  FileText,
  History,
  Keyboard,
  Layers,
  Loader2,
  RefreshCw,
  Undo2,
  AlertTriangle,
} from 'lucide-react'

import { DashboardLayout } from '@/components/layout/dashboard-layout'
import {
  type Review,
  type HistoryEntry,
  type AdjustmentSettings,
  defaultAdjustments,
  ReviewCard,
  MatchConfidenceMeter,
  ReviewActions,
  AdjustmentControls,
  QueueProgress,
  ReviewNavigation,
  BulkModeGrid,
  HistoryLog,
  ReviewStatsCards,
} from '@/components/review-queue'
import { useNotifications } from '@/hooks/use-notifications'
import {
  useMatchReviews,
  useMatchReviewStats,
  useUpdateMatchReviewStatus,
  useBulkUpdateMatchReviews,
} from '@/hooks/use-match-reviews'
import { cn } from '@/lib/utils'

export const Route = createFileRoute('/review-queue')({
  component: ReviewQueue,
})

type ReviewWithUuid = Review & { uuid: string }

export default function ReviewQueue() {
  const {
    data: reviewData,
    isLoading,
    error,
    refetch,
  } = useMatchReviews({ limit: 50 })
  const { data: apiStats } = useMatchReviewStats()
  const updateMutation = useUpdateMatchReviewStatus()
  const bulkMutation = useBulkUpdateMatchReviews()

  const [currentIndex, setCurrentIndex] = useState(0)
  const [bulkMode, setBulkMode] = useState(false)
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set())
  const [showHistory, setShowHistory] = useState(false)
  const [history, setHistory] = useState<HistoryEntry[]>([])
  const [showExportMenu, setShowExportMenu] = useState(false)
  const [adjustments, setAdjustments] =
    useState<AdjustmentSettings>(defaultAdjustments)
  const [optimisticallyRemoved, setOptimisticallyRemoved] = useState<
    Set<string>
  >(new Set())

  const { notifyHighPriorityReview, notifyLowApprovalRate, settings } =
    useNotifications()
  const notifiedReviewsRef = useRef<Set<number>>(new Set())
  const lastApprovalRateRef = useRef<number | null>(null)

  const pendingReviews = (reviewData?.items ?? []).filter(
    (item) => !optimisticallyRemoved.has(item.uuid),
  ) as ReviewWithUuid[]

  const current = pendingReviews[currentIndex] as ReviewWithUuid | undefined
  const totalReviews = reviewData?.total ?? 0

  const approvalRate =
    history.length > 0
      ? (history.filter((h) => h.action === 'approved').length /
          history.length) *
        100
      : 100

  useEffect(() => {
    if (currentIndex >= pendingReviews.length && pendingReviews.length > 0) {
      setCurrentIndex(Math.max(0, pendingReviews.length - 1))
    }
  }, [pendingReviews.length, currentIndex])

  useEffect(() => {
    pendingReviews.forEach((review) => {
      if (
        review.confidence < settings.highPriorityThreshold &&
        !notifiedReviewsRef.current.has(review.id)
      ) {
        notifyHighPriorityReview(review.offer.product, review.confidence)
        notifiedReviewsRef.current.add(review.id)
      }
    })
  }, [pendingReviews, settings.highPriorityThreshold, notifyHighPriorityReview])

  useEffect(() => {
    if (history.length >= 5) {
      if (
        lastApprovalRateRef.current !== null &&
        lastApprovalRateRef.current >= settings.approvalRateThreshold &&
        approvalRate < settings.approvalRateThreshold
      ) {
        notifyLowApprovalRate(approvalRate)
      }
      lastApprovalRateRef.current = approvalRate
    }
  }, [
    approvalRate,
    history.length,
    settings.approvalRateThreshold,
    notifyLowApprovalRate,
  ])

  const nextReview = useCallback(() => {
    if (pendingReviews.length > 0) {
      setCurrentIndex((i) => (i + 1) % pendingReviews.length)
    }
  }, [pendingReviews.length])

  const prevReview = useCallback(() => {
    if (pendingReviews.length > 0) {
      setCurrentIndex(
        (i) => (i - 1 + pendingReviews.length) % pendingReviews.length,
      )
    }
  }, [pendingReviews.length])

  const handleSingleAction = useCallback(
    (id: number, action: 'approved' | 'rejected') => {
      const review = pendingReviews.find((r) => r.id === id) as
        | ReviewWithUuid
        | undefined
      if (!review) return

      const entry: HistoryEntry = {
        id: `${id}-${Date.now()}`,
        reviewId: id,
        product: review.offer.product,
        action,
        timestamp: new Date(),
        confidence: review.confidence,
        adjustments: { ...adjustments },
        originalReview: review,
      }

      setHistory((prev) => [entry, ...prev])
      setOptimisticallyRemoved((prev) => new Set(prev).add(review.uuid))

      updateMutation.mutate(
        { id: review.uuid, action },
        {
          onError: () => {
            setOptimisticallyRemoved((prev) => {
              const next = new Set(prev)
              next.delete(review.uuid)
              return next
            })
            setHistory((prev) => prev.slice(1))
            toast.error('Failed to update match status')
          },
        },
      )

      if (currentIndex >= pendingReviews.length - 1) {
        setCurrentIndex(Math.max(0, pendingReviews.length - 2))
      }

      toast.success(`Match ${action}`, {
        description: review.offer.product,
        action: { label: 'Undo', onClick: () => restoreFromHistory(entry.id) },
      })
    },
    [pendingReviews, adjustments, currentIndex, updateMutation],
  )

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (showHistory) return
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement
      )
        return

      switch (e.key) {
        case 'ArrowLeft':
          e.preventDefault()
          prevReview()
          break
        case 'ArrowRight':
          e.preventDefault()
          nextReview()
          break
        case 'Enter':
          if (!bulkMode && current) {
            e.preventDefault()
            handleSingleAction(current.id, 'approved')
          }
          break
        case 'Escape':
          if (bulkMode) {
            e.preventDefault()
            setBulkMode(false)
            setSelectedIds(new Set())
          }
          break
        case 'Backspace':
        case 'Delete':
          if (!bulkMode && current) {
            e.preventDefault()
            handleSingleAction(current.id, 'rejected')
          }
          break
        case 'z':
          if ((e.ctrlKey || e.metaKey) && history.length > 0) {
            e.preventDefault()
            undoLastAction()
          }
          break
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [
    nextReview,
    prevReview,
    bulkMode,
    current,
    showHistory,
    history,
    handleSingleAction,
  ])

  const toggleSelection = (id: number) => {
    const newSelected = new Set(selectedIds)
    if (newSelected.has(id)) newSelected.delete(id)
    else newSelected.add(id)
    setSelectedIds(newSelected)
  }

  const selectAll = () => {
    if (selectedIds.size === pendingReviews.length) setSelectedIds(new Set())
    else setSelectedIds(new Set(pendingReviews.map((r) => r.id)))
  }

  const handleBulkAction = (action: 'approved' | 'rejected') => {
    const entries: HistoryEntry[] = []
    const uuids: string[] = []

    selectedIds.forEach((id) => {
      const review = pendingReviews.find((r) => r.id === id) as
        | ReviewWithUuid
        | undefined
      if (review) {
        entries.push({
          id: `${id}-${Date.now()}`,
          reviewId: id,
          product: review.offer.product,
          action,
          timestamp: new Date(),
          confidence: review.confidence,
          adjustments: { ...adjustments },
          originalReview: review,
        })
        uuids.push(review.uuid)
      }
    })

    setHistory((prev) => [...entries, ...prev])
    setOptimisticallyRemoved((prev) => {
      const next = new Set(prev)
      uuids.forEach((uuid) => next.add(uuid))
      return next
    })

    bulkMutation.mutate(
      { ids: uuids, action, reviewed_by: 'current-user' },
      {
        onError: () => {
          setOptimisticallyRemoved((prev) => {
            const next = new Set(prev)
            uuids.forEach((uuid) => next.delete(uuid))
            return next
          })
          setHistory((prev) => prev.slice(entries.length))
          toast.error('Failed to bulk update matches')
        },
      },
    )

    setSelectedIds(new Set())
    setCurrentIndex(0)
    setBulkMode(false)
    toast.success(`${entries.length} matches ${action}`)
  }

  const undoLastAction = () => {
    if (history.length === 0) return
    restoreFromHistory(history[0].id)
  }

  const restoreFromHistory = (historyId: string) => {
    const entry = history.find((h) => h.id === historyId)
    if (!entry) return

    const originalReview = entry.originalReview as ReviewWithUuid
    if (pendingReviews.some((r) => r.id === entry.reviewId)) {
      toast.error('Already restored', { description: entry.product })
      return
    }

    setOptimisticallyRemoved((prev) => {
      const next = new Set(prev)
      next.delete(originalReview.uuid)
      return next
    })

    setHistory((prev) => prev.filter((h) => h.id !== historyId))
    toast.success('Match restored to queue', { description: entry.product })
  }

  const formatDateTime = (date: Date) =>
    date.toISOString().replace('T', ' ').substring(0, 19)

  const exportToCSV = () => {
    if (history.length === 0) {
      toast.error('No history to export')
      return
    }
    const headers = [
      'Timestamp',
      'Product',
      'Action',
      'Confidence (%)',
      'Offer Source',
      'Request Source',
    ]
    const rows = history.map((e) => [
      formatDateTime(e.timestamp),
      e.product,
      e.action.toUpperCase(),
      e.confidence,
      e.originalReview.offer.source,
      e.originalReview.request.source,
    ])
    const csvContent = [
      headers.join(','),
      ...rows.map((row) => row.map((cell) => `"${cell}"`).join(',')),
    ].join('\n')
    const blob = new Blob([csvContent], { type: 'text/csv' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = 'review-history.csv'
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
    toast.success('CSV exported successfully')
    setShowExportMenu(false)
  }

  const exportToPDF = () => {
    if (history.length === 0) {
      toast.error('No history to export')
      return
    }
    const htmlContent = `<!DOCTYPE html><html><head><title>PharmaBroker Review History</title><style>body{font-family:Arial,sans-serif;padding:40px}h1{border-bottom:2px solid #00F2FF;padding-bottom:10px}table{width:100%;border-collapse:collapse;margin-top:20px}th{background:#0B0E14;color:#fff;padding:12px;text-align:left}td{padding:10px;border-bottom:1px solid #ddd}.approved{color:#00E676;font-weight:bold}.rejected{color:#ef4444;font-weight:bold}</style></head><body><h1>PharmaBroker Review History</h1><p>Generated: ${new Date().toLocaleString()}</p><table><thead><tr><th>Timestamp</th><th>Product</th><th>Action</th><th>Confidence</th></tr></thead><tbody>${history.map((e) => `<tr><td>${formatDateTime(e.timestamp)}</td><td>${e.product}</td><td class="${e.action}">${e.action.toUpperCase()}</td><td>${e.confidence}%</td></tr>`).join('')}</tbody></table></body></html>`
    const printWindow = window.open('', '_blank')
    if (printWindow) {
      printWindow.document.write(htmlContent)
      printWindow.document.close()
      printWindow.print()
    }
    toast.success('PDF report opened for printing')
    setShowExportMenu(false)
  }

  if (isLoading) {
    return (
      <DashboardLayout>
        <div className="flex items-center justify-center min-h-[400px]">
          <div className="flex flex-col items-center gap-4">
            <Loader2 className="w-8 h-8 text-teal animate-spin" />
            <p className="text-muted-foreground">Loading review queue...</p>
          </div>
        </div>
      </DashboardLayout>
    )
  }

  if (error) {
    return (
      <DashboardLayout>
        <div className="flex items-center justify-center min-h-[400px]">
          <div className="flex flex-col items-center gap-4 text-center">
            <div className="p-4 rounded-full bg-red-500/10">
              <AlertTriangle className="w-8 h-8 text-red-400" />
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
                className="flex items-center gap-2 px-4 py-2 rounded-lg bg-teal hover:bg-teal/80 text-white transition-colors mx-auto"
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

  if (pendingReviews.length === 0) {
    return (
      <DashboardLayout>
        <div className="space-y-6">
          <div className="flex items-center justify-between">
            <div>
              <h1 className="text-2xl font-bold text-foreground">
                Review Queue
              </h1>
              <p className="text-muted-foreground">
                All matches have been reviewed
              </p>
            </div>
            <HeaderActions
              historyCount={history.length}
              showHistory={showHistory}
              showExportMenu={showExportMenu}
              onUndo={undoLastAction}
              onToggleHistory={() => setShowHistory(!showHistory)}
              onToggleExport={() => setShowExportMenu(!showExportMenu)}
              onExportCSV={exportToCSV}
              onExportPDF={exportToPDF}
            />
          </div>
          {showHistory && (
            <HistoryLog history={history} onRestore={restoreFromHistory} />
          )}
          <div className="glass-card-enhanced p-12 rounded-2xl text-center">
            <CheckCircle className="w-16 h-16 text-emerald mx-auto mb-4" />
            <h2 className="text-xl font-semibold text-foreground mb-2">
              Queue Empty
            </h2>
            <p className="text-muted-foreground">
              No pending matches require review at this time.
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
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold text-foreground">Review Queue</h1>
            <p className="text-muted-foreground">
              Verify low-confidence AI matches
            </p>
          </div>
          <div className="flex items-center gap-3">
            <div className="hidden lg:flex items-center gap-2 px-3 py-1.5 rounded-lg bg-secondary/50 text-xs text-muted-foreground">
              <Keyboard className="w-3.5 h-3.5" />
              <span>←→ Nav</span>
              <span className="mx-1">|</span>
              <span>⌘Z Undo</span>
            </div>
            <HeaderActions
              historyCount={history.length}
              showHistory={showHistory}
              showExportMenu={showExportMenu}
              onUndo={undoLastAction}
              onToggleHistory={() => setShowHistory(!showHistory)}
              onToggleExport={() => setShowExportMenu(!showExportMenu)}
              onExportCSV={exportToCSV}
              onExportPDF={exportToPDF}
            />
            <button
              onClick={() => {
                setBulkMode(!bulkMode)
                if (bulkMode) setSelectedIds(new Set())
              }}
              className={cn(
                'flex items-center gap-2 px-4 py-2 rounded-lg font-medium transition-colors',
                bulkMode
                  ? 'bg-amber text-primary-foreground'
                  : 'bg-secondary text-foreground hover:bg-secondary/80',
              )}
            >
              <Layers className="w-4 h-4" />
              {bulkMode ? 'Exit Bulk Mode' : 'Bulk Review'}
            </button>
          </div>
        </div>

        <ReviewStatsCards
          pending={pendingReviews.length}
          approved={history.filter((h) => h.action === 'approved').length}
          rejected={history.filter((h) => h.action === 'rejected').length}
          avgConfidence={
            pendingReviews.length > 0
              ? Math.round(
                  pendingReviews.reduce((acc, r) => acc + r.confidence, 0) /
                    pendingReviews.length,
                )
              : (apiStats?.avgConfidence ?? 0)
          }
        />
        <QueueProgress pending={pendingReviews.length} total={totalReviews} />
        {showHistory && (
          <HistoryLog history={history} onRestore={restoreFromHistory} />
        )}
        {bulkMode && (
          <BulkModeGrid
            reviews={pendingReviews}
            selectedIds={selectedIds}
            onToggle={toggleSelection}
            onSelectAll={selectAll}
            onBulkAction={handleBulkAction}
          />
        )}

        {!bulkMode && current && (
          <>
            <ReviewNavigation
              currentIndex={currentIndex}
              total={pendingReviews.length}
              onPrev={prevReview}
              onNext={nextReview}
              onSelect={setCurrentIndex}
            />
            <div className="glass-card-enhanced p-8 rounded-2xl animate-scale-in">
              <div className="grid grid-cols-1 lg:grid-cols-7 gap-6 items-stretch">
                <div className="lg:col-span-2">
                  <ReviewCard type="offer" offer={current.offer} />
                </div>
                <div className="lg:col-span-3 flex flex-col items-center justify-center py-6">
                  <MatchConfidenceMeter confidence={current.confidence} />
                  <div className="w-full max-w-sm space-y-2 mt-6">
                    {current.issues.map((issue, idx) => (
                      <div
                        key={idx}
                        className="flex items-start gap-2 p-2 rounded-lg bg-amber/10 border border-amber/20 animate-fade-in"
                        style={{ animationDelay: `${idx * 100}ms` }}
                      >
                        <AlertTriangle className="w-4 h-4 text-amber shrink-0 mt-0.5" />
                        <span className="text-xs text-amber">{issue}</span>
                      </div>
                    ))}
                  </div>
                </div>
                <div className="lg:col-span-2">
                  <ReviewCard type="request" request={current.request} />
                </div>
              </div>
              <ReviewActions
                onApprove={() => handleSingleAction(current.id, 'approved')}
                onReject={() => handleSingleAction(current.id, 'rejected')}
              />
            </div>
            <AdjustmentControls
              adjustments={adjustments}
              onAdjustmentsChange={setAdjustments}
            />
          </>
        )}
      </div>
    </DashboardLayout>
  )
}

function HeaderActions({
  historyCount,
  showHistory,
  showExportMenu,
  onUndo,
  onToggleHistory,
  onToggleExport,
  onExportCSV,
  onExportPDF,
}: {
  historyCount: number
  showHistory: boolean
  showExportMenu: boolean
  onUndo: () => void
  onToggleHistory: () => void
  onToggleExport: () => void
  onExportCSV: () => void
  onExportPDF: () => void
}) {
  return (
    <>
      {historyCount > 0 && (
        <button
          onClick={onUndo}
          className="flex items-center gap-2 px-4 py-2 rounded-lg bg-amber/20 text-amber border border-amber/30 hover:bg-amber/30 transition-colors"
        >
          <Undo2 className="w-4 h-4" />
          Undo
        </button>
      )}
      <div className="relative">
        <button
          onClick={onToggleExport}
          className="flex items-center gap-2 px-4 py-2 rounded-lg bg-secondary text-foreground hover:bg-secondary/80 transition-colors"
        >
          <Download className="w-4 h-4" />
          Export
        </button>
        {showExportMenu && (
          <div className="absolute right-0 top-full mt-2 w-48 glass-card rounded-lg border border-border p-2 z-50 animate-scale-in">
            <button
              onClick={onExportCSV}
              className="flex items-center gap-3 w-full px-3 py-2 rounded-lg hover:bg-secondary/50 text-sm text-foreground transition-colors"
            >
              <FileSpreadsheet className="w-4 h-4 text-emerald" />
              Export as CSV
            </button>
            <button
              onClick={onExportPDF}
              className="flex items-center gap-3 w-full px-3 py-2 rounded-lg hover:bg-secondary/50 text-sm text-foreground transition-colors"
            >
              <FileText className="w-4 h-4 text-destructive" />
              Export as PDF
            </button>
          </div>
        )}
      </div>
      <button
        onClick={onToggleHistory}
        className={cn(
          'flex items-center gap-2 px-4 py-2 rounded-lg transition-colors',
          showHistory
            ? 'bg-teal text-primary-foreground'
            : 'bg-secondary text-foreground hover:bg-secondary/80',
        )}
      >
        <History className="w-4 h-4" />
        History ({historyCount})
      </button>
    </>
  )
}
