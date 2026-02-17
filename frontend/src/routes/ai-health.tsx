import { createFileRoute } from '@tanstack/react-router'
import {
  Activity,
  AlertCircle,
  CheckCircle2,
  Clock,
  Loader2,
  RefreshCw,
  Server,
  TrendingUp,
  Zap,
} from 'lucide-react'
import { DashboardLayout } from '@/components/layout/dashboard-layout'
import { useAiHealth, useTestConnection } from '@/hooks/use-ai-health'
import { cn } from '@/lib/utils'
import { toast } from 'sonner'

export const Route = createFileRoute('/ai-health')({
  component: AiHealthDashboard,
})

function AiHealthDashboard() {
  const { data, isLoading, error } = useAiHealth()
  const testConnection = useTestConnection()

  const handleTestConnection = () => {
    toast.info('Testing AI gateway connection...', {
      description: 'This may take a few seconds',
    })

    testConnection.mutate(undefined, {
      onSuccess: (result) => {
        if (result.success) {
          toast.success('Connection successful!', {
            description: `Response time: ${result.responseTimeMs}ms`,
          })
        } else {
          toast.error('Connection failed', {
            description: result.error || 'Unknown error',
          })
        }
      },
      onError: (error) => {
        toast.error('Connection test failed', {
          description: error.message,
        })
      },
    })
  }

  if (isLoading) {
    return (
      <DashboardLayout>
        <div className="flex items-center justify-center h-64">
          <Loader2 className="w-8 h-8 animate-spin text-teal" />
        </div>
      </DashboardLayout>
    )
  }

  if (error) {
    return (
      <DashboardLayout>
        <div className="flex flex-col items-center justify-center h-64 gap-4">
          <AlertCircle className="w-12 h-12 text-destructive" />
          <p className="text-muted-foreground">Failed to load AI health data</p>
          <p className="text-sm text-destructive">{error.message}</p>
        </div>
      </DashboardLayout>
    )
  }

  if (!data) {
    return null
  }

  const statusColor =
    data.status === 'healthy'
      ? 'text-green-500'
      : data.status === 'warning'
        ? 'text-yellow-500'
        : 'text-red-500'

  const statusBg =
    data.status === 'healthy'
      ? 'bg-green-500/10 border-green-500/30'
      : data.status === 'warning'
        ? 'bg-yellow-500/10 border-yellow-500/30'
        : 'bg-red-500/10 border-red-500/30'

  return (
    <DashboardLayout>
      <div className="space-y-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold text-foreground">
              AI Health Monitoring
            </h1>
            <p className="text-muted-foreground">
              Monitor AI gateway health and performance
            </p>
          </div>
          <div
            className={cn(
              'flex items-center gap-2 px-4 py-2 rounded-lg border',
              statusBg,
            )}
          >
            <Activity className={cn('w-5 h-5', statusColor)} />
            <span className={cn('font-semibold capitalize', statusColor)}>
              {data.status}
            </span>
          </div>
        </div>

        {/* Main Grid */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          {/* Circuit Breaker Card */}
          <div className="bg-card border border-border rounded-lg p-6 space-y-4">
            <div className="flex items-center justify-between">
              <h2 className="text-lg font-semibold text-foreground flex items-center gap-2">
                <Zap className="w-5 h-5 text-violet-400" />
                Circuit Breaker
              </h2>
              <CircuitBreakerBadge state={data.circuitBreaker.state} />
            </div>

            <div className="space-y-3">
              <div className="flex justify-between items-center">
                <span className="text-sm text-muted-foreground">Failures</span>
                <span className="text-lg font-semibold text-foreground">
                  {data.circuitBreaker.failureCount}
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm text-muted-foreground">Successes</span>
                <span className="text-lg font-semibold text-green-500">
                  {data.circuitBreaker.successCount}
                </span>
              </div>
              {data.circuitBreaker.lastFailureTime && (
                <div className="flex justify-between items-center">
                  <span className="text-sm text-muted-foreground">
                    Last Failure
                  </span>
                  <span className="text-sm text-foreground">
                    {new Date(
                      data.circuitBreaker.lastFailureTime,
                    ).toLocaleTimeString()}
                  </span>
                </div>
              )}
            </div>

            <button
              onClick={handleTestConnection}
              disabled={testConnection.isPending}
              className={cn(
                'w-full flex items-center justify-center gap-2 px-4 py-2 rounded-lg font-medium transition-all duration-300',
                'bg-gradient-to-r from-violet-500/20 to-fuchsia-500/20',
                'border border-violet-500/30 hover:border-violet-500/50',
                'text-violet-400 hover:text-violet-300',
                'hover:scale-105 active:scale-95',
                'disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100',
              )}
            >
              {testConnection.isPending ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" />
                  Testing...
                </>
              ) : (
                <>
                  <RefreshCw className="w-4 h-4" />
                  Test Connection
                </>
              )}
            </button>
          </div>

          {/* Performance Card */}
          <div className="bg-card border border-border rounded-lg p-6 space-y-4">
            <h2 className="text-lg font-semibold text-foreground flex items-center gap-2">
              <TrendingUp className="w-5 h-5 text-teal-400" />
              Performance
            </h2>

            <div className="space-y-3">
              <div className="flex justify-between items-center">
                <span className="text-sm text-muted-foreground">
                  Success Rate (1h)
                </span>
                <span className="text-lg font-semibold text-green-500">
                  {(data.performance.successRate1h * 100).toFixed(1)}%
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm text-muted-foreground">
                  Success Rate (24h)
                </span>
                <span className="text-lg font-semibold text-green-500">
                  {(data.performance.successRate24h * 100).toFixed(1)}%
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm text-muted-foreground">
                  Avg Response Time
                </span>
                <span className="text-lg font-semibold text-foreground">
                  {data.performance.avgResponseTimeMs.toFixed(0)}ms
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm text-muted-foreground">
                  P95 Response Time
                </span>
                <span className="text-lg font-semibold text-foreground">
                  {data.performance.p95ResponseTimeMs.toFixed(0)}ms
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm text-muted-foreground">
                  Requests (1h)
                </span>
                <span className="text-lg font-semibold text-foreground">
                  {data.performance.totalRequests1h}
                </span>
              </div>
            </div>
          </div>

          {/* Retry Queue Card */}
          <div className="bg-card border border-border rounded-lg p-6 space-y-4">
            <h2 className="text-lg font-semibold text-foreground flex items-center gap-2">
              <Clock className="w-5 h-5 text-amber-400" />
              Retry Queue
            </h2>

            <div className="space-y-3">
              <div className="flex justify-between items-center">
                <span className="text-sm text-muted-foreground">Pending</span>
                <span className="text-lg font-semibold text-amber-500">
                  {data.retryQueue.pending}
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm text-muted-foreground">
                  Processing
                </span>
                <span className="text-lg font-semibold text-blue-500">
                  {data.retryQueue.processing}
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm text-muted-foreground">Completed</span>
                <span className="text-lg font-semibold text-green-500">
                  {data.retryQueue.completed}
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm text-muted-foreground">Failed</span>
                <span className="text-lg font-semibold text-red-500">
                  {data.retryQueue.failed}
                </span>
              </div>
            </div>

            {data.retryQueue.byReason.length > 0 && (
              <div className="pt-3 border-t border-border">
                <p className="text-sm font-medium text-muted-foreground mb-2">
                  By Reason:
                </p>
                <div className="space-y-2">
                  {data.retryQueue.byReason.map((reason) => (
                    <div
                      key={reason.reason}
                      className="flex justify-between items-center text-sm"
                    >
                      <span className="text-muted-foreground">
                        {reason.reason.replace(/_/g, ' ')}
                      </span>
                      <span className="text-foreground font-medium">
                        {reason.count}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>

          {/* Model Info Card */}
          <div className="bg-card border border-border rounded-lg p-6 space-y-4">
            <h2 className="text-lg font-semibold text-foreground flex items-center gap-2">
              <Server className="w-5 h-5 text-cyan-400" />
              Model Info
            </h2>

            <div className="space-y-3">
              <div>
                <span className="text-sm text-muted-foreground block mb-1">
                  Endpoint
                </span>
                <span className="text-sm text-foreground font-mono bg-muted px-2 py-1 rounded">
                  {data.modelInfo.endpoint}
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm text-muted-foreground">Model</span>
                <span className="text-sm text-foreground font-medium">
                  {data.modelInfo.modelName}
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm text-muted-foreground">Timeout</span>
                <span className="text-sm text-foreground font-medium">
                  {data.modelInfo.timeoutSeconds}s
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm text-muted-foreground">
                  Max Retries
                </span>
                <span className="text-sm text-foreground font-medium">
                  {data.modelInfo.maxRetries}
                </span>
              </div>
            </div>
          </div>
        </div>

        {/* Recent Errors */}
        {data.recentErrors.length > 0 && (
          <div className="bg-card border border-border rounded-lg p-6">
            <h2 className="text-lg font-semibold text-foreground mb-4 flex items-center gap-2">
              <AlertCircle className="w-5 h-5 text-red-400" />
              Recent Errors
            </h2>
            <div className="space-y-2">
              {data.recentErrors.map((error, index) => (
                <div
                  key={index}
                  className="flex items-start gap-3 p-3 bg-red-500/5 border border-red-500/20 rounded-lg"
                >
                  <AlertCircle className="w-4 h-4 text-red-400 mt-0.5 flex-shrink-0" />
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 mb-1">
                      <span className="text-sm font-medium text-red-400">
                        {error.errorType}
                      </span>
                      <span className="text-xs text-muted-foreground">
                        {new Date(error.timestamp).toLocaleString()}
                      </span>
                    </div>
                    <p className="text-sm text-muted-foreground truncate">
                      {error.message}
                    </p>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </DashboardLayout>
  )
}

function CircuitBreakerBadge({ state }: { state: string }) {
  const config = {
    closed: {
      icon: CheckCircle2,
      color: 'text-green-500',
      bg: 'bg-green-500/10 border-green-500/30',
      label: 'Closed',
    },
    open: {
      icon: AlertCircle,
      color: 'text-red-500',
      bg: 'bg-red-500/10 border-red-500/30',
      label: 'Open',
    },
    half_open: {
      icon: Activity,
      color: 'text-yellow-500',
      bg: 'bg-yellow-500/10 border-yellow-500/30',
      label: 'Half Open',
    },
  }

  const { icon: Icon, color, bg, label } = config[state as keyof typeof config]

  return (
    <div
      className={cn(
        'flex items-center gap-1.5 px-3 py-1 rounded-full border',
        bg,
      )}
    >
      <Icon className={cn('w-4 h-4', color)} />
      <span className={cn('text-sm font-medium', color)}>{label}</span>
    </div>
  )
}
