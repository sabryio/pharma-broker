// Pipeline Timeline Component
// Visual timeline of pipeline execution stages

import { cn } from '@/lib/utils'
import { CheckCircle, XCircle, Clock, SkipForward, Loader2 } from 'lucide-react'
import type { PipelineRecording, PipelineStep } from './pipeline-types'
import { STAGE_COLORS, STATUS_COLORS } from './pipeline-types'

interface PipelineTimelineProps {
  recording: PipelineRecording
  onStepClick?: (step: PipelineStep) => void
  selectedStepId?: string
}

function formatDuration(ms: number): string {
  if (ms < 1) return '<1ms'
  if (ms < 1000) return `${Math.round(ms)}ms`
  return `${(ms / 1000).toFixed(2)}s`
}

function StepNode({ step, isSelected, onClick }: { step: PipelineStep; isSelected: boolean; onClick?: () => void }) {
  const stageColors = STAGE_COLORS[step.stage]
  const statusColors = STATUS_COLORS[step.status]

  const StatusIcon = {
    pending: Clock,
    running: Loader2,
    success: CheckCircle,
    error: XCircle,
    skipped: SkipForward,
  }[step.status]

  return (
    <button
      onClick={onClick}
      className={cn(
        'relative flex items-center gap-3 p-3 rounded-xl transition-all duration-200',
        'hover:bg-secondary/50 group',
        isSelected && 'bg-secondary/70 ring-2 ring-teal-500/50',
      )}
    >
      {/* Stage icon */}
      <div className={cn(
        'w-10 h-10 rounded-xl flex items-center justify-center text-lg shadow-md',
        'transition-transform group-hover:scale-110',
        stageColors.bg,
      )}>
        {stageColors.icon}
      </div>

      {/* Content */}
      <div className="flex-1 text-left">
        <div className="flex items-center gap-2">
          <span className={cn('text-sm font-medium', stageColors.text)}>
            {step.stage.replace(/_/g, ' ').replace(/\b\w/g, l => l.toUpperCase())}
          </span>
          <div className={cn(
            'flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium',
            statusColors.bg, statusColors.text,
          )}>
            <StatusIcon className={cn('w-3 h-3', step.status === 'running' && 'animate-spin')} />
            {step.status}
          </div>
        </div>
        <div className="flex items-center gap-2 text-xs text-muted-foreground mt-0.5">
          {step.durationMs !== undefined && (
            <span>{formatDuration(step.durationMs)}</span>
          )}
          {step.output && Object.keys(step.output).length > 0 && (
            <span className="text-muted-foreground/50">• Has output</span>
          )}
        </div>
      </div>

      {/* Connector line */}
      <div className="absolute left-[1.4rem] top-full w-0.5 h-3 bg-border/50" />
    </button>
  )
}

export function PipelineTimeline({ recording, onStepClick, selectedStepId }: PipelineTimelineProps) {
  const groupedSteps = recording.steps.reduce((acc, step) => {
    const category = getStepCategory(step.stage)
    if (!acc[category]) acc[category] = []
    acc[category].push(step)
    return acc
  }, {} as Record<string, PipelineStep[]>)

  return (
    <div className="space-y-6">
      {/* Summary stats */}
      <div className="grid grid-cols-4 gap-3">
        <div className="p-3 rounded-xl bg-secondary/30 border border-border/30 text-center">
          <p className="text-2xl font-bold text-foreground">{recording.steps.length}</p>
          <p className="text-xs text-muted-foreground">Total Steps</p>
        </div>
        <div className="p-3 rounded-xl bg-emerald-500/10 border border-emerald-500/20 text-center">
          <p className="text-2xl font-bold text-emerald-400">
            {recording.steps.filter(s => s.status === 'success').length}
          </p>
          <p className="text-xs text-muted-foreground">Successful</p>
        </div>
        <div className="p-3 rounded-xl bg-red-500/10 border border-red-500/20 text-center">
          <p className="text-2xl font-bold text-red-400">
            {recording.steps.filter(s => s.status === 'error').length}
          </p>
          <p className="text-xs text-muted-foreground">Errors</p>
        </div>
        <div className="p-3 rounded-xl bg-amber-500/10 border border-amber-500/20 text-center">
          <p className="text-2xl font-bold text-amber-400">
            {recording.totalDurationMs ? formatDuration(recording.totalDurationMs) : '—'}
          </p>
          <p className="text-xs text-muted-foreground">Total Time</p>
        </div>
      </div>

      {/* Timeline by category */}
      {Object.entries(groupedSteps).map(([category, steps]) => (
        <div key={category} className="space-y-2">
          <h4 className="text-xs font-semibold text-muted-foreground uppercase tracking-wider px-1">
            {category}
          </h4>
          <div className="space-y-1 relative">
            {steps.map((step) => (
              <StepNode
                key={step.id}
                step={step}
                isSelected={selectedStepId === step.id}
                onClick={() => onStepClick?.(step)}
              />
            ))}
          </div>
        </div>
      ))}
    </div>
  )
}

function getStepCategory(stage: string): string {
  if (stage.includes('message') || stage.includes('parsing')) return 'Input Processing'
  if (stage.includes('resolution')) return 'Medication Resolution'
  if (stage.includes('offer') || stage.includes('request')) return 'Entity Creation'
  if (stage.includes('match') || stage.includes('hierarchical') || stage.includes('score')) return 'Matching'
  if (stage.includes('ai') || stage.includes('consensus') || stage.includes('contrastive') || stage.includes('calibration')) return 'Validation'
  if (stage.includes('queue') || stage.includes('notification')) return 'Output'
  return 'Other'
}
