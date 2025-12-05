import { Card, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Progress } from '@/components/ui/progress'
import type { Match } from '@/lib/types'
import { Check, X, ArrowLeft } from 'lucide-react'

interface MatchCardProps {
  match: Match
  onConfirm: (id: string) => void
  onReject: (id: string) => void
  isLoading?: boolean
}

export function MatchCard({
  match,
  onConfirm,
  onReject,
  isLoading,
}: MatchCardProps) {
  const offer = match.offer
  const request = match.request
  const scorePercent = Math.round(match.score * 100)

  return (
    <Card className="mb-3 border-l-4 border-l-green-500" dir="rtl">
      <CardContent className="p-4">
        {/* Score Bar */}
        <div className="flex items-center gap-2 mb-3">
          <span className="text-sm font-semibold text-primary">
            {scorePercent}%
          </span>
          <Progress value={scorePercent} className="flex-1 h-2" />
          <span className="text-xs text-muted-foreground">نسبة التطابق</span>
        </div>

        {/* Offer and Request Grid */}
        <div className="grid grid-cols-[1fr_auto_1fr] gap-3 items-center">
          {/* Offer Side */}
          <div className="bg-blue-50 dark:bg-blue-950/30 p-3 rounded-lg border border-blue-200 dark:border-blue-800">
            <p className="text-xs text-blue-600 font-semibold mb-1">
              عرض (راكد)
            </p>
            {offer?.group_name && (
              <p className="text-[10px] text-muted-foreground mb-1">
                📍 {offer.group_name}
              </p>
            )}
            <p className="font-semibold text-sm">
              {offer?.medication || 'غير معروف'}
            </p>
            <p className="text-xs text-muted-foreground">
              {offer?.quantity || '?'} {offer?.unit || ''} بسعر{' '}
              {offer?.price || '?'} جنيه
            </p>
            <p className="text-xs text-muted-foreground mt-1">
              {offer?.source_name || 'غير معروف'}
            </p>
            {offer?.source_phone && (
              <p
                className="text-[10px] font-mono text-muted-foreground"
                dir="ltr"
              >
                {offer.source_phone}
              </p>
            )}
          </div>

          {/* Arrow */}
          <ArrowLeft className="h-6 w-6 text-muted-foreground" />

          {/* Request Side */}
          <div className="bg-red-50 dark:bg-red-950/30 p-3 rounded-lg border border-red-200 dark:border-red-800">
            <p className="text-xs text-red-600 font-semibold mb-1">
              طلب (ناقص)
            </p>
            {request?.group_name && (
              <p className="text-[10px] text-muted-foreground mb-1">
                📍 {request.group_name}
              </p>
            )}
            <p className="font-semibold text-sm">
              {request?.medication || 'غير معروف'}
            </p>
            <p className="text-xs text-muted-foreground">
              {request?.quantity || '?'} {request?.unit || ''}{' '}
              {request?.max_price ? `أقصى ${request.max_price} جنيه` : ''}
            </p>
            <p className="text-xs text-muted-foreground mt-1">
              {request?.source_name || 'غير معروف'}
            </p>
            {request?.source_phone && (
              <p
                className="text-[10px] font-mono text-muted-foreground"
                dir="ltr"
              >
                {request.source_phone}
              </p>
            )}
          </div>
        </div>

        {/* Action Buttons */}
        <div className="flex gap-2 mt-4">
          <Button
            variant="outline"
            className="flex-1 text-red-600 hover:bg-red-50 hover:text-red-700"
            onClick={() => onReject(match.id)}
            disabled={isLoading}
          >
            <X className="ml-2 h-4 w-4" />
            رفض
          </Button>
          <Button
            className="flex-1 bg-green-600 hover:bg-green-700"
            onClick={() => onConfirm(match.id)}
            disabled={isLoading}
          >
            <Check className="ml-2 h-4 w-4" />
            تأكيد التطابق
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
