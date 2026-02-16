import { createFileRoute } from '@tanstack/react-router'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'
import {
  AlertTriangle,
  Keyboard,
  Loader2,
  RefreshCw,
  Sparkles,
  WifiOff,
} from 'lucide-react'

import { DashboardLayout } from '@/components/layout/dashboard-layout'
import {
  StatsDashboard,
  FilterPanel,
  defaultFilterState,
  type FilterState,
  applyFiltersAndSorting,
  MatchCard,
  ConfirmActionDialog,
  showUndoToast,
} from '@/components/matches'
import {
  useMatchReviewsManual,
  useMatchReviewStats,
} from '@/hooks/use-match-reviews'
import { useMatchAction } from '@/hooks/use-match-action'
import { useMatchWebSocket } from '@/hooks/use-match-websocket'
import type { MatchReviewItem } from '@/schema/match-review'

export const Route = createFileRoute('/matches')({
  component: MatchesPage,
})

export default function MatchesPage() {
  const [filters, setFilters] = useState<FilterState>(defaultFilterState)
  const [selectedMatchId, setSelectedMatchId] = useState<string | null>(null)
  const [expandedMatchId, setExpandedMatchId] = useState<string | null>(null)
  const [focusedIndex, setFocusedIndex] = useState(0)

  // Confirmation dialog state
  const [dialogState, setDialogState] = useState<{
    isOpen: boolean
    actionType: 'approve' | 'reject'
    match: MatchReviewItem | null
  }>({
    isOpen: false,
    actionType: 'approve',
    match: null,
  })

  // Use manual hook to bypass Redux filters - we handle filtering client-side
  const {
    data: matchData,
    isLoading,
    error,
    refetch,
  } = useMatchReviewsManual({ limit: 100 })

  const { data: stats } = useMatchReviewStats()

  // Use the new match action hook with debouncing and undo support
  const { executeAction, undoAction, isProcessing, getUndoState } =
    useMatchAction({
      onSuccess: (matchId, action) => {
        const undoState = getUndoState(matchId)
        if (undoState) {
          // Show undo toast
          showUndoToast({
            matchId,
            productName: undoState.productName,
            action,
            onUndo: () => undoAction(matchId),
            duration: 8000,
          })
        }
        // Collapse the card after action
        if (expandedMatchId === matchId) {
          setExpandedMatchId(null)
        }
      },
      onError: (error) => {
        toast.error('Action failed', {
          description: error.message,
        })
      },
    })

  // WebSocket for real-time updates from other users
  const { isConnected: wsConnected } = useMatchWebSocket({
    onMatchUpdated: useCallback(
      (matchId: string, newStatus: string, _byUserId: string) => {
        // Refetch data when another user updates a match
        refetch()

        // Show notification for remote changes (don't show for own actions)
        toast.info(`Match ${newStatus.toLowerCase()}`, {
          description: `Updated by another user`,
          duration: 3000,
        })

        // If the updated match is currently expanded, show inline notification
        if (expandedMatchId === matchId) {
          toast.info('This match was updated', {
            description: `Status changed to ${newStatus} by another user`,
          })
        }
      },
      [refetch, expandedMatchId],
    ),
    onConnectionChange: useCallback((connected: boolean) => {
      if (!connected) {
        console.log('WebSocket disconnected, will auto-reconnect')
      }
    }, []),
  })

  const matches = matchData?.items ?? []

  // Apply filters and sorting using utility functions
  const filteredMatches = useMemo(
    () => applyFiltersAndSorting(matches, filters),
    [matches, filters],
  )

  // Keep focused index in bounds when filtered matches change
  useEffect(() => {
    if (focusedIndex >= filteredMatches.length && filteredMatches.length > 0) {
      setFocusedIndex(filteredMatches.length - 1)
    }
  }, [filteredMatches.length, focusedIndex])

  // Update selected match when focused index changes
  useEffect(() => {
    if (filteredMatches.length > 0 && focusedIndex < filteredMatches.length) {
      const match = filteredMatches[focusedIndex]
      if (match) {
        setSelectedMatchId(match.id)
      }
    }
  }, [focusedIndex, filteredMatches])

  // Keyboard navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Don't handle if user is typing in an input
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement ||
        e.target instanceof HTMLSelectElement
      ) {
        return
      }

      switch (e.key) {
        case 'ArrowUp':
        case 'k':
          e.preventDefault()
          setFocusedIndex((prev) => Math.max(0, prev - 1))
          break
        case 'ArrowDown':
        case 'j':
          e.preventDefault()
          setFocusedIndex((prev) =>
            Math.min(filteredMatches.length - 1, prev + 1),
          )
          break
        case 'Enter':
          e.preventDefault()
          if (filteredMatches.length > 0) {
            const match = filteredMatches[focusedIndex]
            if (match) {
              setExpandedMatchId((prev) =>
                prev === match.id ? null : match.id,
              )
            }
          }
          break
        case 'Escape':
          e.preventDefault()
          setExpandedMatchId(null)
          setDialogState({ isOpen: false, actionType: 'approve', match: null })
          break
        case 'Home':
          e.preventDefault()
          setFocusedIndex(0)
          break
        case 'End':
          e.preventDefault()
          setFocusedIndex(Math.max(0, filteredMatches.length - 1))
          break
        case 'a':
          // Approve shortcut - only works when a match is selected and is PENDING
          e.preventDefault()
          if (filteredMatches.length > 0) {
            const match = filteredMatches[focusedIndex]
            if (match && match.status === 'PENDING') {
              setDialogState({
                isOpen: true,
                actionType: 'approve',
                match,
              })
            }
          }
          break
        case 'r':
          // Reject shortcut - only works when a match is selected and is PENDING
          e.preventDefault()
          if (filteredMatches.length > 0) {
            const match = filteredMatches[focusedIndex]
            if (match && match.status === 'PENDING') {
              setDialogState({
                isOpen: true,
                actionType: 'reject',
                match,
              })
            }
          }
          break
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [filteredMatches, focusedIndex])

  // Handle approve action - opens confirmation dialog
  const handleApprove = useCallback((match: MatchReviewItem) => {
    setDialogState({
      isOpen: true,
      actionType: 'approve',
      match,
    })
  }, [])

  // Handle reject action - opens confirmation dialog
  const handleReject = useCallback((match: MatchReviewItem) => {
    setDialogState({
      isOpen: true,
      actionType: 'reject',
      match,
    })
  }, [])

  // Handle dialog confirmation
  const handleDialogConfirm = useCallback(
    (reason?: string) => {
      if (!dialogState.match) return

      const { match, actionType } = dialogState
      const action = actionType === 'approve' ? 'approved' : 'rejected'

      executeAction(match.id, action, reason)
      setDialogState({ isOpen: false, actionType: 'approve', match: null })
    },
    [dialogState, executeAction],
  )

  // Handle dialog close
  const handleDialogClose = useCallback(() => {
    setDialogState({ isOpen: false, actionType: 'approve', match: null })
  }, [])

  // Loading state
  if (isLoading) {
    return (
      <DashboardLayout>
        <div className="flex items-center justify-center min-h-[400px]">
          <div className="flex flex-col items-center gap-4">
            <Loader2 className="w-8 h-8 text-teal animate-spin" />
            <p className="text-muted-foreground">Loading matches...</p>
          </div>
        </div>
      </DashboardLayout>
    )
  }

  // Error state
  if (error) {
    return (
      <DashboardLayout>
        <div className="flex items-center justify-center min-h-[400px]">
          <div className="flex flex-col items-center gap-4 text-center">
            <div className="p-4 rounded-full bg-red-500/10">
              <AlertTriangle className="w-8 h-8 text-red-400" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-foreground mb-1">
                Failed to load matches
              </h2>
              <p className="text-muted-foreground text-sm mb-4">
                {error.message}
              </p>
              <button
                onClick={() => refetch()}
                className="flex items-center gap-2 px-4 py-2 rounded-lg bg-teal hover:bg-teal/80 text-white transition-colors mx-auto"
              >
                <RefreshCw className="w-4 h-4" />
                Retry
              </button>
            </div>
          </div>
        </div>
      </DashboardLayout>
    )
  }

  // Empty state
  if (matches.length === 0) {
    return (
      <DashboardLayout>
        <div className="space-y-6">
          <div>
            <h1 className="text-2xl font-bold text-foreground">Matches</h1>
            <p className="text-muted-foreground">
              View and manage offer-request matches
            </p>
          </div>
          <div className="glass-card-enhanced p-12 rounded-2xl text-center">
            <Sparkles className="w-16 h-16 text-teal/50 mx-auto mb-4" />
            <h2 className="text-xl font-semibold text-foreground mb-2">
              No Matches Found
            </h2>
            <p className="text-muted-foreground">
              There are no matches to display at this time.
            </p>
            <button
              onClick={() => refetch()}
              className="mt-4 flex items-center gap-2 px-4 py-2 rounded-lg bg-secondary hover:bg-secondary/80 text-foreground transition-colors mx-auto"
            >
              <RefreshCw className="w-4 h-4" />
              Refresh
            </button>
          </div>
        </div>
      </DashboardLayout>
    )
  }

  return (
    <DashboardLayout>
      <div className="space-y-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold text-foreground">Matches</h1>
            <p className="text-muted-foreground">
              View and manage offer-request matches
            </p>
          </div>
          <div className="flex items-center gap-3">
            {/* WebSocket Connection Status */}
            {!wsConnected && (
              <div className="flex items-center gap-1.5 px-2 py-1 rounded-lg bg-amber/10 text-amber text-xs">
                <WifiOff className="w-3.5 h-3.5" />
                <span>Reconnecting...</span>
              </div>
            )}
            {/* Keyboard Shortcuts Hint */}
            <div className="hidden lg:flex items-center gap-2 px-3 py-1.5 rounded-lg bg-secondary/50 text-xs text-muted-foreground">
              <Keyboard className="w-3.5 h-3.5" />
              <span>↑↓ Navigate</span>
              <span className="mx-1">|</span>
              <span>Enter Expand</span>
              <span className="mx-1">|</span>
              <span>a Approve</span>
              <span className="mx-1">|</span>
              <span>r Reject</span>
              <span className="mx-1">|</span>
              <span>Esc Close</span>
            </div>
            <button
              onClick={() => refetch()}
              className="flex items-center gap-2 px-4 py-2 rounded-lg bg-secondary hover:bg-secondary/80 text-foreground transition-colors"
            >
              <RefreshCw className="w-4 h-4" />
              Refresh
            </button>
          </div>
        </div>

        {/* Stats Dashboard */}
        <StatsDashboard stats={stats} matches={matches} />

        {/* Filter Panel */}
        <FilterPanel
          filters={filters}
          onFiltersChange={setFilters}
          totalCount={matches.length}
          filteredCount={filteredMatches.length}
        />

        {/* Matches List */}
        <div className="space-y-3">
          {filteredMatches.length === 0 ? (
            <div className="glass-card p-8 rounded-xl text-center">
              <p className="text-muted-foreground">
                No matches found with current filters.
              </p>
            </div>
          ) : (
            filteredMatches.map((match) => (
              <MatchCard
                key={match.id}
                match={match}
                isSelected={selectedMatchId === match.id}
                isExpanded={expandedMatchId === match.id}
                onSelect={() => setSelectedMatchId(match.id)}
                onExpand={() =>
                  setExpandedMatchId(
                    expandedMatchId === match.id ? null : match.id,
                  )
                }
                onApprove={() => handleApprove(match)}
                onReject={() => handleReject(match)}
                isApproving={isProcessing(match.id)}
                isRejecting={isProcessing(match.id)}
              />
            ))
          )}
        </div>

        {/* Confirmation Dialog */}
        {dialogState.match && (
          <ConfirmActionDialog
            isOpen={dialogState.isOpen}
            onClose={handleDialogClose}
            onConfirm={handleDialogConfirm}
            actionType={dialogState.actionType}
            match={dialogState.match}
            isLoading={isProcessing(dialogState.match.id)}
          />
        )}
      </div>
    </DashboardLayout>
  )
}
