// Sender Profile Component
// Rich sender profile with avatar, stats, and trust indicators

import { useState } from 'react'
import { cn } from '@/lib/utils'
import { useParticipantStats, useParticipantByJid } from '@/hooks/use-participants'
import {
  Shield,
  ShieldCheck,
  ShieldAlert,
  TrendingUp,
  Package,
  ShoppingCart,
  CheckCircle,
  XCircle,
  Clock,
  Loader2,
  ChevronDown,
  ChevronUp,
} from 'lucide-react'

interface SenderProfileProps {
  participantId?: string
  senderName?: string | null
  senderJid?: string | null
  compact?: boolean
  showStats?: boolean
}

// Generate avatar color from name/jid
function getAvatarColor(name: string): string {
  const colors = [
    'bg-teal/20 text-teal',
    'bg-amber/20 text-amber',
    'bg-emerald/20 text-emerald',
    'bg-violet-500/20 text-violet-400',
    'bg-rose-500/20 text-rose-400',
    'bg-cyan-500/20 text-cyan-400',
    'bg-orange-500/20 text-orange-400',
  ]
  let hash = 0
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash)
  }
  return colors[Math.abs(hash) % colors.length]
}

// Get initials from name
function getInitials(name: string): string {
  const parts = name.trim().split(/\s+/)
  if (parts.length >= 2) {
    return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase()
  }
  return name.slice(0, 2).toUpperCase()
}

// Reputation badge component
function ReputationBadge({ reputation }: { reputation: 'new' | 'regular' | 'trusted' }) {
  const config = {
    new: {
      icon: ShieldAlert,
      label: 'New',
      className: 'bg-amber-500/20 text-amber-400 border-amber-500/30',
    },
    regular: {
      icon: Shield,
      label: 'Regular',
      className: 'bg-blue-500/20 text-blue-400 border-blue-500/30',
    },
    trusted: {
      icon: ShieldCheck,
      label: 'Trusted',
      className: 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30',
    },
  }

  const { icon: Icon, label, className } = config[reputation]

  return (
    <div
      className={cn(
        'flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium border',
        className,
      )}
    >
      <Icon className="w-3 h-3" />
      {label}
    </div>
  )
}

export function SenderProfile({
  participantId,
  senderName,
  senderJid,
  compact = false,
  showStats = true,
}: SenderProfileProps) {
  const [expanded, setExpanded] = useState(false)
  
  // Try to fetch stats by ID first, then by JID
  const { data: statsById, isLoading: loadingById } = useParticipantStats(participantId)
  const { data: statsByJid, isLoading: loadingByJid } = useParticipantByJid(
    !participantId && senderJid ? senderJid : undefined
  )
  
  const stats = statsById || statsByJid
  const isLoading = loadingById || loadingByJid

  const displayName = stats?.displayName || senderName || 'Unknown Sender'
  const avatarColor = getAvatarColor(displayName)
  const initials = getInitials(displayName)

  if (compact) {
    return (
      <div className="flex items-center gap-2">
        {/* Avatar */}
        <div
          className={cn(
            'w-8 h-8 rounded-full flex items-center justify-center text-xs font-bold',
            avatarColor,
          )}
        >
          {initials}
        </div>
        
        {/* Name & reputation */}
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-foreground truncate max-w-[120px]">
            {displayName}
          </span>
          {stats && <ReputationBadge reputation={stats.reputation} />}
        </div>
      </div>
    )
  }

  return (
    <div className="space-y-2">
      {/* Header */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center justify-between p-3 rounded-xl bg-secondary/30 hover:bg-secondary/50 transition-colors"
      >
        <div className="flex items-center gap-3">
          {/* Avatar */}
          <div
            className={cn(
              'w-10 h-10 rounded-full flex items-center justify-center text-sm font-bold',
              avatarColor,
            )}
          >
            {initials}
          </div>

          {/* Info */}
          <div className="text-left">
            <div className="flex items-center gap-2">
              <span className="font-medium text-foreground">{displayName}</span>
              {stats && <ReputationBadge reputation={stats.reputation} />}
            </div>
            {senderJid && (
              <p className="text-xs text-muted-foreground truncate max-w-[200px]">
                {senderJid}
              </p>
            )}
          </div>
        </div>

        {showStats && (
          <div className="flex items-center gap-2">
            {isLoading ? (
              <Loader2 className="w-4 h-4 animate-spin text-muted-foreground" />
            ) : (
              expanded ? (
                <ChevronUp className="w-4 h-4 text-muted-foreground" />
              ) : (
                <ChevronDown className="w-4 h-4 text-muted-foreground" />
              )
            )}
          </div>
        )}
      </button>

      {/* Expanded stats */}
      {expanded && showStats && stats && (
        <div className="p-3 rounded-xl bg-secondary/20 border border-border/30 space-y-3 animate-in slide-in-from-top-2 duration-200">
          {/* Activity stats */}
          <div className="grid grid-cols-2 gap-2">
            <div className="flex items-center gap-2 p-2 rounded-lg bg-teal/10">
              <Package className="w-4 h-4 text-teal" />
              <div>
                <p className="text-xs text-muted-foreground">Offers</p>
                <p className="text-sm font-bold text-teal">{stats.totalOffers}</p>
              </div>
            </div>
            <div className="flex items-center gap-2 p-2 rounded-lg bg-amber/10">
              <ShoppingCart className="w-4 h-4 text-amber" />
              <div>
                <p className="text-xs text-muted-foreground">Requests</p>
                <p className="text-sm font-bold text-amber">{stats.totalRequests}</p>
              </div>
            </div>
          </div>

          {/* Match stats */}
          <div className="grid grid-cols-3 gap-2">
            <div className="flex items-center gap-1.5 p-2 rounded-lg bg-emerald/10">
              <CheckCircle className="w-3.5 h-3.5 text-emerald" />
              <div>
                <p className="text-[10px] text-muted-foreground">Confirmed</p>
                <p className="text-xs font-bold text-emerald">{stats.confirmedMatches}</p>
              </div>
            </div>
            <div className="flex items-center gap-1.5 p-2 rounded-lg bg-red-500/10">
              <XCircle className="w-3.5 h-3.5 text-red-400" />
              <div>
                <p className="text-[10px] text-muted-foreground">Rejected</p>
                <p className="text-xs font-bold text-red-400">{stats.rejectedMatches}</p>
              </div>
            </div>
            <div className="flex items-center gap-1.5 p-2 rounded-lg bg-violet-500/10">
              <TrendingUp className="w-3.5 h-3.5 text-violet-400" />
              <div>
                <p className="text-[10px] text-muted-foreground">Rate</p>
                <p className="text-xs font-bold text-violet-400">{stats.approvalRate.toFixed(0)}%</p>
              </div>
            </div>
          </div>

          {/* Last activity */}
          {stats.lastActivity && (
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <Clock className="w-3 h-3" />
              <span>
                Last active: {new Date(stats.lastActivity).toLocaleDateString()}
              </span>
            </div>
          )}
        </div>
      )}
    </div>
  )
}

// Compact inline sender indicator
export function SenderIndicator({
  senderName,
  reputation,
}: {
  senderName?: string | null
  reputation?: 'new' | 'regular' | 'trusted'
}) {
  const displayName = senderName || 'Unknown'
  const avatarColor = getAvatarColor(displayName)
  const initials = getInitials(displayName)

  return (
    <div className="flex items-center gap-1.5">
      <div
        className={cn(
          'w-5 h-5 rounded-full flex items-center justify-center text-[8px] font-bold',
          avatarColor,
        )}
      >
        {initials}
      </div>
      <span className="text-xs text-muted-foreground truncate max-w-[80px]">
        {displayName}
      </span>
      {reputation === 'trusted' && (
        <ShieldCheck className="w-3 h-3 text-emerald" />
      )}
    </div>
  )
}
