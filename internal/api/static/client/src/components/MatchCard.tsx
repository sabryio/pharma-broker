import { Card, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import type { Match } from '@/lib/types'

interface MatchCardProps {
  match: Match
  onConfirm: (id: string) => void
  onReject: (id: string) => void
  isLoading?: boolean
}

export function MatchCard({
  match,
  onConfirm,
  onReject,
  isLoading,
}: MatchCardProps) {
  const offer = match.offer
  const request = match.request
  const scorePercent = Math.round(match.score * 100)

  return (
    <Card className="mb-3">
      <CardContent className="p-4">
        <div className="flex items-center gap-2 mb-3">
          <Progress value={scorePercent} className="flex-1 h-2" />
          <span className="text-sm font-semibold text-primary">
            {scorePercent}%
          </span>
        </div>

        <div className="grid grid-cols-[1fr_auto_1fr] gap-3 items-center">
          <div className="bg-secondary p-3 rounded-lg">
            <p className="text-xs text-muted-foreground uppercase tracking-wide mb-1">
              Offer
            </p>
            {offer?.group_name && (
              <p className="text-[10px] text-muted-foreground mb-1">
                📍 {offer.group_name}
              </p>
            )}
            <p className="font-semibold text-sm">
              {offer?.medication || 'Unknown'}
            </p>
            <p className="text-xs text-muted-foreground">
              {offer?.quantity || '?'} {offer?.unit || ''} @{' '}
              {offer?.price || '?'} EGP
            </p>
            <p className="text-xs text-muted-foreground mt-1">
              {offer?.source_name || 'Unknown'}
            </p>
            {offer?.source_phone && (
              <p className="text-[10px] font-mono text-muted-foreground">
                {offer.source_phone}
              </p>
            )}
          </div>

          <span className="text-2xl text-muted-foreground">→</span>

          <div className="bg-secondary p-3 rounded-lg">
            <p className="text-xs text-muted-foreground uppercase tracking-wide mb-1">
              Request
            </p>
            {request?.group_name && (
              <p className="text-[10px] text-muted-foreground mb-1">
                📍 {request.group_name}
              </p>
            )}
            <p className="font-semibold text-sm">
              {request?.medication || 'Unknown'}
            </p>
            <p className="text-xs text-muted-foreground">
              {request?.quantity || '?'} {request?.unit || ''}{' '}
              {request?.max_price ? `max ${request.max_price} EGP` : ''}
            </p>
            <p className="text-xs text-muted-foreground mt-1">
              {request?.source_name || 'Unknown'}
            </p>
            {request?.source_phone && (
              <p className="text-[10px] font-mono text-muted-foreground">
                {request.source_phone}
              </p>
            )}
          </div>
        </div>

        <div className="flex gap-2 mt-4">
          <Button
            variant="outline"
            className="flex-1"
            onClick={() => onReject(match.id)}
            disabled={isLoading}
          >
            Reject
          </Button>
          <Button
            className="flex-1 bg-green-600 hover:bg-green-700"
            onClick={() => onConfirm(match.id)}
            disabled={isLoading}
          >
            Confirm Match
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
