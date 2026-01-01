import { CheckCircle, X } from 'lucide-react'

interface ReviewActionsProps {
  onApprove: () => void
  onReject: () => void
  loading?: boolean
}

export function ReviewActions({
  onApprove,
  onReject,
  loading = false,
}: ReviewActionsProps) {
  return (
    <div className="flex items-center justify-center gap-4 mt-8 pt-6 border-t border-border">
      <button
        onClick={onApprove}
        disabled={loading}
        className="flex items-center gap-2 px-8 py-3 rounded-lg bg-emerald text-primary-foreground font-semibold hover:bg-emerald/90 transition-all hover:scale-105 glow-emerald disabled:opacity-50 disabled:cursor-not-allowed"
      >
        <CheckCircle className="w-5 h-5" />
        Approve Match
      </button>
      <button
        onClick={onReject}
        disabled={loading}
        className="flex items-center gap-2 px-8 py-3 rounded-lg bg-destructive/20 text-destructive border border-destructive/50 font-semibold hover:bg-destructive/30 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
      >
        <X className="w-5 h-5" />
        Reject
      </button>
    </div>
  )
}
