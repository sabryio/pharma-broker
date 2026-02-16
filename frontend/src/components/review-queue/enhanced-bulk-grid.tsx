// Enhanced Bulk Mode Grid Component
// Gallery view with quick actions and visual selection

import { useState, useMemo, useCallback } from 'react'
import { cn } from '@/lib/utils'
import { Checkbox } from '@/components/ui/checkbox'
import {
  CheckCircle,
  X,
  Sparkles,
  Filter,
  Grid3X3,
  LayoutList,
  ChevronLeft,
  ChevronRight,
  Zap,
  AlertTriangle,
  Eye,
  Loader2,
} from 'lucide-react'
import type { Review } from './types'
import { getConfidenceColor } from './types'

type ViewMode = 'grid' | 'gallery'
type QuickFilter = 'all' | 'high' | 'medium' | 'low' | 'ai-flagged'

interface EnhancedBulkGridProps {
  reviews: Review[]
  selectedIds: Set<string>
  onToggle: (id: string) => void
  onSelectAll: () => void
  onBulkAction: (action: 'approved' | 'rejected') => void
  isProcessing?: boolean
}

// Quick filter presets
const quickFilters: {
  id: QuickFilter
  label: string
  icon: React.ElementType
}[] = [
  { id: 'all', label: 'All', icon: Grid3X3 },
  { id: 'high', label: 'High (80%+)', icon: CheckCircle },
  { id: 'medium', label: 'Medium', icon: AlertTriangle },
  { id: 'low', label: 'Low (<50%)', icon: X },
  { id: 'ai-flagged', label: 'AI Flagged', icon: Sparkles },
]

// Gallery Card Component
function GalleryCard({
  review,
  isSelected,
  onToggle,
  onPreview,
}: {
  review: Review
  isSelected: boolean
  onToggle: () => void
  onPreview: () => void
}) {
  return (
    <div
      className={cn(
        'relative group rounded-xl overflow-hidden transition-all duration-300',
        'border-2 cursor-pointer',
        isSelected
          ? 'border-teal shadow-lg shadow-teal/20 scale-[1.02]'
          : 'border-transparent hover:border-border/50',
      )}
    >
      {/* Selection overlay */}
      {isSelected && (
        <div className="absolute inset-0 bg-teal/10 z-10 pointer-events-none" />
      )}

      {/* Card content */}
      <div
        onClick={onToggle}
        className="p-4 bg-linear-to-br from-secondary/80 to-secondary/40 backdrop-blur-sm"
      >
        {/* Header */}
        <div className="flex items-start justify-between mb-3">
          <div className="flex items-center gap-2">
            <Checkbox
              checked={isSelected}
              onCheckedChange={onToggle}
              className="data-[state=checked]:bg-teal data-[state=checked]:border-teal"
            />
            <div
              className={cn(
                'px-2 py-0.5 rounded-full text-xs font-bold',
                review.confidence >= 80 && 'bg-emerald-500/20 text-emerald-400',
                review.confidence >= 50 &&
                  review.confidence < 80 &&
                  'bg-amber-500/20 text-amber-400',
                review.confidence < 50 && 'bg-red-500/20 text-red-400',
              )}
            >
              {review.confidence}%
            </div>
          </div>

          {/* Status Badge - Show match status instead of removed aiStatus */}
          {review.status && review.status !== 'PENDING' && (
            <div
              className={cn(
                'flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium',
                review.status === 'CONFIRMED' &&
                  'bg-emerald-500/20 text-emerald-400',
                review.status === 'REJECTED' && 'bg-red-500/20 text-red-400',
                review.status === 'EXPIRED' && 'bg-amber-500/20 text-amber-400',
              )}
            >
              <Sparkles className="w-3 h-3" />
              {review.status}
            </div>
          )}
        </div>

        {/* Products */}
        <div className="space-y-2 mb-3">
          <div className="flex items-center gap-2">
            <div className="w-1.5 h-1.5 rounded-full bg-teal" />
            <p className="text-sm font-medium text-foreground truncate">
              {review.offer.product}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <div className="w-1.5 h-1.5 rounded-full bg-amber" />
            <p className="text-sm text-muted-foreground truncate">
              {review.request.product}
            </p>
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between text-xs text-muted-foreground">
          <span>{review.offer.source}</span>
          <button
            onClick={(e) => {
              e.stopPropagation()
              onPreview()
            }}
            className="flex items-center gap-1 px-2 py-1 rounded-lg bg-white/5 hover:bg-white/10 transition-colors"
          >
            <Eye className="w-3 h-3" />
            Preview
          </button>
        </div>
      </div>

      {/* Selection indicator */}
      {isSelected && (
        <div className="absolute top-2 right-2 w-6 h-6 rounded-full bg-teal flex items-center justify-center z-20">
          <CheckCircle className="w-4 h-4 text-white" />
        </div>
      )}
    </div>
  )
}

// Preview Modal
function PreviewModal({
  review,
  onClose,
  onApprove,
  onReject,
}: {
  review: Review
  onClose: () => void
  onApprove: () => void
  onReject: () => void
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm">
      <div className="w-full max-w-lg bg-card rounded-2xl border border-border shadow-2xl animate-in zoom-in-95 duration-200">
        <div className="p-6">
          {/* Header */}
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-lg font-semibold text-foreground">
              Match Preview
            </h3>
            <button
              onClick={onClose}
              className="p-2 rounded-lg hover:bg-secondary transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          {/* Content */}
          <div className="space-y-4">
            {/* Confidence */}
            <div className="flex items-center justify-center">
              <div
                className={cn(
                  'text-4xl font-bold',
                  getConfidenceColor(review.confidence),
                )}
              >
                {review.confidence}%
              </div>
            </div>

            {/* Offer */}
            <div className="p-4 rounded-xl bg-teal/10 border border-teal/20">
              <p className="text-xs text-teal mb-1 font-medium">OFFER</p>
              <p className="text-foreground font-medium">
                {review.offer.product}
              </p>
              <p className="text-sm text-muted-foreground mt-1">
                {review.offer.quantity} • {review.offer.price}
              </p>
            </div>

            {/* Request */}
            <div className="p-4 rounded-xl bg-amber/10 border border-amber/20">
              <p className="text-xs text-amber mb-1 font-medium">REQUEST</p>
              <p className="text-foreground font-medium">
                {review.request.product}
              </p>
              <p className="text-sm text-muted-foreground mt-1">
                {review.request.quantity} • Max: {review.request.maxPrice}
              </p>
            </div>

            {/* Issues */}
            {review.issues.length > 0 && (
              <div className="space-y-2">
                {review.issues.map((issue, idx) => (
                  <div
                    key={idx}
                    className="flex items-start gap-2 p-2 rounded-lg bg-amber/10 text-xs text-amber"
                  >
                    <AlertTriangle className="w-3 h-3 mt-0.5 shrink-0" />
                    {issue}
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Actions */}
          <div className="flex items-center gap-3 mt-6 pt-4 border-t border-border">
            <button
              onClick={() => {
                onReject()
                onClose()
              }}
              className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl bg-red-500/20 text-red-400 hover:bg-red-500/30 transition-colors font-medium"
            >
              <X className="w-4 h-4" />
              Reject
            </button>
            <button
              onClick={() => {
                onApprove()
                onClose()
              }}
              className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl bg-emerald-500/20 text-emerald-400 hover:bg-emerald-500/30 transition-colors font-medium"
            >
              <CheckCircle className="w-4 h-4" />
              Approve
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

export function EnhancedBulkGrid({
  reviews,
  selectedIds,
  onToggle,
  onSelectAll,
  onBulkAction,
  isProcessing = false,
}: EnhancedBulkGridProps) {
  const [viewMode, setViewMode] = useState<ViewMode>('grid')
  const [quickFilter, setQuickFilter] = useState<QuickFilter>('all')
  const [previewReview, setPreviewReview] = useState<Review | null>(null)
  const [galleryIndex, setGalleryIndex] = useState(0)

  // Filter reviews
  const filteredReviews = useMemo(() => {
    switch (quickFilter) {
      case 'high':
        return reviews.filter((r) => r.confidence >= 80)
      case 'medium':
        return reviews.filter((r) => r.confidence >= 50 && r.confidence < 80)
      case 'low':
        return reviews.filter((r) => r.confidence < 50)
      case 'ai-flagged':
        // Filter by low confidence as proxy for flagged items
        return reviews.filter((r) => r.confidence < 60)
      default:
        return reviews
    }
  }, [reviews, quickFilter])

  // Select all high confidence
  const selectAllHighConfidence = useCallback(() => {
    const highConfIds = reviews
      .filter((r) => r.confidence >= 80)
      .map((r) => r.id)
    highConfIds.forEach((id) => {
      if (!selectedIds.has(id)) {
        onToggle(id)
      }
    })
  }, [reviews, selectedIds, onToggle])

  const allSelected =
    selectedIds.size === filteredReviews.length && filteredReviews.length > 0
  const highConfCount = reviews.filter((r) => r.confidence >= 80).length

  return (
    <div className="space-y-4 animate-scale-in">
      {/* Header */}
      <div className="flex flex-wrap items-center justify-between gap-4 p-4 rounded-xl bg-linear-to-r from-secondary/50 to-secondary/30 border border-border/50">
        {/* Left: Selection controls */}
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <Checkbox
              checked={allSelected}
              onCheckedChange={onSelectAll}
              className="data-[state=checked]:bg-teal data-[state=checked]:border-teal"
            />
            <span className="text-sm font-medium text-foreground">
              {selectedIds.size}/{filteredReviews.length} selected
            </span>
          </div>

          {/* Quick select high confidence */}
          {highConfCount > 0 && (
            <button
              onClick={selectAllHighConfidence}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-emerald-500/20 text-emerald-400 hover:bg-emerald-500/30 transition-colors text-xs font-medium"
            >
              <Zap className="w-3 h-3" />
              Select High ({highConfCount})
            </button>
          )}
        </div>

        {/* Center: Quick filters */}
        <div className="flex items-center gap-1.5">
          <Filter className="w-4 h-4 text-muted-foreground mr-1" />
          {quickFilters.map((filter) => (
            <button
              key={filter.id}
              onClick={() => setQuickFilter(filter.id)}
              className={cn(
                'flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs font-medium transition-all',
                quickFilter === filter.id
                  ? 'bg-teal/20 text-teal border border-teal/30'
                  : 'bg-secondary/50 text-muted-foreground hover:text-foreground border border-transparent',
              )}
            >
              <filter.icon className="w-3 h-3" />
              <span className="hidden sm:inline">{filter.label}</span>
            </button>
          ))}
        </div>

        {/* Right: View mode & actions */}
        <div className="flex items-center gap-3">
          {/* View mode toggle */}
          <div className="flex items-center gap-1 p-1 rounded-lg bg-secondary/50">
            <button
              onClick={() => setViewMode('grid')}
              className={cn(
                'p-1.5 rounded-md transition-colors',
                viewMode === 'grid'
                  ? 'bg-teal text-white'
                  : 'text-muted-foreground hover:text-foreground',
              )}
            >
              <Grid3X3 className="w-4 h-4" />
            </button>
            <button
              onClick={() => setViewMode('gallery')}
              className={cn(
                'p-1.5 rounded-md transition-colors',
                viewMode === 'gallery'
                  ? 'bg-teal text-white'
                  : 'text-muted-foreground hover:text-foreground',
              )}
            >
              <LayoutList className="w-4 h-4" />
            </button>
          </div>

          {/* Bulk actions */}
          {selectedIds.size > 0 && (
            <div className="flex items-center gap-2">
              <button
                onClick={() => onBulkAction('rejected')}
                disabled={isProcessing}
                className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-red-500/20 text-red-400 hover:bg-red-500/30 transition-colors text-sm font-medium disabled:opacity-50"
              >
                {isProcessing ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <X className="w-4 h-4" />
                )}
                Reject
              </button>
              <button
                onClick={() => onBulkAction('approved')}
                disabled={isProcessing}
                className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-emerald-500/20 text-emerald-400 hover:bg-emerald-500/30 transition-colors text-sm font-medium disabled:opacity-50"
              >
                {isProcessing ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <CheckCircle className="w-4 h-4" />
                )}
                Approve ({selectedIds.size})
              </button>
            </div>
          )}
        </div>
      </div>

      {/* Content */}
      {viewMode === 'grid' ? (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
          {filteredReviews.map((review) => (
            <GalleryCard
              key={review.id}
              review={review}
              isSelected={selectedIds.has(review.id)}
              onToggle={() => onToggle(review.id)}
              onPreview={() => setPreviewReview(review)}
            />
          ))}
        </div>
      ) : (
        /* Gallery/Carousel View */
        <div className="relative">
          {filteredReviews.length > 0 && (
            <>
              <div className="flex items-center justify-center gap-4">
                <button
                  onClick={() => setGalleryIndex(Math.max(0, galleryIndex - 1))}
                  disabled={galleryIndex === 0}
                  className="p-3 rounded-full bg-secondary/50 hover:bg-secondary disabled:opacity-30 transition-all"
                >
                  <ChevronLeft className="w-6 h-6" />
                </button>

                <div className="w-full max-w-md">
                  {filteredReviews[galleryIndex] && (
                    <GalleryCard
                      review={filteredReviews[galleryIndex]}
                      isSelected={selectedIds.has(
                        filteredReviews[galleryIndex].id,
                      )}
                      onToggle={() =>
                        onToggle(filteredReviews[galleryIndex]!.id)
                      }
                      onPreview={() => {
                        const review = filteredReviews[galleryIndex]
                        if (review) {
                          setPreviewReview(review)
                        }
                      }}
                    />
                  )}
                </div>

                <button
                  onClick={() =>
                    setGalleryIndex(
                      Math.min(filteredReviews.length - 1, galleryIndex + 1),
                    )
                  }
                  disabled={galleryIndex === filteredReviews.length - 1}
                  className="p-3 rounded-full bg-secondary/50 hover:bg-secondary disabled:opacity-30 transition-all"
                >
                  <ChevronRight className="w-6 h-6" />
                </button>
              </div>

              {/* Gallery dots */}
              <div className="flex items-center justify-center gap-1.5 mt-4">
                {filteredReviews.slice(0, 10).map((_, idx) => (
                  <button
                    key={idx}
                    onClick={() => setGalleryIndex(idx)}
                    className={cn(
                      'w-2 h-2 rounded-full transition-all',
                      idx === galleryIndex
                        ? 'w-6 bg-teal'
                        : 'bg-muted-foreground/30 hover:bg-muted-foreground/50',
                    )}
                  />
                ))}
                {filteredReviews.length > 10 && (
                  <span className="text-xs text-muted-foreground ml-2">
                    +{filteredReviews.length - 10} more
                  </span>
                )}
              </div>
            </>
          )}
        </div>
      )}

      {/* Empty state */}
      {filteredReviews.length === 0 && (
        <div className="text-center py-12">
          <Filter className="w-12 h-12 text-muted-foreground/30 mx-auto mb-3" />
          <p className="text-muted-foreground">
            No matches found with current filter
          </p>
        </div>
      )}

      {/* Preview Modal */}
      {previewReview && (
        <PreviewModal
          review={previewReview}
          onClose={() => setPreviewReview(null)}
          onApprove={() => onBulkAction('approved')}
          onReject={() => onBulkAction('rejected')}
        />
      )}
    </div>
  )
}
