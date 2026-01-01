import React from 'react'
import { CheckCircle2, AlertCircle, Sparkles, HelpCircle } from 'lucide-react'
import { cn } from '@/lib/utils'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'

interface MedicationCurationBadgeProps {
  status?: string | null
  masterId?: string | null
  className?: string
  onClick?: (e: React.MouseEvent) => void
}

export const MedicationCurationBadge: React.FC<
  MedicationCurationBadgeProps
> = ({ status, masterId, className, onClick }) => {
  const isApproved = status === 'Approved' || status === '"Approved"'
  const isRejected = status === 'Rejected' || status === '"Rejected"'
  const isPending = status === 'Pending' || status === '"Pending"'

  let Icon = HelpCircle
  let colorClass = 'text-muted-foreground bg-secondary/50'
  let label = 'Unknown'
  let description = 'Curation status unknown'

  if (isApproved && masterId) {
    Icon = CheckCircle2
    colorClass = 'text-emerald bg-emerald/10 border-emerald/20'
    label = 'Verified'
    description = 'Linked to canonical master record'
  } else if (isApproved) {
    Icon = Sparkles
    colorClass = 'text-blue-400 bg-blue-400/10 border-blue-400/20'
    label = 'Auto-Matched'
    description = 'Automatically matched to master'
  } else if (isRejected) {
    Icon = AlertCircle
    colorClass = 'text-destructive bg-destructive/10 border-destructive/20'
    label = 'Flagged'
    description = 'Invalid or blacklisted medication'
  } else if (isPending) {
    Icon = Sparkles
    colorClass = 'text-amber-400 bg-amber-400/10 border-amber-400/20'
    label = 'Unverified'
    description = 'Needs manual curation'
  }

  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            onClick={onClick}
            className={cn(
              'flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-medium border transition-all hover:scale-105 active:scale-95',
              colorClass,
              className,
            )}
          >
            <Icon className="w-3 h-3" />
            <span>{label}</span>
          </button>
        </TooltipTrigger>
        <TooltipContent side="top" className="text-xs">
          <p className="font-semibold">{label}</p>
          <p className="text-muted-foreground opacity-80">{description}</p>
          {!isApproved && (
            <p className="mt-1 flex items-center gap-1 text-teal">
              <Sparkles className="w-3 h-3" /> Click to curate
            </p>
          )}
        </TooltipContent>
      </Tooltip>
    </TooltipProvider>
  )
}
