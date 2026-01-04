// Audit Records Viewer Component
// Displays audit records from the backend API with filtering and detail view
//
// Feature: debug-recording-enhancement
// Implements: Requirements 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 7.1, 7.2, 7.4, 7.5

import { useState, useRef } from 'react'
import { cn } from '@/lib/utils'
import {
  useAuditRecords,
  useAuditRecord,
  useAuditRecorderStatus,
  useUpdateAuditReview,
  usePipelineVisualization,
  type FrontendAuditRecord,
  type ListAuditRecordsParams,
  type ScoreComponentVisualization,
  type PipelineVisualizationResponse,
} from '@/hooks/use-audit-records'
import { useRecordingExport } from '@/hooks/use-recording-export'
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
  BarChart3,
  Zap,
  Target,
  Download,
  Upload,
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
// Score Breakdown Component (Requirements 6.5)
// =============================================================================

interface ScoreBreakdownPanelProps {
  scoreBreakdown: {
    finalScore: number
    formula: string
    components: ScoreComponentVisualization[]
    totalWeight: number
  }
}

function ScoreBreakdownPanel({ scoreBreakdown }: ScoreBreakdownPanelProps) {
  return (
    <div>
      <h4 className="text-sm font-semibold text-foreground mb-3 flex items-center gap-2">
        <BarChart3 className="w-4 h-4 text-pink-400" />
        Score Breakdown
      </h4>
      <div className="space-y-3">
        {/* Component scores */}
        <div className="grid grid-cols-2 md:grid-cols-3 gap-2">
          {scoreBreakdown.components.map((component) => (
            <div
              key={component.name}
              className="p-3 rounded-lg bg-background/50 border border-border/30"
            >
              <div className="flex items-center justify-between mb-1">
                <p className="text-xs text-muted-foreground capitalize">
                  {component.name}
                </p>
                <span className="text-[10px] text-muted-foreground/70">
                  w: {(component.weight * 100).toFixed(0)}%
                </span>
              </div>
              <div className="flex items-end gap-2">
                <p
                  className={cn(
                    'text-lg font-bold',
                    component.rawScore >= 0.8
                      ? 'text-emerald-400'
                      : component.rawScore >= 0.5
                        ? 'text-amber-400'
                        : 'text-red-400',
                  )}
                >
                  {(component.rawScore * 100).toFixed(0)}%
                </p>
                <p className="text-xs text-muted-foreground mb-0.5">
                  → {(component.weightedScore * 100).toFixed(1)}%
                </p>
              </div>
              {/* Progress bar */}
              <div className="mt-2 h-1.5 bg-secondary rounded-full overflow-hidden">
                <div
                  className={cn(
                    'h-full rounded-full transition-all',
                    component.rawScore >= 0.8
                      ? 'bg-emerald-500'
                      : component.rawScore >= 0.5
                        ? 'bg-amber-500'
                        : 'bg-red-500',
                  )}
                  style={{ width: `${component.rawScore * 100}%` }}
                />
              </div>
            </div>
          ))}
        </div>

        {/* Formula and final score */}
        <div className="p-3 rounded-lg bg-teal-500/10 border border-teal-500/20">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-xs text-muted-foreground mb-1">Formula</p>
              <p className="text-sm font-mono text-foreground">
                {scoreBreakdown.formula}
              </p>
            </div>
            <div className="text-right">
              <p className="text-xs text-muted-foreground mb-1">Final Score</p>
              <p className="text-2xl font-bold text-teal-400">
                {(scoreBreakdown.finalScore * 100).toFixed(1)}%
              </p>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

// =============================================================================
// Pipeline Visualization Panel (Requirements 6.2, 6.3, 6.4)
// =============================================================================

interface PipelineVisualizationPanelProps {
  visualization: PipelineVisualizationResponse
}

function PipelineVisualizationPanel({
  visualization,
}: PipelineVisualizationPanelProps) {
  return (
    <div className="space-y-4">
      {/* Pipeline Stages */}
      <div>
        <h4 className="text-sm font-semibold text-foreground mb-3 flex items-center gap-2">
          <Layers className="w-4 h-4 text-teal-400" />
          Pipeline Stages ({visualization.stages.length})
        </h4>
        <div className="space-y-2">
          {visualization.stages.map((stage, idx) => (
            <div
              key={idx}
              className={cn(
                'flex items-center gap-3 p-3 rounded-lg border',
                stage.involvesAi
                  ? 'bg-violet-500/10 border-violet-500/20'
                  : 'bg-background/50 border-border/30',
              )}
            >
              <div
                className={cn(
                  'w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold',
                  stage.involvesAi
                    ? 'bg-violet-500/20 text-violet-400'
                    : 'bg-teal-500/20 text-teal-400',
                )}
              >
                {idx + 1}
              </div>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-2">
                  <p className="text-sm font-medium text-foreground truncate">
                    {stage.stageName}
                  </p>
                  {stage.involvesAi && (
                    <Sparkles className="w-3 h-3 text-violet-400 flex-shrink-0" />
                  )}
                </div>
                <p className="text-xs text-muted-foreground">
                  {stage.candidatesIn} → {stage.candidatesOut} candidates
                </p>
              </div>
              <div className="text-right flex-shrink-0">
                <span className="text-xs text-muted-foreground">
                  {stage.durationMs}ms
                </span>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Hierarchical Details */}
      {visualization.hierarchicalDetails &&
        visualization.hierarchicalDetails.length > 0 && (
          <div>
            <h4 className="text-sm font-semibold text-foreground mb-3 flex items-center gap-2">
              <Target className="w-4 h-4 text-orange-400" />
              Hierarchical Matching
            </h4>
            <div className="space-y-2">
              {visualization.hierarchicalDetails.map((stage) => (
                <div
                  key={stage.stageNumber}
                  className="p-3 rounded-lg bg-orange-500/10 border border-orange-500/20"
                >
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-sm font-medium text-foreground">
                      Stage {stage.stageNumber}: {stage.stageName}
                    </span>
                    <span className="text-xs text-muted-foreground">
                      Threshold: {(stage.threshold * 100).toFixed(0)}%
                    </span>
                  </div>
                  <div className="flex items-center gap-4 text-xs text-muted-foreground">
                    <span>
                      {stage.candidatesIn} → {stage.candidatesOut} candidates
                    </span>
                    <span>{stage.durationMs}ms</span>
                    {stage.hasMatches && (
                      <span className="text-emerald-400">✓ Has matches</span>
                    )}
                  </div>
                  {stage.candidates.length > 0 && (
                    <div className="mt-2 flex flex-wrap gap-1">
                      {stage.candidates.slice(0, 5).map((candidate) => (
                        <span
                          key={candidate.id}
                          className={cn(
                            'px-2 py-0.5 rounded text-[10px]',
                            candidate.passed
                              ? 'bg-emerald-500/20 text-emerald-400'
                              : 'bg-secondary text-muted-foreground',
                          )}
                        >
                          {(candidate.score * 100).toFixed(0)}%
                        </span>
                      ))}
                      {stage.candidates.length > 5 && (
                        <span className="text-[10px] text-muted-foreground">
                          +{stage.candidates.length - 5} more
                        </span>
                      )}
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}

      {/* AI Review Details */}
      {visualization.aiReview && (
        <div>
          <h4 className="text-sm font-semibold text-foreground mb-3 flex items-center gap-2">
            <Sparkles className="w-4 h-4 text-violet-400" />
            AI Review
          </h4>
          <div className="p-4 rounded-lg bg-violet-500/10 border border-violet-500/20">
            <div className="grid grid-cols-3 gap-3 mb-3">
              <div>
                <p className="text-xs text-muted-foreground">Model</p>
                <p className="text-sm font-medium text-foreground">
                  {visualization.aiReview.modelName}
                </p>
              </div>
              <div>
                <p className="text-xs text-muted-foreground">Decision</p>
                <p
                  className={cn(
                    'text-sm font-medium',
                    visualization.aiReview.decision === 'approve'
                      ? 'text-emerald-400'
                      : visualization.aiReview.decision === 'reject'
                        ? 'text-red-400'
                        : 'text-amber-400',
                  )}
                >
                  {visualization.aiReview.decision}
                </p>
              </div>
              <div>
                <p className="text-xs text-muted-foreground">Confidence</p>
                <p className="text-sm font-medium text-foreground">
                  {(visualization.aiReview.confidence * 100).toFixed(0)}%
                </p>
              </div>
            </div>
            <div className="flex items-center gap-4 text-xs text-muted-foreground">
              <span>
                <Clock className="w-3 h-3 inline mr-1" />
                {visualization.aiReview.latencyMs}ms
              </span>
              {visualization.aiReview.tokenUsage && (
                <span>
                  {visualization.aiReview.tokenUsage.totalTokens} tokens
                </span>
              )}
            </div>
            {visualization.aiReview.reasoning && (
              <div className="mt-3 p-2 rounded bg-background/50 text-xs text-muted-foreground">
                {visualization.aiReview.reasoning}
              </div>
            )}
          </div>
        </div>
      )}

      {/* Score Breakdown */}
      {visualization.scoreBreakdown && (
        <ScoreBreakdownPanel scoreBreakdown={visualization.scoreBreakdown} />
      )}

      {/* Performance Metrics */}
      <div>
        <h4 className="text-sm font-semibold text-foreground mb-3 flex items-center gap-2">
          <Zap className="w-4 h-4 text-amber-400" />
          Performance Metrics
        </h4>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-2">
          <div className="p-2 rounded-lg bg-background/50 border border-border/30">
            <p className="text-xs text-muted-foreground">Total Latency</p>
            <p className="text-sm font-medium text-foreground">
              {visualization.totalLatencyMs}ms
            </p>
          </div>
          {visualization.performanceMetrics.totalAiTimeMs && (
            <div className="p-2 rounded-lg bg-background/50 border border-border/30">
              <p className="text-xs text-muted-foreground">AI Time</p>
              <p className="text-sm font-medium text-violet-400">
                {visualization.performanceMetrics.totalAiTimeMs}ms
              </p>
            </div>
          )}
          <div className="p-2 rounded-lg bg-background/50 border border-border/30">
            <p className="text-xs text-muted-foreground">DB Queries</p>
            <p className="text-sm font-medium text-foreground">
              {visualization.performanceMetrics.dbQueryCount}
            </p>
          </div>
          <div className="p-2 rounded-lg bg-background/50 border border-border/30">
            <p className="text-xs text-muted-foreground">DB Time</p>
            <p className="text-sm font-medium text-foreground">
              {visualization.performanceMetrics.dbTotalMs}ms
            </p>
          </div>
        </div>
      </div>
    </div>
  )
}

// =============================================================================
// Audit Record Detail Panel (Requirements 6.2, 6.3, 6.4, 6.5, 6.6, 7.1)
// =============================================================================

interface AuditRecordDetailProps {
  matchId: string
  onClose: () => void
}

function AuditRecordDetail({ matchId, onClose }: AuditRecordDetailProps) {
  const { data, isLoading, error, refetch } = useAuditRecord(matchId)
  const {
    data: pipelineData,
    isLoading: pipelineLoading,
    error: pipelineError,
    refetch: refetchPipeline,
  } = usePipelineVisualization(matchId)
  const updateReview = useUpdateAuditReview()
  const { exportRecording, isExporting } = useRecordingExport()
  const [activeTab, setActiveTab] = useState<'overview' | 'pipeline' | 'raw'>(
    'overview',
  )

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

  // Export handler (Requirement 7.1)
  const handleExport = async () => {
    try {
      await exportRecording(matchId, { includeBackendRecord: true })
      toast.success('Recording exported successfully')
    } catch (err) {
      toast.error('Failed to export recording')
    }
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

  // Error handling with retry (Requirement 6.6)
  if (error || !data) {
    return (
      <div className="p-6 rounded-xl bg-red-500/10 border border-red-500/20">
        <div className="text-center">
          <AlertTriangle className="w-8 h-8 text-red-400 mx-auto mb-2" />
          <p className="text-red-400 mb-3">Failed to load audit record</p>
          <button
            onClick={() => refetch()}
            className="px-4 py-2 rounded-lg bg-red-500/20 text-red-400 hover:bg-red-500/30 transition-colors text-sm"
          >
            Retry
          </button>
        </div>
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
        <div className="flex items-center gap-2">
          {/* Export Button (Requirement 7.1) */}
          <button
            onClick={handleExport}
            disabled={isExporting}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-cyan-500/20 text-cyan-400 hover:bg-cyan-500/30 transition-colors text-sm disabled:opacity-50"
            title="Export recording"
          >
            {isExporting ? (
              <RefreshCw className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <Download className="w-3.5 h-3.5" />
            )}
            Export
          </button>
          <button
            onClick={onClose}
            className="p-2 rounded-lg hover:bg-secondary transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex items-center gap-1 p-1 rounded-lg bg-secondary/30">
        {(['overview', 'pipeline', 'raw'] as const).map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            className={cn(
              'flex-1 px-4 py-2 rounded-md text-sm font-medium transition-all',
              activeTab === tab
                ? 'bg-teal-500 text-white shadow-lg'
                : 'text-muted-foreground hover:text-foreground hover:bg-secondary/50',
            )}
          >
            {tab.charAt(0).toUpperCase() + tab.slice(1)}
          </button>
        ))}
      </div>

      {/* Tab Content */}
      {activeTab === 'overview' && (
        <>
          {/* Score & Timing */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <div className="p-3 rounded-lg bg-background/50 border border-border/30">
              <p className="text-xs text-muted-foreground mb-1">Final Score</p>
              <p className="text-2xl font-bold text-teal-400">
                {(record.finalScore * 100).toFixed(1)}%
              </p>
            </div>
            <div className="p-3 rounded-lg bg-background/50 border border-border/30">
              <p className="text-xs text-muted-foreground mb-1">
                Total Latency
              </p>
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

          {/* Pipeline Stages Summary */}
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
            <span className="text-sm text-muted-foreground">
              Review Status:
            </span>
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
        </>
      )}

      {activeTab === 'pipeline' && (
        <>
          {pipelineLoading ? (
            <div className="flex items-center justify-center py-12">
              <RefreshCw className="w-6 h-6 text-muted-foreground animate-spin" />
            </div>
          ) : pipelineError ? (
            <div className="p-4 rounded-xl bg-red-500/10 border border-red-500/20 text-center">
              <AlertTriangle className="w-6 h-6 text-red-400 mx-auto mb-2" />
              <p className="text-red-400 mb-3">
                Failed to load pipeline visualization
              </p>
              <button
                onClick={() => refetchPipeline()}
                className="px-4 py-2 rounded-lg bg-red-500/20 text-red-400 hover:bg-red-500/30 transition-colors text-sm"
              >
                Retry
              </button>
            </div>
          ) : pipelineData ? (
            <PipelineVisualizationPanel visualization={pipelineData} />
          ) : (
            <div className="text-center py-12">
              <Activity className="w-12 h-12 text-muted-foreground/20 mx-auto mb-3" />
              <p className="text-muted-foreground">
                No pipeline visualization available
              </p>
            </div>
          )}
        </>
      )}

      {activeTab === 'raw' && (
        <>
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

          {/* Raw Score Breakdown */}
          <div>
            <h4 className="text-sm font-semibold text-foreground mb-3 flex items-center gap-2">
              <FileJson className="w-4 h-4 text-cyan-400" />
              Raw Score Breakdown
            </h4>
            <pre className="p-3 rounded-lg bg-background/50 border border-border/30 text-xs text-muted-foreground overflow-auto max-h-40">
              {JSON.stringify(record.scoreBreakdown, null, 2)}
            </pre>
          </div>
        </>
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
  const [importErrors, setImportErrors] = useState<string[]>([])
  const fileInputRef = useRef<HTMLInputElement>(null)

  const { data, isLoading, error, refetch } = useAuditRecords(params)
  const { importFromFile, isImporting } = useRecordingExport()

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

  // Import handler (Requirements 7.2, 7.5)
  const handleImport = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    if (!file) return

    setImportErrors([])
    const result = await importFromFile(file)

    if (result.valid) {
      toast.success('Recording imported successfully')
    } else {
      const errors = result.errors.map((e) => `${e.field}: ${e.message}`)
      setImportErrors(errors)
      toast.error('Import validation failed')
    }

    // Reset file input
    if (fileInputRef.current) {
      fileInputRef.current.value = ''
    }
  }

  return (
    <div className={cn('space-y-6', className)}>
      {/* Recorder Status */}
      <RecorderStatusPanel />

      {/* Import Errors Display (Requirement 7.5) */}
      {importErrors.length > 0 && (
        <div className="p-4 rounded-xl bg-red-500/10 border border-red-500/20">
          <div className="flex items-start justify-between mb-2">
            <h4 className="text-sm font-semibold text-red-400 flex items-center gap-2">
              <AlertTriangle className="w-4 h-4" />
              Import Validation Errors
            </h4>
            <button
              onClick={() => setImportErrors([])}
              className="p-1 rounded hover:bg-red-500/20 transition-colors"
            >
              <X className="w-4 h-4 text-red-400" />
            </button>
          </div>
          <ul className="space-y-1">
            {importErrors.map((error, idx) => (
              <li key={idx} className="text-xs text-red-400/80">
                • {error}
              </li>
            ))}
          </ul>
        </div>
      )}

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

        {/* Import Button (Requirements 7.2, 7.5) */}
        <input
          ref={fileInputRef}
          type="file"
          accept=".json"
          onChange={handleImport}
          className="hidden"
        />
        <button
          onClick={() => fileInputRef.current?.click()}
          disabled={isImporting}
          className="flex items-center gap-2 px-3 py-2 rounded-lg text-sm bg-cyan-500/20 text-cyan-400 hover:bg-cyan-500/30 transition-colors disabled:opacity-50"
          title="Import recording from JSON file"
        >
          {isImporting ? (
            <RefreshCw className="w-4 h-4 animate-spin" />
          ) : (
            <Upload className="w-4 h-4" />
          )}
          Import
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
              <AlertTriangle className="w-6 h-6 text-red-400 mx-auto mb-2" />
              <p className="text-red-400 mb-2">Failed to load audit records</p>
              <button
                onClick={() => refetch()}
                className="px-4 py-2 rounded-lg bg-red-500/20 text-red-400 hover:bg-red-500/30 transition-colors text-sm"
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
