import { cn } from '@/lib/utils'
import { Clock, User } from 'lucide-react'

interface MessageBubbleProps {
  text: string
  senderName: string
  groupName: string
  timestamp: Date
  isHighlighted?: boolean
}

export function MessageBubble({
  text,
  senderName,
  groupName,
  timestamp,
  isHighlighted = false,
}: MessageBubbleProps) {
  const formatTime = (date: Date) => {
    return date.toLocaleTimeString('en-US', {
      hour: '2-digit',
      minute: '2-digit',
      hour12: true,
    })
  }

  return (
    <div className="flex flex-col gap-2">
      {/* Group Header */}
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <div className="w-6 h-6 rounded-full bg-emerald/20 flex items-center justify-center">
          <User className="w-3 h-3 text-emerald" />
        </div>
        <span className="font-medium text-foreground">{groupName}</span>
      </div>

      {/* Message Bubble - WhatsApp Style */}
      <div
        className={cn(
          'relative max-w-full rounded-lg p-4 transition-all duration-300',
          'bg-[#1A2E1A] border border-emerald/20',
          isHighlighted &&
            'ring-2 ring-purple-500/50 shadow-lg shadow-purple-500/10',
        )}
      >
        {/* Sender Name */}
        <div className="flex items-center gap-2 mb-2">
          <span className="text-sm font-semibold text-emerald">
            {senderName}
          </span>
        </div>

        {/* Message Text */}
        <p className="text-sm text-foreground whitespace-pre-wrap leading-relaxed">
          {text}
        </p>

        {/* Timestamp */}
        <div className="flex items-center justify-end gap-1 mt-2 text-xs text-muted-foreground">
          <Clock className="w-3 h-3" />
          <span>{formatTime(timestamp)}</span>
        </div>

        {/* WhatsApp-style tail */}
        <div
          className="absolute top-0 -left-2 w-0 h-0"
          style={{
            borderTop: '8px solid transparent',
            borderBottom: '8px solid transparent',
            borderRight: '8px solid #1A2E1A',
          }}
        />
      </div>

      {/* Decorative Elements */}
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <div className="flex-1 h-px bg-linear-to-r from-emerald/20 to-transparent" />
        <span>Original Message</span>
        <div className="flex-1 h-px bg-linear-to-l from-emerald/20 to-transparent" />
      </div>
    </div>
  )
}
