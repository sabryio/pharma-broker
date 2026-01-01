import { cn } from '@/lib/utils'
import { Badge } from '@/components/ui/badge'
import type { ReviewOffer, ReviewRequest } from './types'
import {
  Package,
  DollarSign,
  Calendar,
  Hash,
  TrendingUp,
  AlertCircle,
  Building2,
} from 'lucide-react'

interface ReviewCardProps {
  type: 'offer' | 'request'
  offer?: ReviewOffer
  request?: ReviewRequest
}

export function ReviewCard({ type, offer, request }: ReviewCardProps) {
  const isOffer = type === 'offer'

  if (isOffer && offer) {
    return (
      <div className="glass-card p-6 rounded-xl border border-teal/30 hover:border-teal/50 transition-all duration-500 shadow-lg shadow-teal/5">
        {/* Header */}
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <div className="w-2 h-2 rounded-full bg-teal animate-pulse" />
            <span className="text-sm font-semibold text-teal uppercase tracking-wider">
              Supply Offer
            </span>
          </div>
          <div className="w-8 h-8 rounded-lg bg-teal/20 flex items-center justify-center">
            <Package className="w-4 h-4 text-teal" />
          </div>
        </div>

        {/* Source */}
        <div className="flex items-center gap-2 text-xs text-muted-foreground mb-4 p-2 rounded-lg bg-teal/5 border border-teal/10">
          <Building2 className="w-3.5 h-3.5 text-teal" />
          <span>{offer.source}</span>
        </div>

        {/* Content */}
        <div className="space-y-3">
          {/* Product */}
          <div className="p-3 rounded-lg bg-teal/10 border border-teal/20 backdrop-blur-sm">
            <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
              <Package className="w-3.5 h-3.5" />
              <span>Product</span>
            </div>
            <span className="text-sm font-medium text-foreground">
              {offer.product}
            </span>
          </div>

          {/* Grid */}
          <div className="grid grid-cols-2 gap-3">
            <div className="p-3 rounded-lg bg-secondary/30 border border-border/50">
              <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
                <Hash className="w-3.5 h-3.5" />
                <span>Quantity</span>
              </div>
              <span className="text-sm font-medium text-teal">
                {offer.quantity}
              </span>
            </div>
            <div className="p-3 rounded-lg bg-secondary/30 border border-border/50">
              <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
                <DollarSign className="w-3.5 h-3.5" />
                <span>Price</span>
              </div>
              <span className="text-sm font-medium text-teal">
                {offer.price}
              </span>
            </div>
          </div>

          {/* Expiry */}
          <div className="p-3 rounded-lg bg-secondary/30 border border-border/50">
            <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
              <Calendar className="w-3.5 h-3.5" />
              <span>Expiry Date</span>
            </div>
            <span className="text-sm font-medium text-foreground">
              {offer.expiry}
            </span>
          </div>
        </div>

        {/* Footer decoration */}
        <div className="flex items-center gap-2 mt-4 text-xs text-muted-foreground">
          <div className="flex-1 h-px bg-linear-to-r from-teal/30 to-transparent" />
          <span>Supply Side</span>
          <div className="flex-1 h-px bg-linear-to-l from-teal/30 to-transparent" />
        </div>
      </div>
    )
  }

  if (!isOffer && request) {
    return (
      <div className="glass-card p-6 rounded-xl border border-amber/30 hover:border-amber/50 transition-all duration-500 shadow-lg shadow-amber/5">
        {/* Header */}
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-2">
            <div className="w-2 h-2 rounded-full bg-amber animate-pulse" />
            <span className="text-sm font-semibold text-amber uppercase tracking-wider">
              Demand Request
            </span>
          </div>
          <div className="w-8 h-8 rounded-lg bg-amber/20 flex items-center justify-center">
            <TrendingUp className="w-4 h-4 text-amber" />
          </div>
        </div>

        {/* Source */}
        <div className="flex items-center gap-2 text-xs text-muted-foreground mb-4 p-2 rounded-lg bg-amber/5 border border-amber/10">
          <Building2 className="w-3.5 h-3.5 text-amber" />
          <span>{request.source}</span>
        </div>

        {/* Content */}
        <div className="space-y-3">
          {/* Product */}
          <div className="p-3 rounded-lg bg-amber/10 border border-amber/20 backdrop-blur-sm">
            <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
              <Package className="w-3.5 h-3.5" />
              <span>Product Needed</span>
            </div>
            <span className="text-sm font-medium text-foreground">
              {request.product}
            </span>
          </div>

          {/* Grid */}
          <div className="grid grid-cols-2 gap-3">
            <div className="p-3 rounded-lg bg-secondary/30 border border-border/50">
              <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
                <Hash className="w-3.5 h-3.5" />
                <span>Quantity</span>
              </div>
              <span className="text-sm font-medium text-amber">
                {request.quantity}
              </span>
            </div>
            <div className="p-3 rounded-lg bg-secondary/30 border border-border/50">
              <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
                <DollarSign className="w-3.5 h-3.5" />
                <span>Max Price</span>
              </div>
              <span className="text-sm font-medium text-amber">
                {request.maxPrice}
              </span>
            </div>
          </div>

          {/* Urgency */}
          <div className="p-3 rounded-lg bg-secondary/30 border border-border/50">
            <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
              <AlertCircle className="w-3.5 h-3.5" />
              <span>Urgency</span>
            </div>
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

        {/* Footer decoration */}
        <div className="flex items-center gap-2 mt-4 text-xs text-muted-foreground">
          <div className="flex-1 h-px bg-linear-to-r from-amber/30 to-transparent" />
          <span>Demand Side</span>
          <div className="flex-1 h-px bg-linear-to-l from-amber/30 to-transparent" />
        </div>
      </div>
    )
  }

  return null
}
