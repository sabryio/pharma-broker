import React from 'react'
import { Database, CheckCircle2, Clock, Percent } from 'lucide-react'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Skeleton } from '@/components/ui/skeleton'
import { useCurationStats } from '@/hooks/use-curation'

export const CurationStats: React.FC = () => {
  const { data: stats, isLoading } = useCurationStats()

  if (isLoading) {
    return (
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
        {[1, 2, 3, 4].map((i) => (
          <Card key={i} className="glass-card border-white/5 animate-pulse">
            <CardHeader className="pb-2">
              <Skeleton className="h-4 w-24" />
            </CardHeader>
            <CardContent>
              <Skeleton className="h-8 w-16" />
            </CardContent>
          </Card>
        ))}
      </div>
    )
  }

  const items = [
    {
      label: 'Total Aliases',
      value: stats?.totalAliases ?? 0,
      icon: Database,
      color: 'text-blue-400',
    },
    {
      label: 'Pending',
      value: stats?.pendingCount ?? 0,
      icon: Clock,
      color: 'text-amber-400',
    },
    {
      label: 'Verified',
      value: stats?.approvedCount ?? 0,
      icon: CheckCircle2,
      color: 'text-emerald',
    },
    {
      label: 'Coverage',
      value: `${Math.round(stats?.curationPercentage ?? 0)}%`,
      icon: Percent,
      color: 'text-teal',
    },
  ]

  return (
    <div className="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
      {items.map((item) => (
        <Card
          key={item.label}
          className="glass-card border-white/5 bg-white/2 hover:bg-white/4 transition-colors"
        >
          <CardHeader className="flex flex-row items-center justify-between pb-2 space-y-0">
            <CardTitle className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
              {item.label}
            </CardTitle>
            <item.icon className={`w-4 h-4 ${item.color} opacity-80`} />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold font-mono tracking-tight">
              {item.value}
            </div>
          </CardContent>
        </Card>
      ))}
    </div>
  )
}
