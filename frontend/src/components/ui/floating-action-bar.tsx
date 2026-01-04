// Floating Action Bar Component
// Reusable floating action bar for bulk operations with selection count

import { useState, useCallback, useEffect } from 'react'
import { cn } from '@/lib/utils'
import {
  X,
  ChevronUp,
  ChevronDown,
  Keyboard,
  CheckSquare,
  Square,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'

export interface ActionConfig {
  id: string
  label: string
  icon: React.ReactNode
  variant: 'default' | 'destructive' | 'success' | 'warning'
  shortcut?: string
  onClick: () => void
  disabled?: boolean
}

interface FloatingActionBarProps {
  selectedCount: number
  totalCount: number
  actions: ActionConfig[]
  onSelectAll?: () => void
  onClearSelection?: () => void
  isAllSelected?: boolean
  loading?: boolean
  position?: 'bottom' | 'top'
  showKeyboardHints?: boolean
  className?: string
}

type ActionState = 'idle' | 'processing' | 'success' | 'error'

const variantStyles = {
  default: 'bg-secondary/80 hover:bg-secondary text-foreground border-border/50',
  destructive:
    'bg-gradient-to-r from-red-500/20 to-rose-500/20 border-red-500/30 hover:border-red-500/60 text-red-400 hover:text-red-300',
  success:
    'bg-gradient-to-r from-emerald-500/20 to-teal-500/20 border-emerald-500/30 hover:border-emerald-500/60 text-emerald-400 hover:text-emerald-300 shadow-lg shadow-emerald-500/10',
  warning:
    'bg-gradient-to-r from-amber-500/20 to-yellow-500/20 border-amber-500/30 hover:border-amber-500/60 text-amber-400 hover:text-amber-300',
}

export function FloatingActionBar({
  selectedCount,
  totalCount,
  actions,
  onSelectAll,
  onClearSelection,
  isAllSelected = false,
  loading = false,
  position = 'bottom',
  showKeyboardHints = true,
  className,
}: FloatingActionBarProps) {
  const [isMinimized, setIsMinimized] = useState(false)
  const [actionStates, setActionStates] = useState<Record<string, ActionState>>({})

  // Reset states when selection changes
  useEffect(() => {
    setActionStates({})
  }, [selectedCount])

  const handleAction = useCallback(
    (action: ActionConfig) => {
      if (loading || actionStates[action.id] === 'processing') return

      setActionStates((prev) => ({ ...prev, [action.id]: 'processing' }))

      // Animate then execute
      setTimeout(() => {
        setActionStates((prev) => ({ ...prev, [action.id]: 'success' }))
        action.onClick()
        // Reset after animation
        setTimeout(() => {
          setActionStates((prev) => ({ ...prev, [action.id]: 'idle' }))
        }, 300)
      }, 200)
    },
    [loading, actionStates],
  )

  // Don't show if nothing selected
  if (selectedCount === 0) return null

  if (isMinimized) {
    return (
      <button
        onClick={() => setIsMinimized(false)}
        className={cn(
          'fixed z-50',
          position === 'bottom' ? 'bottom-6' : 'top-20',
          'left-1/2 -translate-x-1/2',
          'flex items-center gap-2 px-4 py-2 rounded-full',
          'bg-secondary/80 backdrop-blur-xl border border-border/50',
          'text-muted-foreground hover:text-foreground transition-all',
          'shadow-lg hover:shadow-xl',
        )}
      >
        {position === 'bottom' ? (
          <ChevronUp className="w-4 h-4" />
        ) : (
          <ChevronDown className="w-4 h-4" />
        )}
        <span className="text-sm font-medium">
          {selectedCount} selected
        </span>
      </button>
    )
  }

  return (
    <div
      className={cn(
        'fixed z-50 left-1/2 -translate-x-1/2',
        position === 'bottom' ? 'bottom-6' : 'top-20',
        'animate-in slide-in-from-bottom-4 duration-300',
        className,
      )}
    >
      <div
        className={cn(
          'flex items-center gap-2 p-2 rounded-xl',
          'bg-gradient-to-r from-slate-900/95 via-slate-800/95 to-slate-900/95',
          'backdrop-blur-xl border border-white/10',
          'shadow-2xl shadow-black/50',
        )}
      >
        {/* Minimize button */}
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8 text-muted-foreground hover:text-foreground"
              onClick={() => setIsMinimized(true)}
            >
              {position === 'bottom' ? (
                <ChevronDown className="w-4 h-4" />
              ) : (
                <ChevronUp className="w-4 h-4" />
              )}
            </Button>
          </TooltipTrigger>
          <TooltipContent>Minimize</TooltipContent>
        </Tooltip>

        {/* Selection info */}
        <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-white/5">
          <span className="text-sm font-medium text-foreground tabular-nums">
            {selectedCount}
          </span>
          <span className="text-xs text-muted-foreground">
            of {totalCount} selected
          </span>
        </div>

        {/* Select All / Clear */}
        {(onSelectAll || onClearSelection) && (
          <>
            <div className="w-px h-6 bg-white/10" />
            <div className="flex items-center gap-1">
              {onSelectAll && !isAllSelected && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-8 px-2 text-xs"
                      onClick={onSelectAll}
                    >
                      <CheckSquare className="w-3.5 h-3.5 mr-1" />
                      All
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>Select all</TooltipContent>
                </Tooltip>
              )}
              {onClearSelection && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-8 px-2 text-xs"
                      onClick={onClearSelection}
                    >
                      <Square className="w-3.5 h-3.5 mr-1" />
                      Clear
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>Clear selection</TooltipContent>
                </Tooltip>
              )}
            </div>
          </>
        )}

        {/* Divider */}
        <div className="w-px h-6 bg-white/10" />

        {/* Actions */}
        {actions.map((action) => {
          const state = actionStates[action.id] || 'idle'
          const isProcessing = state === 'processing'

          return (
            <Tooltip key={action.id}>
              <TooltipTrigger asChild>
                <button
                  onClick={() => handleAction(action)}
                  disabled={loading || action.disabled || isProcessing}
                  className={cn(
                    'relative flex items-center gap-2 px-4 py-2 rounded-lg transition-all duration-200',
                    'text-sm font-medium overflow-hidden border',
                    'hover:scale-105 active:scale-95',
                    'disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100',
                    variantStyles[action.variant],
                    isProcessing && 'scale-95 opacity-70',
                    state === 'success' && 'scale-100 opacity-100',
                  )}
                >
                  {/* Ripple effect */}
                  {isProcessing && (
                    <span
                      className={cn(
                        'absolute inset-0 animate-ping rounded-lg',
                        action.variant === 'destructive' && 'bg-red-500/30',
                        action.variant === 'success' && 'bg-emerald-500/30',
                        action.variant === 'warning' && 'bg-amber-500/30',
                        action.variant === 'default' && 'bg-white/10',
                      )}
                    />
                  )}
                  <span
                    className={cn(
                      'w-4 h-4',
                      state === 'success' && 'animate-bounce',
                    )}
                  >
                    {action.icon}
                  </span>
                  <span>{action.label}</span>
                  {showKeyboardHints && action.shortcut && (
                    <kbd className="hidden sm:inline px-1.5 py-0.5 rounded bg-black/30 text-[10px] font-mono ml-1">
                      {action.shortcut}
                    </kbd>
                  )}
                </button>
              </TooltipTrigger>
              <TooltipContent>{action.label}</TooltipContent>
            </Tooltip>
          )
        })}

        {/* Keyboard hints indicator */}
        {showKeyboardHints && (
          <>
            <div className="w-px h-6 bg-white/10" />
            <div className="flex items-center gap-1.5 px-2 py-1.5 rounded-lg bg-white/5">
              <Keyboard className="w-3.5 h-3.5 text-muted-foreground" />
            </div>
          </>
        )}

        {/* Close button */}
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8 text-muted-foreground hover:text-foreground"
              onClick={onClearSelection}
            >
              <X className="w-4 h-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Close</TooltipContent>
        </Tooltip>
      </div>
    </div>
  )
}
