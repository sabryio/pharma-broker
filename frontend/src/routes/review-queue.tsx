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
  Undo2,
} from 'lucide-react'

import { DashboardLayout } from '@/components/layout/dashboard-layout'
import {
  type Review,
  type HistoryEntry,
  type AdjustmentSettings,
  defaultAdjustments,
  ReviewCard,
  ConfidenceIndicator,
  ReviewActions,
  AdjustmentControls,
  QueueProgress,
  ReviewNavigation,
  BulkModeGrid,
  HistoryLog,
} from '@/components/review-queue'
import { useNotifications } from '@/hooks/use-notifications'
import { cn } from '@/lib/utils'

// Sample data - will be replaced with API data
const initialPendingReviews: Review[] = [
  {
    id: 1,
    confidence: 67,
    offer: {
      product: 'Amoxicillin 500mg',
      source: 'Cairo Pharma',
      quantity: '500 units',
      price: '45 EGP',
      expiry: '06/2025',
    },
    request: {
      product: 'Amoxicillin 500mg',
      source: 'Delta Clinic',
      quantity: '400 units',
      maxPrice: '50 EGP',
      urgency: 'Medium',
    },
    issues: [
      'Quantity mismatch: 20% oversupply',
      'Price within acceptable range',
    ],
  },
  {
    id: 2,
    confidence: 58,
    offer: {
      product: 'Metformin 850mg',
      source: 'Alex Distributors',
      quantity: '300 units',
      price: '38 EGP',
      expiry: '03/2025',
    },
    request: {
      product: 'Metformin 500mg',
      source: 'Giza Medical',
      quantity: '350 units',
      maxPrice: '42 EGP',
      urgency: 'High',
    },
    issues: ['Dosage mismatch: 850mg vs 500mg', 'Quantity shortage: 14%'],
  },
  {
    id: 3,
    confidence: 72,
    offer: {
      product: 'Omeprazole 20mg',
      source: 'Nile Pharma',
      quantity: '200 units',
      price: '55 EGP',
      expiry: '09/2025',
    },
    request: {
      product: 'Omeprazole 20mg',
      source: 'Aswan Hospital',
      quantity: '180 units',
      maxPrice: '52 EGP',
      urgency: 'Low',
    },
    issues: ['Price exceeds max by 5.8%'],
  },
  {
    id: 4,
    confidence: 61,
    offer: {
      product: 'Lipitor 20mg',
      source: 'MedSupply Co',
      quantity: '150 units',
      price: '120 EGP',
      expiry: '08/2025',
    },
    request: {
      product: 'Lipitor 20mg',
      source: 'Regional Hospital',
      quantity: '200 units',
      maxPrice: '110 EGP',
      urgency: 'Medium',
    },
    issues: ['Quantity shortage: 25%', 'Price exceeds max by 9%'],
  },
  {
    id: 5,
    confidence: 55,
    offer: {
      product: 'Ventolin Inhaler',
      source: 'PharmaDist',
      quantity: '80 units',
      price: '95 EGP',
      expiry: '12/2025',
    },
    request: {
      product: 'Ventolin Inhaler',
      source: 'City Clinic',
      quantity: '100 units',
      maxPrice: '90 EGP',
      urgency: 'High',
    },
    issues: ['Quantity shortage: 20%', 'Price exceeds max by 5.5%'],
  },
]

export const Route = createFileRoute('/review-queue')({
  component: ReviewQueue,
})

export default function ReviewQueue() {
  const [pendingReviews, setPendingReviews] = useState<Review[]>(
    initialPendingReviews,
  )
  const [currentIndex, setCurrentIndex] = useState(0)
  const [bulkMode, setBulkMode] = useState(false)
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set())
  const [showHistory, setShowHistory] = useState(false)
  const [history, setHistory] = useState<HistoryEntry[]>([])
  const [showExportMenu, setShowExportMenu] = useState(false)
  const [adjustments, setAdjustments] =
    useState<AdjustmentSettings>(defaultAdjustments)

  const { notifyHighPriorityReview, notifyLowApprovalRate, settings } =
    useNotifications()
  const notifiedReviewsRef = useRef<Set<number>>(new Set())
  const lastApprovalRateRef = useRef<number | null>(null)

  const current = pendingReviews[currentIndex]
  const totalReviews = initialPendingReviews.length

  const approvalRate =
    history.length > 0
      ? (history.filter((h) => h.action === 'approved').length /
          history.length) *
        100
      : 100

  // Notification effects
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

  // Navigation
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

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (showHistory) return

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
  }, [nextReview, prevReview, bulkMode, current, showHistory, history])

  // Selection handlers
  const toggleSelection = (id: number) => {
    const newSelected = new Set(selectedIds)
    if (newSelected.has(id)) {
      newSelected.delete(id)
    } else {
      newSelected.add(id)
    }
    setSelectedIds(newSelected)
  }

  const selectAll = () => {
    if (selectedIds.size === pendingReviews.length) {
      setSelectedIds(new Set())
    } else {
      setSelectedIds(new Set(pendingReviews.map((r) => r.id)))
    }
  }

  // Action handlers
  const handleSingleAction = (id: number, action: 'approved' | 'rejected') => {
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
    setPendingReviews((prev) => prev.filter((r) => r.id !== id))

    if (currentIndex >= pendingReviews.length - 1) {
      setCurrentIndex(Math.max(0, pendingReviews.length - 2))
    }

    toast.success(`Match ${action}`, {
      description: review.offer.product,
      action: {
        label: 'Undo',
        onClick: () => restoreFromHistory(entry.id),
      },
    })
  }

  const handleBulkAction = (action: 'approved' | 'rejected') => {
    const entries: HistoryEntry[] = []

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
      }
    })

    setHistory((prev) => [...entries, ...prev])
    setPendingReviews((prev) => prev.filter((r) => !selectedIds.has(r.id)))
    setSelectedIds(new Set())
    setCurrentIndex(0)
    setBulkMode(false)

    toast.success(`${entries.length} matches ${action}`, {
      action: {
        label: 'Undo All',
        onClick: () => {
          entries.forEach((e) => restoreFromHistory(e.id))
        },
      },
    })
  }

  const undoLastAction = () => {
    if (history.length === 0) return
    restoreFromHistory(history[0].id)
  }

  const restoreFromHistory = (historyId: string) => {
    const entry = history.find((h) => h.id === historyId)
    if (!entry) return

    if (pendingReviews.some((r) => r.id === entry.reviewId)) {
      toast.error('Already restored', { description: entry.product })
      return
    }

    setPendingReviews((prev) =>
      [...prev, entry.originalReview].sort((a, b) => a.id - b.id),
    )
    setHistory((prev) => prev.filter((h) => h.id !== historyId))

    toast.success('Match restored to queue', { description: entry.product })
  }

  // Export functions
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

    const rows = history.map((entry) => [
      formatDateTime(entry.timestamp),
      entry.product,
      entry.action.toUpperCase(),
      entry.confidence,
      entry.originalReview.offer.source,
      entry.originalReview.request.source,
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

    const htmlContent = `
<!DOCTYPE html>
<html>
<head>
  <title>PharmaBroker Review History</title>
  <style>
    body { font-family: Arial, sans-serif; padding: 40px; }
    h1 { border-bottom: 2px solid #00F2FF; padding-bottom: 10px; }
    table { width: 100%; border-collapse: collapse; margin-top: 20px; }
    th { background: #0B0E14; color: #fff; padding: 12px; text-align: left; }
    td { padding: 10px; border-bottom: 1px solid #ddd; }
    .approved { color: #00E676; font-weight: bold; }
    .rejected { color: #ef4444; font-weight: bold; }
  </style>
</head>
<body>
  <h1>PharmaBroker Review History</h1>
  <p>Generated: ${new Date().toLocaleString()}</p>
  <table>
    <thead>
      <tr><th>Timestamp</th><th>Product</th><th>Action</th><th>Confidence</th></tr>
    </thead>
    <tbody>
      ${history.map((e) => `<tr><td>${formatDateTime(e.timestamp)}</td><td>${e.product}</td><td class="${e.action}">${e.action.toUpperCase()}</td><td>${e.confidence}%</td></tr>`).join('')}
    </tbody>
  </table>
</body>
</html>`

    const printWindow = window.open('', '_blank')
    if (printWindow) {
      printWindow.document.write(htmlContent)
      printWindow.document.close()
      printWindow.print()
    }

    toast.success('PDF report opened for printing')
    setShowExportMenu(false)
  }

  // Empty queue state
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

                <div className="lg:col-span-3">
                  <ConfidenceIndicator
                    confidence={current.confidence}
                    issues={current.issues}
                  />
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

// Header Actions Component (inline for simplicity)
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
