// Reasoning Panel Component
// Score breakdown visualization with AI reasoning

import { useState } from 'react'
import { cn } from '@/lib/utils'
import {
  ChevronDown,
  ChevronUp,
  Sparkles,
  Package,
  DollarSign,
  Hash,
  Calendar,
  MessageSquare,
  Lightbulb,
  BarChart3,
  Info,
} from 'lucide-react'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'

interface ScoreFactor {
  name: string
  weight: number
  score: number
  contribution: number
  description: string
  icon: React.ReactNode
}

interface ReasoningPanelProps {
  confidence: number
  reasoning: string | null
  aiStatus?: string | null
  aiConfidence?: number | null
  aiExplanation?: string | null
  issues: string[]
  className?: string
}

// Simulated score breakdown - in production this would come from the API
function generateScoreBreakdown(confidence: number): ScoreFactor[] {
  // Distribute the confidence score across factors
  const baseScore = confidence / 100

  return [
    {
      name: 'Medication Match',
      weight: 0.4,
      score: Math.min(1, baseScore * 1.1),
      contribution: 0,
      description: 'How closely the medication names match',
      icon: <Package className="w-4 h-4" />,
    },
    {
      name: 'Quantity Fit',
      weight: 0.2,
      score: Math.min(1, baseScore * 0.95),
      contribution: 0,
      description: 'Whether the offered quantity meets the request',
      icon: <Hash className="w-4 h-4" />,
    },
    {
      name: 'Price Match',
      weight: 0.2,
      score: Math.min(1, baseScore * 1.05),
      contribution: 0,
      description: 'Price within acceptable range',
      icon: <DollarSign className="w-4 h-4" />,
    },
    {
      name: 'Freshness',
      weight: 0.1,
      score: Math.min(1, baseScore * 0.9),
      contribution: 0,
      description: 'How recent the offer/request is',
      icon: <Calendar className="w-4 h-4" />,
    },
    {
      name: 'Source Trust',
      weight: 0.1,
      score: Math.min(1, baseScore * 1.0),
      contribution: 0,
      description: 'Reliability of the source group',
      icon: <MessageSquare className="w-4 h-4" />,
    },
  ].map((factor) => ({
    ...factor,
    contribution: factor.weight * factor.score * 100,
  }))
}

function ScoreBar({
  factor,
  maxContribution,
}: {
  factor: ScoreFactor
  maxContribution: number
}) {
  const percentage = (factor.contribution / maxContribution) * 100
  const scoreColor =
    factor.score >= 0.8
      ? 'from-emerald to-emerald/60'
      : factor.score >= 0.5
        ? 'from-amber to-amber/60'
        : 'from-red-400 to-red-400/60'

  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>
          <div className="group cursor-help">
            <div className="flex items-center justify-between mb-1">
              <div className="flex items-center gap-2">
                <div
                  className={cn(
                    'w-6 h-6 rounded-md flex items-center justify-center',
                    'bg-secondary/80 group-hover:bg-secondary transition-colors',
                  )}
                >
                  {factor.icon}
                </div>
                <span className="text-xs font-medium text-foreground">
                  {factor.name}
                </span>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-[10px] text-muted-foreground">
                  {Math.round(factor.weight * 100)}% weight
                </span>
                <span
                  className={cn(
                    'text-xs font-bold',
                    factor.score >= 0.8 && 'text-emerald',
                    factor.score >= 0.5 && factor.score < 0.8 && 'text-amber',
                    factor.score < 0.5 && 'text-red-400',
                  )}
                >
                  {Math.round(factor.score * 100)}%
                </span>
              </div>
            </div>
            <div className="h-2 bg-secondary/50 rounded-full overflow-hidden">
              <div
                className={cn(
                  'h-full rounded-full bg-linear-to-r transition-all duration-500',
                  scoreColor,
                )}
                style={{ width: `${percentage}%` }}
              />
            </div>
          </div>
        </TooltipTrigger>
        <TooltipContent side="top" className="max-w-[200px]">
          <p className="text-xs">{factor.description}</p>
          <p className="text-xs text-muted-foreground mt-1">
            Contributes {factor.contribution.toFixed(1)} points to final score
          </p>
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}

export function ReasoningPanel({
  confidence,
  reasoning,
  aiStatus,
  aiConfidence,
  aiExplanation,
  issues,
  className,
}: ReasoningPanelProps) {
  const [isExpanded, setIsExpanded] = useState(false)
  const factors = generateScoreBreakdown(confidence)
  const maxContribution = Math.max(...factors.map((f) => f.contribution))

  return (
    <div
      className={cn(
        'rounded-xl border overflow-hidden transition-all duration-300',
        'bg-linear-to-br from-slate-900/80 via-slate-800/60 to-slate-900/80',
        'border-slate-700/50',
        className,
      )}
    >
      {/* Header - Always visible */}
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center justify-between p-4 hover:bg-white/5 transition-colors"
      >
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 rounded-lg bg-linear-to-br from-cyan-500/30 to-blue-500/30 flex items-center justify-center">
            <BarChart3 className="w-4 h-4 text-cyan-400" />
          </div>
          <div className="text-left">
            <h4 className="text-sm font-semibold text-white">
              Score Breakdown
            </h4>
            <p className="text-[10px] text-slate-400">
              {factors.length} factors • {confidence.toFixed(1)}% confidence
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {aiStatus && (
            <span
              className={cn(
                'px-2 py-0.5 rounded-full text-[10px] font-medium',
                aiStatus === 'Approved' && 'bg-emerald/20 text-emerald',
                aiStatus === 'Flagged' && 'bg-amber/20 text-amber',
                aiStatus === 'Rejected' && 'bg-red-400/20 text-red-400',
              )}
            >
              AI {aiStatus}
            </span>
          )}
          {isExpanded ? (
            <ChevronUp className="w-4 h-4 text-muted-foreground" />
          ) : (
            <ChevronDown className="w-4 h-4 text-muted-foreground" />
          )}
        </div>
      </button>

      {/* Expanded Content */}
      {isExpanded && (
        <div className="px-4 pb-4 space-y-4 animate-fade-in">
          {/* Score Factors */}
          <div className="space-y-3">
            {factors.map((factor, idx) => (
              <div
                key={factor.name}
                className="animate-fade-in"
                style={{ animationDelay: `${idx * 50}ms` }}
              >
                <ScoreBar factor={factor} maxContribution={maxContribution} />
              </div>
            ))}
          </div>

          {/* Visual Pie Chart Representation */}
          <div className="flex items-center justify-center py-4">
            <div className="relative w-32 h-32">
              <svg viewBox="0 0 100 100" className="w-full h-full -rotate-90">
                {
                  factors.reduce(
                    (acc, factor, idx) => {
                      const percentage =
                        (factor.contribution /
                          factors.reduce((s, f) => s + f.contribution, 0)) *
                        100
                      const colors = [
                        '#00E676',
                        '#F59E0B',
                        '#00F2FF',
                        '#A855F7',
                        '#EC4899',
                      ]
                      const circumference = 2 * Math.PI * 40
                      const strokeDasharray = `${(percentage / 100) * circumference} ${circumference}`

                      acc.elements.push(
                        <circle
                          key={factor.name}
                          cx="50"
                          cy="50"
                          r="40"
                          fill="none"
                          stroke={colors[idx % colors.length]}
                          strokeWidth="20"
                          strokeDasharray={strokeDasharray}
                          strokeDashoffset={-acc.offset}
                          className="transition-all duration-500"
                          style={{ animationDelay: `${idx * 100}ms` }}
                        />,
                      )
                      acc.offset += (percentage / 100) * circumference
                      return acc
                    },
                    { elements: [] as React.ReactNode[], offset: 0 },
                  ).elements
                }
              </svg>
              <div className="absolute inset-0 flex flex-col items-center justify-center">
                <span className="text-2xl font-bold text-white">
                  {confidence.toFixed(0)}%
                </span>
                <span className="text-[10px] text-slate-400">Total</span>
              </div>
            </div>
          </div>

          {/* AI Reasoning */}
          {(reasoning || aiExplanation) && (
            <div className="p-3 rounded-lg bg-violet-500/10 border border-violet-500/20">
              <div className="flex items-start gap-2">
                <Sparkles className="w-4 h-4 text-violet-400 mt-0.5 shrink-0" />
                <div>
                  <h5 className="text-xs font-medium text-violet-300 mb-1">
                    AI Analysis
                  </h5>
                  <p className="text-xs text-slate-300 leading-relaxed">
                    {aiExplanation || reasoning}
                  </p>
                  {aiConfidence && (
                    <div className="mt-2 flex items-center gap-2">
                      <div className="flex-1 h-1 bg-slate-700 rounded-full overflow-hidden">
                        <div
                          className="h-full bg-violet-500 rounded-full"
                          style={{ width: `${aiConfidence * 100}%` }}
                        />
                      </div>
                      <span className="text-[10px] text-violet-400">
                        {Math.round(aiConfidence * 100)}% AI confidence
                      </span>
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}

          {/* Issues */}
          {issues.length > 0 && (
            <div className="space-y-2">
              <h5 className="text-xs font-medium text-slate-300 flex items-center gap-2">
                <Info className="w-3.5 h-3.5 text-amber" />
                Potential Issues
              </h5>
              {issues.map((issue, idx) => (
                <div
                  key={idx}
                  className="flex items-start gap-2 p-2 rounded-lg bg-amber/10 border border-amber/20 text-xs text-amber"
                >
                  <span className="shrink-0">⚠️</span>
                  <span>{issue}</span>
                </div>
              ))}
            </div>
          )}

          {/* Suggestions */}
          {confidence < 70 && (
            <div className="p-3 rounded-lg bg-cyan-500/10 border border-cyan-500/20">
              <div className="flex items-start gap-2">
                <Lightbulb className="w-4 h-4 text-cyan-400 mt-0.5 shrink-0" />
                <div>
                  <h5 className="text-xs font-medium text-cyan-300 mb-1">
                    Suggestions
                  </h5>
                  <ul className="text-xs text-slate-300 space-y-1">
                    {confidence < 50 && (
                      <li>
                        • Consider manual verification of medication names
                      </li>
                    )}
                    {confidence < 70 && (
                      <li>• Check if quantities are compatible</li>
                    )}
                    <li>• Verify pricing is within acceptable range</li>
                  </ul>
                </div>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
