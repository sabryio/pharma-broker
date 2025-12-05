import { useState, useMemo } from 'react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  useOffers,
  useRequests,
  useMatches,
  useStats,
  useConfirmMatch,
  useRejectMatch,
  useGroups,
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
  const [selectedGroup, setSelectedGroup] = useState<string>('all')

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
  const { data: groups } = useGroups()

  // Filter by group (client-side)
  const filteredOffers = useMemo(() => {
    if (!offers) return []
    if (selectedGroup === 'all') return offers
    return offers.filter((o) => o.source_group === selectedGroup)
  }, [offers, selectedGroup])

  const filteredRequests = useMemo(() => {
    if (!requests) return []
    if (selectedGroup === 'all') return requests
    return requests.filter((r) => r.source_group === selectedGroup)
  }, [requests, selectedGroup])

  const filteredMatches = useMemo(() => {
    if (!matches) return []
    if (selectedGroup === 'all') return matches
    return matches.filter(
      (m) =>
        m.offer?.source_group === selectedGroup ||
        m.request?.source_group === selectedGroup,
    )
  }, [matches, selectedGroup])

  // Mutations
  const confirmMatch = useConfirmMatch()
  const rejectMatch = useRejectMatch()

  const handleConfirm = (id: string) => confirmMatch.mutate(id)
  const handleReject = (id: string) => rejectMatch.mutate(id)

  // Group name lookup
  const getGroupName = (jid: string) => {
    const group = groups?.find((g) => g.jid === jid)
    return group?.name || jid
  }

  return (
    <div className="flex flex-col h-screen bg-background" dir="rtl">
      {/* Header */}
      <header className="border-b p-4">
        <div className="flex items-center justify-between max-w-[1800px] mx-auto">
          <div className="flex items-center gap-3">
            <h1 className="text-2xl font-bold bg-linear-to-l from-primary to-primary/60 bg-clip-text text-transparent">
              💊 فارما بروكر
            </h1>
            <span className="text-muted-foreground">لوحة التحكم</span>
          </div>

          <div className="flex items-center gap-6">
            {/* Group Filter */}
            <div className="flex items-center gap-2">
              <span className="text-sm text-muted-foreground">المجموعة:</span>
              <Select value={selectedGroup} onValueChange={setSelectedGroup}>
                <SelectTrigger className="w-[200px] h-9">
                  <SelectValue placeholder="كل المجموعات" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="all">
                    <span className="flex items-center gap-2">
                      🌐 كل المجموعات
                    </span>
                  </SelectItem>
                  {groups
                    ?.filter((g) => g.monitored)
                    .map((group) => (
                      <SelectItem key={group.jid} value={group.jid}>
                        <span className="flex items-center gap-2">
                          💬 {group.name}
                        </span>
                      </SelectItem>
                    ))}
                </SelectContent>
              </Select>
              {selectedGroup !== 'all' && (
                <Badge variant="secondary" className="text-xs">
                  {filteredOffers.length + filteredRequests.length} عنصر
                </Badge>
              )}
            </div>

            <div className="flex gap-8">
              <StatItem label="العروض" value={stats?.active_offers ?? '-'} />
              <StatItem label="الطلبات" value={stats?.active_requests ?? '-'} />
              <StatItem
                label="معلق"
                value={stats?.pending_matches ?? '-'}
                highlight="warning"
              />
              <StatItem
                label="اليوم"
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
                {connected ? 'متصل' : 'جاري الاتصال...'}
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
              <CardTitle className="text-base">
                📦 العروض (الراكد)
                {selectedGroup !== 'all' && (
                  <Badge variant="outline" className="mr-2 text-xs">
                    {filteredOffers.length}
                  </Badge>
                )}
              </CardTitle>
              <Input
                placeholder="بحث..."
                value={offersQuery}
                onChange={(e) => setOffersQuery(e.target.value)}
                className="w-40 h-8"
              />
            </div>
          </CardHeader>
          <CardContent className="flex-1 overflow-y-auto space-y-2">
            {offersLoading && (
              <p className="text-muted-foreground text-center py-8">
                جاري التحميل...
              </p>
            )}
            {!offersLoading && filteredOffers.length === 0 && (
              <p className="text-muted-foreground text-center py-8">
                {selectedGroup !== 'all'
                  ? `لا توجد عروض من ${getGroupName(selectedGroup)}`
                  : 'لا توجد عروض نشطة'}
              </p>
            )}
            {filteredOffers.map((offer) => (
              <OfferCard key={offer.id} offer={offer} />
            ))}
          </CardContent>
        </Card>

        {/* Matches Panel */}
        <Card className="flex flex-col overflow-hidden">
          <CardHeader className="pb-3">
            <div className="flex items-center justify-between">
              <CardTitle className="text-base">
                🎯 التطابقات المقترحة
                {selectedGroup !== 'all' && (
                  <Badge variant="outline" className="mr-2 text-xs">
                    {filteredMatches.length}
                  </Badge>
                )}
              </CardTitle>
              <Badge variant="secondary">{filteredMatches.length}</Badge>
            </div>
          </CardHeader>
          <CardContent className="flex-1 overflow-y-auto">
            {matchesLoading && (
              <p className="text-muted-foreground text-center py-8">
                جاري التحميل...
              </p>
            )}
            {!matchesLoading && filteredMatches.length === 0 && (
              <p className="text-muted-foreground text-center py-8">
                {selectedGroup !== 'all'
                  ? `لا توجد تطابقات من ${getGroupName(selectedGroup)}`
                  : 'لا توجد تطابقات معلقة'}
              </p>
            )}
            {filteredMatches.map((match) => (
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
              <CardTitle className="text-base">
                🔍 الطلبات (الناقص)
                {selectedGroup !== 'all' && (
                  <Badge variant="outline" className="mr-2 text-xs">
                    {filteredRequests.length}
                  </Badge>
                )}
              </CardTitle>
              <Input
                placeholder="بحث..."
                value={requestsQuery}
                onChange={(e) => setRequestsQuery(e.target.value)}
                className="w-40 h-8"
              />
            </div>
          </CardHeader>
          <CardContent className="flex-1 overflow-y-auto space-y-2">
            {requestsLoading && (
              <p className="text-muted-foreground text-center py-8">
                جاري التحميل...
              </p>
            )}
            {!requestsLoading && filteredRequests.length === 0 && (
              <p className="text-muted-foreground text-center py-8">
                {selectedGroup !== 'all'
                  ? `لا توجد طلبات من ${getGroupName(selectedGroup)}`
                  : 'لا توجد طلبات نشطة'}
              </p>
            )}
            {filteredRequests.map((request) => (
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
