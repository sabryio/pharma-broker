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
import { Search, Filter, TrendingUp, Package, ShoppingCart } from 'lucide-react'

export function Dashboard() {
  const [connected, setConnected] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [selectedGroup, setSelectedGroup] = useState<string>('all')

  // SSE for real-time updates
  useSSE(setConnected)

  // Data fetching
  const { data: offers, isLoading: offersLoading } = useOffers(
    searchQuery || undefined,
  )
  const { data: requests, isLoading: requestsLoading } = useRequests(
    searchQuery || undefined,
  )
  const { data: matches } = useMatches()
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

  // Mutations
  const confirmMatch = useConfirmMatch()
  const rejectMatch = useRejectMatch()

  const handleConfirm = (id: string) => confirmMatch.mutate(id)
  const handleReject = (id: string) => rejectMatch.mutate(id)

  // Stats values
  const totalOffers = stats?.active_offers ?? 0
  const totalRequests = stats?.active_requests ?? 0
  const totalMatches = stats?.pending_matches ?? 0
  const total = totalOffers + totalRequests

  return (
    <div
      className="flex flex-col h-screen bg-gray-50 dark:bg-gray-900"
      dir="rtl"
    >
      {/* Header */}
      <header className="bg-white dark:bg-gray-800 border-b p-4 shadow-sm">
        <div className="max-w-[1600px] mx-auto">
          {/* Top row: Search and Stats */}
          <div className="flex items-center justify-between gap-8">
            {/* Search and Filter */}
            <div className="flex-1 max-w-md space-y-2">
              {/* Search Input */}
              <div className="relative">
                <Search className="absolute right-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
                <Input
                  placeholder="ابحث عن اسم دواء..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="pr-10 bg-gray-50 dark:bg-gray-900"
                />
              </div>
              {/* Group Filter */}
              <div className="flex items-center gap-2">
                <Filter className="h-4 w-4 text-muted-foreground" />
                <Select value={selectedGroup} onValueChange={setSelectedGroup}>
                  <SelectTrigger className="flex-1 bg-gray-50 dark:bg-gray-900">
                    <SelectValue placeholder="فلترة حسب الجروب" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">كل الجروبات</SelectItem>
                    {groups
                      ?.filter((g) => g.monitored)
                      .map((group) => (
                        <SelectItem key={group.jid} value={group.jid}>
                          {group.name}
                        </SelectItem>
                      ))}
                  </SelectContent>
                </Select>
              </div>
              {/* Summary */}
              <p className="text-xs text-muted-foreground">
                الكل: {total} • عروض: {totalOffers} • طلبات: {totalRequests}
              </p>
            </div>

            {/* Stats Bar Chart */}
            <div className="flex items-end gap-1 h-16">
              <div
                className="w-8 bg-green-500 rounded-t"
                style={{
                  height: `${Math.min((totalOffers / (total || 1)) * 100, 100)}%`,
                  minHeight: '8px',
                }}
                title={`عروض: ${totalOffers}`}
              />
              <div
                className="w-8 bg-red-500 rounded-t"
                style={{
                  height: `${Math.min((totalRequests / (total || 1)) * 100, 100)}%`,
                  minHeight: '8px',
                }}
                title={`طلبات: ${totalRequests}`}
              />
              <div
                className="w-8 bg-blue-500 rounded-t"
                style={{
                  height: `${Math.min((totalMatches / (total || 1)) * 100, 100)}%`,
                  minHeight: '8px',
                }}
                title={`تطابقات: ${totalMatches}`}
              />
            </div>

            {/* Stats Cards */}
            <div className="flex gap-6">
              <StatCard
                icon={<TrendingUp className="h-5 w-5" />}
                value={stats?.confirmed_today ?? 0}
                label="فرصة ناجحة"
                color="text-green-600"
              />
              <StatCard
                icon={<ShoppingCart className="h-5 w-5" />}
                value={totalRequests}
                label="طلب استلام"
                color="text-red-600"
              />
              <StatCard
                icon={<Package className="h-5 w-5" />}
                value={totalOffers}
                label="عرض متاح"
                color="text-blue-600"
              />
            </div>

            {/* Connection Status */}
            <div
              className={`flex items-center gap-2 px-3 py-1.5 rounded-full text-xs ${
                connected
                  ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400'
                  : 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400'
              }`}
            >
              <span
                className={`w-2 h-2 rounded-full ${
                  connected ? 'bg-green-500' : 'bg-yellow-500 animate-pulse'
                }`}
              />
              {connected ? 'متصل' : 'جاري الاتصال...'}
            </div>
          </div>
        </div>
      </header>

      {/* Main Content - Two Columns */}
      <main className="flex-1 overflow-hidden p-4">
        <div className="max-w-[1600px] mx-auto h-full grid grid-cols-2 gap-4">
          {/* Requests Column (Left) */}
          <Card className="flex flex-col overflow-hidden border-t-4 border-t-red-500">
            <CardHeader className="py-3 px-4 bg-red-50 dark:bg-red-950/20">
              <div className="flex items-center justify-between">
                <CardTitle className="text-base flex items-center gap-2">
                  <Badge className="bg-red-500 text-white">
                    {filteredRequests.length}
                  </Badge>
                  <span>الطلبات (Requests)</span>
                </CardTitle>
              </div>
            </CardHeader>
            <CardContent className="flex-1 overflow-y-auto p-3 space-y-3">
              {requestsLoading && (
                <p className="text-muted-foreground text-center py-8">
                  جاري التحميل...
                </p>
              )}
              {!requestsLoading && filteredRequests.length === 0 && (
                <p className="text-muted-foreground text-center py-8">
                  لا توجد طلبات نشطة
                </p>
              )}
              {filteredRequests.map((request) => (
                <RequestCard key={request.id} request={request} />
              ))}
            </CardContent>
          </Card>

          {/* Offers Column (Right) */}
          <Card className="flex flex-col overflow-hidden border-t-4 border-t-green-500">
            <CardHeader className="py-3 px-4 bg-green-50 dark:bg-green-950/20">
              <div className="flex items-center justify-between">
                <CardTitle className="text-base flex items-center gap-2">
                  <Badge className="bg-green-500 text-white">
                    {filteredOffers.length}
                  </Badge>
                  <span>العروض (Offers)</span>
                </CardTitle>
              </div>
            </CardHeader>
            <CardContent className="flex-1 overflow-y-auto p-3 space-y-3">
              {offersLoading && (
                <p className="text-muted-foreground text-center py-8">
                  جاري التحميل...
                </p>
              )}
              {!offersLoading && filteredOffers.length === 0 && (
                <p className="text-muted-foreground text-center py-8">
                  لا توجد عروض نشطة
                </p>
              )}
              {filteredOffers.map((offer) => (
                <OfferCard key={offer.id} offer={offer} />
              ))}
            </CardContent>
          </Card>
        </div>
      </main>

      {/* Matches Drawer (if any pending) */}
      {matches && matches.length > 0 && (
        <div className="fixed bottom-20 left-1/2 -translate-x-1/2 z-40">
          <Card className="shadow-2xl border-2 border-green-500 max-w-lg">
            <CardHeader className="py-2 px-4 bg-green-50 dark:bg-green-950/20">
              <CardTitle className="text-sm flex items-center gap-2">
                🎯 تطابقات معلقة ({matches.length})
              </CardTitle>
            </CardHeader>
            <CardContent className="p-3 max-h-64 overflow-y-auto">
              {matches.slice(0, 3).map((match) => (
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
        </div>
      )}

      {/* FAB Modals */}
      <GroupsModal />
      <AnalyzeModal />
      <ConfigPanel />
    </div>
  )
}

function StatCard({
  icon,
  value,
  label,
  color,
}: {
  icon: React.ReactNode
  value: number
  label: string
  color: string
}) {
  return (
    <div className="flex flex-col items-center text-center">
      <div className={`${color} mb-1`}>{icon}</div>
      <span className={`text-2xl font-bold ${color}`}>{value}</span>
      <span className="text-xs text-muted-foreground">{label}</span>
    </div>
  )
}
