import { ProgressRing } from '@/components/custom-ui/progress-ring'
import { StatCard } from '@/components/custom-ui/stat-card'
import { ActivityChart } from '@/components/dashboard/activity-chart'
import { RecentActivity } from '@/components/dashboard/recent-activity'
import { DashboardLayout } from '@/components/layout/dashboard-layout'
import { ArrowRightLeft, FileText, Globe, TrendingUp } from 'lucide-react'

import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/')({
  component: Dashboard,
})

function Dashboard() {
  return (
    <DashboardLayout>
      <div className="space-y-6">
        {/* Page Header */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold text-foreground">
              Dashboard Overview
            </h1>
            <p className="text-muted-foreground">
              Welcome back. Here's your trading summary.
            </p>
          </div>
          <div className="flex items-center gap-2 px-4 py-2 rounded-lg bg-emerald/10 border border-emerald/30">
            <div className="w-2 h-2 rounded-full bg-emerald animate-pulse" />
            <span className="text-sm font-medium text-emerald">
              All Systems Operational
            </span>
          </div>
        </div>

        {/* Stats Grid */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          <StatCard
            title="Total Offers"
            value="1,245"
            subtitle="↗ All Regions"
            icon={FileText}
            variant="teal"
            trend={{ value: 12, isPositive: true }}
          />
          <StatCard
            title="Total Requests"
            value="892"
            subtitle="Active Trading"
            icon={ArrowRightLeft}
            variant="amber"
            trend={{ value: 8, isPositive: true }}
          />
          <StatCard
            title="Trade Volume"
            value="$28.3M"
            subtitle="This Month"
            icon={TrendingUp}
            variant="default"
            trend={{ value: 23, isPositive: true }}
          />
          <div className="glass-card p-5 rounded-xl flex items-center justify-between glow-emerald border border-emerald/30">
            <div>
              <span className="text-sm font-medium text-muted-foreground">
                Match Rate
              </span>
              <p className="text-xs text-muted-foreground mt-1">Last 30 Days</p>
            </div>
            <ProgressRing
              value={85}
              size={90}
              strokeWidth={6}
              label="Match Rate"
            />
          </div>
        </div>

        {/* Chart and Activity */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          <div className="lg:col-span-2">
            <ActivityChart />
          </div>
          <div className="lg:col-span-1">
            <RecentActivity />
          </div>
        </div>

        {/* Quick Actions */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <button className="glass-card p-5 rounded-xl text-left transition-all duration-300 hover:scale-[1.02] hover:border-teal/50 group">
            <div className="flex items-center gap-3 mb-3">
              <div className="p-2 rounded-lg bg-teal/10 text-teal group-hover:bg-teal/20 transition-colors">
                <FileText className="w-5 h-5" />
              </div>
              <span className="text-lg font-semibold text-foreground">
                Create Offer
              </span>
            </div>
            <p className="text-sm text-muted-foreground">
              List new medications for trading
            </p>
          </button>

          <button className="glass-card p-5 rounded-xl text-left transition-all duration-300 hover:scale-[1.02] hover:border-amber/50 group">
            <div className="flex items-center gap-3 mb-3">
              <div className="p-2 rounded-lg bg-amber/10 text-amber group-hover:bg-amber/20 transition-colors">
                <ArrowRightLeft className="w-5 h-5" />
              </div>
              <span className="text-lg font-semibold text-foreground">
                Post Request
              </span>
            </div>
            <p className="text-sm text-muted-foreground">
              Request medications you need
            </p>
          </button>

          <button className="glass-card p-5 rounded-xl text-left transition-all duration-300 hover:scale-[1.02] hover:border-emerald/50 group">
            <div className="flex items-center gap-3 mb-3">
              <div className="p-2 rounded-lg bg-emerald/10 text-emerald group-hover:bg-emerald/20 transition-colors">
                <Globe className="w-5 h-5" />
              </div>
              <span className="text-lg font-semibold text-foreground">
                View Matches
              </span>
            </div>
            <p className="text-sm text-muted-foreground">
              Review AI-powered matches
            </p>
          </button>
        </div>
      </div>
    </DashboardLayout>
  )
}
