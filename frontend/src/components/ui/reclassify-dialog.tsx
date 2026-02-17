'use client'

import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import {
  AlertCircle,
  ArrowRightLeft,
  CheckCircle2,
  Loader2,
  Package,
  ShoppingCart,
  Sparkles,
} from 'lucide-react'
import type { ItemType } from '@/api/offers'
import { cn } from '@/lib/utils'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { reclassifyItem } from '@/api/offers'

interface ReclassifyDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  itemId: string
  itemType: ItemType
  medication: string
  onSuccess?: (newId: string, newType: ItemType) => void
}

// Placeholder user ID until auth is implemented
const PLACEHOLDER_USER_ID = '00000000-0000-4000-8000-000000000001'

export function ReclassifyDialog({
  open,
  onOpenChange,
  itemId,
  itemType,
  medication,
  onSuccess,
}: ReclassifyDialogProps) {
  const [notes, setNotes] = useState('')
  const [showSuccess, setShowSuccess] = useState(false)
  const queryClient = useQueryClient()

  const targetType: ItemType = itemType === 'offer' ? 'request' : 'offer'

  const mutation = useMutation({
    mutationFn: () =>
      reclassifyItem({
        sourceId: itemId,
        sourceType: itemType,
        targetType,
        reclassifiedBy: PLACEHOLDER_USER_ID,
        notes: notes || undefined,
      }),
    onSuccess: (data) => {
      setShowSuccess(true)
      // Invalidate offers, requests, and match-reviews queries
      queryClient.invalidateQueries({ queryKey: ['offers'] })
      queryClient.invalidateQueries({ queryKey: ['requests'] })
      queryClient.invalidateQueries({ queryKey: ['match-reviews'] })

      // Call success callback after animation
      setTimeout(() => {
        onSuccess?.(data.newId, data.newType)
        onOpenChange(false)
        setShowSuccess(false)
        setNotes('')
      }, 1500)
    },
  })

  const handleReclassify = () => {
    mutation.mutate()
  }

  const handleClose = () => {
    if (!mutation.isPending) {
      onOpenChange(false)
      setNotes('')
      setShowSuccess(false)
      mutation.reset()
    }
  }

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <ArrowRightLeft className="w-5 h-5 text-teal" />
            Reclassify Item
          </DialogTitle>
          <DialogDescription>
            Change the classification of this item from{' '}
            <span className="font-medium text-foreground">{itemType}</span> to{' '}
            <span className="font-medium text-foreground">{targetType}</span>
          </DialogDescription>
        </DialogHeader>

        {showSuccess ? (
          <div className="flex flex-col items-center justify-center py-8 gap-4 animate-in fade-in zoom-in duration-300">
            <div className="relative">
              <CheckCircle2 className="w-16 h-16 text-emerald animate-bounce" />
              <Sparkles className="w-6 h-6 text-amber absolute -top-1 -right-1 animate-pulse" />
            </div>
            <p className="text-lg font-medium text-emerald">
              Successfully Reclassified!
            </p>
            <p className="text-sm text-muted-foreground">
              Item is now a {targetType}
            </p>
          </div>
        ) : (
          <>
            {/* Item Preview */}
            <div className="space-y-4">
              <div className="p-4 rounded-lg bg-secondary/50 border border-border">
                <div className="flex items-start gap-3">
                  <div
                    className={cn(
                      'p-2 rounded-lg',
                      itemType === 'offer'
                        ? 'bg-emerald/10 text-emerald'
                        : 'bg-amber/10 text-amber',
                    )}
                  >
                    {itemType === 'offer' ? (
                      <Package className="w-5 h-5" />
                    ) : (
                      <ShoppingCart className="w-5 h-5" />
                    )}
                  </div>
                  <div className="flex-1 min-w-0">
                    <p className="font-medium text-foreground truncate">
                      {medication}
                    </p>
                    <p className="text-xs text-muted-foreground mt-1">
                      ID: {itemId.slice(0, 8)}...
                    </p>
                  </div>
                </div>
              </div>

              {/* Transformation Arrow */}
              <div className="flex items-center justify-center gap-4">
                <div
                  className={cn(
                    'flex items-center gap-2 px-3 py-1.5 rounded-full text-sm font-medium',
                    itemType === 'offer'
                      ? 'bg-emerald/10 text-emerald border border-emerald/30'
                      : 'bg-amber/10 text-amber border border-amber/30',
                  )}
                >
                  {itemType === 'offer' ? (
                    <Package className="w-4 h-4" />
                  ) : (
                    <ShoppingCart className="w-4 h-4" />
                  )}
                  {itemType === 'offer' ? 'Offer' : 'Request'}
                </div>

                <ArrowRightLeft className="w-5 h-5 text-muted-foreground animate-pulse" />

                <div
                  className={cn(
                    'flex items-center gap-2 px-3 py-1.5 rounded-full text-sm font-medium',
                    targetType === 'offer'
                      ? 'bg-emerald/10 text-emerald border border-emerald/30'
                      : 'bg-amber/10 text-amber border border-amber/30',
                  )}
                >
                  {targetType === 'offer' ? (
                    <Package className="w-4 h-4" />
                  ) : (
                    <ShoppingCart className="w-4 h-4" />
                  )}
                  {targetType === 'offer' ? 'Offer' : 'Request'}
                </div>
              </div>

              {/* Notes */}
              <div className="space-y-2">
                <label className="text-sm font-medium text-foreground">
                  Notes (optional)
                </label>
                <Textarea
                  placeholder="Why are you reclassifying this item?"
                  value={notes}
                  onChange={(e) => setNotes(e.target.value)}
                  className="resize-none"
                  rows={2}
                />
              </div>

              {/* Error Message */}
              {mutation.isError && (
                <div className="flex items-center gap-2 p-3 rounded-lg bg-destructive/10 text-destructive text-sm">
                  <AlertCircle className="w-4 h-4 shrink-0" />
                  <p>
                    {mutation.error instanceof Error
                      ? mutation.error.message
                      : 'Failed to reclassify item'}
                  </p>
                </div>
              )}
            </div>

            <DialogFooter className="gap-2 sm:gap-0">
              <Button
                variant="outline"
                onClick={handleClose}
                disabled={mutation.isPending}
              >
                Cancel
              </Button>
              <Button
                onClick={handleReclassify}
                disabled={mutation.isPending}
                className={cn(
                  'gap-2',
                  targetType === 'offer'
                    ? 'bg-emerald hover:bg-emerald/90'
                    : 'bg-amber hover:bg-amber/90',
                )}
              >
                {mutation.isPending ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    Reclassifying...
                  </>
                ) : (
                  <>
                    <ArrowRightLeft className="w-4 h-4" />
                    Convert to {targetType === 'offer' ? 'Offer' : 'Request'}
                  </>
                )}
              </Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}
