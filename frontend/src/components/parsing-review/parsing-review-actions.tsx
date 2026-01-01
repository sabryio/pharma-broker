import { CheckCircle, X, SkipForward, Edit3 } from 'lucide-react'

interface ParsingReviewActionsProps {
  onApprove: () => void
  onReject: () => void
  onSkip: () => void
  onCorrect?: () => void
  loading?: boolean
}

export function ParsingReviewActions({
  onApprove,
  onReject,
  onSkip,
  onCorrect,
  loading = false,
}: ParsingReviewActionsProps) {
  return (
    <div className="flex flex-col gap-3">
      {/* Primary Actions */}
      <div className="flex items-center gap-3">
        <button
          onClick={onApprove}
          disabled={loading}
          className="flex-1 flex items-center justify-center gap-2 px-6 py-3 rounded-lg bg-purple-500 text-white font-semibold hover:bg-purple-600 transition-all hover:scale-[1.02] disabled:opacity-50 disabled:cursor-not-allowed shadow-lg shadow-purple-500/25"
        >
          <CheckCircle className="w-5 h-5" />
          Approve
        </button>
        <button
          onClick={onReject}
          disabled={loading}
          className="flex-1 flex items-center justify-center gap-2 px-6 py-3 rounded-lg bg-destructive/20 text-destructive border border-destructive/50 font-semibold hover:bg-destructive/30 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <X className="w-5 h-5" />
          Reject
        </button>
      </div>

      {/* Secondary Actions */}
      <div className="flex items-center gap-3">
        <button
          onClick={onSkip}
          disabled={loading}
          className="flex-1 flex items-center justify-center gap-2 px-4 py-2 rounded-lg bg-secondary text-muted-foreground hover:text-foreground hover:bg-secondary/80 transition-all text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <SkipForward className="w-4 h-4" />
          Skip for Later
        </button>
        {onCorrect && (
          <button
            onClick={onCorrect}
            disabled={loading}
            className="flex-1 flex items-center justify-center gap-2 px-4 py-2 rounded-lg bg-amber/20 text-amber border border-amber/30 hover:bg-amber/30 transition-all text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Edit3 className="w-4 h-4" />
            Correct & Approve
          </button>
        )}
      </div>
    </div>
  )
}
