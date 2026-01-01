import { ProgressRing } from '@/components/custom-ui/progress-ring'
import { AlertTriangle } from 'lucide-react'
import { cn } from '@/lib/utils'

interface ConfidenceIndicatorProps {
  confidence: number
  issues: string[]
}

export function ConfidenceIndicator({
  confidence,
  issues,
}: ConfidenceIndicatorProps) {
  return (
    <div className="flex flex-col items-center justify-center py-6">
      <div className="relative mb-6">
        <div
          className={cn(
            'absolute inset-0 rounded-full blur-2xl animate-pulse-slow',
            confidence >= 70 ? 'bg-amber/30' : 'bg-destructive/20',
          )}
        />
        <ProgressRing
          value={confidence}
          size={180}
          strokeWidth={12}
          label="Match"
          sublabel="Confidence"
        />
      </div>

      {/* Issues List */}
      <div className="w-full max-w-sm space-y-2">
        {issues.map((issue, idx) => (
          <div
            key={idx}
            className="flex items-start gap-2 p-2 rounded-lg bg-amber/10 border border-amber/20 animate-fade-in"
            style={{ animationDelay: `${idx * 100}ms` }}
          >
            <AlertTriangle className="w-4 h-4 text-amber shrink-0 mt-0.5" />
            <span className="text-xs text-amber">{issue}</span>
          </div>
        ))}
      </div>
    </div>
  )
}
