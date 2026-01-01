// Related Match Carousel Component
// Beautiful carousel for navigating through related matches

import { useCallback, useState } from 'react'
import { cn } from '@/lib/utils'
import { ChevronLeft, ChevronRight, Layers, ArrowLeftRight } from 'lucide-react'
import type { OfferWithMatches, RequestWithMatches } from './types'
import { ReviewCard } from './review-card'
import { MatchConfidenceMeter } from './match-confidence-meter'
import { CurationDialog } from './curation-dialog'
import { ReclassifyDialog } from '@/components/ui/reclassify-dialog'
import type { ItemType } from '@/api/offers'

interface RelatedMatchCarouselProps {
  /** Grouped data - either offers with their requests or requests with their offers */
  groupedByOffer: OfferWithMatches[]
  groupedByRequest: RequestWithMatches[]
  /** Current mode - anchor by offer or request */
  anchorMode: 'offer' | 'request'
  onAnchorModeChange: (mode: 'offer' | 'request') => void
  /** Current anchor index (which offer/request is shown) */
  anchorIndex: number
  onAnchorIndexChange: (index: number) => void
  /** Current related item index (carousel within matches) */
  relatedIndex: number
  onRelatedIndexChange: (index: number) => void
  /** Issues for current match */
  issues: string[]
  /** Action handlers */
  onApprove: (matchId: string) => void
  onReject: (matchId: string) => void
}

export function RelatedMatchCarousel({
  groupedByOffer,
  groupedByRequest,
  anchorMode,
  onAnchorModeChange,
  anchorIndex,
  onAnchorIndexChange,
  relatedIndex,
  onRelatedIndexChange,
  issues,
  onApprove,
  onReject,
}: RelatedMatchCarouselProps) {
  const [curationMedication, setCurationMedication] = useState<string | null>(
    null,
  )
  const [curationAliasId, setCurationAliasId] = useState<string | null>(null)
  
  // Reclassify dialog state
  const [reclassifyItem, setReclassifyItem] = useState<{
    id: string
    type: ItemType
    medication: string
    medicationRaw?: string
  } | null>(null)

  const handleReclassify = useCallback((id: string, type: 'offer' | 'request', medication: string, medicationRaw?: string) => {
    setReclassifyItem({ id, type, medication, medicationRaw })
  }, [])

  const isOfferMode = anchorMode === 'offer'
  const groups = isOfferMode ? groupedByOffer : groupedByRequest
  const currentGroup = groups[anchorIndex]

  if (!currentGroup) return null

  const matches = currentGroup.matches
  const currentMatch = matches[relatedIndex]

  if (!currentMatch) return null

  const totalAnchors = groups.length
  const totalMatches = matches.length

  // Navigation handlers
  const prevAnchor = useCallback(() => {
    if (anchorIndex > 0) {
      onAnchorIndexChange(anchorIndex - 1)
      onRelatedIndexChange(0) // Reset related index
    }
  }, [anchorIndex, onAnchorIndexChange, onRelatedIndexChange])

  const nextAnchor = useCallback(() => {
    if (anchorIndex < totalAnchors - 1) {
      onAnchorIndexChange(anchorIndex + 1)
      onRelatedIndexChange(0) // Reset related index
    }
  }, [anchorIndex, totalAnchors, onAnchorIndexChange, onRelatedIndexChange])

  const prevRelated = useCallback(() => {
    if (relatedIndex > 0) {
      onRelatedIndexChange(relatedIndex - 1)
    }
  }, [relatedIndex, onRelatedIndexChange])

  const nextRelated = useCallback(() => {
    if (relatedIndex < totalMatches - 1) {
      onRelatedIndexChange(relatedIndex + 1)
    }
  }, [relatedIndex, totalMatches, onRelatedIndexChange])

  const handleAction = (action: 'approved' | 'rejected') => {
    const handler = action === 'approved' ? onApprove : onReject
    handler(currentMatch.matchId)

    // Auto-advance to next match
    if (relatedIndex < totalMatches - 1) {
      onRelatedIndexChange(relatedIndex + 1)
    } else if (anchorIndex < totalAnchors - 1) {
      onAnchorIndexChange(anchorIndex + 1)
      onRelatedIndexChange(0)
    }
  }

  return (
    <div className="space-y-4">
      {/* Anchor Mode Toggle & Navigation */}
      <div className="flex items-center justify-between">
        {/* Left: Anchor Navigation */}
        <div className="flex items-center gap-2">
          <button
            onClick={prevAnchor}
            disabled={anchorIndex === 0}
            className={cn(
              'p-2 rounded-lg transition-all duration-200',
              'bg-secondary/50 hover:bg-secondary',
              'disabled:opacity-30 disabled:cursor-not-allowed',
            )}
          >
            <ChevronLeft className="w-5 h-5" />
          </button>
          <div className="px-4 py-2 rounded-lg bg-linear-to-r from-secondary/80 to-secondary/40 border border-border/50">
            <span className="text-sm font-medium">
              {isOfferMode ? 'Offer' : 'Request'}{' '}
              <span className="text-teal font-bold">{anchorIndex + 1}</span>
              <span className="text-muted-foreground"> of {totalAnchors}</span>
            </span>
          </div>
          <button
            onClick={nextAnchor}
            disabled={anchorIndex === totalAnchors - 1}
            className={cn(
              'p-2 rounded-lg transition-all duration-200',
              'bg-secondary/50 hover:bg-secondary',
              'disabled:opacity-30 disabled:cursor-not-allowed',
            )}
          >
            <ChevronRight className="w-5 h-5" />
          </button>
        </div>

        {/* Center: Mode Toggle */}
        <button
          onClick={() => onAnchorModeChange(isOfferMode ? 'request' : 'offer')}
          className={cn(
            'flex items-center gap-2 px-4 py-2 rounded-lg',
            'bg-linear-to-r from-violet-500/20 to-fuchsia-500/20',
            'border border-violet-500/30 hover:border-violet-500/50',
            'transition-all duration-300 hover:scale-105',
          )}
        >
          <ArrowLeftRight className="w-4 h-4 text-violet-400" />
          <span className="text-sm font-medium text-violet-300">
            Switch to {isOfferMode ? 'Request' : 'Offer'} View
          </span>
        </button>

        {/* Right: Match Count */}
        <div className="flex items-center gap-2 px-4 py-2 rounded-lg bg-secondary/30 border border-border/30">
          <Layers className="w-4 h-4 text-muted-foreground" />
          <span className="text-sm">
            <span className="text-amber font-bold">{totalMatches}</span>
            <span className="text-muted-foreground">
              {' '}
              match{totalMatches !== 1 ? 'es' : ''}
            </span>
          </span>
        </div>
      </div>

      {/* Main Carousel Content */}
      <div className="glass-card-enhanced p-8 rounded-2xl animate-scale-in">
        <div className="grid grid-cols-1 lg:grid-cols-7 gap-6 items-stretch">
          {/* Left: Fixed Card (Anchor) */}
          <div className="lg:col-span-2 relative">
            {isOfferMode ? (
              <ReviewCard
                type="offer"
                offer={(currentGroup as OfferWithMatches).offer}
                onCurate={(name, id) => {
                  setCurationMedication(name)
                  setCurationAliasId(id ?? null)
                }}
                onReclassify={handleReclassify}
              />
            ) : (
              <ReviewCard
                type="request"
                request={(currentGroup as RequestWithMatches).request}
                onCurate={(name, id) => {
                  setCurationMedication(name)
                  setCurationAliasId(id ?? null)
                }}
                onReclassify={handleReclassify}
              />
            )}
            {/* Fixed indicator badge */}
            <div className="absolute -top-2 left-1/2 -translate-x-1/2 px-3 py-1 rounded-full bg-linear-to-r from-teal/80 to-emerald/80 text-white text-xs font-medium shadow-lg">
              ⚓ Anchored
            </div>
          </div>

          {/* Center: Match Confidence + Mini Navigation */}
          <div className="lg:col-span-3 flex flex-col items-center justify-center py-6">
            <MatchConfidenceMeter confidence={currentMatch.confidence} />

            {/* Issues */}
            {issues.length > 0 && (
              <div className="w-full max-w-sm space-y-2 mt-6">
                {issues.slice(0, 3).map((issue, idx) => (
                  <div
                    key={idx}
                    className="flex items-start gap-2 p-2 rounded-lg bg-amber/10 border border-amber/20 animate-fade-in text-xs text-amber"
                    style={{ animationDelay: `${idx * 100}ms` }}
                  >
                    {issue}
                  </div>
                ))}
              </div>
            )}

            {/* Inner Carousel Navigation (Related Items) */}
            {totalMatches > 1 && (
              <div className="flex items-center gap-3 mt-6">
                <button
                  onClick={prevRelated}
                  disabled={relatedIndex === 0}
                  className={cn(
                    'p-2 rounded-full transition-all duration-200',
                    'bg-linear-to-r from-amber/20 to-orange-500/20',
                    'border border-amber/30 hover:border-amber/60',
                    'hover:scale-110 active:scale-95',
                    'disabled:opacity-30 disabled:cursor-not-allowed',
                  )}
                >
                  <ChevronLeft className="w-4 h-4 text-amber" />
                </button>

                {/* Dot indicators */}
                <div className="flex items-center gap-1.5">
                  {matches.slice(0, 7).map((_, idx) => (
                    <button
                      key={idx}
                      onClick={() => onRelatedIndexChange(idx)}
                      className={cn(
                        'w-2 h-2 rounded-full transition-all duration-300',
                        idx === relatedIndex
                          ? 'w-6 bg-linear-to-r from-amber to-orange-500'
                          : 'bg-muted-foreground/30 hover:bg-muted-foreground/50',
                      )}
                    />
                  ))}
                  {totalMatches > 7 && (
                    <span className="text-xs text-muted-foreground ml-1">
                      +{totalMatches - 7}
                    </span>
                  )}
                </div>

                <button
                  onClick={nextRelated}
                  disabled={relatedIndex === totalMatches - 1}
                  className={cn(
                    'p-2 rounded-full transition-all duration-200',
                    'bg-linear-to-r from-amber/20 to-orange-500/20',
                    'border border-amber/30 hover:border-amber/60',
                    'hover:scale-110 active:scale-95',
                    'disabled:opacity-30 disabled:cursor-not-allowed',
                  )}
                >
                  <ChevronRight className="w-4 h-4 text-amber" />
                </button>
              </div>
            )}
          </div>

          {/* Right: Dynamic Card (Carousel) */}
          <div className="lg:col-span-2 relative">
            {isOfferMode ? (
              <ReviewCard
                type="request"
                request={(currentMatch as { request: any }).request}
                onCurate={(name, id) => {
                  setCurationMedication(name)
                  setCurationAliasId(id ?? null)
                }}
                onReclassify={handleReclassify}
                aiStatus={
                  currentMatch.aiStatus as
                    | 'Approved'
                    | 'Flagged'
                    | 'Rejected'
                    | null
                    | undefined
                }
                aiConfidence={currentMatch.aiConfidence}
                aiExplanation={currentMatch.aiExplanation}
              />
            ) : (
              <ReviewCard
                type="offer"
                offer={(currentMatch as { offer: any }).offer}
                onCurate={(name, id) => {
                  setCurationMedication(name)
                  setCurationAliasId(id ?? null)
                }}
                onReclassify={handleReclassify}
                aiStatus={
                  currentMatch.aiStatus as
                    | 'Approved'
                    | 'Flagged'
                    | 'Rejected'
                    | null
                    | undefined
                }
                aiConfidence={currentMatch.aiConfidence}
                aiExplanation={currentMatch.aiExplanation}
              />
            )}
            {/* Carousel indicator badge */}
            <div className="absolute -top-2 left-1/2 -translate-x-1/2 px-3 py-1 rounded-full bg-linear-to-r from-amber/80 to-orange-500/80 text-white text-xs font-medium shadow-lg">
              🔄 {relatedIndex + 1}/{totalMatches}
            </div>
          </div>
        </div>

        {/* Action Buttons */}
        <div className="flex items-center justify-center gap-4 mt-8">
          <button
            onClick={() => handleAction('rejected')}
            className={cn(
              'flex items-center gap-2 px-8 py-3 rounded-xl font-medium',
              'bg-linear-to-r from-red-500/20 to-rose-500/20',
              'border border-red-500/30 hover:border-red-500/60',
              'text-red-400 hover:text-red-300',
              'transition-all duration-300 hover:scale-105 active:scale-95',
              'shadow-lg hover:shadow-red-500/20',
            )}
          >
            ✗ Reject
          </button>
          <button
            onClick={() => handleAction('approved')}
            className={cn(
              'flex items-center gap-2 px-8 py-3 rounded-xl font-medium',
              'bg-linear-to-r from-emerald/20 to-teal/20',
              'border border-emerald/30 hover:border-emerald/60',
              'text-emerald hover:text-emerald/90',
              'transition-all duration-300 hover:scale-105 active:scale-95',
              'shadow-lg hover:shadow-emerald/20',
            )}
          >
            ✓ Approve
          </button>
        </div>
      </div>

      <CurationDialog
        isOpen={!!curationMedication}
        onClose={() => {
          setCurationMedication(null)
          setCurationAliasId(null)
        }}
        medicationRaw={curationMedication || ''}
        aliasId={curationAliasId}
        onSuccess={() => {
          // Success handled via toast and optimistic UI if we had it
          // For now, it stays matched but shows as "Verified" if refreshed
        }}
      />

      <ReclassifyDialog
        open={!!reclassifyItem}
        onOpenChange={(open) => {
          if (!open) setReclassifyItem(null)
        }}
        itemId={reclassifyItem?.id ?? ''}
        itemType={reclassifyItem?.type ?? 'offer'}
        medication={reclassifyItem?.medication ?? ''}
        medicationRaw={reclassifyItem?.medicationRaw}
        onSuccess={() => {
          setReclassifyItem(null)
          // The dialog will invalidate queries automatically
        }}
      />
    </div>
  )
}
