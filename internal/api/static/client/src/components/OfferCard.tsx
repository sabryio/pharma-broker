import { Card, CardContent } from '@/components/ui/card'
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
          <span>{offer.source_name || offer.source_phone}</span>
          <span>{timeAgo(offer.created_at)}</span>
        </div>
      </CardContent>
    </Card>
  )
}
