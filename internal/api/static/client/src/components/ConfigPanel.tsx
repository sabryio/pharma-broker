import { useState, useEffect } from 'react'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Slider } from '@/components/ui/slider'
import { Switch } from '@/components/ui/switch'
import { Spinner } from '@/components/ui/spinner'
import { useConfig, useUpdateConfig } from '@/lib/api'
import { Settings } from 'lucide-react'

export function ConfigPanel() {
  const [open, setOpen] = useState(false)
  const { data: config, isLoading } = useConfig()
  const updateConfig = useUpdateConfig()

  const [localAutoParse, setLocalAutoParse] = useState(true)
  const [localThreshold, setLocalThreshold] = useState(0.5)
  const [localBatchSize, setLocalBatchSize] = useState(5)
  const [localDelay, setLocalDelay] = useState(5)

  // Sync local state when config loads
  useEffect(() => {
    if (config) {
      setLocalAutoParse(config.auto_parse_enabled)
      setLocalThreshold(config.match_threshold)
      setLocalBatchSize(config.batch_size)
      setLocalDelay(config.process_delay_seconds)
    }
  }, [config])

  const handleSave = () => {
    updateConfig.mutate({
      auto_parse_enabled: localAutoParse,
      match_threshold: localThreshold,
      batch_size: localBatchSize,
      process_delay_seconds: localDelay,
    })
  }

  // Quick toggle for auto-parse (saves immediately)
  const handleAutoParseToggle = (enabled: boolean) => {
    setLocalAutoParse(enabled)
    updateConfig.mutate({ auto_parse_enabled: enabled })
  }

  const hasChanges =
    config &&
    (localAutoParse !== config.auto_parse_enabled ||
      localThreshold !== config.match_threshold ||
      localBatchSize !== config.batch_size ||
      localDelay !== config.process_delay_seconds)

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button
          variant="outline"
          className="fixed bottom-6 left-6 h-14 w-14 rounded-full shadow-lg border-2 hover:bg-secondary"
          size="icon"
        >
          <Settings className="h-6 w-6" />
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-md" dir="rtl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Settings className="h-6 w-6" />
            إعدادات الذكاء الاصطناعي
          </DialogTitle>
        </DialogHeader>

        {isLoading ? (
          <div className="flex items-center justify-center py-8">
            <Spinner className="h-8 w-8" />
          </div>
        ) : (
          <div className="space-y-6 py-4">
            {/* Auto Parse Toggle - Primary Control */}
            <div
              className={`p-4 rounded-lg border-2 ${localAutoParse ? 'border-green-500/50 bg-green-500/10' : 'border-red-500/50 bg-red-500/10'}`}
            >
              <div className="flex items-center justify-between">
                <div className="space-y-0.5">
                  <Label
                    htmlFor="autoParse"
                    className="text-base font-semibold"
                  >
                    التحليل التلقائي للرسائل
                  </Label>
                  <p className="text-sm text-muted-foreground">
                    {localAutoParse
                      ? '✅ يتم معالجة الرسائل الواردة تلقائياً'
                      : '⏸️ معالجة الرسائل متوقفة مؤقتاً'}
                  </p>
                </div>
                <Switch
                  id="autoParse"
                  checked={localAutoParse}
                  onCheckedChange={handleAutoParseToggle}
                  disabled={updateConfig.isPending}
                />
              </div>
            </div>

            {/* Match Threshold */}
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <Label htmlFor="threshold">نسبة التطابق</Label>
                <span className="text-sm font-mono bg-secondary px-2 py-1 rounded">
                  {Math.round(localThreshold * 100)}%
                </span>
              </div>
              <Slider
                id="threshold"
                min={0}
                max={100}
                step={5}
                value={[localThreshold * 100]}
                onValueChange={(v) => setLocalThreshold(v[0] / 100)}
                className="w-full"
              />
              <p className="text-xs text-muted-foreground">
                أقل نسبة تشابه لاقتراح تطابق بين العروض والطلبات
              </p>
            </div>

            {/* Batch Size */}
            <div className="space-y-2">
              <Label htmlFor="batchSize">حجم الدفعة</Label>
              <Input
                id="batchSize"
                type="number"
                min={1}
                max={20}
                value={localBatchSize}
                onChange={(e) => setLocalBatchSize(parseInt(e.target.value))}
                className="font-mono"
              />
              <p className="text-xs text-muted-foreground">
                عدد الرسائل المعالجة في طلب واحد للذكاء الاصطناعي
              </p>
            </div>

            {/* Process Delay */}
            <div className="space-y-2">
              <Label htmlFor="delay">تأخير المعالجة (ثواني)</Label>
              <Input
                id="delay"
                type="number"
                min={1}
                max={60}
                value={localDelay}
                onChange={(e) => setLocalDelay(parseInt(e.target.value))}
                className="font-mono"
              />
              <p className="text-xs text-muted-foreground">
                وقت الانتظار قبل معالجة الرسائل المتراكمة
              </p>
            </div>

            {/* Save Button */}
            <Button
              onClick={handleSave}
              disabled={!hasChanges || updateConfig.isPending}
              className="w-full bg-linear-to-l from-blue-500 to-cyan-500 hover:from-blue-600 hover:to-cyan-600"
            >
              {updateConfig.isPending ? (
                <>
                  <Spinner className="ml-2 h-4 w-4" />
                  جاري الحفظ...
                </>
              ) : hasChanges ? (
                '💾 حفظ التغييرات'
              ) : (
                '✓ لا توجد تغييرات'
              )}
            </Button>

            {/* Success Message */}
            {updateConfig.isSuccess && (
              <p className="text-center text-sm text-green-500">
                ✓ تم حفظ الإعدادات بنجاح!
              </p>
            )}
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
