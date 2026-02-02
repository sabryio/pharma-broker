// Match Card Component
// Displays a single match with offer and request details

import {
  ChevronDown,
  ChevronUp,
  CheckCircle,
  XCircle,
  Bot,
  User,
} from 'lucide-react'
import type { MatchReviewItem } from '@/schema/match-review'
import { cn } from '@/lib/utils'

interface MatchCardProps {
  match: MatchReviewItem
  isSelected: boolean
  isExpanded: boolean
  onSelect: () => void
  onExpand: () => void
  onApprove?: () => void
  onReject?: () => void
  isApproving?: boolean
  isRejecting?: boolean
}

/**
 * Get the color class for a confidence score
 * - emerald for high confidence (≥80%)
 * - amber for medium confidence (50-79%)
 * - red for low confidence (<50%)
 */
export function getConfidenceColor(confidence: number): string {
  if (confidence >= 80) {
    return 'text-emerald bg-emerald/10 border-emerald/30'
  }
  if (confidence >= 50) {
    return 'text-amber bg-amber/10 border-amber/30'
  }
  return 'text-red-400 bg-red-400/10 border-red-400/30'
}

/**
 * Get the color class for a match status
 */
export function getStatusColor(status: MatchReviewItem['status']): string {
  const statusColors = {
    PENDING: 'text-amber bg-amber/10 border-amber/30',
    CONFIRMED: 'text-emerald bg-emerald/10 border-emerald/30',
    REJECTED: 'text-red-400 bg-red-400/10 border-red-400/30',
    EXPIRED: 'text-muted-foreground bg-muted/10 border-muted/30',
  }
  return statusColors[status]
}

export function MatchCard({
  match,
  isSelected,
  isExpanded,
  onSelect,
  onExpand,
  onApprove,
  onReject,
  isApproving = false,
  isRejecting = false,
}: MatchCardProps) {
  const confidenceColor = getConfidenceColor(match.confidence)
  const statusColor = getStatusColor(match.status)
  const isPending = match.status === 'PENDING'

  return (
    <div
      className={cn(
        'glass-card rounded-xl border transition-all cursor-pointer',
        isSelected
          ? 'border-teal/50 ring-2 ring-teal/20'
          : 'border-border hover:border-teal/30',
      )}
      onClick={onSelect}
    >
      <div className="p-4">
        <div className="flex items-center justify-between gap-4">
          {/* Offer & Request Info */}
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 mb-1">
              <span className="text-sm font-medium text-foreground truncate">
                {match.offer.product}
              </span>
              <span className="text-muted-foreground">→</span>
              <span className="text-sm font-medium text-foreground truncate">
                {match.request.product}
              </span>
            </div>
            <p className="text-xs text-muted-foreground">
              Created {new Date(match.createdAt).toLocaleDateString()}
            </p>
          </div>

          {/* Confidence Score */}
          <div
            className={cn(
              'px-3 py-1 rounded-lg border text-sm font-medium',
              confidenceColor,
            )}
          >
            {Math.round(match.confidence)}%
          </div>

          {/* Status Badge */}
          <div
            className={cn(
              'px-3 py-1 rounded-lg border text-xs font-medium',
              statusColor,
            )}
          >
            {match.status}
          </div>

          {/* AI/Human Approved Indicator - Requirements: 4.5 */}
          {match.status === 'CONFIRMED' && (
            <div
              className={cn(
                'flex items-center gap-1.5 px-2.5 py-1 rounded-lg border text-xs font-medium',
                match.aiAutoApproved
                  ? 'bg-violet-400/10 text-violet-400 border-violet-400/30'
                  : 'bg-teal/10 text-teal border-teal/30',
              )}
              title={
                match.aiAutoApproved
                  ? `AI auto-approved${match.aiApprovedAt ? ` at ${new Date(match.aiApprovedAt).toLocaleString()}` : ''}`
                  : 'Human approved'
              }
            >
              {match.aiAutoApproved ? (
                <>
                  <Bot className="w-3.5 h-3.5" />
                  AI
                </>
              ) : (
                <>
                  <User className="w-3.5 h-3.5" />
                  Human
                </>
              )}
            </div>
          )}

          {/* Expand Button */}
          <button
            onClick={(e) => {
              e.stopPropagation()
              onExpand()
            }}
            className="p-2 rounded-lg hover:bg-secondary/50 transition-colors"
            aria-label={isExpanded ? 'Collapse details' : 'Expand details'}
          >
            {isExpanded ? (
              <ChevronUp className="w-5 h-5 text-teal" />
            ) : (
              <ChevronDown className="w-5 h-5 text-muted-foreground" />
            )}
          </button>
        </div>

        {/* Expanded Details */}
        {isExpanded && (
          <div className="mt-4 pt-4 border-t border-border space-y-4 animate-fade-in">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              {/* Offer Details */}
              <div className="space-y-2">
                <h4 className="text-sm font-medium text-teal">Offer Details</h4>
                <div className="text-xs space-y-1">
                  <p>
                    <span className="text-muted-foreground">Product:</span>{' '}
                    {match.offer.product}
                  </p>
                  <p>
                    <span className="text-muted-foreground">Price:</span>{' '}
                    {match.offer.price ?? 'N/A'}
                  </p>
                  <p>
                    <span className="text-muted-foreground">Quantity:</span>{' '}
                    {match.offer.quantity ?? 'N/A'}
                  </p>
                  <p>
                    <span className="text-muted-foreground">Expiry:</span>{' '}
                    {match.offer.expiry ?? 'N/A'}
                  </p>
                  <p>
                    <span className="text-muted-foreground">Sender:</span>{' '}
                    {match.offer.senderName ?? 'Unknown'}
                  </p>
                  <p>
                    <span className="text-muted-foreground">Group:</span>{' '}
                    {match.offer.sourceGroup ?? 'N/A'}
                  </p>
                  {match.offer.rawMessage && (
                    <p className="mt-2 p-2 bg-secondary/30 rounded text-muted-foreground">
                      {match.offer.rawMessage}
                    </p>
                  )}
                </div>
              </div>

              {/* Request Details */}
              <div className="space-y-2">
                <h4 className="text-sm font-medium text-amber">
                  Request Details
                </h4>
                <div className="text-xs space-y-1">
                  <p>
                    <span className="text-muted-foreground">Product:</span>{' '}
                    {match.request.product}
                  </p>
                  <p>
                    <span className="text-muted-foreground">Max Price:</span>{' '}
                    {match.request.maxPrice ?? 'N/A'}
                  </p>
                  <p>
                    <span className="text-muted-foreground">Quantity:</span>{' '}
                    {match.request.quantity ?? 'N/A'}
                  </p>
                  <p>
                    <span className="text-muted-foreground">Urgency:</span>{' '}
                    {match.request.urgency}
                  </p>
                  <p>
                    <span className="text-muted-foreground">Sender:</span>{' '}
                    {match.request.senderName ?? 'Unknown'}
                  </p>
                  <p>
                    <span className="text-muted-foreground">Group:</span>{' '}
                    {match.request.sourceGroup ?? 'N/A'}
                  </p>
                  {match.request.rawMessage && (
                    <p className="mt-2 p-2 bg-secondary/30 rounded text-muted-foreground">
                      {match.request.rawMessage}
                    </p>
                  )}
                </div>
              </div>
            </div>

            {/* AI Reasoning */}
            {(match.reasoning || match.issues.length > 0) && (
              <div className="space-y-2">
                <h4 className="text-sm font-medium text-foreground">
                  AI Analysis
                </h4>
                {match.reasoning && (
                  <div className="p-3 rounded-lg bg-violet-400/10 border border-violet-400/20">
                    <div className="flex items-center gap-2 mb-1">
                      <Bot className="w-4 h-4 text-violet-400" />
                      <span className="text-xs font-medium text-violet-400">
                        AI Reasoning
                        {match.aiConfidence !== null &&
                          match.aiConfidence !== undefined && (
                            <span className="ml-2 text-muted-foreground">
                              ({(match.aiConfidence * 100).toFixed(0)}%
                              confidence)
                            </span>
                          )}
                      </span>
                    </div>
                    <p className="text-xs text-muted-foreground">
                      {match.reasoning}
                    </p>
                  </div>
                )}
                {match.issues.length > 0 && (
                  <div className="flex flex-wrap gap-2">
                    {match.issues.map((issue, idx) => (
                      <span
                        key={idx}
                        className="px-2 py-1 text-xs rounded bg-red-400/10 text-red-400 border border-red-400/30"
                      >
                        {issue}
                      </span>
                    ))}
                  </div>
                )}
              </div>
            )}

            {/* Curation Status */}
            <div className="flex items-center gap-4 pt-2">
              {match.offer.curationStatus && (
                <div className="text-xs">
                  <span className="text-muted-foreground">Offer Curation:</span>{' '}
                  <span className="text-foreground">
                    {match.offer.curationStatus}
                  </span>
                </div>
              )}
              {match.request.curationStatus && (
                <div className="text-xs">
                  <span className="text-muted-foreground">
                    Request Curation:
                  </span>{' '}
                  <span className="text-foreground">
                    {match.request.curationStatus}
                  </span>
                </div>
              )}
            </div>

            {/* Action Buttons */}
            {isPending && onApprove && onReject && (
              <div className="flex items-center justify-end gap-2 pt-3 border-t border-border">
                <button
                  onClick={(e) => {
                    e.stopPropagation()
                    onReject()
                  }}
                  disabled={isRejecting || isApproving}
                  className={cn(
                    'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors',
                    'bg-red-400/10 text-red-400 border border-red-400/30',
                    'hover:bg-red-400/20',
                    'disabled:opacity-50 disabled:cursor-not-allowed',
                  )}
                >
                  <XCircle className="w-3.5 h-3.5" />
                  {isRejecting ? 'Rejecting...' : 'Reject'}
                </button>
                <button
                  onClick={(e) => {
                    e.stopPropagation()
                    onApprove()
                  }}
                  disabled={isApproving || isRejecting}
                  className={cn(
                    'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors',
                    'bg-emerald/10 text-emerald border border-emerald/30',
                    'hover:bg-emerald/20',
                    'disabled:opacity-50 disabled:cursor-not-allowed',
                  )}
                >
                  <CheckCircle className="w-3.5 h-3.5" />
                  {isApproving ? 'Approving...' : 'Approve'}
                </button>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  )
}
