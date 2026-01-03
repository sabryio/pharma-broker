// Pipeline Viewer Component
// Main component for viewing internal pipeline recordings

import { useState, useMemo } from 'react'
import { cn } from '@/lib/utils'
import {
  X,
  Maximize2,
  Minimize2,
  Download,
  RefreshCw,
  Clock,
  Layers,
  AlertCircle,
} from 'lucide-react'
import type { PipelineRecording } from './pipeline-types'
import { PipelineTimeline } from './pipeline-timeline'

interface PipelineViewerProps {
  recording: PipelineRecording | null
  onClose?: () => void
  onRefresh?: () => void
  onExport?: () => void
}

type ViewTab =
  | 'timeline'
  | 'parsing'
  | 'resolution'
  | 'matching'
  | 'validation'
  | 'scores'

function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`
  return `${(ms / 1000).toFixed(2)}s`
}

export function PipelineViewer({
  recording,
  onClose,
  onRefresh,
  onExport,
}: PipelineViewerProps) {
  const [isExpanded, setIsExpanded] = useState(false)
  const [activeTab, setActiveTab] = useState<ViewTab>('timeline')

  const tabs: {
    id: ViewTab
    label: string
    icon: string
    available: boolean
  }[] = useMemo(
    () => [
      { id: 'timeline', label: 'Timeline', icon: '📊', available: true },
      {
        id: 'parsing',
        label: 'Parsing',
        icon: '🤖',
        available: !!recording?.parsing,
      },
      {
        id: 'resolution',
        label: 'Resolution',
        icon: '💊',
        available: !!recording?.resolution,
      },
      {
        id: 'matching',
        label: 'Matching',
        icon: '🔗',
        available: !!recording?.hierarchicalStages,
      },
      {
        id: 'validation',
        label: 'Validation',
        icon: '✅',
        available: !!(
          recording?.aiReview ||
          recording?.consensus ||
          recording?.contrastive
        ),
      },
      {
        id: 'scores',
        label: 'Scores',
        icon: '📈',
        available: !!(recording?.scoreBreakdown || recording?.calibration),
      },
    ],
    [recording],
  )

  if (!recording) {
    return (
      <div className="rounded-2xl border border-border/50 bg-gradient-to-br from-secondary/40 to-secondary/20 p-8 text-center">
        <AlertCircle className="w-12 h-12 text-muted-foreground/30 mx-auto mb-4" />
        <h3 className="text-lg font-semibold text-foreground mb-2">
          No Pipeline Recording
        </h3>
        <p className="text-sm text-muted-foreground mb-4">
          Pipeline recordings show the internal processing stages during
          matching.
        </p>
        <p className="text-xs text-muted-foreground">
          This feature requires backend integration to capture pipeline data.
        </p>
      </div>
    )
  }

  return (
    <div
      className={cn(
        'rounded-2xl overflow-hidden transition-all duration-500 border border-border/50',
        'bg-gradient-to-br from-background via-background to-secondary/30 backdrop-blur-xl',
        isExpanded ? 'fixed inset-4 z-50' : 'relative',
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-border/50 bg-gradient-to-r from-slate-900/80 to-slate-800/80">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-violet-500/30 to-purple-500/30 flex items-center justify-center shadow-lg">
            <Layers className="w-5 h-5 text-violet-400" />
          </div>
          <div>
            <h3 className="text-sm font-semibold text-foreground">
              Pipeline Recording
            </h3>
            <p className="text-xs text-muted-foreground">
              Match{' '}
              <span className="font-mono">
                #{recording.matchId.slice(0, 8)}
              </span>{' '}
              • {recording.steps.length} stages
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <div
            className={cn(
              'flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-semibold',
              recording.status === 'completed'
                ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30'
                : recording.status === 'running'
                  ? 'bg-amber-500/20 text-amber-400 border border-amber-500/30 animate-pulse'
                  : 'bg-red-500/20 text-red-400 border border-red-500/30',
            )}
          >
            {recording.status === 'running' && (
              <RefreshCw className="w-3 h-3 animate-spin" />
            )}
            {recording.status.charAt(0).toUpperCase() +
              recording.status.slice(1)}
          </div>

          {recording.totalDurationMs && (
            <div className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-secondary/50 text-xs text-muted-foreground">
              <Clock className="w-3.5 h-3.5" />
              {formatDuration(recording.totalDurationMs)}
            </div>
          )}

          {onRefresh && (
            <button
              onClick={onRefresh}
              className="p-2 rounded-lg bg-secondary/50 hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
              title="Refresh"
            >
              <RefreshCw className="w-4 h-4" />
            </button>
          )}
          {onExport && (
            <button
              onClick={onExport}
              className="p-2 rounded-lg bg-secondary/50 hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
              title="Export JSON"
            >
              <Download className="w-4 h-4" />
            </button>
          )}
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="p-2 rounded-lg bg-secondary/50 hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
            title={isExpanded ? 'Minimize' : 'Maximize'}
          >
            {isExpanded ? (
              <Minimize2 className="w-4 h-4" />
            ) : (
              <Maximize2 className="w-4 h-4" />
            )}
          </button>
          {onClose && (
            <button
              onClick={onClose}
              className="p-2 rounded-lg bg-secondary/50 hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
              title="Close"
            >
              <X className="w-4 h-4" />
            </button>
          )}
        </div>
      </div>

      {/* Tabs */}
      <div className="flex items-center gap-1 p-2 border-b border-border/30 bg-secondary/10 overflow-x-auto">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => tab.available && setActiveTab(tab.id)}
            disabled={!tab.available}
            className={cn(
              'flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-all whitespace-nowrap',
              activeTab === tab.id
                ? 'bg-teal-500 text-white shadow-lg shadow-teal-500/20'
                : tab.available
                  ? 'text-muted-foreground hover:text-foreground hover:bg-secondary/50'
                  : 'text-muted-foreground/30 cursor-not-allowed',
            )}
          >
            <span>{tab.icon}</span>
            {tab.label}
          </button>
        ))}
      </div>

      {/* Content */}
      <div
        className={cn(
          'p-6 overflow-auto',
          isExpanded ? 'max-h-[calc(100vh-200px)]' : 'max-h-[600px]',
        )}
      >
        {activeTab === 'timeline' && <PipelineTimeline recording={recording} />}

        {activeTab === 'parsing' && recording.parsing && (
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <ParsingCard parsing={recording.parsing.offer} type="offer" />
            <ParsingCard parsing={recording.parsing.request} type="request" />
          </div>
        )}

        {activeTab === 'scores' && recording.scoreBreakdown && (
          <ScoreBreakdownCard breakdown={recording.scoreBreakdown} />
        )}
      </div>

      {/* Footer with final score */}
      {recording.finalScore !== undefined && (
        <div className="flex items-center justify-between p-4 border-t border-border/30 bg-secondary/10">
          <div className="flex items-center gap-4">
            <span className="text-sm text-muted-foreground">Final Score:</span>
            <span
              className={cn(
                'text-2xl font-bold tabular-nums',
                recording.finalScore >= 0.8
                  ? 'text-emerald-400'
                  : recording.finalScore >= 0.6
                    ? 'text-amber-400'
                    : 'text-red-400',
              )}
            >
              {(recording.finalScore * 100).toFixed(1)}%
            </span>
          </div>
          {recording.finalStatus && (
            <div
              className={cn(
                'px-4 py-2 rounded-lg text-sm font-semibold',
                recording.finalStatus === 'auto_approved'
                  ? 'bg-emerald-500/20 text-emerald-400'
                  : recording.finalStatus === 'needs_review'
                    ? 'bg-amber-500/20 text-amber-400'
                    : 'bg-secondary text-muted-foreground',
              )}
            >
              {recording.finalStatus === 'auto_approved'
                ? '✓ Auto-Approved'
                : recording.finalStatus === 'needs_review'
                  ? '⏳ Needs Review'
                  : '○ Pending'}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

// Helper components
function ParsingCard({
  parsing,
  type,
}: {
  parsing: any
  type: 'offer' | 'request'
}) {
  return (
    <div className="p-4 rounded-xl bg-secondary/30 border border-border/30">
      <div className="flex items-center gap-2 mb-3">
        <div
          className={cn(
            'w-6 h-6 rounded-md flex items-center justify-center text-xs font-bold',
            type === 'offer'
              ? 'bg-teal-500/20 text-teal-400'
              : 'bg-violet-500/20 text-violet-400',
          )}
        >
          {type === 'offer' ? 'O' : 'R'}
        </div>
        <span className="text-sm font-semibold text-foreground capitalize">
          {type} Parsing
        </span>
      </div>
      <div className="space-y-2 text-sm">
        <div className="p-2 rounded-lg bg-background/50">
          <p className="text-xs text-muted-foreground mb-1">Raw Message</p>
          <p className="text-foreground" dir="auto">
            {parsing.rawMessage}
          </p>
        </div>
        <div className="grid grid-cols-2 gap-2">
          <div className="p-2 rounded-lg bg-background/50">
            <p className="text-xs text-muted-foreground">Medication</p>
            <p className="text-foreground font-medium">
              {parsing.parsedFields.medication}
            </p>
          </div>
          <div className="p-2 rounded-lg bg-background/50">
            <p className="text-xs text-muted-foreground">Confidence</p>
            <p className="text-foreground font-medium">
              {(parsing.confidence * 100).toFixed(0)}%
            </p>
          </div>
        </div>
      </div>
    </div>
  )
}

function ScoreBreakdownCard({ breakdown }: { breakdown: any }) {
  const scores = [
    {
      label: 'Medication',
      score: breakdown.medicationScore,
      weight: breakdown.medicationWeight,
    },
    {
      label: 'Raw Text',
      score: breakdown.rawTextScore,
      weight: breakdown.rawTextWeight,
    },
    {
      label: 'Embedding',
      score: breakdown.embeddingScore,
      weight: breakdown.embeddingWeight,
    },
    {
      label: 'Dosage',
      score: breakdown.dosageScore,
      weight: breakdown.dosageWeight,
    },
    {
      label: 'Quantity',
      score: breakdown.quantityScore,
      weight: breakdown.quantityWeight,
    },
  ].filter((s) => s.score !== null)

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-3">
        {scores.map(({ label, score, weight }) => (
          <div
            key={label}
            className="p-3 rounded-xl bg-secondary/30 border border-border/30 text-center"
          >
            <p className="text-xs text-muted-foreground mb-1">{label}</p>
            <p
              className={cn(
                'text-xl font-bold',
                score >= 0.8
                  ? 'text-emerald-400'
                  : score >= 0.6
                    ? 'text-amber-400'
                    : 'text-red-400',
              )}
            >
              {(score * 100).toFixed(0)}%
            </p>
            <p className="text-[10px] text-muted-foreground/70">
              Weight: {(weight * 100).toFixed(0)}%
            </p>
          </div>
        ))}
      </div>
      <div className="p-4 rounded-xl bg-teal-500/10 border border-teal-500/20 text-center">
        <p className="text-sm text-muted-foreground mb-1">Final Score</p>
        <p className="text-3xl font-bold text-teal-400">
          {(breakdown.finalScore * 100).toFixed(1)}%
        </p>
      </div>
    </div>
  )
}

// Mock data generator
export function generateMockPipelineRecording(
  matchId: string,
): PipelineRecording {
  const now = new Date()
  const startTime = new Date(now.getTime() - 2500)

  return {
    id: `pipeline-${matchId}`,
    matchId,
    offerId: `offer-${matchId.slice(0, 8)}`,
    requestId: `request-${matchId.slice(0, 8)}`,
    startedAt: startTime.toISOString(),
    completedAt: now.toISOString(),
    totalDurationMs: 2500,
    status: 'completed',
    steps: [
      {
        id: 'step-1',
        stage: 'message_received',
        status: 'success',
        startedAt: startTime.toISOString(),
        durationMs: 50,
      },
      {
        id: 'step-2',
        stage: 'ai_parsing',
        status: 'success',
        startedAt: new Date(startTime.getTime() + 50).toISOString(),
        durationMs: 400,
      },
      {
        id: 'step-3',
        stage: 'medication_resolution',
        status: 'success',
        startedAt: new Date(startTime.getTime() + 450).toISOString(),
        durationMs: 100,
      },
      {
        id: 'step-4',
        stage: 'match_candidate_search',
        status: 'success',
        startedAt: new Date(startTime.getTime() + 550).toISOString(),
        durationMs: 100,
      },
      {
        id: 'step-5',
        stage: 'hierarchical_stage_1',
        status: 'success',
        startedAt: new Date(startTime.getTime() + 650).toISOString(),
        durationMs: 20,
      },
      {
        id: 'step-6',
        stage: 'score_calculation',
        status: 'success',
        startedAt: new Date(startTime.getTime() + 670).toISOString(),
        durationMs: 30,
      },
      {
        id: 'step-7',
        stage: 'ai_review',
        status: 'success',
        startedAt: new Date(startTime.getTime() + 700).toISOString(),
        durationMs: 750,
      },
      {
        id: 'step-8',
        stage: 'calibration',
        status: 'success',
        startedAt: new Date(startTime.getTime() + 1450).toISOString(),
        durationMs: 20,
      },
      {
        id: 'step-9',
        stage: 'match_created',
        status: 'success',
        startedAt: new Date(startTime.getTime() + 1470).toISOString(),
        durationMs: 30,
      },
    ],
    parsing: {
      offer: {
        rawMessage: 'عندي اوجمنتين 1 جرام 20 علبة السعر 180 جنيه',
        language: 'ar',
        detectedType: 'offer',
        aiModel: 'gpt-4o-mini',
        temperature: 0.2,
        promptTokens: 150,
        completionTokens: 95,
        parsedFields: {
          medication: 'Augmentin 1g',
          medicationRaw: 'اوجمنتين 1 جرام',
          quantity: 20,
          unit: 'boxes',
          price: 180,
          currency: 'EGP',
          expiryDate: null,
          urgencyLevel: null,
        },
        confidence: 0.94,
      },
      request: {
        rawMessage: 'محتاج اوجمنتين جرام 10 علب ضروري',
        language: 'ar',
        detectedType: 'request',
        aiModel: 'gpt-4o-mini',
        temperature: 0.2,
        promptTokens: 140,
        completionTokens: 88,
        parsedFields: {
          medication: 'Augmentin 1g',
          medicationRaw: 'اوجمنتين جرام',
          quantity: 10,
          unit: 'boxes',
          price: null,
          currency: null,
          expiryDate: null,
          urgencyLevel: 'urgent',
        },
        confidence: 0.91,
      },
    },
    scoreBreakdown: {
      medicationScore: 0.95,
      medicationWeight: 0.85,
      rawTextScore: 0.88,
      rawTextWeight: 0.05,
      embeddingScore: 0.91,
      embeddingWeight: 0.05,
      dosageScore: 1.0,
      dosageWeight: 0.02,
      quantityScore: 0.8,
      quantityWeight: 0.01,
      priceScore: null,
      priceWeight: 0.01,
      recencyScore: 0.95,
      recencyWeight: 0.005,
      aiLogicScore: 0.89,
      aiLogicWeight: 0.005,
      finalScore: 0.82,
      formula: 'Σ(score_i × weight_i)',
    },
    finalScore: 0.78,
    finalStatus: 'needs_review',
  }
}
