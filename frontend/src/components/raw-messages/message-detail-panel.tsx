// Message Detail Panel Component for Raw Messages
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet'
import { Badge } from '@/components/ui/badge'
import { Separator } from '@/components/ui/separator'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  MessageSquare,
  Clock,
  User,
  Users,
  AlertTriangle,
  Reply,
  Hash,
} from 'lucide-react'
import { StatusBadge } from './status-badge'
import { formatCompactDateTime } from './utils'
import type { RawMessage } from './types'

interface MessageDetailPanelProps {
  message: RawMessage | null
  onClose: () => void
}

export function MessageDetailPanel({
  message,
  onClose,
}: MessageDetailPanelProps) {
  return (
    <Sheet open={!!message} onOpenChange={(open) => !open && onClose()}>
      <SheetContent className="w-[400px] sm:w-[450px] p-0">
        {message && (
          <>
            <SheetHeader className="px-4 py-3 border-b pr-12">
              <div className="flex items-center justify-between">
                <SheetTitle className="text-sm font-medium">
                  Message Details
                </SheetTitle>
                <StatusBadge message={message} />
              </div>
            </SheetHeader>

            <ScrollArea className="h-[calc(100vh-60px)]">
              <div className="p-4 space-y-4">
                {/* Content Section */}
                <section>
                  <div className="flex items-center gap-2 mb-2">
                    <MessageSquare className="w-3.5 h-3.5 text-muted-foreground" />
                    <span className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                      Content
                    </span>
                  </div>
                  <div className="p-3 bg-muted/50 rounded-lg text-sm whitespace-pre-wrap break-all">
                    {message.content}
                  </div>
                </section>

                {/* Reply Context */}
                {message.replyToId && (
                  <section>
                    <div className="flex items-center gap-2 mb-2">
                      <Reply className="w-3.5 h-3.5 text-muted-foreground" />
                      <span className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                        Reply To
                      </span>
                    </div>
                    <div className="p-3 bg-muted/30 rounded-lg border-l-2 border-primary/50">
                      {message.replyToSender && (
                        <p className="text-xs font-medium text-primary mb-1">
                          {message.replyToSender}
                        </p>
                      )}
                      <p className="text-xs text-muted-foreground">
                        {message.replyToContent ||
                          'Original message unavailable'}
                      </p>
                    </div>
                  </section>
                )}

                <Separator />

                {/* Source Info */}
                <section className="grid grid-cols-2 gap-3">
                  <div>
                    <div className="flex items-center gap-1.5 mb-1">
                      <Users className="w-3 h-3 text-muted-foreground" />
                      <span className="text-[10px] font-medium text-muted-foreground uppercase">
                        Group
                      </span>
                    </div>
                    <p className="text-xs truncate">
                      {message.groupName || message.groupJid}
                    </p>
                  </div>
                  <div>
                    <div className="flex items-center gap-1.5 mb-1">
                      <User className="w-3 h-3 text-muted-foreground" />
                      <span className="text-[10px] font-medium text-muted-foreground uppercase">
                        Sender
                      </span>
                    </div>
                    <p className="text-xs truncate">
                      {message.participantName || message.participantJid}
                    </p>
                  </div>
                </section>

                <Separator />

                {/* Timestamps */}
                <section>
                  <div className="flex items-center gap-2 mb-2">
                    <Clock className="w-3.5 h-3.5 text-muted-foreground" />
                    <span className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                      Timeline
                    </span>
                  </div>
                  <div className="space-y-2 text-xs">
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">Received</span>
                      <span className="tabular-nums">
                        {formatCompactDateTime(message.timestamp)}
                      </span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-muted-foreground">Created</span>
                      <span className="tabular-nums">
                        {formatCompactDateTime(message.createdAt)}
                      </span>
                    </div>
                    {message.processedAt && (
                      <div className="flex justify-between">
                        <span className="text-muted-foreground">Processed</span>
                        <span className="tabular-nums">
                          {formatCompactDateTime(message.processedAt)}
                        </span>
                      </div>
                    )}
                  </div>
                </section>

                {/* Error Section */}
                {message.error && (
                  <>
                    <Separator />
                    <section>
                      <div className="flex items-center gap-2 mb-2">
                        <AlertTriangle className="w-3.5 h-3.5 text-destructive" />
                        <span className="text-xs font-medium text-destructive uppercase tracking-wide">
                          Error
                        </span>
                      </div>
                      <div className="p-3 bg-destructive/10 border border-destructive/20 rounded-lg text-xs text-destructive">
                        {message.error}
                      </div>
                    </section>
                  </>
                )}

                <Separator />

                {/* IDs */}
                <section>
                  <div className="flex items-center gap-2 mb-2">
                    <Hash className="w-3.5 h-3.5 text-muted-foreground" />
                    <span className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                      Identifiers
                    </span>
                  </div>
                  <div className="space-y-2">
                    <div className="flex items-center justify-between">
                      <span className="text-xs text-muted-foreground">ID</span>
                      <Badge
                        variant="secondary"
                        className="font-mono text-[10px]"
                      >
                        {message.id.slice(0, 8)}...
                      </Badge>
                    </div>
                    {message.externalId && (
                      <div className="flex items-center justify-between">
                        <span className="text-xs text-muted-foreground">
                          External
                        </span>
                        <Badge
                          variant="secondary"
                          className="font-mono text-[10px]"
                        >
                          {message.externalId.slice(0, 12)}...
                        </Badge>
                      </div>
                    )}
                  </div>
                </section>
              </div>
            </ScrollArea>
          </>
        )}
      </SheetContent>
    </Sheet>
  )
}
