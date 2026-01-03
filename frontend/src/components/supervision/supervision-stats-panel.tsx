import {
  CheckCircle,
  Clock,
  AlertTriangle,
  TrendingUp,
  Pause,
  Play,
  ShieldAlert,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { StatCard } from '@/components/custom-ui/stat-card'
import type { SupervisionStats, SystemStatus } from '@/schema/supervision'

interface SupervisionStatsPanelProps {
  stats: SupervisionStats
  isLoading?: boolean
  onPause?: () => void
  onResume?: () => void
  className?: string
}

/**
 * Panel displaying AI auto-approve statistics
 * Requirements: 3.2
 */
export function SupervisionStatsPanel({
  stats,
  isLoading,
  onPause,
  onResume,
  className,
}: SupervisionStatsPanelProps) {
  const statusConfig: Record<
    SystemStatus,
    { label: string; color: string; icon: typeof CheckCircle }
  > = {
    active: { label: 'Active', color: 'text-emerald', icon: Play },
    paused: { label: 'Paused', color: 'text-amber', icon: Pause },
    disabled: {
      label: 'Disabled',
      color: 'text-muted-foreground',
      icon: ShieldAlert,
    },
  }

  const currentStatus = statusConfig[stats.systemStatus]
  const StatusIcon = currentStatus.icon

  return (
    <div className={cn('space-y-4', className)}>
      {/* System Status Banner */}
      <div
        className={cn(
          'glass-card p-4 rounded-xl flex items-center justify-between',
          stats.systemStatus === 'active' && 'border-emerald/30',
          stats.systemStatus === 'paused' && 'border-amber/30',
          stats.systemStatus === 'disabled' && 'border-muted/30',
        )}
      >
        <div className="flex items-center gap-3">
          <div
            className={cn(
              'p-2 rounded-lg',
              stats.systemStatus === 'active' && 'bg-emerald/20',
              stats.systemStatus === 'paused' && 'bg-amber/20',
              stats.systemStatus === 'disabled' && 'bg-muted/20',
            )}
          >
            <StatusIcon className={cn('w-5 h-5', currentStatus.color)} />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <span className="font-semibold text-foreground">
                AI Auto-Approve
              </span>
              <span
                className={cn(
                  'px-2 py-0.5 rounded-full text-xs font-medium',
                  stats.systemStatus === 'active' &&
                    'bg-emerald/20 text-emerald',
                  stats.systemStatus === 'paused' && 'bg-amber/20 text-amber',
                  stats.systemStatus === 'disabled' &&
                    'bg-muted/20 text-muted-foreground',
                )}
              >
                {currentStatus.label}
              </span>
            </div>
            {stats.pauseReason && (
              <p className="text-sm text-muted-foreground mt-0.5">
                {stats.pauseReason}
              </p>
            )}
          </div>
        </div>

        {/* Pause/Resume Button */}
        {stats.systemStatus !== 'disabled' && (
          <button
            onClick={stats.systemStatus === 'active' ? onPause : onResume}
            disabled={isLoading}
            className={cn(
              'flex items-center gap-2 px-4 py-2 rounded-lg font-medium transition-colors',
              stats.systemStatus === 'active'
                ? 'bg-amber/20 text-amber hover:bg-amber/30'
                : 'bg-emerald/20 text-emerald hover:bg-emerald/30',
              isLoading && 'opacity-50 cursor-not-allowed',
            )}
          >
            {stats.systemStatus === 'active' ? (
              <>
                <Pause className="w-4 h-4" />
                Pause
              </>
            ) : (
              <>
                <Play className="w-4 h-4" />
                Resume
              </>
            )}
          </button>
        )}
      </div>

      {/* Stats Grid */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
        <StatCard
          title="Auto-Approved Today"
          value={stats.totalApprovedToday}
          icon={CheckCircle}
          variant="emerald"
          subtitle="Matches approved by AI"
        />

        <StatCard
          title="Pending Review"
          value={stats.pendingReviewCount}
          icon={Clock}
          variant="amber"
          subtitle="Awaiting human review"
        />

        <StatCard
          title="Override Rate"
          value={`${(stats.overrideRate * 100).toFixed(1)}%`}
          icon={AlertTriangle}
          variant={stats.overrideRate > 0.1 ? 'amber' : 'default'}
          subtitle="AI decisions overridden"
        />

        <StatCard
          title="Avg. Confidence"
          value={`${(stats.averageConfidence * 100).toFixed(0)}%`}
          icon={TrendingUp}
          variant="teal"
          subtitle="Average AI confidence"
        />
      </div>

      {/* Secondary Stats */}
      <div className="grid grid-cols-2 gap-4">
        <div className="glass-card p-4 rounded-xl">
          <div className="flex items-center justify-between">
            <span className="text-sm text-muted-foreground">
              Queued for Review
            </span>
            <span className="text-lg font-semibold text-foreground">
              {stats.totalQueuedToday}
            </span>
          </div>
        </div>

        <div className="glass-card p-4 rounded-xl">
          <div className="flex items-center justify-between">
            <span className="text-sm text-muted-foreground">
              Blocked by Safety
            </span>
            <span className="text-lg font-semibold text-foreground">
              {stats.totalBlockedToday}
            </span>
          </div>
        </div>
      </div>
    </div>
  )
}
