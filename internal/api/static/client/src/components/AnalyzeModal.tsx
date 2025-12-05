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
        return 'bg-green-500/20 text-green-500'
      case 'REQUEST':
        return 'bg-blue-500/20 text-blue-500'
      case 'BOTH':
        return 'bg-purple-500/20 text-purple-500'
      default:
        return 'bg-secondary text-secondary-foreground'
    }
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button
          className="fixed bottom-20 right-6 h-14 w-14 rounded-full shadow-lg bg-linear-to-br from-violet-500 to-purple-600 hover:from-violet-600 hover:to-purple-700 text-white"
          size="icon"
        >
          <span className="text-xl">🔬</span>
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-2xl max-h-[85vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <span className="text-2xl">🔬</span>
            AI Text Analysis
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto space-y-4">
          {/* Input Section */}
          <div className="space-y-2">
            <label className="text-sm font-medium text-muted-foreground">
              Paste message to analyze:
            </label>
            <Textarea
              value={text}
              onChange={(e) => setText(e.target.value)}
              placeholder="عندي اوجمنتين 1 جم ٥ علب ب ٣٠٠ جنيه الواحدة..."
              className="min-h-[120px] text-right"
              dir="rtl"
            />
            <Button
              onClick={handleAnalyze}
              disabled={!text.trim() || analyze.isPending}
              className="w-full bg-linear-to-r from-violet-500 to-purple-600 hover:from-violet-600 hover:to-purple-700"
            >
              {analyze.isPending ? (
                <>
                  <Spinner className="mr-2 h-4 w-4" />
                  Analyzing...
                </>
              ) : (
                '✨ Analyze with AI'
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
                Found {analyze.data.items.length} item(s):
              </h3>

              {analyze.data.items.length === 0 ? (
                <div className="p-6 text-center text-muted-foreground bg-secondary/50 rounded-lg">
                  No offers or requests detected in this text.
                </div>
              ) : (
                analyze.data.items.map((item: AnalyzeItem, i: number) => (
                  <Card key={i} className="overflow-hidden">
                    <CardContent className="p-4">
                      <div className="flex items-start justify-between mb-3">
                        <div>
                          <Badge className={getTypeColor(item.type)}>
                            {item.type}
                          </Badge>
                        </div>
                        {item.urgent && (
                          <Badge className="bg-red-500/20 text-red-500">
                            URGENT
                          </Badge>
                        )}
                      </div>

                      <div className="space-y-2">
                        <div>
                          <p className="font-semibold text-lg">
                            {item.medication}
                          </p>
                          <p
                            className="text-sm text-muted-foreground"
                            dir="rtl"
                          >
                            {item.medication_raw}
                          </p>
                        </div>

                        <div className="flex flex-wrap gap-2">
                          {item.quantity > 0 && (
                            <Badge variant="outline">
                              {item.quantity} {item.unit || 'units'}
                            </Badge>
                          )}
                          {item.price && item.price > 0 && (
                            <Badge variant="outline" className="text-green-500">
                              {item.price} {item.currency || 'EGP'}
                            </Badge>
                          )}
                          {item.max_price && item.max_price > 0 && (
                            <Badge variant="outline" className="text-blue-500">
                              Max: {item.max_price} {item.currency || 'EGP'}
                            </Badge>
                          )}
                          {item.expiry_date && (
                            <Badge variant="outline">
                              Exp: {item.expiry_date}
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
