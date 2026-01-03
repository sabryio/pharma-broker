// Undo Toast Component
// Displays a toast with countdown timer and undo button after match actions

import { useState, useEffect, useCallback } from 'react'
import { toast } from 'sonner'
import { Undo2, CheckCircle, XCircle } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

export interface UndoToastProps {
  matchId: string
  productName: string
  action: 'approved' | 'rejected'
  onUndo: () => void
  duration: number // milliseconds
}

interface UndoToastContentProps {
  productName: string
  action: 'approved' | 'rejected'
  remainingTime: number
  totalDuration: number
  onUndo: () => void
}

/**
 * Internal component for the toast content with countdown
 */
function UndoToastContent({
  productName,
  action,
  remainingTime,
  totalDuration,
  onUndo,
}: UndoToastContentProps) {
  const isApprove = action === 'approved'
  const actionVerb = isApprove ? 'Approved' : 'Rejected'
  const secondsRemaining = Math.ceil(remainingTime / 1000)
  const progress = (remainingTime / totalDuration) * 100

  return (
    <div className="flex items-center gap-3 w-full">
      <div className="flex-shrink-0">
        {isApprove ? (
          <CheckCircle className="w-5 h-5 text-emerald" />
        ) : (
          <XCircle className="w-5 h-5 text-red-400" />
        )}
      </div>

      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium text-foreground truncate">
          {actionVerb}: {productName}
        </p>
        <div className="mt-1 flex items-center gap-2">
          <div className="flex-1 h-1 bg-secondary rounded-full overflow-hidden">
            <div
              className={cn(
                'h-full transition-all duration-100 ease-linear rounded-full',
                isApprove ? 'bg-emerald' : 'bg-red-400',
              )}
              style={{ width: `${progress}%` }}
            />
          </div>
          <span className="text-xs text-muted-foreground tabular-nums">
            {secondsRemaining}s
          </span>
        </div>
      </div>

      <Button
        variant="outline"
        size="sm"
        onClick={(e) => {
          e.stopPropagation()
          onUndo()
        }}
        className="flex-shrink-0 gap-1.5"
      >
        <Undo2 className="w-3.5 h-3.5" />
        Undo
      </Button>
    </div>
  )
}

/**
 * Shows an undo toast with countdown timer.
 * Returns a function to dismiss the toast early.
 */
export function showUndoToast({
  matchId,
  productName,
  action,
  onUndo,
  duration,
}: UndoToastProps): () => void {
  const toastId = `undo-${matchId}`
  let remainingTime = duration
  let intervalId: NodeJS.Timeout | null = null

  const updateToast = () => {
    toast.custom(
      () => (
        <UndoToastContent
          productName={productName}
          action={action}
          remainingTime={remainingTime}
          totalDuration={duration}
          onUndo={() => {
            if (intervalId) {
              clearInterval(intervalId)
            }
            toast.dismiss(toastId)
            onUndo()
          }}
        />
      ),
      {
        id: toastId,
        duration: Infinity, // We manage duration ourselves
        className: 'w-full max-w-md',
      },
    )
  }

  // Initial render
  updateToast()

  // Update countdown every 100ms for smooth progress bar
  intervalId = setInterval(() => {
    remainingTime -= 100
    if (remainingTime <= 0) {
      if (intervalId) {
        clearInterval(intervalId)
      }
      toast.dismiss(toastId)
    } else {
      updateToast()
    }
  }, 100)

  // Return dismiss function
  return () => {
    if (intervalId) {
      clearInterval(intervalId)
    }
    toast.dismiss(toastId)
  }
}

/**
 * Hook to manage undo toasts for match actions.
 * Automatically shows toast on action and handles cleanup.
 */
export function useUndoToast() {
  const [activeToasts, setActiveToasts] = useState<Map<string, () => void>>(
    new Map(),
  )

  const showToast = useCallback(
    (props: UndoToastProps) => {
      // Dismiss any existing toast for this match
      const existingDismiss = activeToasts.get(props.matchId)
      if (existingDismiss) {
        existingDismiss()
      }

      const dismiss = showUndoToast({
        ...props,
        onUndo: () => {
          setActiveToasts((prev) => {
            const updated = new Map(prev)
            updated.delete(props.matchId)
            return updated
          })
          props.onUndo()
        },
      })

      setActiveToasts((prev) => {
        const updated = new Map(prev)
        updated.set(props.matchId, dismiss)
        return updated
      })

      // Auto-cleanup after duration
      setTimeout(() => {
        setActiveToasts((prev) => {
          const updated = new Map(prev)
          updated.delete(props.matchId)
          return updated
        })
      }, props.duration)
    },
    [activeToasts],
  )

  const dismissToast = useCallback(
    (matchId: string) => {
      const dismiss = activeToasts.get(matchId)
      if (dismiss) {
        dismiss()
        setActiveToasts((prev) => {
          const updated = new Map(prev)
          updated.delete(matchId)
          return updated
        })
      }
    },
    [activeToasts],
  )

  const dismissAll = useCallback(() => {
    activeToasts.forEach((dismiss) => dismiss())
    setActiveToasts(new Map())
  }, [activeToasts])

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      activeToasts.forEach((dismiss) => dismiss())
    }
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  return {
    showToast,
    dismissToast,
    dismissAll,
    hasActiveToast: (matchId: string) => activeToasts.has(matchId),
  }
}
