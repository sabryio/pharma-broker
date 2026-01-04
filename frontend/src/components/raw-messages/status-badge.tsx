// Status Badge Component for Raw Messages
import { Badge } from '@/components/ui/badge'
import { CheckCircle2, Clock, AlertTriangle } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { RawMessage } from './types'

interface StatusBadgeProps {
  message: RawMessage
  compact?: boolean
}

export function StatusBadge({ message, compact = false }: StatusBadgeProps) {
  if (message.error) {
    return (
      <Badge
        variant="outline"
        className={cn(
          'border-destructive/50 text-destructive bg-destructive/10',
          compact && 'px-1.5 py-0 text-[10px]',
        )}
      >
        {!compact && <AlertTriangle className="w-3 h-3 mr-1" />}
        Error
      </Badge>
    )
  }

  if (message.processedAt) {
    return (
      <Badge
        variant="outline"
        className={cn(
          'border-emerald-500/50 text-emerald-500 bg-emerald-500/10',
          compact && 'px-1.5 py-0 text-[10px]',
        )}
      >
        {!compact && <CheckCircle2 className="w-3 h-3 mr-1" />}
        Processed
      </Badge>
    )
  }

  return (
    <Badge
      variant="outline"
      className={cn(
        'border-amber-500/50 text-amber-500 bg-amber-500/10',
        compact && 'px-1.5 py-0 text-[10px]',
      )}
    >
      {!compact && <Clock className="w-3 h-3 mr-1" />}
      Pending
    </Badge>
  )
}
