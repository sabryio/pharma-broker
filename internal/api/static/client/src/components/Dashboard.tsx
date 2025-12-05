import { useState } from 'react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import {
  useOffers,
  useRequests,
  useMatches,
  useStats,
  useConfirmMatch,
  useRejectMatch,
} from '@/lib/api'
import { useSSE } from '@/lib/sse'
import { OfferCard } from './OfferCard'
import { RequestCard } from './RequestCard'
import { MatchCard } from './MatchCard'
import { GroupsModal } from './GroupsModal'
import { AnalyzeModal } from './AnalyzeModal'
import { ConfigPanel } from './ConfigPanel'

export function Dashboard() {
  const [connected, setConnected] = useState(false)
  const [offersQuery, setOffersQuery] = useState('')
  const [requestsQuery, setRequestsQuery] = useState('')

  // SSE for real-time updates
  useSSE(setConnected)

  // Data fetching
  const { data: offers, isLoading: offersLoading } = useOffers(
    offersQuery || undefined,
  )
  const { data: requests, isLoading: requestsLoading } = useRequests(
    requestsQuery || undefined,
  )
  const { data: matches, isLoading: matchesLoading } = useMatches()
  const { data: stats } = useStats()

  // Mutations
  const confirmMatch = useConfirmMatch()
  const rejectMatch = useRejectMatch()

  const handleConfirm = (id: string) => confirmMatch.mutate(id)
  const handleReject = (id: string) => rejectMatch.mutate(id)

  return (
    <div className="flex flex-col h-screen bg-background">
      {/* Header Stats */}
      <header className="border-b p-4">
        <div className="flex items-center justify-between max-w-[1800px] mx-auto">
          <div className="flex items-center gap-3">
            <h1 className="text-2xl font-bold bg-linear-to-r from-primary to-primary/60 bg-clip-text text-transparent">
              💊 PharmaBroker
            </h1>
            <span className="text-muted-foreground">Dashboard</span>
          </div>

          <div className="flex items-center gap-8">
            <div className="flex gap-8">
              <StatItem label="Offers" value={stats?.active_offers ?? '-'} />
              <StatItem
                label="Requests"
                value={stats?.active_requests ?? '-'}
              />
              <StatItem
                label="Pending"
                value={stats?.pending_matches ?? '-'}
                highlight="warning"
              />
              <StatItem
                label="Today"
                value={stats?.confirmed_today ?? '-'}
                highlight="success"
              />
            </div>

            <div
              className={`flex items-center gap-2 px-4 py-2 rounded-full bg-secondary ${connected ? 'text-green-500' : 'text-yellow-500'}`}
            >
              <span
                className={`w-2 h-2 rounded-full ${connected ? 'bg-green-500' : 'bg-yellow-500 animate-pulse'}`}
              />
              <span className="text-sm">
                {connected ? 'Connected' : 'Connecting...'}
              </span>
            </div>
          </div>
        </div>
      </header>

      {/* Main Grid */}
      <main className="flex-1 grid grid-cols-3 gap-4 p-4 overflow-hidden max-w-[1800px] mx-auto w-full">
        {/* Offers Panel */}
        <Card className="flex flex-col overflow-hidden">
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between">
              <CardTitle className="text-base">📦 Offers</CardTitle>
              <Input
                placeholder="Search..."
                value={offersQuery}
                onChange={(e) => setOffersQuery(e.target.value)}
                className="w-40 h-8"
              />
            </div>
          </CardHeader>
          <CardContent className="flex-1 overflow-y-auto space-y-2">
            {offersLoading && (
              <p className="text-muted-foreground text-center py-8">
                Loading...
              </p>
            )}
            {!offersLoading && (!offers || offers.length === 0) && (
              <p className="text-muted-foreground text-center py-8">
                No active offers
              </p>
            )}
            {offers?.map((offer) => (
              <OfferCard key={offer.id} offer={offer} />
            ))}
          </CardContent>
        </Card>

        {/* Matches Panel */}
        <Card className="flex flex-col overflow-hidden">
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between">
              <CardTitle className="text-base">🎯 Suggested Matches</CardTitle>
              <Badge variant="secondary">{matches?.length ?? 0}</Badge>
            </div>
          </CardHeader>
          <CardContent className="flex-1 overflow-y-auto">
            {matchesLoading && (
              <p className="text-muted-foreground text-center py-8">
                Loading...
              </p>
            )}
            {!matchesLoading && (!matches || matches.length === 0) && (
              <p className="text-muted-foreground text-center py-8">
                No pending matches
              </p>
            )}
            {matches?.map((match) => (
              <MatchCard
                key={match.id}
                match={match}
                onConfirm={handleConfirm}
                onReject={handleReject}
                isLoading={confirmMatch.isPending || rejectMatch.isPending}
              />
            ))}
          </CardContent>
        </Card>

        {/* Requests Panel */}
        <Card className="flex flex-col overflow-hidden">
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between">
              <CardTitle className="text-base">🔍 Requests</CardTitle>
              <Input
                placeholder="Search..."
                value={requestsQuery}
                onChange={(e) => setRequestsQuery(e.target.value)}
                className="w-40 h-8"
              />
            </div>
          </CardHeader>
          <CardContent className="flex-1 overflow-y-auto space-y-2">
            {requestsLoading && (
              <p className="text-muted-foreground text-center py-8">
                Loading...
              </p>
            )}
            {!requestsLoading && (!requests || requests.length === 0) && (
              <p className="text-muted-foreground text-center py-8">
                No active requests
              </p>
            )}
            {requests?.map((request) => (
              <RequestCard key={request.id} request={request} />
            ))}
          </CardContent>
        </Card>
      </main>

      {/* FAB Modals */}
      <GroupsModal />
      <AnalyzeModal />
      <ConfigPanel />
    </div>
  )
}

function StatItem({
  label,
  value,
  highlight,
}: {
  label: string
  value: string | number
  highlight?: 'warning' | 'success'
}) {
  const colorClass =
    highlight === 'success'
      ? 'text-green-500'
      : highlight === 'warning'
        ? 'text-yellow-500'
        : ''
  return (
    <div className="flex flex-col items-center">
      <span className={`text-2xl font-bold ${colorClass}`}>{value}</span>
      <span className="text-xs text-muted-foreground uppercase tracking-wide">
        {label}
      </span>
    </div>
  )
}
