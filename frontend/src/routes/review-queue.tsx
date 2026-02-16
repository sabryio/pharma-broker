import { Link, createFileRoute } from '@tanstack/react-router'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import {
  AlertTriangle,
  Bug,
  CheckCircle,
  Download,
  FileSpreadsheet,
  FileText,
  History,
  Keyboard,
  Layers,
  Loader2,
  RefreshCw,
  Stethoscope,
  Undo2,
} from 'lucide-react'

import type {
  AdjustmentSettings,
  FilterState,
  HistoryEntry,
  OfferWithMatches,
  RequestWithMatches,
} from '@/components/review-queue'
import { DashboardLayout } from '@/components/layout/dashboard-layout'
import {
  AdjustmentControls,
  EnhancedBulkGrid,
  FilterBar,
  QueueProgress,
  QuickActionsBar,
  RelatedMatchCarousel,
  StatsDashboard,
  TimelineHistory,
  defaultAdjustments,
  defaultFilterState,
  groupByOffer,
  groupByRequest,
} from '@/components/review-queue'
import { CurationMode } from '@/components/medication-curation'
import { useNotifications } from '@/hooks/use-notifications'
import {
  useBulkUpdateMatchReviews,
  useMatchReviewStats,
  useMatchReviewsManual,
  useUpdateMatchReviewStatus,
} from '@/hooks/use-match-reviews'
import { cn } from '@/lib/utils'
import { useAppSelector, useMatchReviewsActions } from '@/store'
import { selectPageSize } from '@/store/slices/sessionSlice'

export const Route = createFileRoute('/review-queue')({
  component: ReviewQueue,
})

export default function ReviewQueue() {
  const matchReviewsActions = useMatchReviewsActions()
  const pageSize = useAppSelector(selectPageSize)

  const {
    data: reviewData,
    isLoading,
    error,
    refetch,
  } = useMatchReviewsManual({ limit: pageSize })
  const { data: apiStats } = useMatchReviewStats()
  const updateMutation = useUpdateMatchReviewStatus()
  const bulkMutation = useBulkUpdateMatchReviews()

  const [currentIndex, setCurrentIndex] = useState(0)
  const [bulkMode, setBulkMode] = useState(false)
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [showHistory, setShowHistory] = useState(false)
  const [history, setHistory] = useState<Array<HistoryEntry>>([])
  const [showExportMenu, setShowExportMenu] = useState(false)
  const [adjustments] = useState<AdjustmentSettings>(defaultAdjustments)
  const [optimisticallyRemoved, setOptimisticallyRemoved] = useState<
    Set<string>
  >(new Set())
  const [filters, setFilters] = useState<FilterState>(defaultFilterState)

  // Carousel state
  const [anchorMode, setAnchorMode] = useState<'offer' | 'request'>('offer')
  const [anchorIndex, setAnchorIndex] = useState(0)
  const [relatedIndex, setRelatedIndex] = useState(0)
  const [reviewMode, setReviewMode] = useState<'match' | 'curation'>('match')

  const { notifyHighPriorityReview, notifyLowApprovalRate, settings } =
    useNotifications()
  const notifiedReviewsRef = useRef<Set<string>>(new Set())
  const lastApprovalRateRef = useRef<number | null>(null)

  const pendingReviews = (reviewData?.items ?? []).filter(
    (item) => !optimisticallyRemoved.has(item.id),
  )

  // Apply filters
  const filteredReviews = useMemo(() => {
    let result = pendingReviews

    // Confidence band filter
    if (filters.confidenceBand !== 'all') {
      result = result.filter((item) => {
        if (filters.confidenceBand === 'high') return item.confidence >= 80
        if (filters.confidenceBand === 'medium')
          return item.confidence >= 50 && item.confidence < 80
        if (filters.confidenceBand === 'low') return item.confidence < 50
        return true
      })
    }

    // Confidence range filter
    result = result.filter(
      (item) =>
        item.confidence >= filters.minConfidence &&
        item.confidence <= filters.maxConfidence,
    )

    // Medication search filter
    if (filters.medicationSearch) {
      const search = filters.medicationSearch.toLowerCase()
      result = result.filter(
        (item) =>
          item.offer.product.toLowerCase().includes(search) ||
          item.request.product.toLowerCase().includes(search),
      )
    }

    // Status filter (replacing aiStatus filter)
    if (filters.aiStatusFilter !== 'all') {
      result = result.filter((item) => {
        if (filters.aiStatusFilter === 'pending')
          return item.status === 'PENDING'
        if (filters.aiStatusFilter === 'approved')
          return item.status === 'CONFIRMED'
        if (filters.aiStatusFilter === 'rejected')
          return item.status === 'REJECTED'
        return true
      })
    }

    // Sorting
    result = [...result].sort((a, b) => {
      let comparison = 0
      if (filters.sortBy === 'confidence') {
        comparison = a.confidence - b.confidence
      } else if (filters.sortBy === 'age') {
        comparison =
          new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
      } else if (filters.sortBy === 'medication') {
        comparison = a.offer.product.localeCompare(b.offer.product)
      }
      return filters.sortOrder === 'desc' ? -comparison : comparison
    })

    return result
  }, [pendingReviews, filters])

  // Grouped reviews (use filtered)
  const groupedOffers = groupByOffer(filteredReviews)
  const groupedRequests = groupByRequest(filteredReviews)

  const groups = anchorMode === 'offer' ? groupedOffers : groupedRequests
  const currentGroup = groups[anchorIndex]
  const currentMatch = currentGroup?.matches[relatedIndex]

  const current = pendingReviews[currentIndex]
  const totalReviews = reviewData?.total ?? 0

  const approvalRate =
    history.length > 0
      ? (history.filter((h) => h.action === 'approved').length /
          history.length) *
        100
      : 100

  useEffect(() => {
    if (anchorIndex >= groups.length && groups.length > 0) {
      setAnchorIndex(Math.max(0, groups.length - 1))
      setRelatedIndex(0)
    }
  }, [groups.length, anchorIndex])

  useEffect(() => {
    setAnchorIndex(0)
    setRelatedIndex(0)
  }, [anchorMode])

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
    (id: string, action: 'approved' | 'rejected') => {
      const review = pendingReviews.find((r) => r.id === id)
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
      setOptimisticallyRemoved((prev) => new Set(prev).add(review.id))

      // Record action in Redux for history tracking
      matchReviewsActions.recordAction({ type: action, matchId: review.id })

      updateMutation.mutate(
        { id: review.id, action },
        {
          onError: () => {
            setOptimisticallyRemoved((prev) => {
              const next = new Set(prev)
              next.delete(review.id)
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
    [
      pendingReviews,
      adjustments,
      currentIndex,
      updateMutation,
      matchReviewsActions,
    ],
  )

  const handleAnchorModeChange = useCallback(
    (newMode: 'offer' | 'request') => {
      if (newMode === anchorMode) return

      if (currentMatch && currentGroup) {
        if (newMode === 'request') {
          const matchWithReq = currentMatch as any
          if (matchWithReq.request) {
            const reqIndex = groupedRequests.findIndex(
              (g) => g.request.id === matchWithReq.request.id,
            )
            if (reqIndex !== -1) {
              setAnchorIndex(reqIndex)
              const offIndex = groupedRequests[reqIndex]?.matches.findIndex(
                (m) =>
                  m.offer.id === (currentGroup as OfferWithMatches).offer.id,
              )
              setRelatedIndex(Math.max(0, offIndex ?? 0))
            }
          }
        } else {
          const matchWithOff = currentMatch as any
          if (matchWithOff.offer) {
            const offIndex = groupedOffers.findIndex(
              (g) => g.offer.id === matchWithOff.offer.id,
            )
            if (offIndex !== -1) {
              setAnchorIndex(offIndex)
              const reqIndex = groupedOffers[offIndex]?.matches.findIndex(
                (m) =>
                  m.request.id ===
                  (currentGroup as RequestWithMatches).request.id,
              )
              setRelatedIndex(Math.max(0, reqIndex ?? 0))
            }
          }
        }
      }
      setAnchorMode(newMode)
    },
    [anchorMode, currentMatch, currentGroup, groupedOffers, groupedRequests],
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
          if (relatedIndex > 0) {
            setRelatedIndex((i) => i - 1)
          } else if (anchorIndex > 0) {
            setAnchorIndex((i) => i - 1)
            const prevGroup = groups[anchorIndex - 1]
            setRelatedIndex(
              prevGroup?.matches.length ? prevGroup.matches.length - 1 : 0,
            )
          }
          break
        case 'ArrowRight':
          e.preventDefault()
          if (currentGroup && relatedIndex < currentGroup.matches.length - 1) {
            setRelatedIndex((i) => i + 1)
          } else if (anchorIndex < groups.length - 1) {
            setAnchorIndex((i) => i + 1)
            setRelatedIndex(0)
          }
          break
        case 'ArrowUp':
          e.preventDefault()
          if (anchorIndex > 0) {
            setAnchorIndex((i) => i - 1)
            setRelatedIndex(0)
          }
          break
        case 'ArrowDown':
          e.preventDefault()
          if (anchorIndex < groups.length - 1) {
            setAnchorIndex((i) => i + 1)
            setRelatedIndex(0)
          }
          break
        case 'Tab':
          e.preventDefault()
          handleAnchorModeChange(anchorMode === 'offer' ? 'request' : 'offer')
          break
        case 'Enter':
          if (!bulkMode && currentMatch) {
            e.preventDefault()
            handleSingleAction(currentMatch.matchId, 'approved')
          }
          break
        case 'Escape':
          if (bulkMode) {
            e.preventDefault()
            setBulkMode(false)
            setSelectedIds(new Set())
          }
          break
        case '`':
          e.preventDefault()
          setReviewMode((m) => (m === 'match' ? 'curation' : 'match'))
          break
        case 'Backspace':
        case 'Delete':
          if (!bulkMode && currentMatch) {
            e.preventDefault()
            handleSingleAction(currentMatch.matchId, 'rejected')
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

  const toggleSelection = (id: string) => {
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
    const entries: Array<HistoryEntry> = []
    const ids: Array<string> = []

    selectedIds.forEach((id) => {
      const review = pendingReviews.find((r) => r.id === id)
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
        ids.push(review.id)
      }
    })

    setHistory((prev) => [...entries, ...prev])
    setOptimisticallyRemoved((prev) => {
      const next = new Set(prev)
      ids.forEach((id) => next.add(id))
      return next
    })

    bulkMutation.mutate(
      { ids, action, reviewed_by: '00000000-0000-4000-8000-000000000001' },
      {
        onError: () => {
          setOptimisticallyRemoved((prev) => {
            const next = new Set(prev)
            ids.forEach((id) => next.delete(id))
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
    const firstHistory = history[0]
    if (firstHistory) {
      restoreFromHistory(firstHistory.id)
    }
  }

  const restoreFromHistory = (historyId: string) => {
    const entry = history.find((h) => h.id === historyId)
    if (!entry) return

    const originalReview = entry.originalReview
    if (pendingReviews.some((r) => r.id === entry.reviewId)) {
      toast.error('Already restored', { description: entry.product })
      return
    }

    setOptimisticallyRemoved((prev) => {
      const next = new Set(prev)
      next.delete(originalReview.id)
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
            <TimelineHistory history={history} onRestore={restoreFromHistory} />
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
          <div className="flex items-center gap-4 bg-secondary/50 p-1 rounded-xl border border-white/5">
            <button
              onClick={() => setReviewMode('match')}
              className={cn(
                'px-4 py-1.5 rounded-lg text-sm font-bold transition-all flex items-center gap-2',
                reviewMode === 'match'
                  ? 'bg-teal text-white shadow-lg shadow-teal/20'
                  : 'text-muted-foreground hover:text-foreground',
              )}
            >
              <CheckCircle className="w-4 h-4" />
              Match Review
            </button>
            <button
              onClick={() => setReviewMode('curation')}
              className={cn(
                'px-4 py-1.5 rounded-lg text-sm font-bold transition-all flex items-center gap-2',
                reviewMode === 'curation'
                  ? 'bg-teal text-white shadow-lg shadow-teal/20'
                  : 'text-muted-foreground hover:text-foreground',
              )}
            >
              <Stethoscope className="w-4 h-4" />
              Medication Curation
            </button>
          </div>
          <div className="flex items-center gap-3">
            <div className="hidden lg:flex items-center gap-2 px-3 py-1.5 rounded-lg bg-secondary/50 text-xs text-muted-foreground">
              <Keyboard className="w-3.5 h-3.5" />
              <span>←→ Nav</span>
              <span className="mx-1">|</span>
              <span>⌘Z Undo</span>
            </div>
            {/* Debug recordings link */}
            <Link
              to="/debug-recordings"
              className="flex items-center gap-2 px-4 py-2 rounded-lg bg-violet-500/20 text-violet-400 border border-violet-500/30 hover:bg-violet-500/30 transition-colors"
              title="View debug recordings & pipeline data"
            >
              <Bug className="w-4 h-4" />
              Debug
            </Link>
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

        <StatsDashboard
          pending={apiStats?.pending ?? pendingReviews.length}
          approved={apiStats?.confirmedToday ?? 0}
          rejected={apiStats?.rejectedToday ?? 0}
          avgConfidence={apiStats?.avgConfidence ?? 0}
          highConfidenceCount={
            pendingReviews.filter((r) => r.confidence >= 80).length
          }
          mediumConfidenceCount={
            pendingReviews.filter(
              (r) => r.confidence >= 50 && r.confidence < 80,
            ).length
          }
          lowConfidenceCount={
            pendingReviews.filter((r) => r.confidence < 50).length
          }
          compact={bulkMode}
        />

        {reviewMode === 'match' ? (
          <>
            {/* Filter Bar */}
            <FilterBar
              filters={filters}
              onFiltersChange={setFilters}
              totalCount={pendingReviews.length}
              filteredCount={filteredReviews.length}
            />

            <div className="space-y-2">
              <QueueProgress
                pending={filteredReviews.length}
                total={totalReviews}
              />
              {!bulkMode && (
                <div className="flex items-center justify-between text-xs text-muted-foreground px-2">
                  <span>
                    Showing {groupedOffers.length} unique offers from{' '}
                    {filteredReviews.length} matches
                  </span>
                  <span>
                    Total: {apiStats?.pending ?? 0} pending matches in database
                  </span>
                </div>
              )}
            </div>
            {showHistory && (
              <TimelineHistory
                history={history}
                onRestore={restoreFromHistory}
              />
            )}
            {bulkMode && (
              <EnhancedBulkGrid
                reviews={filteredReviews}
                selectedIds={selectedIds}
                onToggle={toggleSelection}
                onSelectAll={selectAll}
                onBulkAction={handleBulkAction}
                isProcessing={bulkMutation.isPending}
              />
            )}

            {!bulkMode && currentMatch && (
              <>
                <RelatedMatchCarousel
                  groupedByOffer={groupedOffers}
                  groupedByRequest={groupedRequests}
                  anchorMode={anchorMode}
                  onAnchorModeChange={handleAnchorModeChange}
                  anchorIndex={anchorIndex}
                  onAnchorIndexChange={setAnchorIndex}
                  relatedIndex={relatedIndex}
                  onRelatedIndexChange={setRelatedIndex}
                  issues={currentMatch.issues}
                  onApprove={(id) => handleSingleAction(id, 'approved')}
                  onReject={(id) => handleSingleAction(id, 'rejected')}
                  apiStats={apiStats}
                />
                <AdjustmentControls />
                <QuickActionsBar
                  onApprove={() =>
                    handleSingleAction(currentMatch.matchId, 'approved')
                  }
                  onReject={() =>
                    handleSingleAction(currentMatch.matchId, 'rejected')
                  }
                  onUndo={undoLastAction}
                  canUndo={history.length > 0}
                  loading={updateMutation.isPending}
                  matchId={currentMatch.matchId}
                  confidence={currentMatch.confidence}
                  position="floating"
                  showKeyboardHints
                />
              </>
            )}
          </>
        ) : (
          <CurationMode />
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
