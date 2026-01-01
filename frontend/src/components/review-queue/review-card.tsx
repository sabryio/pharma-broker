import { cn } from '@/lib/utils'
import { Badge } from '@/components/ui/badge'
import type { ReviewOffer, ReviewRequest } from './types'

interface ReviewCardProps {
  type: 'offer' | 'request'
  offer?: ReviewOffer
  request?: ReviewRequest
}

export function ReviewCard({ type, offer, request }: ReviewCardProps) {
  const isOffer = type === 'offer'
  const accentColor = isOffer ? 'teal' : 'amber'

  if (isOffer && offer) {
    return (
      <div
        className={cn(
          'glass-card p-6 rounded-xl border transition-all duration-500',
          `border-${accentColor}/30 hover-glow-${accentColor}`,
        )}
      >
        <div className="flex items-center gap-2 mb-4">
          <div
            className={`w-2 h-2 rounded-full bg-${accentColor} animate-pulse`}
          />
          <span
            className={`text-sm font-semibold text-${accentColor} uppercase tracking-wider`}
          >
            Supply Offer
          </span>
        </div>
        <div className="text-xs text-muted-foreground mb-4">
          Source: {offer.source}
        </div>

        <div className="space-y-4">
          <div className="p-3 rounded-lg bg-secondary/30 backdrop-blur-sm">
            <span className="text-xs text-muted-foreground block mb-1">
              Product
            </span>
            <span className="text-sm font-medium text-foreground">
              {offer.product}
            </span>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="p-3 rounded-lg bg-secondary/30">
              <span className="text-xs text-muted-foreground block mb-1">
                Quantity
              </span>
              <span className={`text-sm font-medium text-${accentColor}`}>
                {offer.quantity}
              </span>
            </div>
            <div className="p-3 rounded-lg bg-secondary/30">
              <span className="text-xs text-muted-foreground block mb-1">
                Price
              </span>
              <span className={`text-sm font-medium text-${accentColor}`}>
                {offer.price}
              </span>
            </div>
          </div>
          <div className="p-3 rounded-lg bg-secondary/30">
            <span className="text-xs text-muted-foreground block mb-1">
              Expiry Date
            </span>
            <span className="text-sm font-medium text-foreground">
              {offer.expiry}
            </span>
          </div>
        </div>
      </div>
    )
  }

  if (!isOffer && request) {
    return (
      <div
        className={cn(
          'glass-card p-6 rounded-xl border transition-all duration-500',
          `border-${accentColor}/30 hover-glow-${accentColor}`,
        )}
      >
        <div className="flex items-center gap-2 mb-4">
          <div
            className={`w-2 h-2 rounded-full bg-${accentColor} animate-pulse`}
          />
          <span
            className={`text-sm font-semibold text-${accentColor} uppercase tracking-wider`}
          >
            Demand Request
          </span>
        </div>
        <div className="text-xs text-muted-foreground mb-4">
          Source: {request.source}
        </div>

        <div className="space-y-4">
          <div className="p-3 rounded-lg bg-secondary/30 backdrop-blur-sm">
            <span className="text-xs text-muted-foreground block mb-1">
              Product Needed
            </span>
            <span className="text-sm font-medium text-foreground">
              {request.product}
            </span>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="p-3 rounded-lg bg-secondary/30">
              <span className="text-xs text-muted-foreground block mb-1">
                Quantity
              </span>
              <span className={`text-sm font-medium text-${accentColor}`}>
                {request.quantity}
              </span>
            </div>
            <div className="p-3 rounded-lg bg-secondary/30">
              <span className="text-xs text-muted-foreground block mb-1">
                Max Price
              </span>
              <span className={`text-sm font-medium text-${accentColor}`}>
                {request.maxPrice}
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
                request.urgency === 'High' &&
                  'border-destructive/50 text-destructive bg-destructive/10',
                request.urgency === 'Medium' &&
                  'border-amber/50 text-amber bg-amber/10',
                request.urgency === 'Low' &&
                  'border-emerald/50 text-emerald bg-emerald/10',
              )}
            >
              {request.urgency}
            </Badge>
          </div>
        </div>
      </div>
    )
  }

  return null
}
