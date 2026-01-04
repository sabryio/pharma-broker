// Send Message Dialog Component
// Reusable dialog for composing and sending WhatsApp messages

import { useState, useEffect } from 'react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { Send, Loader2, AlertCircle, User, Phone } from 'lucide-react'
import { cn } from '@/lib/utils'
import { useSendMessage } from '@/hooks/use-send-message'

interface SendMessageDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  recipientJid: string
  recipientName?: string
  /** Optional pre-filled message content */
  initialMessage?: string
  /** Optional context about the message (e.g., "Reply to raw message") */
  context?: string
  /** Accent color for styling */
  accentColor?: 'teal' | 'amber' | 'primary'
  /** Callback when message is sent successfully */
  onSuccess?: (messageId: string) => void
}

// Quick message templates
const quickTemplates = [
  { label: '🇪🇬 Ask availability', text: 'مرحباً، هل العرض ما زال متاحاً؟' },
  { label: '🇬🇧 Ask availability', text: 'Hello, is this still available?' },
  { label: '🇪🇬 Thank you', text: 'شكراً على العرض، سأتواصل معك قريباً.' },
  {
    label: '🇬🇧 Follow up',
    text: 'Hi, just following up on my previous message.',
  },
]

export function SendMessageDialog({
  open,
  onOpenChange,
  recipientJid,
  recipientName,
  initialMessage = '',
  context,
  accentColor = 'teal',
  onSuccess,
}: SendMessageDialogProps) {
  const [message, setMessage] = useState(initialMessage)
  const [sendError, setSendError] = useState<string | null>(null)

  // Reset state when dialog opens/closes
  useEffect(() => {
    if (open) {
      setMessage(initialMessage)
      setSendError(null)
    }
  }, [open, initialMessage])

  const displayName = recipientName || 'Contact'
  const displayJid = recipientJid?.split('@')[0] || recipientJid

  // Generate initials for avatar
  const initials = displayName
    .split(' ')
    .map((n) => n[0])
    .slice(0, 2)
    .join('')
    .toUpperCase()

  // Use the send message hook
  const sendMessageMutation = useSendMessage({
    onSuccess: (messageId) => {
      setMessage('')
      setSendError(null)
      onOpenChange(false)
      onSuccess?.(messageId)
    },
    onError: (error) => {
      setSendError(error.message)
    },
  })

  const handleSendMessage = () => {
    if (!message.trim() || !recipientJid) return
    setSendError(null)
    sendMessageMutation.mutate({
      recipient_jid: recipientJid,
      content: message.trim(),
    })
  }

  const isSending = sendMessageMutation.isPending
  const isOverLimit = message.length > 4096

  const accentStyles = {
    teal: {
      header: 'bg-linear-to-r from-teal/50 via-teal to-teal/50',
      avatar: 'bg-teal/20 text-teal',
      ring: 'focus:ring-teal/30 focus:border-teal/50',
      button: 'bg-teal hover:bg-teal/90',
      border: 'border-teal/30',
    },
    amber: {
      header: 'bg-linear-to-r from-amber/50 via-amber to-amber/50',
      avatar: 'bg-amber/20 text-amber',
      ring: 'focus:ring-amber/30 focus:border-amber/50',
      button: 'bg-amber hover:bg-amber/90',
      border: 'border-amber/30',
    },
    primary: {
      header: 'bg-linear-to-r from-primary/50 via-primary to-primary/50',
      avatar: 'bg-primary/20 text-primary',
      ring: 'focus:ring-primary/30 focus:border-primary/50',
      button: 'bg-primary hover:bg-primary/90',
      border: 'border-primary/30',
    },
  }

  const styles = accentStyles[accentColor]

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className={cn('sm:max-w-md overflow-hidden', styles.border)}
      >
        {/* Gradient header */}
        <div
          className={cn('absolute top-0 left-0 right-0 h-1', styles.header)}
        />

        <DialogHeader className="pt-2">
          <DialogTitle className="flex items-center gap-3">
            <div
              className={cn(
                'w-10 h-10 rounded-full flex items-center justify-center font-bold',
                styles.avatar,
              )}
            >
              {initials || <User className="w-5 h-5" />}
            </div>
            <div className="flex flex-col">
              <span>Message {displayName}</span>
              <div className="flex items-center gap-2 text-xs font-normal text-muted-foreground">
                <Phone className="w-3 h-3" />
                <span className="font-mono">{displayJid}</span>
                <span>• WhatsApp</span>
              </div>
            </div>
          </DialogTitle>
          <DialogDescription>
            {context ? (
              <span>{context}</span>
            ) : (
              <span>Compose your message below.</span>
            )}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-4">
          <Textarea
            placeholder="Type your message..."
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            className={cn(
              'min-h-[120px] resize-none focus:ring-2',
              styles.ring,
            )}
            dir="auto"
            onKeyDown={(e) => {
              // Send on Ctrl+Enter or Cmd+Enter
              if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
                e.preventDefault()
                handleSendMessage()
              }
            }}
          />

          {/* Quick message templates */}
          <div className="flex flex-wrap gap-2">
            {quickTemplates.map((template) => (
              <button
                key={template.label}
                onClick={() => setMessage(template.text)}
                className="text-xs px-2 py-1 rounded-full bg-secondary hover:bg-secondary/80 transition-colors"
              >
                {template.label}
              </button>
            ))}
          </div>

          {/* Character count */}
          <div className="flex justify-between text-xs text-muted-foreground">
            <span>{message.length} / 4096 characters</span>
            {isOverLimit && (
              <span className="text-destructive">Message too long</span>
            )}
          </div>

          {/* Keyboard hint */}
          <div className="text-xs text-muted-foreground">
            Press{' '}
            <kbd className="px-1 py-0.5 rounded bg-muted text-[10px]">Ctrl</kbd>
            +
            <kbd className="px-1 py-0.5 rounded bg-muted text-[10px]">
              Enter
            </kbd>{' '}
            to send
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
            onClick={() => onOpenChange(false)}
            disabled={isSending}
          >
            Cancel
          </Button>
          <Button
            onClick={handleSendMessage}
            disabled={!message.trim() || isSending || isOverLimit}
            className={cn('gap-2 min-w-[120px]', styles.button)}
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
  )
}
