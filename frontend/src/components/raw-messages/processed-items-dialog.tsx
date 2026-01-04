// Processed Items Dialog Component
import { useState, useEffect } from 'react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Separator } from '@/components/ui/separator'
import { Package, ShoppingCart, Loader2 } from 'lucide-react'
import type { RawMessage } from './types'
import type { ProcessedItemsResponse } from '@/schema/raw-message'

interface ProcessedItemsDialogProps {
  message: RawMessage | null
  onClose: () => void
}

export function ProcessedItemsDialog({
  message,
  onClose,
}: ProcessedItemsDialogProps) {
  const [items, setItems] = useState<ProcessedItemsResponse | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!message) {
      setItems(null)
      return
    }

    const fetchItems = async () => {
      setLoading(true)
      setError(null)
      try {
        const response = await fetch(`/api/raw-messages/${message.id}/items`)
        const data = await response.json()
        if (data.success) {
          setItems(data.data)
        } else {
          setError(data.error || 'Failed to load items')
        }
      } catch (err) {
        setError('Failed to load items')
      } finally {
        setLoading(false)
      }
    }

    fetchItems()
  }, [message?.id])

  const formatDate = (dateStr: string) => {
    return new Date(dateStr).toLocaleString()
  }

  return (
    <Dialog open={!!message} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="text-sm font-medium">
            Processed Items
          </DialogTitle>
        </DialogHeader>

        {loading && (
          <div className="flex items-center justify-center py-8">
            <Loader2 className="w-6 h-6 animate-spin text-muted-foreground" />
          </div>
        )}

        {error && (
          <div className="text-sm text-destructive text-center py-4">
            {error}
          </div>
        )}

        {items && !loading && (
          <ScrollArea className="max-h-[400px]">
            <div className="space-y-4">
              {/* Offers Section */}
              {items.offers.length > 0 && (
                <section>
                  <div className="flex items-center gap-2 mb-2">
                    <Package className="w-4 h-4 text-emerald-600" />
                    <span className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                      Offers ({items.offers.length})
                    </span>
                  </div>
                  <div className="space-y-2">
                    {items.offers.map((item) => (
                      <div
                        key={item.id}
                        className="p-3 bg-emerald-500/5 border border-emerald-500/20 rounded-lg"
                      >
                        <div className="flex items-start justify-between gap-2">
                          <div className="flex-1 min-w-0">
                            <p className="text-sm font-medium truncate">
                              {item.medication}
                            </p>
                            {item.quantity && (
                              <p className="text-xs text-muted-foreground">
                                Qty: {item.quantity}
                              </p>
                            )}
                          </div>
                          <Badge
                            variant="secondary"
                            className="text-[10px] shrink-0"
                          >
                            {item.status}
                          </Badge>
                        </div>
                        <p className="text-[10px] text-muted-foreground mt-1">
                          {formatDate(item.createdAt)}
                        </p>
                      </div>
                    ))}
                  </div>
                </section>
              )}

              {items.offers.length > 0 && items.requests.length > 0 && (
                <Separator />
              )}

              {/* Requests Section */}
              {items.requests.length > 0 && (
                <section>
                  <div className="flex items-center gap-2 mb-2">
                    <ShoppingCart className="w-4 h-4 text-blue-600" />
                    <span className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                      Requests ({items.requests.length})
                    </span>
                  </div>
                  <div className="space-y-2">
                    {items.requests.map((item) => (
                      <div
                        key={item.id}
                        className="p-3 bg-blue-500/5 border border-blue-500/20 rounded-lg"
                      >
                        <div className="flex items-start justify-between gap-2">
                          <div className="flex-1 min-w-0">
                            <p className="text-sm font-medium truncate">
                              {item.medication}
                            </p>
                            {item.quantity && (
                              <p className="text-xs text-muted-foreground">
                                Qty: {item.quantity}
                              </p>
                            )}
                          </div>
                          <Badge
                            variant="secondary"
                            className="text-[10px] shrink-0"
                          >
                            {item.status}
                          </Badge>
                        </div>
                        <p className="text-[10px] text-muted-foreground mt-1">
                          {formatDate(item.createdAt)}
                        </p>
                      </div>
                    ))}
                  </div>
                </section>
              )}

              {items.offers.length === 0 && items.requests.length === 0 && (
                <div className="text-sm text-muted-foreground text-center py-4">
                  No processed items found
                </div>
              )}
            </div>
          </ScrollArea>
        )}
      </DialogContent>
    </Dialog>
  )
}
