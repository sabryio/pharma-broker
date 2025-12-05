import { useState } from 'react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent } from '@/components/ui/card'
import { Spinner } from '@/components/ui/spinner'
import { useAnalyze } from '@/lib/api'
import type { AnalyzeItem } from '@/lib/types'
import { FlaskConical, Sparkles } from 'lucide-react'

export function AnalyzeModal() {
  const [open, setOpen] = useState(false)
  const [text, setText] = useState('')
  const analyze = useAnalyze()

  const handleAnalyze = () => {
    if (!text.trim()) return
    analyze.mutate({ text })
  }

  const getTypeColor = (type: string) => {
    switch (type) {
      case 'OFFER':
        return 'bg-blue-500/20 text-blue-600'
      case 'REQUEST':
        return 'bg-red-500/20 text-red-600'
      case 'BOTH':
        return 'bg-purple-500/20 text-purple-600'
      default:
        return 'bg-secondary text-secondary-foreground'
    }
  }

  const getTypeLabel = (type: string) => {
    switch (type) {
      case 'OFFER':
        return 'عرض'
      case 'REQUEST':
        return 'طلب'
      case 'BOTH':
        return 'عرض وطلب'
      default:
        return type
    }
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button
          className="fixed bottom-20 left-6 h-14 w-14 rounded-full shadow-lg bg-linear-to-br from-violet-500 to-purple-600 hover:from-violet-600 hover:to-purple-700 text-white"
          size="icon"
        >
          <FlaskConical className="h-6 w-6" />
        </Button>
      </DialogTrigger>
      <DialogContent
        className="max-w-2xl max-h-[85vh] overflow-hidden flex flex-col"
        dir="rtl"
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <FlaskConical className="h-6 w-6" />
            تحليل النص بالذكاء الاصطناعي
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto space-y-4">
          {/* Input Section */}
          <div className="space-y-2">
            <label className="text-sm font-medium text-muted-foreground">
              الصق الرسالة للتحليل:
            </label>
            <Textarea
              value={text}
              onChange={(e) => setText(e.target.value)}
              placeholder="عندي اوجمنتين 1 جم ٥ علب ب ٣٠٠ جنيه الواحدة..."
              className="min-h-[120px]"
              dir="rtl"
            />
            <Button
              onClick={handleAnalyze}
              disabled={!text.trim() || analyze.isPending}
              className="w-full bg-linear-to-l from-violet-500 to-purple-600 hover:from-violet-600 hover:to-purple-700"
            >
              {analyze.isPending ? (
                <>
                  <Spinner className="ml-2 h-4 w-4" />
                  جاري التحليل...
                </>
              ) : (
                <>
                  <Sparkles className="ml-2 h-4 w-4" />
                  تحليل بالذكاء الاصطناعي
                </>
              )}
            </Button>
          </div>

          {/* Error */}
          {analyze.error && (
            <div className="p-3 rounded-lg bg-red-500/10 border border-red-500/20 text-red-500 text-sm">
              {analyze.error.message}
            </div>
          )}

          {/* Results */}
          {analyze.data && (
            <div className="space-y-3">
              <h3 className="text-sm font-medium text-muted-foreground">
                تم العثور على {analyze.data.items.length} عنصر:
              </h3>

              {analyze.data.items.length === 0 ? (
                <div className="p-6 text-center text-muted-foreground bg-secondary/50 rounded-lg">
                  لم يتم العثور على عروض أو طلبات في هذا النص.
                </div>
              ) : (
                analyze.data.items.map((item: AnalyzeItem, i: number) => (
                  <Card key={i} className="overflow-hidden">
                    <CardContent className="p-4">
                      <div className="flex items-start justify-between mb-3">
                        <div>
                          <Badge className={getTypeColor(item.type)}>
                            {getTypeLabel(item.type)}
                          </Badge>
                        </div>
                        {item.urgent && (
                          <Badge className="bg-red-500/20 text-red-500">
                            عاجل
                          </Badge>
                        )}
                      </div>

                      <div className="space-y-2">
                        <div>
                          <p className="font-semibold text-lg">
                            {item.medication}
                          </p>
                          <p className="text-sm text-muted-foreground">
                            {item.medication_raw}
                          </p>
                        </div>

                        <div className="flex flex-wrap gap-2">
                          {item.quantity > 0 && (
                            <Badge variant="outline">
                              {item.quantity} {item.unit || 'وحدة'}
                            </Badge>
                          )}
                          {item.price && item.price > 0 && (
                            <Badge variant="outline" className="text-green-600">
                              {item.price} {item.currency || 'جنيه'}
                            </Badge>
                          )}
                          {item.max_price && item.max_price > 0 && (
                            <Badge variant="outline" className="text-blue-600">
                              أقصى: {item.max_price} {item.currency || 'جنيه'}
                            </Badge>
                          )}
                          {item.expiry_date && (
                            <Badge variant="outline">
                              الصلاحية: {item.expiry_date}
                            </Badge>
                          )}
                        </div>

                        {item.notes && (
                          <p className="text-sm text-muted-foreground mt-2">
                            📝 {item.notes}
                          </p>
                        )}
                      </div>
                    </CardContent>
                  </Card>
                ))
              )}
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
