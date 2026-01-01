import { Slider } from '@/components/ui/slider'
import type { AdjustmentSettings } from './types'

interface AdjustmentControlsProps {
  adjustments: AdjustmentSettings
  onAdjustmentsChange: (adjustments: AdjustmentSettings) => void
}

export function AdjustmentControls({
  adjustments,
  onAdjustmentsChange,
}: AdjustmentControlsProps) {
  return (
    <div className="glass-card p-6 rounded-xl">
      <h3 className="text-lg font-semibold text-foreground mb-6">
        Adjustment Controls
      </h3>
      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-sm text-muted-foreground">
              Price Flexibility
            </span>
            <span className="text-sm font-medium text-teal">
              {adjustments.priceFlexibility}%
            </span>
          </div>
          <Slider
            value={[adjustments.priceFlexibility]}
            onValueChange={(v) =>
              onAdjustmentsChange({ ...adjustments, priceFlexibility: v[0] })
            }
            max={50}
            step={1}
            className="slider-teal"
          />
        </div>
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-sm text-muted-foreground">
              Quantity Tolerance
            </span>
            <span className="text-sm font-medium text-amber">
              {adjustments.quantityTolerance}%
            </span>
          </div>
          <Slider
            value={[adjustments.quantityTolerance]}
            onValueChange={(v) =>
              onAdjustmentsChange({ ...adjustments, quantityTolerance: v[0] })
            }
            max={50}
            step={1}
            className="slider-amber"
          />
        </div>
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-sm text-muted-foreground">
              Dosage Strictness
            </span>
            <span className="text-sm font-medium text-purple-400">
              {adjustments.dosageStrictness}%
            </span>
          </div>
          <Slider
            value={[adjustments.dosageStrictness]}
            onValueChange={(v) =>
              onAdjustmentsChange({ ...adjustments, dosageStrictness: v[0] })
            }
            max={100}
            step={1}
            className="slider-purple"
          />
        </div>
      </div>
    </div>
  )
}
