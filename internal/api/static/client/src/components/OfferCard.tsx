import { useState } from 'react'
import { Card } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { timeAgo } from '@/lib/sse'
import { showSuccess, showInfo, showWarning } from '@/lib/toast'
import type { Offer } from '@/lib/types'
import {
  Clock,
  Users,
  MessageCircle,
  CheckSquare,
  AlarmClock,
  X,
  Check,
  Copy,
} from 'lucide-react'

interface OfferCardProps {
  offer: Offer
  isMatch?: boolean
  onDismiss?: (id: string) => void
  onContact?: (id: string) => void
}

export function OfferCard({
  offer,
  isMatch = false,
  onDismiss,
  onContact,
}: OfferCardProps) {
  const [status, setStatus] = useState<'active' | 'contacted' | 'dismissed'>(
    'active',
  )
  const [snoozed, setSnoozed] = useState(false)

  const price = offer.price ? `${offer.price} ${offer.currency || 'EGP'}` : ''
  const qty = offer.quantity ? `${offer.quantity} ${offer.unit || ''}` : ''

  // Dynamic styles based on match status
  const borderColor = isMatch
    ? 'border-green-600 ring-2 ring-green-500 ring-offset-1'
    : 'border-blue-300 hover:border-blue-400'

  const bgColor = isMatch
    ? 'bg-gradient-to-r from-green-50 to-white'
    : 'bg-white'

  // Open WhatsApp chat
  const handleWhatsAppClick = () => {
    if (offer.source_phone) {
      const cleanPhone = offer.source_phone.replace(/\D/g, '')
      const text = `السلام عليكم، بخصوص *${offer.medication}* اللي حضرتك عارض في جروب *${offer.group_name || 'الواتساب'}*... هل لسه متاح؟`
      window.open(
        `https://web.whatsapp.com/send?phone=${cleanPhone}&text=${encodeURIComponent(text)}`,
        '_blank',
      )
      showInfo('تم فتح واتساب', 'تذكر تحديث حالة العرض بعد التواصل')
    }
  }

  // Mark as contacted/claimed
  const handleClaim = () => {
    setStatus('contacted')
    onContact?.(offer.id)
    showSuccess('تم الحجز', `${offer.medication} - تم تحديد كمحجوز`)
  }

  // Snooze (hide temporarily)
  const handleSnooze = () => {
    setSnoozed(true)
    showWarning('تم التأجيل', 'سيظهر العرض مرة أخرى بعد 30 دقيقة')
    setTimeout(
      () => {
        setSnoozed(false)
        showInfo('انتهى التأجيل', `${offer.medication} - ظهر مرة أخرى`)
      },
      30 * 60 * 1000,
    ) // 30 minutes
  }

  // Dismiss permanently
  const handleDismiss = () => {
    setStatus('dismissed')
    onDismiss?.(offer.id)
    showInfo('تم الإخفاء', `${offer.medication} - تم إخفاء العرض`)
  }

  // Copy details
  const handleCopy = () => {
    const details = `${offer.medication}${qty ? ` - ${qty}` : ''}${price ? ` - ${price}` : ''}\nمن: ${offer.source_name || offer.source_phone}`
    navigator.clipboard.writeText(details)
    showSuccess('تم النسخ', 'تم نسخ تفاصيل العرض')
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
            {offer.medication}
            {offer.unit && (
              <span className="text-xs font-normal text-gray-500 bg-gray-100 px-2 py-1 rounded-full border border-gray-200">
                {offer.unit}
              </span>
            )}
          </h3>

          {/* Meta Info Line */}
          <div className="text-xs text-gray-500 flex items-center gap-3 mt-1.5 flex-wrap">
            <span className="flex items-center gap-1">
              <Clock size={12} />
              {timeAgo(offer.created_at)}
            </span>

            {offer.group_name && (
              <span className="flex items-center gap-1 bg-slate-100 px-2 py-0.5 rounded text-slate-600 font-medium">
                <Users size={12} />
                {offer.group_name}
              </span>
            )}
          </div>
        </div>
        <Badge className="bg-blue-100 text-blue-800 border-blue-200 whitespace-nowrap">
          عرض (راكد)
        </Badge>
      </div>

      {/* Details */}
      <div className="flex gap-4 mb-3 text-sm text-gray-700" dir="rtl">
        <div className="flex-1 bg-gray-50/80 p-2 rounded border border-gray-100">
          <span className="text-gray-400 text-xs block">الكمية</span>
          <span className="font-semibold">{qty || 'غير محدد'}</span>
        </div>
        {price && (
          <div className="flex-1 bg-gray-50/80 p-2 rounded border border-gray-100">
            <span className="text-gray-400 text-xs block">السعر</span>
            <span className="font-semibold text-green-600">{price}</span>
          </div>
        )}
        {offer.expiry_date && (
          <div className="flex-1 bg-gray-50/80 p-2 rounded border border-gray-100">
            <span className="text-gray-400 text-xs block">الصلاحية</span>
            <span className="font-semibold">
              {offer.expiry_date.substring(0, 7)}
            </span>
          </div>
        )}
      </div>

      {/* Notes */}
      {offer.notes && (
        <div
          className="mb-3 bg-gray-50/80 p-2 rounded border border-gray-100 text-sm"
          dir="rtl"
        >
          <span className="text-gray-400 text-xs block">ملاحظات</span>
          <span className="font-medium">{offer.notes}</span>
        </div>
      )}

      {/* Match Alert */}
      {isMatch && (
        <div
          className="mb-3 flex items-center gap-2 text-green-800 text-sm bg-green-100 p-2 rounded border border-green-200 shadow-sm"
          dir="rtl"
        >
          <span className="text-green-600">⚡</span>
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
              disabled={!offer.source_phone}
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
        <span>{offer.source_name || 'غير معروف'}</span>
        {offer.source_phone && (
          <span className="font-mono" dir="ltr">
            {offer.source_phone}
          </span>
        )}
      </div>
    </Card>
  )
}
