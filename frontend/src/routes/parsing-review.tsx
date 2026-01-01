import { createFileRoute } from '@tanstack/react-router'
import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import {
  Brain,
  ChevronLeft,
  ChevronRight,
  Keyboard,
  Sparkles,
  Undo2,
  Zap,
} from 'lucide-react'

import { DashboardLayout } from '@/components/layout/dashboard-layout'
import {
  type ParsingReviewItem,
  type ParsingStats,
  MessageBubble,
  ParsedResultCard,
  AIConfidenceMeter,
  ParsingReviewActions,
  ParsingStatsCards,
} from '@/components/parsing-review'
import { cn } from '@/lib/utils'

// Sample data - will be replaced with API data
const mockReviewItems: ParsingReviewItem[] = [
  {
    id: '1',
    rawMessageId: 'msg-001',
    originalText:
      'عندي امبسيللين 500 مغ 200 علبة بسعر 45 جنيه للعلبة تاريخ الصلاحية 6/2025',
    senderName: 'Ahmed Pharma',
    groupName: 'Egyptian Pharma Trading',
    timestamp: new Date('2026-01-01T10:30:00'),
    aiResult: {
      type: 'offer',
      medication: 'Ampicillin 500mg',
      quantity: '200 boxes',
      price: '45 EGP',
      expiry: '06/2025',
    },
    confidence: 0.72,
    reason: 'Arabic text with medication name transliteration',
    status: 'pending',
  },
  {
    id: '2',
    rawMessageId: 'msg-002',
    originalText: 'محتاج ميتفورمين 850 عاجل - أي كمية متاحة؟',
    senderName: 'Dr. Mohamed Clinic',
    groupName: 'Cairo Medical Supplies',
    timestamp: new Date('2026-01-01T11:15:00'),
    aiResult: {
      type: 'request',
      medication: 'Metformin 850mg',
      urgency: 'high',
      notes: 'Any quantity available',
    },
    confidence: 0.58,
    reason: 'Urgency detected but quantity unclear',
    status: 'pending',
  },
  {
    id: '3',
    rawMessageId: 'msg-003',
    originalText: 'متوفر omeprazole 20mg 500 units @ 55 EGP exp 09/25',
    senderName: 'Nile Distributors',
    groupName: 'Egyptian Pharma Trading',
    timestamp: new Date('2026-01-01T12:00:00'),
    aiResult: {
      type: 'offer',
      medication: 'Omeprazole 20mg',
      quantity: '500 units',
      price: '55 EGP',
      expiry: '09/2025',
    },
    confidence: 0.91,
    reason: 'Clear structured message with all fields',
    status: 'pending',
  },
  {
    id: '4',
    rawMessageId: 'msg-004',
    originalText: 'لو حد عنده فينتولين سبراي ابعتلي الكمية والسعر',
    senderName: 'Delta Pharmacy',
    groupName: 'Alexandria Pharma Network',
    timestamp: new Date('2026-01-01T14:30:00'),
    aiResult: {
      type: 'request',
      medication: 'Ventolin Inhaler',
      urgency: 'medium',
    },
    confidence: 0.65,
    reason: 'Request detected, medication identified from Arabic',
    status: 'pending',
  },
]

const mockStats: ParsingStats = {
  pending: 23,
  approved: 156,
  rejected: 12,
  skipped: 8,
  avgConfidence: 0.68,
  todayReviewed: 34,
}

export const Route = createFileRoute('/parsing-review')({
  component: ParsingReview,
})

export default function ParsingReview() {
  const [reviewItems, setReviewItems] = useState(mockReviewItems)
  const [currentIndex, setCurrentIndex] = useState(0)
  const [history, setHistory] = useState<
    Array<{ item: ParsingReviewItem; action: string }>
  >([])
  const [stats, setStats] = useState(mockStats)

  const current = reviewItems[currentIndex]

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

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
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
  }, [nextItem, prevItem, current, history])

  const handleAction = (action: 'approved' | 'rejected' | 'skipped') => {
    if (!current) return

    setHistory((prev) => [{ item: current, action }, ...prev])
    setReviewItems((prev) => prev.filter((item) => item.id !== current.id))

    if (currentIndex >= reviewItems.length - 1) {
      setCurrentIndex(Math.max(0, reviewItems.length - 2))
    }

    setStats((prev) => ({
      ...prev,
      pending: prev.pending - 1,
      [action === 'approved' ? 'todayReviewed' : action]:
        prev[action === 'approved' ? 'todayReviewed' : action] + 1,
    }))

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
  }

  const undoLast = () => {
    if (history.length === 0) return
    const last = history[0]
    setReviewItems((prev) =>
      [...prev, last.item].sort(
        (a, b) =>
          new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime(),
      ),
    )
    setHistory((prev) => prev.slice(1))
    toast.success('Restored to queue')
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

          <ParsingStatsCards stats={stats} />

          <div className="glass-card-enhanced p-12 rounded-2xl text-center">
            <Sparkles className="w-16 h-16 text-purple-400 mx-auto mb-4" />
            <h2 className="text-xl font-semibold text-foreground mb-2">
              Queue Empty
            </h2>
            <p className="text-muted-foreground">
              No pending AI parsing results require review at this time.
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
        <ParsingStatsCards stats={stats} />

        {/* Navigation */}
        <div className="flex items-center justify-between">
          <button
            onClick={prevItem}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-secondary hover:bg-secondary/80 text-foreground transition-all hover-lift"
          >
            <ChevronLeft className="w-4 h-4" /> Previous
          </button>
          <div className="flex items-center gap-2">
            {reviewItems.map((_, idx) => (
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
          </div>
          <button
            onClick={nextItem}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-secondary hover:bg-secondary/80 text-foreground transition-all hover-lift"
          >
            Next <ChevronRight className="w-4 h-4" />
          </button>
        </div>

        {/* Main Comparison View */}
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
                width: `${((mockReviewItems.length - reviewItems.length) / mockReviewItems.length) * 100}%`,
              }}
            />
          </div>
          <span className="text-sm text-muted-foreground">
            {mockReviewItems.length - reviewItems.length} reviewed
          </span>
        </div>
      </div>
    </DashboardLayout>
  )
}
