import { Card, CardContent } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { timeAgo } from '@/lib/sse'
import type { Offer } from '@/lib/types'

interface OfferCardProps {
  offer: Offer
}

export function OfferCard({ offer }: OfferCardProps) {
  const price = offer.price ? `${offer.price} ${offer.currency || 'EGP'}` : '-'
  const qty = offer.quantity ? `${offer.quantity} ${offer.unit || ''}` : ''

  return (
    <Card className="cursor-pointer hover:border-primary transition-colors">
      <CardContent className="p-4">
        {/* Group Badge */}
        {offer.group_name && (
          <Badge variant="outline" className="mb-2 text-xs">
            📍 {offer.group_name}
          </Badge>
        )}

        <div className="flex justify-between items-start mb-2">
          <div>
            <p className="font-semibold">{offer.medication}</p>
            <p
              className="text-sm text-muted-foreground rtl text-right"
              dir="rtl"
            >
              {offer.medication_raw}
            </p>
          </div>
          <span className="font-bold text-green-500">{price}</span>
        </div>
        <div className="flex flex-wrap gap-2 mt-2">
          {qty && (
            <span className="px-2 py-1 bg-secondary text-secondary-foreground rounded text-xs">
              {qty}
            </span>
          )}
          {offer.expiry_date && (
            <span className="px-2 py-1 bg-secondary text-secondary-foreground rounded text-xs">
              Exp: {offer.expiry_date.substring(0, 7)}
            </span>
          )}
        </div>
        <div className="flex justify-between text-xs text-muted-foreground mt-3">
          <div className="flex flex-col">
            <span>{offer.source_name || 'Unknown'}</span>
            {offer.source_phone && (
              <span className="font-mono text-[10px]">
                {offer.source_phone}
              </span>
            )}
          </div>
          <span>{timeAgo(offer.created_at)}</span>
        </div>
      </CardContent>
    </Card>
  )
}
