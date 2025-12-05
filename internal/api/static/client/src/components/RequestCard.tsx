import { Card, CardContent } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
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
        {/* Group Badge */}
        {request.group_name && (
          <Badge variant="outline" className="mb-2 text-xs">
            📍 {request.group_name}
          </Badge>
        )}

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
          <div className="flex flex-col">
            <span>{request.source_name || 'Unknown'}</span>
            {request.source_phone && (
              <span className="font-mono text-[10px]">
                {request.source_phone}
              </span>
            )}
          </div>
          <span>{timeAgo(request.created_at)}</span>
        </div>
      </CardContent>
    </Card>
  )
}
