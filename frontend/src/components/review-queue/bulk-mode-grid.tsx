import { Checkbox } from '@/components/ui/checkbox'
import { CheckCircle, X } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { Review } from './types'
import { getConfidenceColor } from './types'

interface BulkModeGridProps {
  reviews: Review[]
  selectedIds: Set<number>
  onToggle: (id: number) => void
  onSelectAll: () => void
  onBulkAction: (action: 'approved' | 'rejected') => void
}

export function BulkModeGrid({
  reviews,
  selectedIds,
  onToggle,
  onSelectAll,
  onBulkAction,
}: BulkModeGridProps) {
  const allSelected = selectedIds.size === reviews.length

  return (
    <div className="glass-card p-6 rounded-xl animate-scale-in">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <Checkbox checked={allSelected} onCheckedChange={onSelectAll} />
          <span className="text-sm font-medium text-foreground">
            Select All ({selectedIds.size}/{reviews.length} selected)
          </span>
        </div>

        {selectedIds.size > 0 && (
          <div className="flex items-center gap-3">
            <button
              onClick={() => onBulkAction('approved')}
              className="flex items-center gap-2 px-4 py-2 rounded-lg bg-emerald text-primary-foreground font-medium hover:bg-emerald/90 transition-colors"
            >
              <CheckCircle className="w-4 h-4" />
              Approve Selected ({selectedIds.size})
            </button>
            <button
              onClick={() => onBulkAction('rejected')}
              className="flex items-center gap-2 px-4 py-2 rounded-lg bg-destructive/20 text-destructive border border-destructive/50 font-medium hover:bg-destructive/30 transition-colors"
            >
              <X className="w-4 h-4" />
              Reject Selected
            </button>
          </div>
        )}
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
        {reviews.map((review) => (
          <div
            key={review.id}
            onClick={() => onToggle(review.id)}
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
  )
}
