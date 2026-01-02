// Quick Actions Bar Component
// Floating action bar with keyboard hints and confirmation animations

import { useState, useEffect, useCallback } from 'react'
import { cn } from '@/lib/utils'
import {
  CheckCircle,
  X,
  Undo2,
  Keyboard,
  ChevronUp,
  ChevronDown,
  Zap,
  SkipForward,
} from 'lucide-react'

interface QuickActionsBarProps {
  onApprove: () => void
  onReject: () => void
  onUndo?: () => void
  onSkip?: () => void
  canUndo?: boolean
  loading?: boolean
  matchId?: string
  confidence?: number
  position?: 'bottom' | 'floating'
  showKeyboardHints?: boolean
}

// Confirmation animation state
type ActionState = 'idle' | 'approving' | 'rejecting' | 'approved' | 'rejected'

export function QuickActionsBar({
  onApprove,
  onReject,
  onUndo,
  onSkip,
  canUndo = false,
  loading = false,
  matchId,
  confidence,
  position = 'floating',
  showKeyboardHints = true,
}: QuickActionsBarProps) {
  const [actionState, setActionState] = useState<ActionState>('idle')
  const [isMinimized, setIsMinimized] = useState(false)

  // Reset action state when match changes
  useEffect(() => {
    setActionState('idle')
  }, [matchId])

  const handleApprove = useCallback(() => {
    if (loading || actionState !== 'idle') return
    setActionState('approving')
    
    // Animate then execute
    setTimeout(() => {
      setActionState('approved')
      onApprove()
      // Reset after animation
      setTimeout(() => setActionState('idle'), 300)
    }, 200)
  }, [loading, actionState, onApprove])

  const handleReject = useCallback(() => {
    if (loading || actionState !== 'idle') return
    setActionState('rejecting')
    
    setTimeout(() => {
      setActionState('rejected')
      onReject()
      setTimeout(() => setActionState('idle'), 300)
    }, 200)
  }, [loading, actionState, onReject])

  if (isMinimized) {
    return (
      <button
        onClick={() => setIsMinimized(false)}
        className={cn(
          'fixed bottom-6 left-1/2 -translate-x-1/2 z-50',
          'flex items-center gap-2 px-4 py-2 rounded-full',
          'bg-secondary/80 backdrop-blur-xl border border-border/50',
          'text-muted-foreground hover:text-foreground transition-all',
          'shadow-lg hover:shadow-xl',
        )}
      >
        <ChevronUp className="w-4 h-4" />
        <span className="text-sm font-medium">Show Actions</span>
      </button>
    )
  }

  return (
    <div
      className={cn(
        'z-50 transition-all duration-300',
        position === 'floating' && 'fixed bottom-6 left-1/2 -translate-x-1/2',
        position === 'bottom' && 'w-full',
      )}
    >
      <div
        className={cn(
          'flex items-center gap-3 p-3 rounded-2xl',
          'bg-gradient-to-r from-slate-900/95 via-slate-800/95 to-slate-900/95',
          'backdrop-blur-xl border border-white/10',
          'shadow-2xl shadow-black/50',
          position === 'floating' && 'animate-in slide-in-from-bottom-4 duration-300',
        )}
      >
        {/* Minimize button */}
        {position === 'floating' && (
          <button
            onClick={() => setIsMinimized(true)}
            className="p-2 rounded-lg hover:bg-white/5 text-muted-foreground hover:text-foreground transition-colors"
            title="Minimize"
          >
            <ChevronDown className="w-4 h-4" />
          </button>
        )}

        {/* Undo button */}
        {onUndo && (
          <button
            onClick={onUndo}
            disabled={!canUndo || loading}
            className={cn(
              'flex items-center gap-2 px-4 py-2.5 rounded-xl transition-all duration-200',
              'text-sm font-medium',
              canUndo
                ? 'bg-amber-500/20 text-amber-400 hover:bg-amber-500/30 border border-amber-500/30'
                : 'bg-secondary/30 text-muted-foreground/50 cursor-not-allowed',
            )}
            title="Undo last action (Ctrl+Z)"
          >
            <Undo2 className="w-4 h-4" />
            {showKeyboardHints && (
              <kbd className="hidden sm:inline px-1.5 py-0.5 rounded bg-black/30 text-[10px] font-mono">
                ⌘Z
              </kbd>
            )}
          </button>
        )}

        {/* Divider */}
        <div className="w-px h-8 bg-white/10" />

        {/* Reject button */}
        <button
          onClick={handleReject}
          disabled={loading || actionState !== 'idle'}
          className={cn(
            'relative flex items-center gap-2 px-6 py-3 rounded-xl transition-all duration-200',
            'text-sm font-semibold overflow-hidden',
            'bg-gradient-to-r from-red-500/20 to-rose-500/20',
            'border border-red-500/30 hover:border-red-500/60',
            'text-red-400 hover:text-red-300',
            'hover:scale-105 active:scale-95',
            'disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100',
            actionState === 'rejecting' && 'scale-95 opacity-70',
            actionState === 'rejected' && 'bg-red-500/40 border-red-500/60',
          )}
          title="Reject match (Backspace)"
        >
          {/* Ripple effect */}
          {actionState === 'rejecting' && (
            <span className="absolute inset-0 bg-red-500/30 animate-ping rounded-xl" />
          )}
          <X className={cn('w-5 h-5', actionState === 'rejected' && 'animate-bounce')} />
          <span>Reject</span>
          {showKeyboardHints && (
            <kbd className="hidden sm:inline px-1.5 py-0.5 rounded bg-black/30 text-[10px] font-mono ml-1">
              ⌫
            </kbd>
          )}
        </button>

        {/* Skip button */}
        {onSkip && (
          <button
            onClick={onSkip}
            disabled={loading}
            className={cn(
              'flex items-center gap-2 px-4 py-3 rounded-xl transition-all duration-200',
              'text-sm font-medium',
              'bg-secondary/30 hover:bg-secondary/50',
              'border border-border/30 hover:border-border/50',
              'text-muted-foreground hover:text-foreground',
              'hover:scale-105 active:scale-95',
            )}
            title="Skip to next (→)"
          >
            <SkipForward className="w-4 h-4" />
            {showKeyboardHints && (
              <kbd className="hidden sm:inline px-1.5 py-0.5 rounded bg-black/30 text-[10px] font-mono">
                →
              </kbd>
            )}
          </button>
        )}

        {/* Approve button */}
        <button
          onClick={handleApprove}
          disabled={loading || actionState !== 'idle'}
          className={cn(
            'relative flex items-center gap-2 px-6 py-3 rounded-xl transition-all duration-200',
            'text-sm font-semibold overflow-hidden',
            'bg-gradient-to-r from-emerald-500/20 to-teal-500/20',
            'border border-emerald-500/30 hover:border-emerald-500/60',
            'text-emerald-400 hover:text-emerald-300',
            'hover:scale-105 active:scale-95',
            'shadow-lg shadow-emerald-500/10 hover:shadow-emerald-500/20',
            'disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100',
            actionState === 'approving' && 'scale-95 opacity-70',
            actionState === 'approved' && 'bg-emerald-500/40 border-emerald-500/60',
          )}
          title="Approve match (Enter)"
        >
          {/* Ripple effect */}
          {actionState === 'approving' && (
            <span className="absolute inset-0 bg-emerald-500/30 animate-ping rounded-xl" />
          )}
          <CheckCircle className={cn('w-5 h-5', actionState === 'approved' && 'animate-bounce')} />
          <span>Approve</span>
          {showKeyboardHints && (
            <kbd className="hidden sm:inline px-1.5 py-0.5 rounded bg-black/30 text-[10px] font-mono ml-1">
              ↵
            </kbd>
          )}
        </button>

        {/* Divider */}
        <div className="w-px h-8 bg-white/10" />

        {/* Keyboard hints toggle */}
        <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-white/5">
          <Keyboard className="w-4 h-4 text-muted-foreground" />
          <span className="text-[10px] text-muted-foreground hidden sm:inline">
            Shortcuts enabled
          </span>
        </div>

        {/* Quick confidence indicator */}
        {confidence !== undefined && (
          <div
            className={cn(
              'flex items-center gap-1.5 px-3 py-2 rounded-lg',
              confidence >= 80 && 'bg-emerald-500/10 text-emerald-400',
              confidence >= 50 && confidence < 80 && 'bg-amber-500/10 text-amber-400',
              confidence < 50 && 'bg-red-500/10 text-red-400',
            )}
          >
            <Zap className="w-3.5 h-3.5" />
            <span className="text-xs font-bold">{confidence.toFixed(0)}%</span>
          </div>
        )}
      </div>
    </div>
  )
}
