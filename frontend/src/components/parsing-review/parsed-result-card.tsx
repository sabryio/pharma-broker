import { cn } from '@/lib/utils'
import { Badge } from '@/components/ui/badge'
import type { ParsedResult } from './types'
import {
  Package,
  DollarSign,
  Calendar,
  Hash,
  FileText,
  AlertCircle,
  TrendingUp,
} from 'lucide-react'

interface ParsedResultCardProps {
  result: ParsedResult
  confidence: number
}

export function ParsedResultCard({
  result,
  confidence,
}: ParsedResultCardProps) {
  const isOffer = result.type === 'offer'

  return (
    <div
      className={cn(
        'glass-card p-6 rounded-xl border transition-all duration-500',
        isOffer
          ? 'border-purple-500/30 hover:border-purple-500/50'
          : 'border-amber/30 hover:border-amber/50',
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <div
            className={cn(
              'w-2 h-2 rounded-full animate-pulse',
              isOffer ? 'bg-purple-500' : 'bg-amber',
            )}
          />
          <span
            className={cn(
              'text-sm font-semibold uppercase tracking-wider',
              isOffer ? 'text-purple-400' : 'text-amber',
            )}
          >
            {isOffer ? 'Parsed as Offer' : 'Parsed as Request'}
          </span>
        </div>
        <Badge
          variant="outline"
          className={cn(
            'text-xs',
            confidence >= 0.7
              ? 'border-emerald/50 text-emerald bg-emerald/10'
              : confidence >= 0.5
                ? 'border-amber/50 text-amber bg-amber/10'
                : 'border-destructive/50 text-destructive bg-destructive/10',
          )}
        >
          {Math.round(confidence * 100)}% confidence
        </Badge>
      </div>

      {/* Content */}
      <div className="space-y-3">
        {/* Medication - Always present */}
        <div
          className={cn(
            'p-3 rounded-lg backdrop-blur-sm border',
            isOffer
              ? 'bg-purple-500/10 border-purple-500/20'
              : 'bg-amber/10 border-amber/20',
          )}
        >
          <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
            <Package className="w-3.5 h-3.5" />
            <span>Medication</span>
          </div>
          <span className="text-sm font-medium text-foreground">
            {result.medication}
          </span>
        </div>

        {/* Grid for other fields */}
        <div className="grid grid-cols-2 gap-3">
          {result.quantity && (
            <div className="p-3 rounded-lg bg-secondary/30 border border-border/50">
              <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
                <Hash className="w-3.5 h-3.5" />
                <span>Quantity</span>
              </div>
              <span
                className={cn(
                  'text-sm font-medium',
                  isOffer ? 'text-purple-400' : 'text-amber',
                )}
              >
                {result.quantity}
              </span>
            </div>
          )}

          {isOffer && result.price && (
            <div className="p-3 rounded-lg bg-secondary/30 border border-border/50">
              <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
                <DollarSign className="w-3.5 h-3.5" />
                <span>Price</span>
              </div>
              <span className="text-sm font-medium text-purple-400">
                {result.price}
              </span>
            </div>
          )}

          {!isOffer && result.maxPrice && (
            <div className="p-3 rounded-lg bg-secondary/30 border border-border/50">
              <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
                <TrendingUp className="w-3.5 h-3.5" />
                <span>Max Price</span>
              </div>
              <span className="text-sm font-medium text-amber">
                {result.maxPrice}
              </span>
            </div>
          )}

          {isOffer && result.expiry && (
            <div className="p-3 rounded-lg bg-secondary/30 border border-border/50">
              <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
                <Calendar className="w-3.5 h-3.5" />
                <span>Expiry</span>
              </div>
              <span className="text-sm font-medium text-foreground">
                {result.expiry}
              </span>
            </div>
          )}

          {!isOffer && result.urgency && (
            <div className="p-3 rounded-lg bg-secondary/30 border border-border/50">
              <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
                <AlertCircle className="w-3.5 h-3.5" />
                <span>Urgency</span>
              </div>
              <Badge
                variant="outline"
                className={cn(
                  'text-xs capitalize',
                  result.urgency === 'high' &&
                    'border-destructive/50 text-destructive',
                  result.urgency === 'medium' && 'border-amber/50 text-amber',
                  result.urgency === 'low' && 'border-emerald/50 text-emerald',
                )}
              >
                {result.urgency}
              </Badge>
            </div>
          )}
        </div>

        {/* Notes if present */}
        {result.notes && (
          <div className="p-3 rounded-lg bg-secondary/30 border border-border/50">
            <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
              <FileText className="w-3.5 h-3.5" />
              <span>Notes</span>
            </div>
            <span className="text-sm text-muted-foreground">
              {result.notes}
            </span>
          </div>
        )}
      </div>

      {/* Footer decoration */}
      <div className="flex items-center gap-2 mt-4 text-xs text-muted-foreground">
        <div
          className={cn(
            'flex-1 h-px bg-linear-to-r to-transparent',
            isOffer ? 'from-purple-500/20' : 'from-amber/20',
          )}
        />
        <span>AI Interpretation</span>
        <div
          className={cn(
            'flex-1 h-px bg-linear-to-l to-transparent',
            isOffer ? 'from-purple-500/20' : 'from-amber/20',
          )}
        />
      </div>
    </div>
  )
}
