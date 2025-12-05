import { useState } from 'react'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { timeAgo } from '@/lib/sse'
import type { Request } from '@/lib/types'
import {
  Clock,
  Users,
  MessageCircle,
  CheckSquare,
  AlarmClock,
  X,
  Check,
  Copy,
  AlertCircle,
} from 'lucide-react'

interface RequestCardProps {
  request: Request
  isMatch?: boolean
  onDismiss?: (id: string) => void
  onContact?: (id: string) => void
}

export function RequestCard({
  request,
  isMatch = false,
  onDismiss,
  onContact,
}: RequestCardProps) {
  const [status, setStatus] = useState<'active' | 'contacted' | 'dismissed'>(
    'active',
  )
  const [snoozed, setSnoozed] = useState(false)

  const qty = request.quantity
    ? `${request.quantity} ${request.unit || ''}`
    : ''
  const maxPrice = request.max_price
    ? `${request.max_price} ${request.currency || 'EGP'}`
    : ''

  // Dynamic styles based on match status
  const borderColor = isMatch
    ? 'border-green-600 ring-2 ring-green-500 ring-offset-1'
    : 'border-red-300 hover:border-red-400'

  const bgColor = isMatch
    ? 'bg-gradient-to-r from-green-50 to-white'
    : 'bg-white'

  // Open WhatsApp chat
  const handleWhatsAppClick = () => {
    if (request.source_phone) {
      const cleanPhone = request.source_phone.replace(/\D/g, '')
      const text = `السلام عليكم، بخصوص *${request.medication}* اللي حضرتك طالب في جروب *${request.group_name || 'الواتساب'}*... أنا عندي الصنف ده متاح.`
      window.open(
        `https://web.whatsapp.com/send?phone=${cleanPhone}&text=${encodeURIComponent(text)}`,
        '_blank',
      )
    }
  }

  // Mark as contacted/claimed
  const handleClaim = () => {
    setStatus('contacted')
    onContact?.(request.id)
  }

  // Snooze (hide temporarily)
  const handleSnooze = () => {
    setSnoozed(true)
    setTimeout(() => setSnoozed(false), 30 * 60 * 1000) // 30 minutes
  }

  // Dismiss permanently
  const handleDismiss = () => {
    setStatus('dismissed')
    onDismiss?.(request.id)
  }

  // Copy details
  const handleCopy = () => {
    const details = `${request.medication}${qty ? ` - ${qty}` : ''}${maxPrice ? ` - Max: ${maxPrice}` : ''}\nمن: ${request.source_name || request.source_phone}`
    navigator.clipboard.writeText(details)
  }

  if (status === 'dismissed' || snoozed) return null

  return (
    <Card
      className={`relative p-4 mb-3 rounded-lg border-l-4 shadow-sm transition-all duration-300 ${borderColor} ${bgColor} ${isMatch ? 'animate-pulse' : ''}`}
    >
      {/* Header */}
      <div className="flex justify-between items-start mb-2" dir="rtl">
        <div className="flex-1">
          <h3 className="font-bold text-lg text-gray-800 flex items-center gap-2 flex-wrap">
            {request.medication}
            {request.unit && (
              <span className="text-xs font-normal text-gray-500 bg-gray-100 px-2 py-1 rounded-full border border-gray-200">
                {request.unit}
              </span>
            )}
          </h3>

          {/* Meta Info Line */}
          <div className="text-xs text-gray-500 flex items-center gap-3 mt-1.5 flex-wrap">
            <span className="flex items-center gap-1">
              <Clock size={12} />
              {timeAgo(request.created_at)}
            </span>

            {request.group_name && (
              <span className="flex items-center gap-1 bg-slate-100 px-2 py-0.5 rounded text-slate-600 font-medium">
                <Users size={12} />
                {request.group_name}
              </span>
            )}
          </div>
        </div>
        <Badge
          className={`whitespace-nowrap ${request.urgent ? 'bg-red-100 text-red-800 border-red-200' : 'bg-orange-100 text-orange-800 border-orange-200'}`}
        >
          طلب {request.urgent ? '(عاجل)' : '(ناقص)'}
        </Badge>
      </div>

      {/* Details */}
      <div className="flex gap-4 mb-3 text-sm text-gray-700" dir="rtl">
        <div className="flex-1 bg-gray-50/80 p-2 rounded border border-gray-100">
          <span className="text-gray-400 text-xs block">الكمية</span>
          <span className="font-semibold">{qty || 'غير محدد'}</span>
        </div>
        {maxPrice && (
          <div className="flex-1 bg-gray-50/80 p-2 rounded border border-gray-100">
            <span className="text-gray-400 text-xs block">أقصى سعر</span>
            <span className="font-semibold text-blue-600">{maxPrice}</span>
          </div>
        )}
      </div>

      {/* Notes */}
      {request.notes && (
        <div
          className="mb-3 bg-gray-50/80 p-2 rounded border border-gray-100 text-sm"
          dir="rtl"
        >
          <span className="text-gray-400 text-xs block">ملاحظات</span>
          <span className="font-medium">{request.notes}</span>
        </div>
      )}

      {/* Match Alert */}
      {isMatch && (
        <div
          className="mb-3 flex items-center gap-2 text-green-800 text-sm bg-green-100 p-2 rounded border border-green-200 shadow-sm"
          dir="rtl"
        >
          <AlertCircle size={18} className="text-green-600" />
          <strong className="font-bold">فرصة ممتازة!</strong>
          <span>يوجد طرف آخر مهتم بهذا الصنف حالياً.</span>
        </div>
      )}

      {/* Actions */}
      <div
        className="flex gap-2 mt-2 pt-3 border-t border-gray-200/60"
        dir="rtl"
      >
        {status !== 'contacted' ? (
          <>
            {/* WhatsApp Button */}
            <Button
              onClick={handleWhatsAppClick}
              disabled={!request.source_phone}
              className="flex-1 bg-green-600 hover:bg-green-700 text-white shadow-sm"
              size="sm"
            >
              <MessageCircle size={16} />
              <span>تواصل</span>
            </Button>

            {/* Claim / Reserve */}
            <Button
              onClick={handleClaim}
              variant="outline"
              size="sm"
              className="bg-blue-50 text-blue-600 hover:bg-blue-100 border-blue-200"
            >
              <CheckSquare size={16} />
              <span className="hidden sm:inline">حجز</span>
            </Button>

            {/* Copy */}
            <Button
              onClick={handleCopy}
              variant="ghost"
              size="icon"
              className="h-8 w-8 text-gray-500 hover:text-gray-700"
            >
              <Copy size={16} />
            </Button>

            {/* Snooze */}
            <Button
              onClick={handleSnooze}
              variant="ghost"
              size="icon"
              className="h-8 w-8 text-gray-500 hover:text-orange-600 hover:bg-orange-50"
            >
              <AlarmClock size={16} />
            </Button>

            {/* Dismiss */}
            <Button
              onClick={handleDismiss}
              variant="ghost"
              size="icon"
              className="h-8 w-8 text-gray-400 hover:text-red-500 hover:bg-red-50"
            >
              <X size={16} />
            </Button>
          </>
        ) : (
          <div className="flex-1 flex items-center justify-center gap-2 text-gray-500 bg-gray-100 rounded py-1.5 text-sm font-bold opacity-75 cursor-default">
            <Check size={16} /> تم التواصل
          </div>
        )}
      </div>

      {/* Source Footer */}
      <div
        className="mt-3 pt-2 border-t border-gray-100 text-xs text-gray-400 flex justify-between"
        dir="rtl"
      >
        <span>{request.source_name || 'غير معروف'}</span>
        {request.source_phone && (
          <span className="font-mono" dir="ltr">
            {request.source_phone}
          </span>
        )}
      </div>
    </Card>
  )
}
