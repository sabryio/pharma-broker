// Related Match Carousel Component
// Beautiful carousel for navigating through related matches

import { useCallback, useState } from 'react'
import { useQueryClient, useMutation } from '@tanstack/react-query'
import { toast } from 'sonner'
import { cn } from '@/lib/utils'
import { ChevronLeft, ChevronRight, Layers, ArrowLeftRight, Sparkles, Calculator, GitCompare, Activity } from 'lucide-react'
import type { OfferWithMatches, RequestWithMatches } from './types'
import { ReviewCard } from './review-card'
import { MatchConfidenceMeter } from './match-confidence-meter'
import { MatchComparison } from './match-comparison'
import { ReasoningPanel } from './reasoning-panel'
import { CurationDialog } from './curation-dialog'
import { SenderProfile } from './sender-profile'
import { NotesPanel } from './notes-panel'
import { ReclassifyDialog } from '@/components/ui/reclassify-dialog'
import { ReparseDialog } from '@/components/ui/reparse-dialog'
import { UncertaintyIndicator } from '@/components/debug-recordings'
import type { ItemType } from '@/api/offers'
import { rematchItem } from '@/api/matching'
import { reAuditMatch, recalculateConfidence } from '@/api/match-reviews'

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
  const queryClient = useQueryClient()
  const [curationMedication, setCurationMedication] = useState<string | null>(
    null,
  )
  const [curationAliasId, setCurationAliasId] = useState<string | null>(null)
  const [showComparison, setShowComparison] = useState(false)

  // Reclassify dialog state
  const [reclassifyItem, setReclassifyItem] = useState<{
    id: string
    type: ItemType
    medication: string
    medicationRaw?: string
  } | null>(null)

  // Reparse dialog state
  const [reparseItem, setReparseItem] = useState<{
    id: string
    type: ItemType
    medication: string
    medicationRaw?: string
  } | null>(null)

  const handleReclassify = useCallback(
    (
      id: string,
      type: 'offer' | 'request',
      medication: string,
      medicationRaw?: string,
    ) => {
      setReclassifyItem({ id, type, medication, medicationRaw })
    },
    [],
  )

  const handleReparse = useCallback(
    (
      id: string,
      type: 'offer' | 'request',
      medication: string,
      medicationRaw?: string,
    ) => {
      setReparseItem({ id, type, medication, medicationRaw })
    },
    [],
  )

  const isOfferMode = anchorMode === 'offer'
  const groups = isOfferMode ? groupedByOffer : groupedByRequest
  const currentGroup = groups[anchorIndex]
  
  // Get current match early so it can be used in callbacks
  const matches = currentGroup?.matches ?? []
  const currentMatch = matches[relatedIndex]

  const rematchMutation = useMutation({
    mutationFn: rematchItem,
    onSuccess: (data) => {
      toast.success('Rematch triggered', {
        description: `Cleared ${data.matches_cleared} old matches, queued ${data.items_queued} items for re-matching.`,
      })
      // Reset related index to show first new match
      onRelatedIndexChange(0)
      // Invalidate queries to refresh the match reviews
      queryClient.invalidateQueries({ queryKey: ['match-reviews'] })
      queryClient.invalidateQueries({ queryKey: ['match-review-stats'] })
    },
    onError: (error) => {
      toast.error('Failed to trigger rematch', {
        description: error instanceof Error ? error.message : 'Unknown error',
      })
    },
  })

  const reAuditMutation = useMutation({
    mutationFn: reAuditMatch,
    onSuccess: (data) => {
      toast.success('AI Re-audit completed', {
        description: `Status: ${data.aiStatus} (${data.aiConfidence ? Math.round(data.aiConfidence * 100) : 0}% confidence)`,
      })
      // Invalidate queries to refresh the match reviews
      queryClient.invalidateQueries({ queryKey: ['match-reviews'] })
    },
    onError: (error) => {
      toast.error('Failed to re-audit match', {
        description: error instanceof Error ? error.message : 'Unknown error',
      })
    },
  })

  const recalculateMutation = useMutation({
    mutationFn: recalculateConfidence,
    onSuccess: (data) => {
      const change = data.newScore - data.oldScore
      const changeStr = change >= 0 ? `+${(change * 100).toFixed(1)}%` : `${(change * 100).toFixed(1)}%`
      toast.success('Confidence recalculated', {
        description: `${(data.oldScore * 100).toFixed(1)}% → ${(data.newScore * 100).toFixed(1)}% (${changeStr})`,
      })
      // Invalidate queries to refresh the match reviews
      queryClient.invalidateQueries({ queryKey: ['match-reviews'] })
    },
    onError: (error) => {
      toast.error('Failed to recalculate confidence', {
        description: error instanceof Error ? error.message : 'Unknown error',
      })
    },
  })

  const handleReAudit = useCallback(() => {
    if (!currentMatch) return
    
    toast.info('Running AI audit...', {
      description: 'Analyzing match with AI reviewer',
    })
    
    reAuditMutation.mutate(currentMatch.matchId)
  }, [currentMatch, reAuditMutation])

  const handleRecalculate = useCallback(() => {
    if (!currentMatch) return
    
    toast.info('Recalculating confidence...', {
      description: 'Using raw text validation',
    })
    
    recalculateMutation.mutate(currentMatch.matchId)
  }, [currentMatch, recalculateMutation])

  const handleRematch = useCallback(() => {
    if (!currentGroup) return

    const itemId = isOfferMode
      ? (currentGroup as OfferWithMatches).offer.id
      : (currentGroup as RequestWithMatches).request.id

    const itemName = isOfferMode
      ? (currentGroup as OfferWithMatches).offer.product
      : (currentGroup as RequestWithMatches).request.product

    toast.info(`Rematching ${isOfferMode ? 'offer' : 'request'}`, {
      description: `Finding new matches for "${itemName}"...`,
    })

    rematchMutation.mutate({
      item_id: itemId,
      item_type: anchorMode as ItemType,
    })
  }, [currentGroup, isOfferMode, anchorMode, rematchMutation])

  if (!currentGroup) return null
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
        <div className="flex items-center gap-2">
          {/* Comparison Toggle */}
          <button
            onClick={() => setShowComparison(!showComparison)}
            className={cn(
              'flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium',
              'transition-all duration-200',
              showComparison
                ? 'bg-violet-500/20 text-violet-400 border border-violet-500/30'
                : 'bg-secondary/50 text-muted-foreground border border-border/50 hover:border-border',
            )}
          >
            <GitCompare className="w-3.5 h-3.5" />
            Compare
          </button>
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
                onReparse={handleReparse}
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
                onReparse={handleReparse}
              />
            )}
            {/* Fixed indicator badge */}
            <div className={cn(
              "absolute -top-2 left-1/2 -translate-x-1/2 px-3 py-1 rounded-full text-white text-xs font-medium shadow-lg transition-all duration-300",
              rematchMutation.isPending
                ? "bg-linear-to-r from-amber/80 to-orange-500/80 animate-pulse"
                : "bg-linear-to-r from-teal/80 to-emerald/80"
            )}>
              {rematchMutation.isPending ? '🔄 Rematching...' : '⚓ Anchored'}
            </div>
          </div>

          {/* Center: Match Confidence + Mini Navigation */}
          <div className="lg:col-span-3 flex flex-col items-center justify-center py-6">
            <MatchConfidenceMeter
              confidence={currentMatch.confidence}
              onClick={handleRematch}
              isPending={rematchMutation.isPending}
            />

            {/* Uncertainty Indicator */}
            <div className="mt-3 flex items-center gap-2 px-3 py-1.5 rounded-full bg-secondary/30 border border-border/30">
              <Activity className="w-3.5 h-3.5 text-cyan-400" />
              <span className="text-xs text-muted-foreground">Uncertainty:</span>
              <UncertaintyIndicator matchId={currentMatch.matchId} />
            </div>

            {/* AI Tools Panel */}
            <div className="mt-6 w-full max-w-xs">
              <div className="relative p-4 rounded-2xl bg-linear-to-br from-slate-900/80 via-slate-800/60 to-slate-900/80 border border-slate-700/50 backdrop-blur-xl shadow-2xl">
                {/* Decorative glow */}
                <div className="absolute inset-0 rounded-2xl bg-linear-to-br from-violet-500/10 via-transparent to-cyan-500/10 pointer-events-none" />
                
                {/* Header */}
                <div className="flex items-center gap-2 mb-4 pb-3 border-b border-slate-700/50">
                  <div className="w-8 h-8 rounded-lg bg-linear-to-br from-violet-500/30 to-purple-500/30 flex items-center justify-center">
                    <Sparkles className="w-4 h-4 text-violet-400" />
                  </div>
                  <div>
                    <h4 className="text-sm font-semibold text-white">AI Tools</h4>
                    <p className="text-[10px] text-slate-400">Analyze & recalculate</p>
                  </div>
                </div>

                {/* Buttons Grid */}
                <div className="grid grid-cols-2 gap-3">
                  {/* Re-audit Button */}
                  <button
                    onClick={handleReAudit}
                    disabled={reAuditMutation.isPending}
                    className={cn(
                      'group relative flex flex-col items-center gap-2 p-3 rounded-xl',
                      'bg-linear-to-br from-violet-500/20 via-purple-500/10 to-violet-500/20',
                      'border border-violet-500/30 hover:border-violet-400/60',
                      'transition-all duration-300 hover:scale-[1.02] active:scale-[0.98]',
                      'disabled:opacity-50 disabled:cursor-not-allowed',
                      'overflow-hidden',
                    )}
                  >
                    {/* Shimmer effect */}
                    <div className="absolute inset-0 bg-linear-to-r from-transparent via-white/5 to-transparent -translate-x-full group-hover:translate-x-full transition-transform duration-700" />
                    
                    <div className={cn(
                      'w-10 h-10 rounded-xl bg-linear-to-br from-violet-500/40 to-purple-600/40 flex items-center justify-center',
                      'shadow-lg shadow-violet-500/20 group-hover:shadow-violet-500/40 transition-shadow',
                    )}>
                      <Sparkles className={cn(
                        'w-5 h-5 text-violet-300',
                        reAuditMutation.isPending && 'animate-spin'
                      )} />
                    </div>
                    <div className="text-center">
                      <span className="text-xs font-medium text-violet-300 group-hover:text-violet-200">
                        {reAuditMutation.isPending ? 'Auditing...' : 'AI Audit'}
                      </span>
                      <p className="text-[9px] text-slate-500 mt-0.5">Expert review</p>
                    </div>
                  </button>

                  {/* Recalculate Button */}
                  <button
                    onClick={handleRecalculate}
                    disabled={recalculateMutation.isPending}
                    className={cn(
                      'group relative flex flex-col items-center gap-2 p-3 rounded-xl',
                      'bg-linear-to-br from-cyan-500/20 via-blue-500/10 to-cyan-500/20',
                      'border border-cyan-500/30 hover:border-cyan-400/60',
                      'transition-all duration-300 hover:scale-[1.02] active:scale-[0.98]',
                      'disabled:opacity-50 disabled:cursor-not-allowed',
                      'overflow-hidden',
                    )}
                  >
                    {/* Shimmer effect */}
                    <div className="absolute inset-0 bg-linear-to-r from-transparent via-white/5 to-transparent -translate-x-full group-hover:translate-x-full transition-transform duration-700" />
                    
                    <div className={cn(
                      'w-10 h-10 rounded-xl bg-linear-to-br from-cyan-500/40 to-blue-600/40 flex items-center justify-center',
                      'shadow-lg shadow-cyan-500/20 group-hover:shadow-cyan-500/40 transition-shadow',
                    )}>
                      <Calculator className={cn(
                        'w-5 h-5 text-cyan-300',
                        recalculateMutation.isPending && 'animate-spin'
                      )} />
                    </div>
                    <div className="text-center">
                      <span className="text-xs font-medium text-cyan-300 group-hover:text-cyan-200">
                        {recalculateMutation.isPending ? 'Calculating...' : 'Recalculate'}
                      </span>
                      <p className="text-[9px] text-slate-500 mt-0.5">Raw text check</p>
                    </div>
                  </button>
                </div>

                {/* Info tooltip */}
                <div className="mt-3 pt-3 border-t border-slate-700/50">
                  <p className="text-[10px] text-slate-500 text-center leading-relaxed">
                    💡 <span className="text-slate-400">AI Audit</span> uses LLM reasoning • <span className="text-slate-400">Recalculate</span> validates raw Arabic text
                  </p>
                </div>
              </div>
            </div>

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
          <div className={cn(
            "lg:col-span-2 relative transition-all duration-300",
            rematchMutation.isPending && "opacity-50 blur-sm"
          )}>
            {isOfferMode ? (
              <ReviewCard
                type="request"
                request={(currentMatch as { request: any }).request}
                onCurate={(name, id) => {
                  setCurationMedication(name)
                  setCurationAliasId(id ?? null)
                }}
                onReclassify={handleReclassify}
                onReparse={handleReparse}
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
                onReparse={handleReparse}
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
            <div className={cn(
              "absolute -top-2 left-1/2 -translate-x-1/2 px-3 py-1 rounded-full text-white text-xs font-medium shadow-lg transition-all duration-300",
              rematchMutation.isPending
                ? "bg-linear-to-r from-gray-500/80 to-gray-600/80"
                : "bg-linear-to-r from-amber/80 to-orange-500/80"
            )}>
              {rematchMutation.isPending ? '⏳ Loading...' : `🔄 ${relatedIndex + 1}/${totalMatches}`}
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

        {/* Comparison Panel */}
        {showComparison && (
          <div className="mt-6 animate-fade-in">
            <MatchComparison
              offer={
                isOfferMode
                  ? (currentGroup as OfferWithMatches).offer
                  : (currentMatch as { offer: any }).offer
              }
              request={
                isOfferMode
                  ? (currentMatch as { request: any }).request
                  : (currentGroup as RequestWithMatches).request
              }
              className="p-4 rounded-xl bg-secondary/20 border border-border/30"
            />
          </div>
        )}

        {/* Reasoning Panel */}
        <div className="mt-6">
          <ReasoningPanel
            confidence={currentMatch.confidence}
            reasoning={null}
            aiStatus={currentMatch.aiStatus}
            aiConfidence={currentMatch.aiConfidence}
            aiExplanation={currentMatch.aiExplanation}
            issues={issues}
          />
        </div>

        {/* Sender Profiles & Notes */}
        <div className="mt-6 grid grid-cols-1 lg:grid-cols-2 gap-4">
          {/* Offer Sender Profile */}
          <div className="p-4 rounded-xl bg-teal/5 border border-teal/20">
            <h4 className="text-xs font-medium text-teal mb-3 uppercase tracking-wider">Offer Sender</h4>
            <SenderProfile
              senderName={
                isOfferMode
                  ? (currentGroup as OfferWithMatches).offer.senderName
                  : (currentMatch as { offer: any }).offer.senderName
              }
              senderJid={
                isOfferMode
                  ? (currentGroup as OfferWithMatches).offer.senderJid
                  : (currentMatch as { offer: any }).offer.senderJid
              }
              showStats={true}
            />
          </div>

          {/* Request Sender Profile */}
          <div className="p-4 rounded-xl bg-amber/5 border border-amber/20">
            <h4 className="text-xs font-medium text-amber mb-3 uppercase tracking-wider">Request Sender</h4>
            <SenderProfile
              senderName={
                isOfferMode
                  ? (currentMatch as { request: any }).request.senderName
                  : (currentGroup as RequestWithMatches).request.senderName
              }
              senderJid={
                isOfferMode
                  ? (currentMatch as { request: any }).request.senderJid
                  : (currentGroup as RequestWithMatches).request.senderJid
              }
              showStats={true}
            />
          </div>
        </div>

        {/* Notes Panel */}
        <div className="mt-4">
          <NotesPanel
            matchId={currentMatch.matchId}
            initialNotes={currentMatch.notes}
          />
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
          // Invalidate match-reviews query to refresh the UI with updated curation status
          queryClient.invalidateQueries({ queryKey: ['match-reviews'] })
          setCurationMedication(null)
          setCurationAliasId(null)
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

      <ReparseDialog
        open={!!reparseItem}
        onOpenChange={(open) => {
          if (!open) setReparseItem(null)
        }}
        itemId={reparseItem?.id ?? ''}
        itemType={reparseItem?.type ?? 'offer'}
        medication={reparseItem?.medication ?? ''}
        medicationRaw={reparseItem?.medicationRaw}
        onSuccess={() => {
          setReparseItem(null)
          // The dialog will invalidate queries automatically
        }}
      />
    </div>
  )
}
