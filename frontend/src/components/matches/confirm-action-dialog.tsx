// Confirm Action Dialog Component
// Displays a confirmation dialog for approve/reject actions on matches

import { useState } from 'react'
import { CheckCircle, XCircle, Loader2 } from 'lucide-react'
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
import type { MatchReviewItem } from '@/schema/match-review'
import { cn } from '@/lib/utils'

const MAX_REASON_LENGTH = 500

export interface ConfirmActionDialogProps {
  isOpen: boolean
  onClose: () => void
  onConfirm: (reason?: string) => void
  actionType: 'approve' | 'reject'
  match: MatchReviewItem
  isLoading: boolean
}

/**
 * Confirmation dialog for match approve/reject actions.
 * Supports Escape key and click-outside dismissal.
 * Shows optional rejection reason field for reject actions.
 */
export function ConfirmActionDialog({
  isOpen,
  onClose,
  onConfirm,
  actionType,
  match,
  isLoading,
}: ConfirmActionDialogProps) {
  const [reason, setReason] = useState('')

  const isApprove = actionType === 'approve'
  const actionVerb = isApprove ? 'Approve' : 'Reject'
  const offerProduct = match.offer.product
  const requestProduct = match.request.product

  const handleConfirm = () => {
    onConfirm(
      actionType === 'reject' && reason.trim() ? reason.trim() : undefined,
    )
  }

  const handleOpenChange = (open: boolean) => {
    if (!open && !isLoading) {
      setReason('')
      onClose()
    }
  }

  const handleReasonChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const value = e.target.value
    if (value.length <= MAX_REASON_LENGTH) {
      setReason(value)
    }
  }

  return (
    <Dialog open={isOpen} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            {isApprove ? (
              <CheckCircle className="w-5 h-5 text-emerald" />
            ) : (
              <XCircle className="w-5 h-5 text-red-400" />
            )}
            {actionVerb} this match?
          </DialogTitle>
          <DialogDescription className="space-y-2">
            <span className="block">
              {actionVerb} this match for{' '}
              <span className="font-medium text-foreground">
                {offerProduct}
              </span>
              ?
            </span>
            <span className="block text-xs">
              <span className="text-muted-foreground">Offer:</span>{' '}
              <span className="text-foreground">{offerProduct}</span>
              <span className="text-muted-foreground mx-2">→</span>
              <span className="text-muted-foreground">Request:</span>{' '}
              <span className="text-foreground">{requestProduct}</span>
            </span>
          </DialogDescription>
        </DialogHeader>

        {/* Rejection reason field - only shown for reject action */}
        {actionType === 'reject' && (
          <div className="space-y-2">
            <label
              htmlFor="rejection-reason"
              className="text-sm font-medium text-foreground"
            >
              Rejection reason{' '}
              <span className="text-muted-foreground font-normal">
                (optional)
              </span>
            </label>
            <Textarea
              id="rejection-reason"
              placeholder="Enter a reason for rejection..."
              value={reason}
              onChange={handleReasonChange}
              disabled={isLoading}
              className="min-h-[80px] resize-none"
            />
            <div className="flex justify-end">
              <span
                className={cn(
                  'text-xs',
                  reason.length >= MAX_REASON_LENGTH
                    ? 'text-red-400'
                    : 'text-muted-foreground',
                )}
              >
                {reason.length}/{MAX_REASON_LENGTH}
              </span>
            </div>
          </div>
        )}

        <DialogFooter className="gap-2 sm:gap-2">
          <Button variant="outline" onClick={onClose} disabled={isLoading}>
            Cancel
          </Button>
          <Button
            variant={isApprove ? 'default' : 'destructive'}
            onClick={handleConfirm}
            disabled={isLoading}
            className={cn(
              isApprove && 'bg-emerald hover:bg-emerald/90 text-white',
            )}
          >
            {isLoading ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" />
                {actionVerb}ing...
              </>
            ) : (
              <>
                {isApprove ? (
                  <CheckCircle className="w-4 h-4" />
                ) : (
                  <XCircle className="w-4 h-4" />
                )}
                {actionVerb}
              </>
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
