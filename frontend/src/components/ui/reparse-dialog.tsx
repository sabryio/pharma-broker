'use client'

import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import {
  RefreshCw,
  Package,
  ShoppingCart,
  Loader2,
  CheckCircle2,
  AlertCircle,
  Sparkles,
  Lightbulb,
} from 'lucide-react'
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
import { Input } from '@/components/ui/input'
import { reparseItem, type ItemType } from '@/api/offers'

interface ReparseDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  itemId: string
  itemType: ItemType
  medication: string
  medicationRaw?: string
  onSuccess?: (newMedication: string) => void
}

// Placeholder user ID until auth is implemented
const PLACEHOLDER_USER_ID = '00000000-0000-0000-0000-000000000001'

export function ReparseDialog({
  open,
  onOpenChange,
  itemId,
  itemType,
  medication,
  medicationRaw,
  onSuccess,
}: ReparseDialogProps) {
  const [hint, setHint] = useState('')
  const [showSuccess, setShowSuccess] = useState(false)
  const [result, setResult] = useState<{ newMedication: string; confidence: number } | null>(null)
  const queryClient = useQueryClient()

  const mutation = useMutation({
    mutationFn: () =>
      reparseItem({
        itemId,
        itemType,
        reparsedBy: PLACEHOLDER_USER_ID,
        hint: hint || undefined,
      }),
    onSuccess: (data) => {
      setShowSuccess(true)
      setResult({ newMedication: data.newMedication, confidence: data.aiConfidence })
      // Invalidate queries
      queryClient.invalidateQueries({ queryKey: ['offers'] })
      queryClient.invalidateQueries({ queryKey: ['requests'] })
      queryClient.invalidateQueries({ queryKey: ['match-reviews'] })

      // Call success callback after animation
      setTimeout(() => {
        onSuccess?.(data.newMedication)
        onOpenChange(false)
        setShowSuccess(false)
        setHint('')
        setResult(null)
      }, 2000)
    },
  })

  const handleReparse = () => {
    mutation.mutate()
  }

  const handleClose = () => {
    if (!mutation.isPending) {
      onOpenChange(false)
      setHint('')
      setShowSuccess(false)
      setResult(null)
      mutation.reset()
    }
  }

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <RefreshCw className="w-5 h-5 text-violet-500" />
            Re-parse with AI
          </DialogTitle>
          <DialogDescription>
            Re-analyze this {itemType} with AI to correct the medication identification
          </DialogDescription>
        </DialogHeader>

        {showSuccess && result ? (
          <div className="flex flex-col items-center justify-center py-8 gap-4 animate-in fade-in zoom-in duration-300">
            <div className="relative">
              <CheckCircle2 className="w-16 h-16 text-emerald animate-bounce" />
              <Sparkles className="w-6 h-6 text-violet-500 absolute -top-1 -right-1 animate-pulse" />
            </div>
            <p className="text-lg font-medium text-emerald">
              Successfully Re-parsed!
            </p>
            <div className="text-center space-y-1">
              <p className="text-sm text-muted-foreground">
                New identification:
              </p>
              <p className="text-base font-semibold text-foreground">
                {result.newMedication}
              </p>
              <p className="text-xs text-muted-foreground">
                Confidence: {Math.round(result.confidence * 100)}%
              </p>
            </div>
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
                    <p className="text-xs text-muted-foreground uppercase tracking-wider mb-1">
                      Current Identification
                    </p>
                    <p className="font-medium text-foreground">
                      {medication}
                    </p>
                    {medicationRaw && medicationRaw !== medication && (
                      <p className="text-sm text-muted-foreground mt-1">
                        Raw: {medicationRaw}
                      </p>
                    )}
                  </div>
                </div>
              </div>

              {/* Hint Input */}
              <div className="space-y-2">
                <label className="flex items-center gap-2 text-sm font-medium text-foreground">
                  <Lightbulb className="w-4 h-4 text-amber" />
                  Correction Hint (optional)
                </label>
                <Input
                  placeholder="e.g., Implanon Contraceptive Implant"
                  value={hint}
                  onChange={(e) => setHint(e.target.value)}
                  className="focus:ring-violet-500/30 focus:border-violet-500/50"
                />
                <p className="text-xs text-muted-foreground">
                  Provide the correct medication name to help the AI identify it better
                </p>
              </div>

              {/* Info Box */}
              <div className="flex items-start gap-2 p-3 rounded-lg bg-violet-500/10 border border-violet-500/20 text-sm">
                <Sparkles className="w-4 h-4 text-violet-500 flex-shrink-0 mt-0.5" />
                <p className="text-violet-300">
                  The AI will re-analyze the original message and update the medication identification.
                  {hint && ' Your hint will guide the AI towards the correct identification.'}
                </p>
              </div>

              {/* Error Message */}
              {mutation.isError && (
                <div className="flex items-center gap-2 p-3 rounded-lg bg-destructive/10 text-destructive text-sm">
                  <AlertCircle className="w-4 h-4 flex-shrink-0" />
                  <p>
                    {mutation.error instanceof Error
                      ? mutation.error.message
                      : 'Failed to re-parse item'}
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
                onClick={handleReparse}
                disabled={mutation.isPending}
                className="gap-2 bg-violet-600 hover:bg-violet-500"
              >
                {mutation.isPending ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    Re-parsing...
                  </>
                ) : (
                  <>
                    <RefreshCw className="w-4 h-4" />
                    Re-parse with AI
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
