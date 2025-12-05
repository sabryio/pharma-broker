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
import { Spinner } from '@/components/ui/spinner'
import { useConfig, useUpdateConfig } from '@/lib/api'

export function ConfigPanel() {
  const [open, setOpen] = useState(false)
  const { data: config, isLoading } = useConfig()
  const updateConfig = useUpdateConfig()

  const [localThreshold, setLocalThreshold] = useState(0.5)
  const [localBatchSize, setLocalBatchSize] = useState(5)
  const [localDelay, setLocalDelay] = useState(5)

  // Sync local state when config loads
  useEffect(() => {
    if (config) {
      setLocalThreshold(config.match_threshold)
      setLocalBatchSize(config.batch_size)
      setLocalDelay(config.process_delay_seconds)
    }
  }, [config])

  const handleSave = () => {
    updateConfig.mutate({
      match_threshold: localThreshold,
      batch_size: localBatchSize,
      process_delay_seconds: localDelay,
    })
  }

  const hasChanges =
    config &&
    (localThreshold !== config.match_threshold ||
      localBatchSize !== config.batch_size ||
      localDelay !== config.process_delay_seconds)

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button
          variant="outline"
          className="fixed bottom-6 right-6 h-14 w-14 rounded-full shadow-lg border-2 hover:bg-secondary"
          size="icon"
        >
          <span className="text-xl">⚙️</span>
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <span className="text-2xl">⚙️</span>
            AI Configuration
          </DialogTitle>
        </DialogHeader>

        {isLoading ? (
          <div className="flex items-center justify-center py-8">
            <Spinner className="h-8 w-8" />
          </div>
        ) : (
          <div className="space-y-6 py-4">
            {/* Match Threshold */}
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <Label htmlFor="threshold">Match Threshold</Label>
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
                Minimum similarity score to suggest a match between offers and
                requests.
              </p>
            </div>

            {/* Batch Size */}
            <div className="space-y-2">
              <Label htmlFor="batchSize">Batch Size</Label>
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
                Number of messages to process in a single AI request.
              </p>
            </div>

            {/* Process Delay */}
            <div className="space-y-2">
              <Label htmlFor="delay">Process Delay (seconds)</Label>
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
                Time to wait before processing accumulated messages.
              </p>
            </div>

            {/* Save Button */}
            <Button
              onClick={handleSave}
              disabled={!hasChanges || updateConfig.isPending}
              className="w-full bg-linear-to-r from-blue-500 to-cyan-500 hover:from-blue-600 hover:to-cyan-600"
            >
              {updateConfig.isPending ? (
                <>
                  <Spinner className="mr-2 h-4 w-4" />
                  Saving...
                </>
              ) : hasChanges ? (
                '💾 Save Changes'
              ) : (
                '✓ No Changes'
              )}
            </Button>

            {/* Success Message */}
            {updateConfig.isSuccess && (
              <p className="text-center text-sm text-green-500">
                ✓ Configuration saved successfully!
              </p>
            )}
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
