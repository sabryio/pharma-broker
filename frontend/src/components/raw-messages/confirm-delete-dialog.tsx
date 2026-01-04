/**
 * Confirmation Dialog for Delete Operations
 * Reusable dialog for single and bulk delete confirmations
 */

import { AlertTriangle, Loader2, Trash2 } from 'lucide-react'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { cn } from '@/lib/utils'

export interface ConfirmDeleteDialogProps {
  /** Whether the dialog is open */
  open: boolean
  /** Callback when dialog open state changes */
  onOpenChange: (open: boolean) => void
  /** Callback when delete is confirmed */
  onConfirm: () => void
  /** Title of the dialog */
  title: string
  /** Description/warning message */
  description: string
  /** Whether the delete operation is in progress */
  isLoading?: boolean
  /** Number of items being deleted (for bulk operations) */
  itemCount?: number
}

/**
 * Confirmation dialog for destructive delete operations.
 * Supports both single and bulk delete scenarios.
 */
export function ConfirmDeleteDialog({
  open,
  onOpenChange,
  onConfirm,
  title,
  description,
  isLoading = false,
  itemCount,
}: ConfirmDeleteDialogProps) {
  const handleConfirm = () => {
    onConfirm()
  }

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle className="flex items-center gap-2 text-destructive">
            <AlertTriangle className="h-5 w-5" />
            {title}
          </AlertDialogTitle>
          <AlertDialogDescription className="space-y-2">
            <span className="block">{description}</span>
            {itemCount && itemCount > 1 && (
              <span className="block text-sm font-medium text-foreground">
                {itemCount} items will be deleted
              </span>
            )}
            <span className="block text-xs text-muted-foreground">
              This action cannot be undone.
            </span>
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel disabled={isLoading}>Cancel</AlertDialogCancel>
          <AlertDialogAction
            onClick={handleConfirm}
            disabled={isLoading}
            className={cn(
              'bg-destructive text-destructive-foreground hover:bg-destructive/90',
            )}
          >
            {isLoading ? (
              <>
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                Deleting...
              </>
            ) : (
              <>
                <Trash2 className="mr-2 h-4 w-4" />
                Delete
              </>
            )}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
