import { useState } from 'react'
import {
  AlertCircle,
  ArrowRightLeft,
  Building2,
  Calendar,
  ChevronDown,
  ChevronUp,
  DollarSign,
  Hash,
  Loader2,
  MessageSquare,
  Package,
  RefreshCw,
  Send,
  Sparkles,
  TrendingUp,
  User,
} from 'lucide-react'
import { MedicationCurationBadge } from './medication-curation-badge'
import type { MatchDetails, ReviewOffer, ReviewRequest } from './types'
import { cn } from '@/lib/utils'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { useSendMessage } from '@/hooks/use-send-message'

interface ReviewCardProps {
  type: 'offer' | 'request'
  offer?: ReviewOffer
  request?: ReviewRequest
  onCurate?: (name: string, aliasId?: string | null) => void
  onReclassify?: (
    id: string,
    type: 'offer' | 'request',
    medication: string,
  ) => void
  onReparse?: (
    id: string,
    type: 'offer' | 'request',
    medication: string,
  ) => void
  aiConfidence?: number | null
  matchDetails?: MatchDetails | null
}

/**
 * Highlights the medication name in raw text
 * Supports both Arabic and English text
 */
function HighlightedMessage({
  text,
  highlight,
  accentColor,
}: {
  text: string
  highlight: string | null
  accentColor: 'teal' | 'amber'
}) {
  if (!highlight || !text) {
    return <span className="text-muted-foreground">{text}</span>
  }

  // Escape special regex characters
  const escapedHighlight = highlight.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const regex = new RegExp(`(${escapedHighlight})`, 'gi')
  const parts = text.split(regex)

  return (
    <span className="text-muted-foreground leading-relaxed">
      {parts.map((part, i) =>
        regex.test(part) ? (
          <mark
            key={i}
            className={cn(
              'font-bold px-1 py-0.5 rounded-sm',
              accentColor === 'teal'
                ? 'bg-teal/30 text-teal'
                : 'bg-amber/30 text-amber',
            )}
          >
            {part}
          </mark>
        ) : (
          <span key={i}>{part}</span>
        ),
      )}
    </span>
  )
}
/**
 * Sender info badge component with creative popover and message composer
 */
function SenderBadge({
  name,
  jid,
  accentColor,
  medicationName,
}: {
  name: string | null
  jid: string | null
  accentColor: 'teal' | 'amber'
  medicationName?: string
}) {
  const [isComposerOpen, setIsComposerOpen] = useState(false)
  const [message, setMessage] = useState('')
  const [sendError, setSendError] = useState<string | null>(null)

  const displayName = name || 'Unknown'
  const displayJid = jid ? jid.split('@')[0] : null
  const fullJid = jid || 'N/A'

  // Generate initials for avatar
  const initials = displayName
    .split(' ')
    .map((n) => n[0])
    .slice(0, 2)
    .join('')
    .toUpperCase()

  // Use the send message hook
  const sendMessageMutation = useSendMessage({
    onSuccess: () => {
      setMessage('')
      setSendError(null)
      setIsComposerOpen(false)
    },
    onError: (error) => {
      setSendError(error.message)
    },
  })

  const handleSendMessage = () => {
    if (!message.trim() || !jid) return
    setSendError(null)
    sendMessageMutation.mutate({
      recipient_jid: jid,
      content: message.trim(),
    })
  }

  const isSending = sendMessageMutation.isPending

  return (
    <>
      <Popover>
        <PopoverTrigger asChild>
          <button
            className={cn(
              'flex items-center gap-2 px-3 py-1.5 rounded-full text-xs',
              'bg-linear-to-r border backdrop-blur-sm cursor-pointer',
              'hover:scale-105 active:scale-95 transition-all duration-200',
              accentColor === 'teal'
                ? 'from-teal/10 to-teal/5 border-teal/30 hover:border-teal/50'
                : 'from-amber/10 to-amber/5 border-amber/30 hover:border-amber/50',
            )}
          >
            <div
              className={cn(
                'w-6 h-6 rounded-full flex items-center justify-center shrink-0',
                'text-[10px] font-bold',
                accentColor === 'teal'
                  ? 'bg-teal/20 text-teal'
                  : 'bg-amber/20 text-amber',
              )}
            >
              {initials || <User className="w-3 h-3" />}
            </div>
            <span className="font-medium text-foreground truncate max-w-[100px]">
              {displayName}
            </span>
          </button>
        </PopoverTrigger>
        <PopoverContent
          className={cn(
            'w-72 p-0 overflow-hidden',
            'bg-linear-to-br from-card via-card to-card/80',
            'border shadow-xl',
            accentColor === 'teal'
              ? 'border-teal/30 shadow-teal/20'
              : 'border-amber/30 shadow-amber/20',
          )}
          sideOffset={8}
        >
          {/* Header with gradient */}
          <div
            className={cn(
              'px-4 py-3 border-b',
              accentColor === 'teal'
                ? 'bg-linear-to-r from-teal/20 to-teal/5 border-teal/20'
                : 'bg-linear-to-r from-amber/20 to-amber/5 border-amber/20',
            )}
          >
            <div className="flex items-center gap-3">
              <div
                className={cn(
                  'w-10 h-10 rounded-full flex items-center justify-center',
                  'font-bold text-lg',
                  accentColor === 'teal'
                    ? 'bg-teal/30 text-teal'
                    : 'bg-amber/30 text-amber',
                )}
              >
                {initials || <User className="w-5 h-5" />}
              </div>
              <div className="flex flex-col min-w-0">
                <span className="font-semibold text-foreground text-sm truncate">
                  {displayName}
                </span>
                <span className="text-[10px] text-muted-foreground">
                  Sender
                </span>
              </div>
            </div>
          </div>

          {/* Content */}
          <div className="p-4 space-y-3">
            {/* JID */}
            {/* <div className="flex items-start gap-2">
              <div
                className={cn(
                  'w-6 h-6 rounded-md flex items-center justify-center shrink-0 mt-0.5',
                  accentColor === 'teal' ? 'bg-teal/10' : 'bg-amber/10',
                )}
              >
                <MessageSquare
                  className={cn(
                    'w-3.5 h-3.5',
                    accentColor === 'teal' ? 'text-teal' : 'text-amber',
                  )}
                />
              </div>
              <div className="flex flex-col min-w-0">
                <span className="text-[10px] text-muted-foreground uppercase tracking-wider">
                  WhatsApp ID
                </span>
                <span className="text-xs font-mono text-foreground break-all">
                  {fullJid}
                </span>
              </div>
            </div> */}

            {/* Short JID */}
            {displayJid && (
              <div className="flex items-start gap-2">
                <div
                  className={cn(
                    'w-6 h-6 rounded-md flex items-center justify-center shrink-0 mt-0.5',
                    accentColor === 'teal' ? 'bg-teal/10' : 'bg-amber/10',
                  )}
                >
                  <Hash
                    className={cn(
                      'w-3.5 h-3.5',
                      accentColor === 'teal' ? 'text-teal' : 'text-amber',
                    )}
                  />
                </div>
                <div className="flex flex-col min-w-0">
                  <span className="text-[10px] text-muted-foreground uppercase tracking-wider">
                    Phone Number
                  </span>
                  <span className="text-xs font-medium text-foreground">
                    {displayJid}
                  </span>
                </div>
              </div>
            )}
          </div>

          {/* Send Message Button */}
          {jid && (
            <div className="px-4 pb-4">
              <Button
                onClick={() => setIsComposerOpen(true)}
                className={cn(
                  'w-full gap-2 group relative overflow-hidden',
                  'transition-all duration-300',
                  accentColor === 'teal'
                    ? 'bg-linear-to-r from-teal to-teal/80 hover:from-teal/90 hover:to-teal/70 text-white shadow-lg shadow-teal/30'
                    : 'bg-linear-to-r from-amber to-amber/80 hover:from-amber/90 hover:to-amber/70 text-white shadow-lg shadow-amber/30',
                )}
              >
                <Send className="w-4 h-4 group-hover:translate-x-0.5 group-hover:-translate-y-0.5 transition-transform" />
                <span className="font-medium">Send Message</span>
                {/* Shimmer effect */}
                <div className="absolute inset-0 bg-linear-to-r from-transparent via-white/20 to-transparent -translate-x-full group-hover:translate-x-full transition-transform duration-700" />
              </Button>
            </div>
          )}

          {/* Footer glow */}
          <div
            className={cn(
              'h-1',
              accentColor === 'teal'
                ? 'bg-linear-to-r from-transparent via-teal/50 to-transparent'
                : 'bg-linear-to-r from-transparent via-amber/50 to-transparent',
            )}
          />
        </PopoverContent>
      </Popover>

      {/* Message Composer Dialog */}
      <Dialog open={isComposerOpen} onOpenChange={setIsComposerOpen}>
        <DialogContent
          className={cn(
            'sm:max-w-md overflow-hidden',
            accentColor === 'teal' ? 'border-teal/30' : 'border-amber/30',
          )}
        >
          {/* Gradient header */}
          <div
            className={cn(
              'absolute top-0 left-0 right-0 h-1',
              accentColor === 'teal'
                ? 'bg-linear-to-r from-teal/50 via-teal to-teal/50'
                : 'bg-linear-to-r from-amber/50 via-amber to-amber/50',
            )}
          />

          <DialogHeader className="pt-2">
            <DialogTitle className="flex items-center gap-3">
              <div
                className={cn(
                  'w-10 h-10 rounded-full flex items-center justify-center font-bold',
                  accentColor === 'teal'
                    ? 'bg-teal/20 text-teal'
                    : 'bg-amber/20 text-amber',
                )}
              >
                {initials || <User className="w-5 h-5" />}
              </div>
              <div className="flex flex-col">
                <span>Message {displayName}</span>
                <span className="text-xs font-normal text-muted-foreground">
                  via WhatsApp
                </span>
              </div>
            </DialogTitle>
            <DialogDescription>
              Compose your message below. It will be sent to{' '}
              {displayJid || fullJid}.
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4 py-4">
            <Textarea
              placeholder="Type your message..."
              value={message}
              onChange={(e) => setMessage(e.target.value)}
              className={cn(
                'min-h-[120px] resize-none',
                'focus:ring-2',
                accentColor === 'teal'
                  ? 'focus:ring-teal/30 focus:border-teal/50'
                  : 'focus:ring-amber/30 focus:border-amber/50',
              )}
              dir="auto"
            />

            {/* Quick message templates */}
            <div className="flex flex-wrap gap-2">
              <button
                onClick={() =>
                  setMessage(
                    `سلام عليكم كنت بسأل على ${medicationName || '(اسم الدواء)'}`,
                  )
                }
                className="text-xs px-2 py-1 rounded-full bg-secondary hover:bg-secondary/80 transition-colors"
              >
                🇪🇬 Ask about med
              </button>
              <button
                onClick={() => setMessage('مرحباً، هل العرض ما زال متاحاً؟')}
                className="text-xs px-2 py-1 rounded-full bg-secondary hover:bg-secondary/80 transition-colors"
              >
                🇪🇬 Ask availability
              </button>
              <button
                onClick={() => setMessage('Hello, is this still available?')}
                className="text-xs px-2 py-1 rounded-full bg-secondary hover:bg-secondary/80 transition-colors"
              >
                🇬🇧 Ask availability
              </button>
              <button
                onClick={() =>
                  setMessage('شكراً على العرض، سأتواصل معك قريباً.')
                }
                className="text-xs px-2 py-1 rounded-full bg-secondary hover:bg-secondary/80 transition-colors"
              >
                🇪🇬 Thank you
              </button>
            </div>

            {/* Character count */}
            <div className="flex justify-between text-xs text-muted-foreground">
              <span>{message.length} / 4096 characters</span>
              {message.length > 4096 && (
                <span className="text-destructive">Message too long</span>
              )}
            </div>

            {/* Error display */}
            {sendError && (
              <div className="flex items-center gap-2 p-3 rounded-lg bg-destructive/10 text-destructive text-sm">
                <AlertCircle className="w-4 h-4 shrink-0" />
                <p>{sendError}</p>
              </div>
            )}
          </div>

          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => {
                setIsComposerOpen(false)
                setSendError(null)
              }}
              disabled={isSending}
            >
              Cancel
            </Button>
            <Button
              onClick={handleSendMessage}
              disabled={!message.trim() || isSending || message.length > 4096}
              className={cn(
                'gap-2 min-w-[120px]',
                accentColor === 'teal'
                  ? 'bg-teal hover:bg-teal/90'
                  : 'bg-amber hover:bg-amber/90',
              )}
            >
              {isSending ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" />
                  Sending...
                </>
              ) : (
                <>
                  <Send className="w-4 h-4" />
                  Send
                </>
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}

/**
 * Collapsible raw message section
 */
function RawMessageSection({
  rawMessage,
  medication,
  accentColor,
}: {
  rawMessage: string | null
  medication: string | null
  accentColor: 'teal' | 'amber'
}) {
  const [isExpanded, setIsExpanded] = useState(false)

  if (!rawMessage) return null

  return (
    <div
      className={cn(
        'mt-3 rounded-lg border overflow-hidden transition-all duration-300',
        accentColor === 'teal'
          ? 'border-teal/20 bg-linear-to-br from-teal/5 to-transparent'
          : 'border-amber/20 bg-linear-to-br from-amber/5 to-transparent',
      )}
    >
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className={cn(
          'w-full flex items-center justify-between px-3 py-2 text-xs',
          'hover:bg-white/5 transition-colors',
        )}
      >
        <div className="flex items-center gap-2">
          <MessageSquare
            className={cn(
              'w-3.5 h-3.5',
              accentColor === 'teal' ? 'text-teal' : 'text-amber',
            )}
          />
          <span className="text-muted-foreground">Original Message</span>
        </div>
        {isExpanded ? (
          <ChevronUp className="w-3.5 h-3.5 text-muted-foreground" />
        ) : (
          <ChevronDown className="w-3.5 h-3.5 text-muted-foreground" />
        )}
      </button>
      {isExpanded && (
        <div className="px-3 pb-3 text-xs leading-relaxed" dir="auto">
          <HighlightedMessage
            text={rawMessage}
            highlight={medication}
            accentColor={accentColor}
          />
        </div>
      )}
    </div>
  )
}

export function ReviewCard({
  type,
  offer,
  request,
  onCurate,
  onReclassify,
  onReparse,
}: ReviewCardProps) {
  const isOffer = type === 'offer'

  if (isOffer && offer) {
    return (
      <div className="flex flex-col h-full">
        <div
          className={cn(
            'relative overflow-hidden flex-1',
            'p-6 rounded-2xl',
            'bg-linear-to-br from-card/80 via-card/60 to-card/40',
            'border border-teal/30 hover:border-teal/50',
            'shadow-xl shadow-teal/10 hover:shadow-teal/20',
            'backdrop-blur-xl',
            'transition-all duration-500 ease-out',
            'hover:translate-y-[-2px]',
          )}
        >
          {/* Animated gradient border effect */}
          <div className="absolute inset-0 -z-10 bg-linear-to-br from-teal/20 via-transparent to-teal/10 opacity-50" />
          <div className="absolute top-0 right-0 w-32 h-32 -z-10 bg-teal/10 rounded-full blur-3xl" />

          {/* Header */}
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-3">
              <div className="relative">
                <div className="w-10 h-10 rounded-xl bg-linear-to-br from-teal to-teal/60 flex items-center justify-center shadow-lg shadow-teal/30">
                  <Package className="w-5 h-5 text-white" />
                </div>
                <div className="absolute -top-1 -right-1 w-3 h-3 rounded-full bg-emerald animate-pulse shadow-lg shadow-emerald/50" />
              </div>
              <div>
                <span className="text-sm font-bold text-teal uppercase tracking-wider">
                  Supply Offer
                </span>
                <div className="flex items-center gap-1 text-[10px] text-muted-foreground">
                  <Sparkles className="w-3 h-3" />
                  <span>Available</span>
                </div>
              </div>
            </div>
          </div>

          {/* Source Group & Sender */}
          <div className="flex flex-wrap items-center gap-2 mb-4">
            <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-teal/10 border border-teal/20">
              <Building2 className="w-3.5 h-3.5 text-teal" />
              <span className="text-xs font-medium text-foreground">
                {offer.sourceGroup ?? 'Unknown Group'}
              </span>
            </div>
            <SenderBadge
              name={offer.senderName}
              jid={offer.senderJid}
              accentColor="teal"
              medicationName={offer.product}
            />
          </div>

          {/* Product - Hero Section */}
          <div className="p-4 rounded-xl bg-linear-to-br from-teal/15 to-teal/5 border border-teal/20 mb-4">
            <div className="flex items-center gap-2 text-xs text-teal/80 mb-1">
              <Package className="w-3.5 h-3.5" />
              <span className="uppercase tracking-wider font-medium">
                Medication
              </span>
            </div>
            <div className="flex items-center justify-between gap-2">
              <span className="text-lg font-bold text-foreground">
                {offer.product}
              </span>
              <MedicationCurationBadge
                status={offer.curationStatus}
                masterId={offer.masterId}
                onClick={(e) => {
                  e.stopPropagation()
                  onCurate?.(offer.product, offer.medicationAliasId)
                }}
              />
            </div>
          </div>

          {/* Stats Grid */}
          <div className="grid grid-cols-3 gap-3 mb-4">
            <div className="p-3 rounded-xl bg-secondary/40 border border-border/50 hover:border-teal/30 transition-colors">
              <div className="flex items-center gap-1.5 text-xs text-muted-foreground mb-1">
                <Hash className="w-3 h-3" />
                <span>Qty</span>
              </div>
              <span className="text-sm font-bold text-teal">
                {offer.quantity}
              </span>
            </div>
            <div className="p-3 rounded-xl bg-secondary/40 border border-border/50 hover:border-teal/30 transition-colors">
              <div className="flex items-center gap-1.5 text-xs text-muted-foreground mb-1">
                <DollarSign className="w-3 h-3" />
                <span>Price</span>
              </div>
              <span className="text-sm font-bold text-teal">{offer.price}</span>
            </div>
            <div className="p-3 rounded-xl bg-secondary/40 border border-border/50 hover:border-teal/30 transition-colors">
              <div className="flex items-center gap-1.5 text-xs text-muted-foreground mb-1">
                <Calendar className="w-3 h-3" />
                <span>Expiry</span>
              </div>
              <span className="text-sm font-bold text-foreground">
                {offer.expiry}
              </span>
            </div>
          </div>

          {/* Raw Message Section */}
          <RawMessageSection
            rawMessage={offer.rawMessage}
            medication={offer.product}
            accentColor="teal"
          />

          {/* Footer */}
          <div className="flex items-center gap-2 mt-4 text-xs text-muted-foreground">
            <div className="flex-1 h-px bg-linear-to-r from-teal/40 to-transparent" />
            <span className="px-2">Supply Side</span>
            <div className="flex-1 h-px bg-linear-to-l from-teal/40 to-transparent" />
          </div>
        </div>

        {/* Reclassify Button - Outside the card */}
        {onReclassify && (
          <button
            onClick={() => onReclassify(offer.id, 'offer', offer.product)}
            className={cn(
              'mt-3 w-full flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl',
              'bg-linear-to-r from-violet-500/10 to-fuchsia-500/10',
              'border border-violet-500/30 hover:border-violet-500/50',
              'text-violet-400 hover:text-violet-300',
              'transition-all duration-300 hover:scale-[1.02] active:scale-[0.98]',
              'shadow-lg hover:shadow-violet-500/20',
              'text-sm font-medium',
            )}
          >
            <ArrowRightLeft className="w-4 h-4" />
            <span>Reclassify as Request</span>
          </button>
        )}

        {/* Reparse Button */}
        {onReparse && (
          <button
            onClick={() => onReparse(offer.id, 'offer', offer.product)}
            className={cn(
              'mt-2 w-full flex items-center justify-center gap-2 px-4 py-2 rounded-xl',
              'bg-linear-to-r from-cyan-500/10 to-blue-500/10',
              'border border-cyan-500/30 hover:border-cyan-500/50',
              'text-cyan-400 hover:text-cyan-300',
              'transition-all duration-300 hover:scale-[1.02] active:scale-[0.98]',
              'text-xs font-medium',
            )}
          >
            <RefreshCw className="w-3.5 h-3.5" />
            <span>Re-parse with AI</span>
          </button>
        )}
      </div>
    )
  }

  if (!isOffer && request) {
    return (
      <div className="flex flex-col h-full">
        <div
          className={cn(
            'relative overflow-hidden flex-1',
            'p-6 rounded-2xl',
            'bg-linear-to-br from-card/80 via-card/60 to-card/40',
            'border border-amber/30 hover:border-amber/50',
            'shadow-xl shadow-amber/10 hover:shadow-amber/20',
            'backdrop-blur-xl',
            'transition-all duration-500 ease-out',
            'hover:translate-y-[-2px]',
          )}
        >
          {/* Animated gradient border effect */}
          <div className="absolute inset-0 -z-10 bg-linear-to-br from-amber/20 via-transparent to-amber/10 opacity-50" />
          <div className="absolute top-0 right-0 w-32 h-32 -z-10 bg-amber/10 rounded-full blur-3xl" />

          {/* Header */}
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-3">
              <div className="relative">
                <div className="w-10 h-10 rounded-xl bg-linear-to-br from-amber to-amber/60 flex items-center justify-center shadow-lg shadow-amber/30">
                  <TrendingUp className="w-5 h-5 text-white" />
                </div>
                <div className="absolute -top-1 -right-1 w-3 h-3 rounded-full bg-amber animate-pulse shadow-lg shadow-amber/50" />
              </div>
              <div>
                <span className="text-sm font-bold text-amber uppercase tracking-wider">
                  Demand Request
                </span>
                <div className="flex items-center gap-1 text-[10px] text-muted-foreground">
                  <Sparkles className="w-3 h-3" />
                  <span>Needed</span>
                </div>
              </div>
            </div>
          </div>

          {/* Source Group & Sender */}
          <div className="flex flex-wrap items-center gap-2 mb-4">
            <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-amber/10 border border-amber/20">
              <Building2 className="w-3.5 h-3.5 text-amber" />
              <span className="text-xs font-medium text-foreground">
                {request.sourceGroup ?? 'Unknown Group'}
              </span>
            </div>
            <SenderBadge
              name={request.senderName}
              jid={request.senderJid}
              accentColor="amber"
              medicationName={request.product}
            />
          </div>

          {/* Product - Hero Section */}
          <div className="p-4 rounded-xl bg-linear-to-br from-amber/15 to-amber/5 border border-amber/20 mb-4">
            <div className="flex items-center gap-2 text-xs text-amber/80 mb-1">
              <Package className="w-3.5 h-3.5" />
              <span className="uppercase tracking-wider font-medium">
                Medication Needed
              </span>
            </div>
            <div className="flex items-center justify-between gap-2">
              <span className="text-lg font-bold text-foreground">
                {request.product}
              </span>
              <MedicationCurationBadge
                status={request.curationStatus}
                masterId={request.masterId}
                onClick={(e) => {
                  e.stopPropagation()
                  onCurate?.(request.product, request.medicationAliasId)
                }}
              />
            </div>
          </div>

          {/* Stats Grid */}
          <div className="grid grid-cols-3 gap-3 mb-4">
            <div className="p-3 rounded-xl bg-secondary/40 border border-border/50 hover:border-amber/30 transition-colors">
              <div className="flex items-center gap-1.5 text-xs text-muted-foreground mb-1">
                <Hash className="w-3 h-3" />
                <span>Qty</span>
              </div>
              <span className="text-sm font-bold text-amber">
                {request.quantity}
              </span>
            </div>
            <div className="p-3 rounded-xl bg-secondary/40 border border-border/50 hover:border-amber/30 transition-colors">
              <div className="flex items-center gap-1.5 text-xs text-muted-foreground mb-1">
                <DollarSign className="w-3 h-3" />
                <span>Max</span>
              </div>
              <span className="text-sm font-bold text-amber">
                {request.maxPrice}
              </span>
            </div>
            <div className="p-3 rounded-xl bg-secondary/40 border border-border/50 hover:border-amber/30 transition-colors">
              <div className="flex items-center gap-1.5 text-xs text-muted-foreground mb-1">
                <AlertCircle className="w-3 h-3" />
                <span>Urgency</span>
              </div>
              <Badge
                variant="outline"
                className={cn(
                  'text-[10px] font-bold px-2 py-0.5',
                  request.urgency === 'High' &&
                    'border-destructive/50 text-destructive bg-destructive/10',
                  request.urgency === 'Medium' &&
                    'border-amber/50 text-amber bg-amber/10',
                  request.urgency === 'Low' &&
                    'border-emerald/50 text-emerald bg-emerald/10',
                )}
              >
                {request.urgency}
              </Badge>
            </div>
          </div>

          {/* Raw Message Section */}
          <RawMessageSection
            rawMessage={request.rawMessage}
            medication={request.product}
            accentColor="amber"
          />

          {/* Footer */}
          <div className="flex items-center gap-2 mt-4 text-xs text-muted-foreground">
            <div className="flex-1 h-px bg-linear-to-r from-amber/40 to-transparent" />
            <span className="px-2">Demand Side</span>
            <div className="flex-1 h-px bg-linear-to-l from-amber/40 to-transparent" />
          </div>
        </div>

        {/* Reclassify Button - Outside the card */}
        {onReclassify && (
          <button
            onClick={() => onReclassify(request.id, 'request', request.product)}
            className={cn(
              'mt-3 w-full flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl',
              'bg-linear-to-r from-violet-500/10 to-fuchsia-500/10',
              'border border-violet-500/30 hover:border-violet-500/50',
              'text-violet-400 hover:text-violet-300',
              'transition-all duration-300 hover:scale-[1.02] active:scale-[0.98]',
              'shadow-lg hover:shadow-violet-500/20',
              'text-sm font-medium',
            )}
          >
            <ArrowRightLeft className="w-4 h-4" />
            <span>Reclassify as Offer</span>
          </button>
        )}

        {/* Reparse Button */}
        {onReparse && (
          <button
            onClick={() => onReparse(request.id, 'request', request.product)}
            className={cn(
              'mt-2 w-full flex items-center justify-center gap-2 px-4 py-2 rounded-xl',
              'bg-linear-to-r from-cyan-500/10 to-blue-500/10',
              'border border-cyan-500/30 hover:border-cyan-500/50',
              'text-cyan-400 hover:text-cyan-300',
              'transition-all duration-300 hover:scale-[1.02] active:scale-[0.98]',
              'text-xs font-medium',
            )}
          >
            <RefreshCw className="w-3.5 h-3.5" />
            <span>Re-parse with AI</span>
          </button>
        )}
      </div>
    )
  }

  return null
}
