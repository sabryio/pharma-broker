// Match Comparison View Component
// Side-by-side layout with visual field connectors

import { useMemo } from 'react'
import { cn } from '@/lib/utils'
import {
  Package,
  DollarSign,
  Hash,
  Calendar,
  AlertTriangle,
  Check,
  X,
  Minus,
} from 'lucide-react'
import type { ReviewOffer, ReviewRequest } from './types'

interface FieldMatch {
  field: string
  label: string
  offerValue: string | null
  requestValue: string | null
  matchType: 'exact' | 'partial' | 'mismatch' | 'na'
  score: number // 0-100
  icon: React.ReactNode
}

interface MatchComparisonProps {
  offer: ReviewOffer
  request: ReviewRequest
  className?: string
}

function getMatchIcon(type: FieldMatch['matchType']) {
  switch (type) {
    case 'exact':
      return <Check className="w-3.5 h-3.5 text-emerald" />
    case 'partial':
      return <Minus className="w-3.5 h-3.5 text-amber" />
    case 'mismatch':
      return <X className="w-3.5 h-3.5 text-red-400" />
    default:
      return <Minus className="w-3.5 h-3.5 text-muted-foreground" />
  }
}

function getMatchColor(type: FieldMatch['matchType']) {
  switch (type) {
    case 'exact':
      return 'border-emerald/50 bg-emerald/10'
    case 'partial':
      return 'border-amber/50 bg-amber/10'
    case 'mismatch':
      return 'border-red-400/50 bg-red-400/10'
    default:
      return 'border-muted/50 bg-muted/10'
  }
}

// Calculate medication similarity using simple string matching
function calculateMedicationMatch(
  offerProduct: string,
  requestProduct: string,
): { type: FieldMatch['matchType']; score: number } {
  const offer = offerProduct.toLowerCase().trim()
  const request = requestProduct.toLowerCase().trim()

  if (offer === request) {
    return { type: 'exact', score: 100 }
  }

  // Check if one contains the other
  if (offer.includes(request) || request.includes(offer)) {
    return { type: 'partial', score: 75 }
  }

  // Simple word overlap
  const offerWords = new Set(offer.split(/\s+/))
  const requestWords = new Set(request.split(/\s+/))
  const intersection = [...offerWords].filter((w) => requestWords.has(w))
  const overlap =
    (intersection.length * 2) / (offerWords.size + requestWords.size)

  if (overlap > 0.5) {
    return { type: 'partial', score: Math.round(overlap * 100) }
  }

  return { type: 'mismatch', score: Math.round(overlap * 100) }
}

// Calculate quantity match
function calculateQuantityMatch(
  offerQty: string | null,
  requestQty: string | null,
): { type: FieldMatch['matchType']; score: number } {
  if (!offerQty || !requestQty) {
    return { type: 'na', score: 0 }
  }

  const offerNum = parseFloat(offerQty.replace(/[^\d.]/g, ''))
  const requestNum = parseFloat(requestQty.replace(/[^\d.]/g, ''))

  if (isNaN(offerNum) || isNaN(requestNum)) {
    return { type: 'na', score: 0 }
  }

  if (offerNum >= requestNum) {
    return { type: 'exact', score: 100 }
  }

  const ratio = offerNum / requestNum
  if (ratio >= 0.8) {
    return { type: 'partial', score: Math.round(ratio * 100) }
  }

  return { type: 'mismatch', score: Math.round(ratio * 100) }
}

// Calculate price match
function calculatePriceMatch(
  offerPrice: string | null,
  maxPrice: string | null,
): { type: FieldMatch['matchType']; score: number } {
  if (!offerPrice || !maxPrice) {
    return { type: 'na', score: 0 }
  }

  const offer = parseFloat(offerPrice.replace(/[^\d.]/g, ''))
  const max = parseFloat(maxPrice.replace(/[^\d.]/g, ''))

  if (isNaN(offer) || isNaN(max)) {
    return { type: 'na', score: 0 }
  }

  if (offer <= max) {
    return { type: 'exact', score: 100 }
  }

  const ratio = max / offer
  if (ratio >= 0.9) {
    return { type: 'partial', score: Math.round(ratio * 100) }
  }

  return { type: 'mismatch', score: Math.round(ratio * 100) }
}

export function MatchComparison({
  offer,
  request,
  className,
}: MatchComparisonProps) {
  const fieldMatches = useMemo<FieldMatch[]>(() => {
    const medMatch = calculateMedicationMatch(offer.product, request.product)
    const qtyMatch = calculateQuantityMatch(offer.quantity, request.quantity)
    const priceMatch = calculatePriceMatch(offer.price, request.maxPrice)

    return [
      {
        field: 'medication',
        label: 'Medication',
        offerValue: offer.product,
        requestValue: request.product,
        matchType: medMatch.type,
        score: medMatch.score,
        icon: <Package className="w-4 h-4" />,
      },
      {
        field: 'quantity',
        label: 'Quantity',
        offerValue: offer.quantity,
        requestValue: request.quantity,
        matchType: qtyMatch.type,
        score: qtyMatch.score,
        icon: <Hash className="w-4 h-4" />,
      },
      {
        field: 'price',
        label: 'Price',
        offerValue: offer.price,
        requestValue: request.maxPrice ? `Max: ${request.maxPrice}` : null,
        matchType: priceMatch.type,
        score: priceMatch.score,
        icon: <DollarSign className="w-4 h-4" />,
      },
      {
        field: 'expiry',
        label: 'Expiry',
        offerValue: offer.expiry,
        requestValue: null,
        matchType: 'na',
        score: 0,
        icon: <Calendar className="w-4 h-4" />,
      },
      {
        field: 'urgency',
        label: 'Urgency',
        offerValue: null,
        requestValue: request.urgency,
        matchType: 'na',
        score: 0,
        icon: <AlertTriangle className="w-4 h-4" />,
      },
    ]
  }, [offer, request])

  const overallScore = useMemo(() => {
    const scoredFields = fieldMatches.filter((f) => f.matchType !== 'na')
    if (scoredFields.length === 0) return 0
    return Math.round(
      scoredFields.reduce((acc, f) => acc + f.score, 0) / scoredFields.length,
    )
  }, [fieldMatches])

  return (
    <div className={cn('relative', className)}>
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-sm font-semibold text-foreground flex items-center gap-2">
          <div className="w-6 h-6 rounded-lg bg-violet-500/20 flex items-center justify-center">
            <Package className="w-3.5 h-3.5 text-violet-400" />
          </div>
          Field Comparison
        </h3>
        <div className="flex items-center gap-2">
          <span className="text-xs text-muted-foreground">Overall:</span>
          <span
            className={cn(
              'text-sm font-bold px-2 py-0.5 rounded-full',
              overallScore >= 80 && 'bg-emerald/20 text-emerald',
              overallScore >= 50 &&
                overallScore < 80 &&
                'bg-amber/20 text-amber',
              overallScore < 50 && 'bg-red-400/20 text-red-400',
            )}
          >
            {overallScore}%
          </span>
        </div>
      </div>

      {/* Comparison Grid */}
      <div className="space-y-2">
        {fieldMatches.map((field, idx) => (
          <div
            key={field.field}
            className="grid grid-cols-[1fr,auto,1fr] gap-2 items-center animate-fade-in"
            style={{ animationDelay: `${idx * 50}ms` }}
          >
            {/* Offer Value */}
            <div
              className={cn(
                'p-2.5 rounded-lg border transition-all duration-300',
                'bg-teal/5 border-teal/20',
                field.offerValue ? 'opacity-100' : 'opacity-40',
              )}
            >
              <div className="flex items-center gap-2 text-xs text-teal/70 mb-1">
                {field.icon}
                <span className="uppercase tracking-wider font-medium">
                  {field.label}
                </span>
              </div>
              <span className="text-sm font-medium text-foreground truncate block">
                {field.offerValue || '—'}
              </span>
            </div>

            {/* Connector */}
            <div className="flex flex-col items-center gap-1 px-2">
              <div
                className={cn(
                  'w-8 h-8 rounded-full flex items-center justify-center border-2 transition-all duration-300',
                  getMatchColor(field.matchType),
                )}
              >
                {getMatchIcon(field.matchType)}
              </div>
              {field.matchType !== 'na' && (
                <span
                  className={cn(
                    'text-[10px] font-bold',
                    field.matchType === 'exact' && 'text-emerald',
                    field.matchType === 'partial' && 'text-amber',
                    field.matchType === 'mismatch' && 'text-red-400',
                  )}
                >
                  {field.score}%
                </span>
              )}
            </div>

            {/* Request Value */}
            <div
              className={cn(
                'p-2.5 rounded-lg border transition-all duration-300',
                'bg-amber/5 border-amber/20',
                field.requestValue ? 'opacity-100' : 'opacity-40',
              )}
            >
              <div className="flex items-center gap-2 text-xs text-amber/70 mb-1">
                {field.icon}
                <span className="uppercase tracking-wider font-medium">
                  {field.label}
                </span>
              </div>
              <span className="text-sm font-medium text-foreground truncate block">
                {field.requestValue || '—'}
              </span>
            </div>
          </div>
        ))}
      </div>

      {/* Legend */}
      <div className="flex items-center justify-center gap-4 mt-4 pt-3 border-t border-border/30">
        <div className="flex items-center gap-1.5 text-xs">
          <div className="w-4 h-4 rounded-full bg-emerald/20 border border-emerald/50 flex items-center justify-center">
            <Check className="w-2.5 h-2.5 text-emerald" />
          </div>
          <span className="text-muted-foreground">Match</span>
        </div>
        <div className="flex items-center gap-1.5 text-xs">
          <div className="w-4 h-4 rounded-full bg-amber/20 border border-amber/50 flex items-center justify-center">
            <Minus className="w-2.5 h-2.5 text-amber" />
          </div>
          <span className="text-muted-foreground">Partial</span>
        </div>
        <div className="flex items-center gap-1.5 text-xs">
          <div className="w-4 h-4 rounded-full bg-red-400/20 border border-red-400/50 flex items-center justify-center">
            <X className="w-2.5 h-2.5 text-red-400" />
          </div>
          <span className="text-muted-foreground">Mismatch</span>
        </div>
      </div>
    </div>
  )
}
