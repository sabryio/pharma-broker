// Matches List Component
// Renders a list of MatchCard components with selection and expansion state

import { useState, useCallback } from 'react'
import type { MatchReviewItem } from '@/schema/match-review'
import { MatchCard } from './match-card'

interface MatchesListProps {
  matches: MatchReviewItem[]
  onApprove: (matchId: string, productName: string) => void
  onReject: (matchId: string, productName: string) => void
  actionInProgress: { id: string; action: 'approve' | 'reject' } | null
}

export function MatchesList({
  matches,
  onApprove,
  onReject,
  actionInProgress,
}: MatchesListProps) {
  const [selectedMatchId, setSelectedMatchId] = useState<string | null>(null)
  const [expandedMatchId, setExpandedMatchId] = useState<string | null>(null)

  const handleSelect = useCallback((matchId: string) => {
    setSelectedMatchId(matchId)
  }, [])

  const handleExpand = useCallback((matchId: string) => {
    setExpandedMatchId((prev) => (prev === matchId ? null : matchId))
  }, [])

  if (matches.length === 0) {
    return (
      <div className="glass-card p-8 rounded-xl text-center">
        <p className="text-muted-foreground">
          No matches found with current filters.
        </p>
      </div>
    )
  }

  return (
    <div className="space-y-3">
      {matches.map((match) => (
        <MatchCard
          key={match.id}
          match={match}
          isSelected={selectedMatchId === match.id}
          isExpanded={expandedMatchId === match.id}
          onSelect={() => handleSelect(match.id)}
          onExpand={() => handleExpand(match.id)}
          onApprove={() => onApprove(match.id, match.offer.product)}
          onReject={() => onReject(match.id, match.offer.product)}
          isApproving={
            actionInProgress?.id === match.id &&
            actionInProgress?.action === 'approve'
          }
          isRejecting={
            actionInProgress?.id === match.id &&
            actionInProgress?.action === 'reject'
          }
        />
      ))}
    </div>
  )
}
