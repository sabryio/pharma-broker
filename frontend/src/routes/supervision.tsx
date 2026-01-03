import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'
import { toast } from 'sonner'
import {
  Bot,
  Activity,
  Settings,
  History,
  Loader2,
  AlertTriangle,
  RefreshCw,
} from 'lucide-react'

import { DashboardLayout } from '@/components/layout/dashboard-layout'
import {
  SupervisionStatsPanel,
  LiveFeedPanel,
  ConfigPanel,
  AuditHistoryPanel,
} from '@/components/supervision'
import {
  useSupervisionStats,
  useSupervisionConfig,
  useUpdateSupervisionConfig,
  useSupervisionAudit,
  useOverrideDecision,
  usePauseSystem,
  useResumeSystem,
  useSupervisionWebSocket,
} from '@/hooks/use-supervision'
import type { AuditQueryParams, AutoApproveConfig } from '@/schema/supervision'
import { cn } from '@/lib/utils'

export const Route = createFileRoute('/supervision')({
  component: SupervisionDashboard,
})

type TabId = 'live' | 'config' | 'audit'

/**
 * AI Supervision Dashboard
 * Requirements: 3.1, 3.2, 3.4, 3.5
 */
export default function SupervisionDashboard() {
  const [activeTab, setActiveTab] = useState<TabId>('live')
  const [auditFilters, setAuditFilters] = useState<AuditQueryParams>({
    limit: 20,
    offset: 0,
  })

  // Data fetching hooks
  const {
    data: stats,
    isLoading: statsLoading,
    error: statsError,
    refetch: refetchStats,
  } = useSupervisionStats()

  const {
    data: configData,
    isLoading: configLoading,
    refetch: refetchConfig,
  } = useSupervisionConfig()

  const {
    data: auditData,
    isLoading: auditLoading,
    refetch: refetchAudit,
  } = useSupervisionAudit(auditFilters)

  // Mutation hooks
  const updateConfig = useUpdateSupervisionConfig()
  const overrideDecision = useOverrideDecision()
  const pauseSystem = usePauseSystem()
  const resumeSystem = useResumeSystem()

  // WebSocket for real-time updates
  const { isConnected, liveFeed, clearFeed } = useSupervisionWebSocket({
    onAutoApproved: () => {
      toast.success('Match auto-approved by AI')
    },
    onOverridden: () => {
      toast.info('AI decision overridden')
    },
    onPaused: (event) => {
      toast.warning(`Auto-approve paused: ${event.reason}`)
    },
    onResumed: () => {
      toast.success('Auto-approve resumed')
    },
    onBlocked: (event) => {
      toast.error(`Match blocked: ${event.blockReason}`)
    },
  })

  // Handlers
  const handlePause = () => {
    pauseSystem.mutate('Manual pause by supervisor', {
      onSuccess: () => {
        toast.success('Auto-approve system paused')
        refetchStats()
      },
      onError: (error) => {
        toast.error(`Failed to pause: ${error.message}`)
      },
    })
  }

  const handleResume = () => {
    resumeSystem.mutate(undefined, {
      onSuccess: () => {
        toast.success('Auto-approve system resumed')
        refetchStats()
      },
      onError: (error) => {
        toast.error(`Failed to resume: ${error.message}`)
      },
    })
  }

  const handleConfigSave = (config: AutoApproveConfig) => {
    updateConfig.mutate(config, {
      onSuccess: () => {
        toast.success('Configuration saved')
        refetchConfig()
        refetchStats()
      },
      onError: (error) => {
        toast.error(`Failed to save: ${error.message}`)
      },
    })
  }

  const handleOverride = (matchId: string, reason: string) => {
    overrideDecision.mutate(
      { matchId, reason },
      {
        onSuccess: () => {
          toast.success('Decision overridden')
          refetchStats()
          refetchAudit()
        },
        onError: (error) => {
          toast.error(`Failed to override: ${error.message}`)
        },
      },
    )
  }

  // Loading state
  if (statsLoading && !stats) {
    return (
      <DashboardLayout>
        <div className="flex items-center justify-center min-h-[400px]">
          <div className="flex flex-col items-center gap-4">
            <Loader2 className="w-8 h-8 text-teal animate-spin" />
            <p className="text-muted-foreground">
              Loading supervision dashboard...
            </p>
          </div>
        </div>
      </DashboardLayout>
    )
  }

  // Error state
  if (statsError) {
    return (
      <DashboardLayout>
        <div className="flex items-center justify-center min-h-[400px]">
          <div className="flex flex-col items-center gap-4 text-center">
            <div className="p-4 rounded-full bg-red-500/10">
              <AlertTriangle className="w-8 h-8 text-red-400" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-foreground mb-1">
                Failed to load supervision data
              </h2>
              <p className="text-muted-foreground text-sm mb-4">
                {statsError.message}
              </p>
              <button
                onClick={() => refetchStats()}
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

  const tabs: { id: TabId; label: string; icon: typeof Activity }[] = [
    { id: 'live', label: 'Live Activity', icon: Activity },
    { id: 'config', label: 'Configuration', icon: Settings },
    { id: 'audit', label: 'Audit History', icon: History },
  ]

  return (
    <DashboardLayout>
      <div className="space-y-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <div className="flex items-center gap-3">
              <div className="p-2 rounded-lg bg-teal/20">
                <Bot className="w-6 h-6 text-teal" />
              </div>
              <div>
                <h1 className="text-2xl font-bold text-foreground">
                  AI Supervision
                </h1>
                <p className="text-muted-foreground">
                  Monitor and control AI auto-approval decisions
                </p>
              </div>
            </div>
          </div>
        </div>

        {/* Stats Panel */}
        {stats && (
          <SupervisionStatsPanel
            stats={stats}
            isLoading={pauseSystem.isPending || resumeSystem.isPending}
            onPause={handlePause}
            onResume={handleResume}
          />
        )}

        {/* Tab Navigation */}
        <div className="flex items-center gap-1 p-1 rounded-xl bg-secondary/50 border border-white/5 w-fit">
          {tabs.map((tab) => {
            const TabIcon = tab.icon
            return (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={cn(
                  'flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-all',
                  activeTab === tab.id
                    ? 'bg-teal text-white shadow-lg shadow-teal/20'
                    : 'text-muted-foreground hover:text-foreground',
                )}
              >
                <TabIcon className="w-4 h-4" />
                {tab.label}
              </button>
            )
          })}
        </div>

        {/* Tab Content */}
        {activeTab === 'live' && (
          <LiveFeedPanel
            items={liveFeed}
            isConnected={isConnected}
            onOverride={handleOverride}
            onClear={clearFeed}
          />
        )}

        {activeTab === 'config' && configData && (
          <ConfigPanel
            config={configData.config}
            onSave={handleConfigSave}
            isSaving={updateConfig.isPending}
          />
        )}

        {activeTab === 'config' && configLoading && (
          <div className="glass-card rounded-xl p-8 text-center">
            <Loader2 className="w-6 h-6 mx-auto mb-2 text-teal animate-spin" />
            <p className="text-muted-foreground">Loading configuration...</p>
          </div>
        )}

        {activeTab === 'audit' && auditData && (
          <AuditHistoryPanel
            entries={auditData.entries}
            total={auditData.total}
            isLoading={auditLoading}
            filters={auditFilters}
            onFiltersChange={setAuditFilters}
            onRefresh={refetchAudit}
          />
        )}

        {activeTab === 'audit' && auditLoading && !auditData && (
          <div className="glass-card rounded-xl p-8 text-center">
            <Loader2 className="w-6 h-6 mx-auto mb-2 text-teal animate-spin" />
            <p className="text-muted-foreground">Loading audit history...</p>
          </div>
        )}
      </div>
    </DashboardLayout>
  )
}
