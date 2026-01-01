import { createFileRoute } from '@tanstack/react-router'

import { ProgressRing } from '@/components/custom-ui/progress-ring'
import { DashboardLayout } from '@/components/layout/dashboard-layout'
import { Badge } from '@/components/ui/badge'
import { Calendar as CalendarComponent } from '@/components/ui/calendar'
import { Checkbox } from '@/components/ui/checkbox'
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
import { Slider } from '@/components/ui/slider'
import { useNotifications } from '@/hooks/use-notifications'
import { cn } from '@/lib/utils'
import { format } from 'date-fns'
import {
  AlertTriangle,
  Calendar,
  CheckCircle,
  ChevronLeft,
  ChevronRight,
  Download,
  FileSpreadsheet,
  FileText,
  Filter,
  History,
  Keyboard,
  Layers,
  Search,
  Undo2,
  X,
  XCircle,
} from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'

interface Review {
  id: number
  confidence: number
  offer: {
    product: string
    source: string
    quantity: string
    price: string
    expiry: string
  }
  request: {
    product: string
    source: string
    quantity: string
    maxPrice: string
    urgency: string
  }
  issues: string[]
}

interface HistoryEntry {
  id: string
  reviewId: number
  product: string
  action: 'approved' | 'rejected'
  timestamp: Date
  confidence: number
  adjustments: {
    priceFlexibility: number
    quantityTolerance: number
    dosageStrictness: number
  }
  originalReview: Review
}

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
  const [adjustments, setAdjustments] = useState({
    priceFlexibility: 10,
    quantityTolerance: 15,
    dosageStrictness: 80,
  })

  const { notifyHighPriorityReview, notifyLowApprovalRate, settings } =
    useNotifications()
  const notifiedReviewsRef = useRef<Set<number>>(new Set())
  const lastApprovalRateRef = useRef<number | null>(null)

  const current = pendingReviews[currentIndex]

  // Calculate approval rate from history
  const approvalRate =
    history.length > 0
      ? (history.filter((h) => h.action === 'approved').length /
          history.length) *
        100
      : 100

  // Check for high priority reviews and notify
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

  // Check approval rate and notify if it drops below threshold
  useEffect(() => {
    if (history.length >= 5) {
      // Only check after a few reviews
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

  const getConfidenceColor = (conf: number) => {
    if (conf >= 80) return 'text-emerald'
    if (conf >= 60) return 'text-amber'
    return 'text-destructive'
  }

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

  // Keyboard navigation
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

    // Check if already restored
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

  const formatTime = (date: Date) => {
    return date.toLocaleTimeString('en-US', {
      hour: '2-digit',
      minute: '2-digit',
      hour12: true,
    })
  }

  const formatDate = (date: Date) => {
    return date.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    })
  }

  const formatDateTime = (date: Date) => {
    return date.toISOString().replace('T', ' ').substring(0, 19)
  }

  // Export functions
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
      'Offer Quantity',
      'Offer Price',
      'Request Source',
      'Request Quantity',
      'Request Max Price',
      'Urgency',
      'Price Flexibility (%)',
      'Quantity Tolerance (%)',
      'Dosage Strictness (%)',
    ]

    const rows = history.map((entry) => [
      formatDateTime(entry.timestamp),
      entry.product,
      entry.action.toUpperCase(),
      entry.confidence,
      entry.originalReview.offer.source,
      entry.originalReview.offer.quantity,
      entry.originalReview.offer.price,
      entry.originalReview.request.source,
      entry.originalReview.request.quantity,
      entry.originalReview.request.maxPrice,
      entry.originalReview.request.urgency,
      entry.adjustments.priceFlexibility,
      entry.adjustments.quantityTolerance,
      entry.adjustments.dosageStrictness,
    ])

    const csvContent = [
      headers.join(','),
      ...rows.map((row) => row.map((cell) => `"${cell}"`).join(',')),
    ].join('\n')

    downloadFile(csvContent, 'review-history.csv', 'text/csv')
    toast.success('CSV exported successfully')
    setShowExportMenu(false)
  }

  const exportToPDF = () => {
    if (history.length === 0) {
      toast.error('No history to export')
      return
    }

    // Generate HTML for PDF
    const htmlContent = `
<!DOCTYPE html>
<html>
<head>
  <title>PharmaBroker Review History Report</title>
  <style>
    body { font-family: Arial, sans-serif; padding: 40px; color: #333; }
    h1 { color: #0B0E14; border-bottom: 2px solid #00F2FF; padding-bottom: 10px; }
    .meta { color: #666; margin-bottom: 30px; }
    table { width: 100%; border-collapse: collapse; margin-top: 20px; }
    th { background: #0B0E14; color: #fff; padding: 12px; text-align: left; }
    td { padding: 10px; border-bottom: 1px solid #ddd; }
    tr:nth-child(even) { background: #f9f9f9; }
    .approved { color: #00E676; font-weight: bold; }
    .rejected { color: #ef4444; font-weight: bold; }
    .summary { margin-top: 30px; padding: 20px; background: #f0f9ff; border-radius: 8px; }
    .footer { margin-top: 40px; font-size: 12px; color: #999; text-align: center; }
  </style>
</head>
<body>
  <h1>PharmaBroker Review History Report</h1>
  <div class="meta">
    <p><strong>Generated:</strong> ${new Date().toLocaleString()}</p>
    <p><strong>Total Reviews:</strong> ${history.length}</p>
  </div>
  
  <table>
    <thead>
      <tr>
        <th>Timestamp</th>
        <th>Product</th>
        <th>Action</th>
        <th>Confidence</th>
        <th>Offer Source</th>
        <th>Request Source</th>
        <th>Adjustments (P/Q/D)</th>
      </tr>
    </thead>
    <tbody>
      ${history
        .map(
          (entry) => `
        <tr>
          <td>${formatDateTime(entry.timestamp)}</td>
          <td>${entry.product}</td>
          <td class="${entry.action}">${entry.action.toUpperCase()}</td>
          <td>${entry.confidence}%</td>
          <td>${entry.originalReview.offer.source}</td>
          <td>${entry.originalReview.request.source}</td>
          <td>${entry.adjustments.priceFlexibility}% / ${entry.adjustments.quantityTolerance}% / ${entry.adjustments.dosageStrictness}%</td>
        </tr>
      `,
        )
        .join('')}
    </tbody>
  </table>
  
  <div class="summary">
    <h3>Summary</h3>
    <p><strong>Approved:</strong> ${history.filter((h) => h.action === 'approved').length}</p>
    <p><strong>Rejected:</strong> ${history.filter((h) => h.action === 'rejected').length}</p>
    <p><strong>Average Confidence:</strong> ${(history.reduce((acc, h) => acc + h.confidence, 0) / history.length).toFixed(1)}%</p>
  </div>
  
  <div class="footer">
    <p>PharmaBroker - Pharmaceutical B2B Trading Platform</p>
    <p>This report is generated for compliance and auditing purposes.</p>
  </div>
</body>
</html>
    `

    // Open in new window for printing
    const printWindow = window.open('', '_blank')
    if (printWindow) {
      printWindow.document.write(htmlContent)
      printWindow.document.close()
      printWindow.print()
    }

    toast.success('PDF report opened for printing')
    setShowExportMenu(false)
  }

  const downloadFile = (content: string, filename: string, type: string) => {
    const blob = new Blob([content], { type })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = filename
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
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
            <div className="flex items-center gap-3">
              {history.length > 0 && (
                <button
                  onClick={undoLastAction}
                  className="flex items-center gap-2 px-4 py-2 rounded-lg bg-amber/20 text-amber border border-amber/30 hover:bg-amber/30 transition-colors"
                >
                  <Undo2 className="w-4 h-4" />
                  Undo Last
                </button>
              )}
              <div className="relative">
                <button
                  onClick={() => setShowExportMenu(!showExportMenu)}
                  className="flex items-center gap-2 px-4 py-2 rounded-lg bg-secondary text-foreground hover:bg-secondary/80 transition-colors"
                >
                  <Download className="w-4 h-4" />
                  Export
                </button>
                {showExportMenu && (
                  <div className="absolute right-0 top-full mt-2 w-48 glass-card rounded-lg border border-border p-2 z-50 animate-scale-in">
                    <button
                      onClick={exportToCSV}
                      className="flex items-center gap-3 w-full px-3 py-2 rounded-lg hover:bg-secondary/50 text-sm text-foreground transition-colors"
                    >
                      <FileSpreadsheet className="w-4 h-4 text-emerald" />
                      Export as CSV
                    </button>
                    <button
                      onClick={exportToPDF}
                      className="flex items-center gap-3 w-full px-3 py-2 rounded-lg hover:bg-secondary/50 text-sm text-foreground transition-colors"
                    >
                      <FileText className="w-4 h-4 text-destructive" />
                      Export as PDF
                    </button>
                  </div>
                )}
              </div>
              <button
                onClick={() => setShowHistory(!showHistory)}
                className={cn(
                  'flex items-center gap-2 px-4 py-2 rounded-lg transition-colors',
                  showHistory
                    ? 'bg-teal text-primary-foreground'
                    : 'bg-secondary text-foreground hover:bg-secondary/80',
                )}
              >
                <History className="w-4 h-4" />
                History ({history.length})
              </button>
            </div>
          </div>

          {showHistory && (
            <HistoryLog
              history={history}
              formatTime={formatTime}
              formatDate={formatDate}
              onRestore={restoreFromHistory}
            />
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
            {/* Keyboard hint */}
            <div className="hidden lg:flex items-center gap-2 px-3 py-1.5 rounded-lg bg-secondary/50 text-xs text-muted-foreground">
              <Keyboard className="w-3.5 h-3.5" />
              <span>←→ Nav</span>
              <span className="mx-1">|</span>
              <span>⌘Z Undo</span>
            </div>

            {history.length > 0 && (
              <button
                onClick={undoLastAction}
                className="flex items-center gap-2 px-4 py-2 rounded-lg bg-amber/20 text-amber border border-amber/30 hover:bg-amber/30 transition-colors"
              >
                <Undo2 className="w-4 h-4" />
                Undo
              </button>
            )}

            <div className="relative">
              <button
                onClick={() => setShowExportMenu(!showExportMenu)}
                className="flex items-center gap-2 px-4 py-2 rounded-lg bg-secondary text-foreground hover:bg-secondary/80 transition-colors"
              >
                <Download className="w-4 h-4" />
                Export
              </button>
              {showExportMenu && (
                <div className="absolute right-0 top-full mt-2 w-48 glass-card rounded-lg border border-border p-2 z-50 animate-scale-in">
                  <button
                    onClick={exportToCSV}
                    className="flex items-center gap-3 w-full px-3 py-2 rounded-lg hover:bg-secondary/50 text-sm text-foreground transition-colors"
                  >
                    <FileSpreadsheet className="w-4 h-4 text-emerald" />
                    Export as CSV
                  </button>
                  <button
                    onClick={exportToPDF}
                    className="flex items-center gap-3 w-full px-3 py-2 rounded-lg hover:bg-secondary/50 text-sm text-foreground transition-colors"
                  >
                    <FileText className="w-4 h-4 text-destructive" />
                    Export as PDF
                  </button>
                </div>
              )}
            </div>

            <button
              onClick={() => setShowHistory(!showHistory)}
              className={cn(
                'flex items-center gap-2 px-4 py-2 rounded-lg transition-colors',
                showHistory
                  ? 'bg-teal text-primary-foreground'
                  : 'bg-secondary text-foreground hover:bg-secondary/80',
              )}
            >
              <History className="w-4 h-4" />
              History ({history.length})
            </button>

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

        {/* Queue Progress */}
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <span className="text-sm text-muted-foreground">Queue:</span>
            <span className="text-lg font-bold text-amber">
              {pendingReviews.length}
            </span>
            <span className="text-sm text-muted-foreground">pending</span>
          </div>
          <div className="flex-1 h-2 bg-secondary rounded-full overflow-hidden">
            <div
              className="h-full bg-linear-to-r from-teal to-emerald transition-all duration-500"
              style={{
                width: `${((initialPendingReviews.length - pendingReviews.length) / initialPendingReviews.length) * 100}%`,
              }}
            />
          </div>
          <span className="text-sm text-muted-foreground">
            {initialPendingReviews.length - pendingReviews.length} reviewed
          </span>
        </div>

        {/* History Log (Collapsible) */}
        {showHistory && (
          <HistoryLog
            history={history}
            formatTime={formatTime}
            formatDate={formatDate}
            onRestore={restoreFromHistory}
          />
        )}

        {/* Bulk Mode Selection Grid */}
        {bulkMode && (
          <div className="glass-card p-6 rounded-xl animate-scale-in">
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center gap-3">
                <Checkbox
                  checked={selectedIds.size === pendingReviews.length}
                  onCheckedChange={selectAll}
                />
                <span className="text-sm font-medium text-foreground">
                  Select All ({selectedIds.size}/{pendingReviews.length}{' '}
                  selected)
                </span>
              </div>

              {selectedIds.size > 0 && (
                <div className="flex items-center gap-3">
                  <button
                    onClick={() => handleBulkAction('approved')}
                    className="flex items-center gap-2 px-4 py-2 rounded-lg bg-emerald text-primary-foreground font-medium hover:bg-emerald/90 transition-colors"
                  >
                    <CheckCircle className="w-4 h-4" />
                    Approve Selected ({selectedIds.size})
                  </button>
                  <button
                    onClick={() => handleBulkAction('rejected')}
                    className="flex items-center gap-2 px-4 py-2 rounded-lg bg-destructive/20 text-destructive border border-destructive/50 font-medium hover:bg-destructive/30 transition-colors"
                  >
                    <X className="w-4 h-4" />
                    Reject Selected
                  </button>
                </div>
              )}
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
              {pendingReviews.map((review) => (
                <div
                  key={review.id}
                  onClick={() => toggleSelection(review.id)}
                  className={cn(
                    'flex items-center gap-3 p-4 rounded-lg cursor-pointer transition-all duration-200',
                    selectedIds.has(review.id)
                      ? 'bg-teal/20 border-2 border-teal'
                      : 'bg-secondary/50 border-2 border-transparent hover:border-border',
                  )}
                >
                  <Checkbox checked={selectedIds.has(review.id)} />
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-medium text-foreground truncate">
                      {review.offer.product}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {review.offer.source} → {review.request.source}
                    </p>
                  </div>
                  <span
                    className={cn(
                      'text-sm font-bold',
                      getConfidenceColor(review.confidence),
                    )}
                  >
                    {review.confidence}%
                  </span>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Single Review Mode */}
        {!bulkMode && current && (
          <>
            {/* Navigation */}
            <div className="flex items-center justify-between">
              <button
                onClick={prevReview}
                className="flex items-center gap-2 px-4 py-2 rounded-lg bg-secondary hover:bg-secondary/80 text-foreground transition-all hover-lift"
              >
                <ChevronLeft className="w-4 h-4" /> Previous
              </button>
              <div className="flex items-center gap-2">
                {pendingReviews.map((_, idx) => (
                  <button
                    key={idx}
                    onClick={() => setCurrentIndex(idx)}
                    className={cn(
                      'w-2.5 h-2.5 rounded-full transition-all duration-300',
                      idx === currentIndex
                        ? 'bg-teal w-6'
                        : 'bg-muted hover:bg-muted-foreground',
                    )}
                  />
                ))}
              </div>
              <button
                onClick={nextReview}
                className="flex items-center gap-2 px-4 py-2 rounded-lg bg-secondary hover:bg-secondary/80 text-foreground transition-all hover-lift"
              >
                Next <ChevronRight className="w-4 h-4" />
              </button>
            </div>

            {/* Comparison View */}
            <div className="glass-card-enhanced p-8 rounded-2xl animate-scale-in">
              <div className="grid grid-cols-1 lg:grid-cols-7 gap-6 items-stretch">
                {/* Offer Card */}
                <div className="lg:col-span-2 glass-card p-6 rounded-xl border border-teal/30 hover-glow-teal transition-all duration-500">
                  <div className="flex items-center gap-2 mb-4">
                    <div className="w-2 h-2 rounded-full bg-teal animate-pulse" />
                    <span className="text-sm font-semibold text-teal uppercase tracking-wider">
                      Supply Offer
                    </span>
                  </div>
                  <div className="text-xs text-muted-foreground mb-4">
                    Source: {current.offer.source}
                  </div>

                  <div className="space-y-4">
                    <div className="p-3 rounded-lg bg-secondary/30 backdrop-blur-sm">
                      <span className="text-xs text-muted-foreground block mb-1">
                        Product
                      </span>
                      <span className="text-sm font-medium text-foreground">
                        {current.offer.product}
                      </span>
                    </div>
                    <div className="grid grid-cols-2 gap-3">
                      <div className="p-3 rounded-lg bg-secondary/30">
                        <span className="text-xs text-muted-foreground block mb-1">
                          Quantity
                        </span>
                        <span className="text-sm font-medium text-teal">
                          {current.offer.quantity}
                        </span>
                      </div>
                      <div className="p-3 rounded-lg bg-secondary/30">
                        <span className="text-xs text-muted-foreground block mb-1">
                          Price
                        </span>
                        <span className="text-sm font-medium text-teal">
                          {current.offer.price}
                        </span>
                      </div>
                    </div>
                    <div className="p-3 rounded-lg bg-secondary/30">
                      <span className="text-xs text-muted-foreground block mb-1">
                        Expiry Date
                      </span>
                      <span className="text-sm font-medium text-foreground">
                        {current.offer.expiry}
                      </span>
                    </div>
                  </div>
                </div>

                {/* Central Confidence Indicator */}
                <div className="lg:col-span-3 flex flex-col items-center justify-center py-6">
                  <div className="relative mb-6">
                    <div
                      className={cn(
                        'absolute inset-0 rounded-full blur-2xl animate-pulse-slow',
                        current.confidence >= 70
                          ? 'bg-amber/30'
                          : 'bg-destructive/20',
                      )}
                    />
                    <ProgressRing
                      value={current.confidence}
                      size={180}
                      strokeWidth={12}
                      label="Match"
                      sublabel="Confidence"
                    />
                  </div>

                  {/* Issues List */}
                  <div className="w-full max-w-sm space-y-2">
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

                {/* Request Card */}
                <div className="lg:col-span-2 glass-card p-6 rounded-xl border border-amber/30 hover-glow-amber transition-all duration-500">
                  <div className="flex items-center gap-2 mb-4">
                    <div className="w-2 h-2 rounded-full bg-amber animate-pulse" />
                    <span className="text-sm font-semibold text-amber uppercase tracking-wider">
                      Demand Request
                    </span>
                  </div>
                  <div className="text-xs text-muted-foreground mb-4">
                    Source: {current.request.source}
                  </div>

                  <div className="space-y-4">
                    <div className="p-3 rounded-lg bg-secondary/30 backdrop-blur-sm">
                      <span className="text-xs text-muted-foreground block mb-1">
                        Product Needed
                      </span>
                      <span className="text-sm font-medium text-foreground">
                        {current.request.product}
                      </span>
                    </div>
                    <div className="grid grid-cols-2 gap-3">
                      <div className="p-3 rounded-lg bg-secondary/30">
                        <span className="text-xs text-muted-foreground block mb-1">
                          Quantity
                        </span>
                        <span className="text-sm font-medium text-amber">
                          {current.request.quantity}
                        </span>
                      </div>
                      <div className="p-3 rounded-lg bg-secondary/30">
                        <span className="text-xs text-muted-foreground block mb-1">
                          Max Price
                        </span>
                        <span className="text-sm font-medium text-amber">
                          {current.request.maxPrice}
                        </span>
                      </div>
                    </div>
                    <div className="p-3 rounded-lg bg-secondary/30">
                      <span className="text-xs text-muted-foreground block mb-1">
                        Urgency
                      </span>
                      <Badge
                        variant="outline"
                        className={cn(
                          'font-medium',
                          current.request.urgency === 'High' &&
                            'border-destructive/50 text-destructive bg-destructive/10',
                          current.request.urgency === 'Medium' &&
                            'border-amber/50 text-amber bg-amber/10',
                          current.request.urgency === 'Low' &&
                            'border-emerald/50 text-emerald bg-emerald/10',
                        )}
                      >
                        {current.request.urgency}
                      </Badge>
                    </div>
                  </div>
                </div>
              </div>

              {/* Action Buttons */}
              <div className="flex items-center justify-center gap-4 mt-8 pt-6 border-t border-border">
                <button
                  onClick={() => handleSingleAction(current.id, 'approved')}
                  className="flex items-center gap-2 px-8 py-3 rounded-lg bg-emerald text-primary-foreground font-semibold hover:bg-emerald/90 transition-all hover:scale-105 glow-emerald"
                >
                  <CheckCircle className="w-5 h-5" />
                  Approve Match
                </button>
                <button
                  onClick={() => handleSingleAction(current.id, 'rejected')}
                  className="flex items-center gap-2 px-8 py-3 rounded-lg bg-destructive/20 text-destructive border border-destructive/50 font-semibold hover:bg-destructive/30 transition-colors"
                >
                  <X className="w-5 h-5" />
                  Reject
                </button>
              </div>
            </div>

            {/* Adjustment Controls */}
            <div className="glass-card p-6 rounded-xl">
              <h3 className="text-lg font-semibold text-foreground mb-6">
                Adjustment Controls
              </h3>
              <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                <div className="space-y-3">
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-muted-foreground">
                      Price Flexibility
                    </span>
                    <span className="text-sm font-medium text-teal">
                      {adjustments.priceFlexibility}%
                    </span>
                  </div>
                  <Slider
                    value={[adjustments.priceFlexibility]}
                    onValueChange={(v) =>
                      setAdjustments({ ...adjustments, priceFlexibility: v[0] })
                    }
                    max={50}
                    step={1}
                    className="slider-teal"
                  />
                </div>
                <div className="space-y-3">
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-muted-foreground">
                      Quantity Tolerance
                    </span>
                    <span className="text-sm font-medium text-amber">
                      {adjustments.quantityTolerance}%
                    </span>
                  </div>
                  <Slider
                    value={[adjustments.quantityTolerance]}
                    onValueChange={(v) =>
                      setAdjustments({
                        ...adjustments,
                        quantityTolerance: v[0],
                      })
                    }
                    max={50}
                    step={1}
                    className="slider-amber"
                  />
                </div>
                <div className="space-y-3">
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-muted-foreground">
                      Dosage Strictness
                    </span>
                    <span className="text-sm font-medium text-purple-400">
                      {adjustments.dosageStrictness}%
                    </span>
                  </div>
                  <Slider
                    value={[adjustments.dosageStrictness]}
                    onValueChange={(v) =>
                      setAdjustments({ ...adjustments, dosageStrictness: v[0] })
                    }
                    max={100}
                    step={1}
                    className="slider-purple"
                  />
                </div>
              </div>
            </div>
          </>
        )}
      </div>
    </DashboardLayout>
  )
}

// History Log Component with Filtering
function HistoryLog({
  history,
  formatTime,
  formatDate,
  onRestore,
}: {
  history: HistoryEntry[]
  formatTime: (date: Date) => string
  formatDate: (date: Date) => string
  onRestore: (id: string) => void
}) {
  const [searchQuery, setSearchQuery] = useState('')
  const [actionFilter, setActionFilter] = useState<
    'all' | 'approved' | 'rejected'
  >('all')
  const [dateFrom, setDateFrom] = useState<Date | undefined>(undefined)
  const [dateTo, setDateTo] = useState<Date | undefined>(undefined)
  const [showFilters, setShowFilters] = useState(false)

  const filteredHistory = history.filter((entry) => {
    // Search filter
    const matchesSearch =
      searchQuery === '' ||
      entry.product.toLowerCase().includes(searchQuery.toLowerCase()) ||
      entry.originalReview.offer.source
        .toLowerCase()
        .includes(searchQuery.toLowerCase()) ||
      entry.originalReview.request.source
        .toLowerCase()
        .includes(searchQuery.toLowerCase())

    // Action filter
    const matchesAction =
      actionFilter === 'all' || entry.action === actionFilter

    // Date range filter
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
          {/* Search Input */}
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

          {/* Filter Toggle */}
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

        {/* Filter Panel */}
        {showFilters && (
          <div className="flex flex-wrap items-center gap-3 p-4 rounded-lg bg-secondary/30 border border-border animate-fade-in">
            {/* Action Type Filter */}
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

            {/* Date From */}
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

            {/* Date To */}
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

            {/* Clear All Filters */}
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

      {/* Results Count */}
      {hasActiveFilters && (
        <div className="text-sm text-muted-foreground mb-3">
          Showing {filteredHistory.length} of {history.length} results
        </div>
      )}

      {/* History List */}
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
