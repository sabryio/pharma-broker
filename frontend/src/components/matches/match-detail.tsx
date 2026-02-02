// Match Detail Component
// Displays full match details with offer and request information

import {
  AlertTriangle,
  Brain,
  Calendar,
  CheckCircle,
  Clock,
  DollarSign,
  FileText,
  Package,
  User,
  Users,
  X,
  XCircle,
} from 'lucide-react'
import { getConfidenceColor, getStatusColor } from './match-card'
import type { MatchReviewItem } from '@/schema/match-review'
import { cn } from '@/lib/utils'

interface MatchDetailProps {
  match: MatchReviewItem
  onClose: () => void
  onApprove: () => void
  onReject: () => void
  isApproving?: boolean
  isRejecting?: boolean
}

export function MatchDetail({
  match,
  onClose,
  onApprove,
  onReject,
  isApproving = false,
  isRejecting = false,
}: MatchDetailProps) {
  const confidenceColor = getConfidenceColor(match.confidence)
  const statusColor = getStatusColor(match.status)
  const isPending = match.status === 'PENDING'

  return (
    <div className="glass-card-enhanced rounded-2xl border border-border overflow-hidden">
      {/* Header */}
      <div className="p-4 border-b border-border bg-secondary/30">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-4">
            <h3 className="text-lg font-semibold text-foreground">
              Match Details
            </h3>
            <div
              className={cn(
                'px-3 py-1 rounded-lg border text-sm font-medium',
                confidenceColor,
              )}
            >
              {Math.round(match.confidence)}% Confidence
            </div>
            <div
              className={cn(
                'px-3 py-1 rounded-lg border text-xs font-medium',
                statusColor,
              )}
            >
              {match.status}
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-2 rounded-lg hover:bg-secondary/50 transition-colors"
            aria-label="Close details"
          >
            <X className="w-5 h-5 text-muted-foreground" />
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="p-6 space-y-6">
        {/* Offer and Request Details */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {/* Offer Details */}
          <div className="space-y-4">
            <div className="flex items-center gap-2">
              <div className="w-8 h-8 rounded-lg bg-teal/20 flex items-center justify-center">
                <Package className="w-4 h-4 text-teal" />
              </div>
              <h4 className="text-sm font-semibold text-teal">Offer Details</h4>
            </div>

            <div className="space-y-3 pl-10">
              <DetailRow
                icon={Package}
                label="Product"
                value={match.offer.product}
                highlight
              />
              <DetailRow
                icon={DollarSign}
                label="Price"
                value={match.offer.price ?? 'Not specified'}
              />
              <DetailRow
                icon={FileText}
                label="Quantity"
                value={match.offer.quantity ?? 'Not specified'}
              />
              <DetailRow
                icon={Calendar}
                label="Expiry"
                value={match.offer.expiry ?? 'Not specified'}
              />
              <DetailRow
                icon={User}
                label="Sender"
                value={match.offer.senderName ?? 'Unknown'}
              />
              <DetailRow
                icon={Users}
                label="Group"
                value={match.offer.sourceGroup ?? 'N/A'}
              />

              {/* Raw Message */}
              {match.offer.rawMessage && (
                <div className="mt-3">
                  <p className="text-xs text-muted-foreground mb-1">
                    Raw Message:
                  </p>
                  <div className="p-3 bg-secondary/30 rounded-lg text-xs text-muted-foreground border border-border/50">
                    {match.offer.rawMessage}
                  </div>
                </div>
              )}

              {/* Curation Status */}
              {match.offer.curationStatus && (
                <div className="mt-2 flex items-center gap-2">
                  <span className="text-xs text-muted-foreground">
                    Curation:
                  </span>
                  <span
                    className={cn(
                      'px-2 py-0.5 rounded text-xs',
                      match.offer.curationStatus === 'CURATED'
                        ? 'bg-emerald/10 text-emerald border border-emerald/30'
                        : 'bg-amber/10 text-amber border border-amber/30',
                    )}
                  >
                    {match.offer.curationStatus}
                  </span>
                </div>
              )}
            </div>
          </div>

          {/* Request Details */}
          <div className="space-y-4">
            <div className="flex items-center gap-2">
              <div className="w-8 h-8 rounded-lg bg-amber/20 flex items-center justify-center">
                <FileText className="w-4 h-4 text-amber" />
              </div>
              <h4 className="text-sm font-semibold text-amber">
                Request Details
              </h4>
            </div>

            <div className="space-y-3 pl-10">
              <DetailRow
                icon={Package}
                label="Product"
                value={match.request.product}
                highlight
              />
              <DetailRow
                icon={DollarSign}
                label="Max Price"
                value={match.request.maxPrice ?? 'Not specified'}
              />
              <DetailRow
                icon={FileText}
                label="Quantity"
                value={match.request.quantity ?? 'Not specified'}
              />
              <DetailRow
                icon={Clock}
                label="Urgency"
                value={match.request.urgency}
                urgency={match.request.urgency}
              />
              <DetailRow
                icon={User}
                label="Sender"
                value={match.request.senderName ?? 'Unknown'}
              />
              <DetailRow
                icon={Users}
                label="Group"
                value={match.request.sourceGroup ?? 'N/A'}
              />

              {/* Raw Message */}
              {match.request.rawMessage && (
                <div className="mt-3">
                  <p className="text-xs text-muted-foreground mb-1">
                    Raw Message:
                  </p>
                  <div className="p-3 bg-secondary/30 rounded-lg text-xs text-muted-foreground border border-border/50">
                    {match.request.rawMessage}
                  </div>
                </div>
              )}

              {/* Curation Status */}
              {match.request.curationStatus && (
                <div className="mt-2 flex items-center gap-2">
                  <span className="text-xs text-muted-foreground">
                    Curation:
                  </span>
                  <span
                    className={cn(
                      'px-2 py-0.5 rounded text-xs',
                      match.request.curationStatus === 'CURATED'
                        ? 'bg-emerald/10 text-emerald border border-emerald/30'
                        : 'bg-amber/10 text-amber border border-amber/30',
                    )}
                  >
                    {match.request.curationStatus}
                  </span>
                </div>
              )}
            </div>
          </div>
        </div>

        {/* AI Analysis Section */}
        {(match.reasoning || match.issues.length > 0) && (
          <div className="space-y-4 pt-4 border-t border-border">
            <div className="flex items-center gap-2">
              <div className="w-8 h-8 rounded-lg bg-violet-500/20 flex items-center justify-center">
                <Brain className="w-4 h-4 text-violet-400" />
              </div>
              <h4 className="text-sm font-semibold text-violet-400">
                AI Analysis
              </h4>
            </div>

            <div className="pl-10 space-y-3">
              {/* AI Reasoning */}
              {match.reasoning && (
                <div>
                  <p className="text-xs text-muted-foreground mb-1">
                    Reasoning:
                  </p>
                  <p className="text-sm text-foreground">{match.reasoning}</p>
                </div>
              )}

              {/* AI Confidence */}
              {match.aiConfidence !== null &&
                match.aiConfidence !== undefined && (
                  <div className="flex items-center gap-2">
                    <span className="text-xs text-muted-foreground">
                      AI Confidence:
                    </span>
                    <span className="text-sm font-medium text-foreground">
                      {Math.round(match.aiConfidence)}%
                    </span>
                  </div>
                )}

              {/* Issues */}
              {match.issues.length > 0 && (
                <div>
                  <p className="text-xs text-muted-foreground mb-2">
                    Identified Issues:
                  </p>
                  <div className="flex flex-wrap gap-2">
                    {match.issues.map((issue, idx) => (
                      <span
                        key={idx}
                        className="flex items-center gap-1 px-2 py-1 text-xs rounded bg-red-400/10 text-red-400 border border-red-400/30"
                      >
                        <AlertTriangle className="w-3 h-3" />
                        {issue}
                      </span>
                    ))}
                  </div>
                </div>
              )}
            </div>
          </div>
        )}

        {/* Metadata */}
        <div className="flex items-center gap-6 pt-4 border-t border-border text-xs text-muted-foreground">
          <div>
            <span>Created: </span>
            <span className="text-foreground">
              {new Date(match.createdAt).toLocaleString()}
            </span>
          </div>
          {match.confirmedAt && (
            <div>
              <span>Confirmed: </span>
              <span className="text-foreground">
                {new Date(match.confirmedAt).toLocaleString()}
              </span>
            </div>
          )}
          {match.reasoning && (
            <div>
              <span>Reasoning: </span>
              <span className="text-foreground">{match.reasoning}</span>
            </div>
          )}
        </div>

        {/* Action Buttons */}
        {isPending && (
          <div className="flex items-center justify-end gap-3 pt-4 border-t border-border">
            <button
              onClick={onReject}
              disabled={isRejecting || isApproving}
              className={cn(
                'flex items-center gap-2 px-4 py-2 rounded-lg font-medium transition-colors',
                'bg-red-400/10 text-red-400 border border-red-400/30',
                'hover:bg-red-400/20',
                'disabled:opacity-50 disabled:cursor-not-allowed',
              )}
            >
              <XCircle className="w-4 h-4" />
              {isRejecting ? 'Rejecting...' : 'Reject'}
            </button>
            <button
              onClick={onApprove}
              disabled={isApproving || isRejecting}
              className={cn(
                'flex items-center gap-2 px-4 py-2 rounded-lg font-medium transition-colors',
                'bg-emerald/10 text-emerald border border-emerald/30',
                'hover:bg-emerald/20',
                'disabled:opacity-50 disabled:cursor-not-allowed',
              )}
            >
              <CheckCircle className="w-4 h-4" />
              {isApproving ? 'Approving...' : 'Approve'}
            </button>
          </div>
        )}
      </div>
    </div>
  )
}

// Detail Row Component
function DetailRow({
  icon: Icon,
  label,
  value,
  highlight = false,
  urgency,
}: {
  icon: React.ComponentType<{ className?: string }>
  label: string
  value: string
  highlight?: boolean
  urgency?: string
}) {
  const urgencyColor = urgency
    ? urgency.toLowerCase() === 'high' || urgency.toLowerCase() === 'urgent'
      ? 'text-red-400'
      : urgency.toLowerCase() === 'medium'
        ? 'text-amber'
        : 'text-muted-foreground'
    : undefined

  return (
    <div className="flex items-start gap-2">
      <Icon className="w-3.5 h-3.5 text-muted-foreground mt-0.5 shrink-0" />
      <div className="flex-1 min-w-0">
        <span className="text-xs text-muted-foreground">{label}: </span>
        <span
          className={cn(
            'text-sm',
            highlight ? 'font-medium text-foreground' : 'text-foreground',
            urgencyColor,
          )}
        >
          {value}
        </span>
      </div>
    </div>
  )
}
