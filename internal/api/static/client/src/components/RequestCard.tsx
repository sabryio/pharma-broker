import { Card, CardContent } from '@/components/ui/card'
import { timeAgo } from '@/lib/sse'
import type { Request } from '@/lib/types'

interface RequestCardProps {
  request: Request
}

export function RequestCard({ request }: RequestCardProps) {
  const maxPrice = request.max_price
    ? `Max: ${request.max_price} ${request.currency || 'EGP'}`
    : ''
  const qty = request.quantity
    ? `${request.quantity} ${request.unit || ''}`
    : ''

  return (
    <Card className="cursor-pointer hover:border-primary transition-colors">
      <CardContent className="p-4">
        <div className="flex justify-between items-start mb-2">
          <div>
            <p className="font-semibold">{request.medication}</p>
            <p
              className="text-sm text-muted-foreground rtl text-right"
              dir="rtl"
            >
              {request.medication_raw}
            </p>
          </div>
        </div>
        <div className="flex flex-wrap gap-2 mt-2">
          {qty && (
            <span className="px-2 py-1 bg-secondary text-secondary-foreground rounded text-xs">
              {qty}
            </span>
          )}
          {maxPrice && (
            <span className="px-2 py-1 bg-secondary text-secondary-foreground rounded text-xs">
              {maxPrice}
            </span>
          )}
          {request.urgent && (
            <span className="px-2 py-1 bg-red-500/20 text-red-500 rounded text-xs font-semibold">
              URGENT
            </span>
          )}
        </div>
        <div className="flex justify-between text-xs text-muted-foreground mt-3">
          <span>{request.source_name || request.source_phone}</span>
          <span>{timeAgo(request.created_at)}</span>
        </div>
      </CardContent>
    </Card>
  )
}
