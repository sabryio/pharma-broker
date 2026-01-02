// Audit Records Viewer Component
// Displays audit records from the backend API with filtering and detail view

import { useState } from 'react'
import { cn } from '@/lib/utils'
import {
  useAuditRecords,
  useAuditRecord,
  useAuditRecorderStatus,
  useUpdateAuditReview,
  type FrontendAuditRecord,
  type ListAuditRecordsParams,
} from '@/hooks/use-audit-records'
import {
  Search,
  RefreshCw,
  Clock,
  Cpu,
  Sparkles,
  ChevronRight,
  X,
  CheckCircle,
  XCircle,
  AlertTriangle,
  Database,
  Activity,
  Layers,
  FileJson,
  Eye,
} from 'lucide-react'
import { Input } from '@/components/ui/input'
import { toast } from 'sonner'

// =============================================================================
// Status Badge Component
// =============================================================================

function StatusBadge({ enabled }: { enabled: boolean }) {
  return (
    <span
      className={cn(
        'flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium',
        enabled
          ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30'
          : 'bg-red-500/20 text-red-400 border border-red-500/30',
      )}
    >
      <span
        className={cn(
          'w-1.5 h-1.5 rounded-full',
          enabled ? 'bg-emerald-400 animate-pulse' : 'bg-red-400',
        )}
      />
      {enabled ? 'Recording' : 'Disabled'}
    </span>
  )
}

// =============================================================================
// Recorder Status Panel
// =============================================================================

function RecorderStatusPanel() {
  const { data: status, isLoading, refetch } = useAuditRecorderStatus()

  if (isLoading) {
    return (
      <div className="p-4 rounded-xl bg-secondary/30 border border-border/50 animate-pulse">
        <div className="h-20" />
      </div>
    )
  }

  if (!status) return null

  const bufferUsage = (status.currentBufferLen / status.bufferSize) * 100

  return (
    <div className="p-4 rounded-xl bg-gradient-to-br from-violet-500/10 to-purple-500/10 border border-violet-500/20">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl bg-violet-500/20 flex items-center justify-center">
            <Database className="w-5 h-5 text-violet-400" />
          </div>
          <div>
            <h3 className="text-sm font-semibold text-foreground">
              Audit Recorder
            </h3>
            <p className="text-xs text-muted-foreground">
              Backend recording status
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <StatusBadge enabled={status.enabled} />
          <button
            onClick={() => refetch()}
            className="p-2 rounded-lg hover:bg-secondary/50 transition-colors"
          >
            <RefreshCw className="w-4 h-4 text-muted-foreground" />
          </button>
        </div>
      </div>

      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <div className="p-3 rounded-lg bg-background/50 border border-border/30">
          <p className="text-xs text-muted-foreground mb-1">Records Created</p>
          <p className="text-lg font-bold text-foreground">
            {status.stats.recordsCreated}
          </p>
        </div>
        <div className="p-3 rounded-lg bg-background/50 border border-border/30">
          <p className="text-xs text-muted-foreground mb-1">Persisted</p>
          <p className="text-lg font-bold text-emerald-400">
            {status.stats.recordsPersisted}
          </p>
        </div>
        <div className="p-3 rounded-lg bg-background/50 border border-border/30">
          <p className="text-xs text-muted-foreground mb-1">Buffer Usage</p>
          <div className="flex items-center gap-2">
            <div className="flex-1 h-2 bg-secondary rounded-full overflow-hidden">
              <div
                className="h-full bg-violet-500 rounded-full transition-all"
                style={{ width: `${bufferUsage}%` }}
              />
            </div>
            <span className="text-xs text-muted-foreground">
              {bufferUsage.toFixed(0)}%
            </span>
          </div>
        </div>
        <div className="p-3 rounded-lg bg-background/50 border border-border/30">
          <p className="text-xs text-muted-foreground mb-1">Sample Rate</p>
          <p className="text-lg font-bold text-foreground">
            {(status.config.sampleRate * 100).toFixed(0)}%
          </p>
        </div>
      </div>
    </div>
  )
}

// =============================================================================
// Audit Record Card
// =============================================================================

interface AuditRecordCardProps {
  record: FrontendAuditRecord
  isSelected: boolean
  onSelect: () => void
}

function AuditRecordCard({
  record,
  isSelected,
  onSelect,
}: AuditRecordCardProps) {
  return (
    <button
      onClick={onSelect}
      className={cn(
        'w-full p-4 rounded-xl border text-left transition-all duration-200',
        'hover:scale-[1.01] active:scale-[0.99]',
        isSelected
          ? 'bg-teal-500/10 border-teal-500/30 shadow-lg shadow-teal-500/10'
          : 'bg-secondary/30 border-border/50 hover:border-border',
      )}
    >
      <div className="flex items-start justify-between mb-3">
        <div className="flex items-center gap-2">
          <div
            className={cn(
              'w-8 h-8 rounded-lg flex items-center justify-center',
              record.aiInvolved
                ? 'bg-violet-500/20 text-violet-400'
                : 'bg-teal-500/20 text-teal-400',
            )}
          >
            {record.aiInvolved ? (
              <Sparkles className="w-4 h-4" />
            ) : (
              <Cpu className="w-4 h-4" />
            )}
          </div>
          <div>
            <p className="text-sm font-medium text-foreground truncate max-w-[150px]">
              {record.offerProduct}
            </p>
            <p className="text-xs text-muted-foreground truncate max-w-[150px]">
              → {record.requestProduct}
            </p>
          </div>
        </div>
        <ChevronRight
          className={cn(
            'w-4 h-4 transition-transform',
            isSelected ? 'text-teal-400 rotate-90' : 'text-muted-foreground',
          )}
        />
      </div>

      <div className="flex items-center gap-3 text-xs">
        <span
          className={cn(
            'px-2 py-0.5 rounded-full font-medium',
            record.finalScore >= 0.8 && 'bg-emerald-500/20 text-emerald-400',
            record.finalScore >= 0.5 &&
              record.finalScore < 0.8 &&
              'bg-amber-500/20 text-amber-400',
            record.finalScore < 0.5 && 'bg-red-500/20 text-red-400',
          )}
        >
          {(record.finalScore * 100).toFixed(0)}%
        </span>
        <span className="text-muted-foreground flex items-center gap-1">
          <Clock className="w-3 h-3" />
          {record.totalLatencyMs}ms
        </span>
        <span className="text-muted-foreground">{record.resolutionStage}</span>
      </div>

      {/* Pipeline Summary Mini */}
      {record.pipelineSummary.length > 0 && (
        <div className="mt-3 flex items-center gap-1">
          {record.pipelineSummary.slice(0, 4).map((stage, idx) => (
            <div
              key={idx}
              className="flex-1 h-1 rounded-full bg-teal-500/30"
              title={`${stage.stage}: ${stage.durationMs}ms`}
            />
          ))}
          {record.pipelineSummary.length > 4 && (
            <span className="text-[10px] text-muted-foreground">
              +{record.pipelineSummary.length - 4}
            </span>
          )}
        </div>
      )}
    </button>
  )
}

// =============================================================================
// Audit Record Detail Panel
// =============================================================================

interface AuditRecordDetailProps {
  matchId: string
  onClose: () => void
}

function AuditRecordDetail({ matchId, onClose }: AuditRecordDetailProps) {
  const { data, isLoading, error } = useAuditRecord(matchId)
  const updateReview = useUpdateAuditReview()

  const handleUpdateReview = (status: string) => {
    updateReview.mutate(
      { matchId, data: { status } },
      {
        onSuccess: () => {
          toast.success(`Review marked as ${status}`)
        },
        onError: () => {
          toast.error('Failed to update review')
        },
      },
    )
  }

  if (isLoading) {
    return (
      <div className="p-6 rounded-xl bg-secondary/30 border border-border/50">
        <div className="flex items-center justify-center py-12">
          <RefreshCw className="w-6 h-6 text-muted-foreground animate-spin" />
        </div>
      </div>
    )
  }

  if (error || !data) {
    return (
      <div className="p-6 rounded-xl bg-red-500/10 border border-red-500/20">
        <p className="text-red-400">Failed to load audit record</p>
      </div>
    )
  }

  const { record, replayContext } = data

  return (
    <div className="p-6 rounded-xl bg-gradient-to-br from-secondary/50 to-secondary/20 border border-border/50 space-y-6">
      {/* Header */}
      <div className="flex items-start justify-between">
        <div>
          <h3 className="text-lg font-semibold text-foreground">
            Audit Record Detail
          </h3>
          <p className="text-sm text-muted-foreground">
            Match ID: {record.matchId.slice(0, 8)}...
          </p>
        </div>
        <button
          onClick={onClose}
          className="p-2 rounded-lg hover:bg-secondary transition-colors"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Score & Timing */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <div className="p-3 rounded-lg bg-background/50 border border-border/30">
          <p className="text-xs text-muted-foreground mb-1">Final Score</p>
          <p className="text-2xl font-bold text-teal-400">
            {(record.finalScore * 100).toFixed(1)}%
          </p>
        </div>
        <div className="p-3 rounded-lg bg-background/50 border border-border/30">
          <p className="text-xs text-muted-foreground mb-1">Total Latency</p>
          <p className="text-2xl font-bold text-foreground">
            {record.totalLatencyMs}ms
          </p>
        </div>
        <div className="p-3 rounded-lg bg-background/50 border border-border/30">
          <p className="text-xs text-muted-foreground mb-1">Resolution</p>
          <p className="text-sm font-medium text-foreground">
            {record.resolutionStage}
          </p>
        </div>
        <div className="p-3 rounded-lg bg-background/50 border border-border/30">
          <p className="text-xs text-muted-foreground mb-1">AI Involved</p>
          <p className="text-sm font-medium">
            {record.aiInvolved ? (
              <span className="text-violet-400">Yes</span>
            ) : (
              <span className="text-muted-foreground">No</span>
            )}
          </p>
        </div>
      </div>

      {/* Pipeline Stages */}
      <div>
        <h4 className="text-sm font-semibold text-foreground mb-3 flex items-center gap-2">
          <Layers className="w-4 h-4 text-teal-400" />
          Pipeline Stages
        </h4>
        <div className="space-y-2">
          {record.pipelineStages.map((stage, idx) => (
            <div
              key={idx}
              className="flex items-center gap-3 p-3 rounded-lg bg-background/50 border border-border/30"
            >
              <div className="w-6 h-6 rounded-full bg-teal-500/20 flex items-center justify-center text-xs font-bold text-teal-400">
                {idx + 1}
              </div>
              <div className="flex-1">
                <p className="text-sm font-medium text-foreground">
                  {stage.stage}
                </p>
                <p className="text-xs text-muted-foreground">
                  {stage.candidatesIn} → {stage.candidatesOut} candidates
                </p>
              </div>
              <span className="text-xs text-muted-foreground">
                {stage.durationMs}ms
              </span>
            </div>
          ))}
        </div>
      </div>

      {/* AI Record */}
      {record.aiRecord && (
        <div>
          <h4 className="text-sm font-semibold text-foreground mb-3 flex items-center gap-2">
            <Sparkles className="w-4 h-4 text-violet-400" />
            AI Analysis
          </h4>
          <div className="p-4 rounded-lg bg-violet-500/10 border border-violet-500/20">
            <div className="grid grid-cols-3 gap-3 mb-3">
              <div>
                <p className="text-xs text-muted-foreground">Model</p>
                <p className="text-sm font-medium text-foreground">
                  {record.aiRecord.model}
                </p>
              </div>
              <div>
                <p className="text-xs text-muted-foreground">Latency</p>
                <p className="text-sm font-medium text-foreground">
                  {record.aiRecord.latencyMs}ms
                </p>
              </div>
              <div>
                <p className="text-xs text-muted-foreground">Tokens</p>
                <p className="text-sm font-medium text-foreground">
                  {(record.aiRecord.promptTokens ?? 0) +
                    (record.aiRecord.completionTokens ?? 0)}
                </p>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Review Actions */}
      <div className="flex items-center gap-3 pt-4 border-t border-border/30">
        <span className="text-sm text-muted-foreground">Review Status:</span>
        <span
          className={cn(
            'px-2 py-1 rounded-full text-xs font-medium',
            record.reviewStatus === 'approved' &&
              'bg-emerald-500/20 text-emerald-400',
            record.reviewStatus === 'rejected' &&
              'bg-red-500/20 text-red-400',
            record.reviewStatus === 'flagged' &&
              'bg-amber-500/20 text-amber-400',
            !record.reviewStatus && 'bg-secondary text-muted-foreground',
          )}
        >
          {record.reviewStatus || 'Pending'}
        </span>
        <div className="flex-1" />
        <button
          onClick={() => handleUpdateReview('approved')}
          disabled={updateReview.isPending}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-emerald-500/20 text-emerald-400 hover:bg-emerald-500/30 transition-colors text-sm"
        >
          <CheckCircle className="w-3.5 h-3.5" />
          Approve
        </button>
        <button
          onClick={() => handleUpdateReview('flagged')}
          disabled={updateReview.isPending}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-amber-500/20 text-amber-400 hover:bg-amber-500/30 transition-colors text-sm"
        >
          <AlertTriangle className="w-3.5 h-3.5" />
          Flag
        </button>
        <button
          onClick={() => handleUpdateReview('rejected')}
          disabled={updateReview.isPending}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-red-500/20 text-red-400 hover:bg-red-500/30 transition-colors text-sm"
        >
          <XCircle className="w-3.5 h-3.5" />
          Reject
        </button>
      </div>

      {/* Replay Context */}
      {replayContext && (
        <div>
          <h4 className="text-sm font-semibold text-foreground mb-3 flex items-center gap-2">
            <FileJson className="w-4 h-4 text-cyan-400" />
            Replay Context
          </h4>
          <pre className="p-3 rounded-lg bg-background/50 border border-border/30 text-xs text-muted-foreground overflow-auto max-h-40">
            {JSON.stringify(replayContext, null, 2)}
          </pre>
        </div>
      )}
    </div>
  )
}

// =============================================================================
// Main Audit Records Viewer
// =============================================================================

export interface AuditRecordsViewerProps {
  className?: string
}

export function AuditRecordsViewer({ className }: AuditRecordsViewerProps) {
  const [params] = useState<ListAuditRecordsParams>({ limit: 50 })
  const [selectedMatchId, setSelectedMatchId] = useState<string | null>(null)
  const [searchQuery, setSearchQuery] = useState('')
  const [filterAiOnly, setFilterAiOnly] = useState(false)

  const { data, isLoading, error, refetch } = useAuditRecords(params)

  // Filter records locally
  const filteredRecords = (data?.records ?? []).filter((record) => {
    if (filterAiOnly && !record.aiInvolved) return false
    if (searchQuery) {
      const query = searchQuery.toLowerCase()
      return (
        record.offerProduct.toLowerCase().includes(query) ||
        record.requestProduct.toLowerCase().includes(query) ||
        record.matchId.toLowerCase().includes(query)
      )
    }
    return true
  })

  return (
    <div className={cn('space-y-6', className)}>
      {/* Recorder Status */}
      <RecorderStatusPanel />

      {/* Filters */}
      <div className="flex flex-wrap items-center gap-3 p-4 rounded-xl bg-secondary/30 border border-border/50">
        <div className="relative flex-1 min-w-[200px]">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
          <Input
            placeholder="Search by product or match ID..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="pl-10 bg-background/50 border-border/50"
          />
        </div>

        <button
          onClick={() => setFilterAiOnly(!filterAiOnly)}
          className={cn(
            'flex items-center gap-2 px-3 py-2 rounded-lg text-sm transition-colors',
            filterAiOnly
              ? 'bg-violet-500/20 text-violet-400 border border-violet-500/30'
              : 'bg-secondary/50 text-muted-foreground border border-border/50',
          )}
        >
          <Sparkles className="w-4 h-4" />
          AI Only
        </button>

        <button
          onClick={() => refetch()}
          className="p-2 rounded-lg bg-secondary/50 hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
        >
          <RefreshCw className="w-4 h-4" />
        </button>

        <span className="text-sm text-muted-foreground">
          {filteredRecords.length} of {data?.total ?? 0} records
        </span>
      </div>

      {/* Content */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Records List */}
        <div className="lg:col-span-1 space-y-3">
          {isLoading ? (
            <div className="flex items-center justify-center py-12">
              <RefreshCw className="w-6 h-6 text-muted-foreground animate-spin" />
            </div>
          ) : error ? (
            <div className="p-4 rounded-xl bg-red-500/10 border border-red-500/20 text-center">
              <p className="text-red-400">Failed to load audit records</p>
              <button
                onClick={() => refetch()}
                className="mt-2 text-sm text-red-400 hover:text-red-300"
              >
                Retry
              </button>
            </div>
          ) : filteredRecords.length === 0 ? (
            <div className="text-center py-12">
              <Eye className="w-12 h-12 text-muted-foreground/20 mx-auto mb-3" />
              <p className="text-muted-foreground">No audit records found</p>
            </div>
          ) : (
            filteredRecords.map((record) => (
              <AuditRecordCard
                key={record.id}
                record={record}
                isSelected={selectedMatchId === record.matchId}
                onSelect={() => setSelectedMatchId(record.matchId)}
              />
            ))
          )}
        </div>

        {/* Detail Panel */}
        <div className="lg:col-span-2">
          {selectedMatchId ? (
            <AuditRecordDetail
              matchId={selectedMatchId}
              onClose={() => setSelectedMatchId(null)}
            />
          ) : (
            <div className="flex items-center justify-center h-full min-h-[400px] rounded-xl bg-secondary/20 border border-border/30">
              <div className="text-center">
                <Activity className="w-12 h-12 text-muted-foreground/20 mx-auto mb-3" />
                <p className="text-muted-foreground">
                  Select a record to view details
                </p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
