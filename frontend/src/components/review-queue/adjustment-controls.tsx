import { useEffect, useState, useCallback } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Slider } from '@/components/ui/slider'
import { Badge } from '@/components/ui/badge'
import { Loader2, Check, AlertCircle, Settings2 } from 'lucide-react'
import {
  getWeights,
  updateWeights,
  slidersToWeights,
  weightsToSliders,
} from '@/api/weights'
import { cn } from '@/lib/utils'

// Debounce helper
function useDebounce<T>(value: T, delay: number): T {
  const [debouncedValue, setDebouncedValue] = useState<T>(value)

  useEffect(() => {
    const handler = setTimeout(() => {
      setDebouncedValue(value)
    }, delay)

    return () => {
      clearTimeout(handler)
    }
  }, [value, delay])

  return debouncedValue
}

interface SliderValues {
  medicationWeight: number
  dosageStrictness: number
  quantityTolerance: number
  priceFlexibility: number
}

const defaultSliders: SliderValues = {
  medicationWeight: 85,
  dosageStrictness: 80,
  quantityTolerance: 15,
  priceFlexibility: 10,
}

export function AdjustmentControls() {
  const queryClient = useQueryClient()
  const [sliders, setSliders] = useState<SliderValues>(defaultSliders)
  const [saveStatus, setSaveStatus] = useState<
    'idle' | 'saving' | 'saved' | 'error'
  >('idle')

  // Fetch current weights
  const {
    data: weightsData,
    isLoading,
    error,
  } = useQuery({
    queryKey: ['weights'],
    queryFn: getWeights,
    staleTime: 30000,
  })

  // Update sliders when weights are fetched
  useEffect(() => {
    if (weightsData?.weights) {
      const converted = weightsToSliders(weightsData.weights)
      setSliders(converted)
    }
  }, [weightsData])

  // Mutation to update weights
  const updateMutation = useMutation({
    mutationFn: updateWeights,
    onMutate: () => {
      setSaveStatus('saving')
    },
    onSuccess: () => {
      setSaveStatus('saved')
      queryClient.invalidateQueries({ queryKey: ['weights'] })
      // Reset status after 2 seconds
      setTimeout(() => setSaveStatus('idle'), 2000)
    },
    onError: () => {
      setSaveStatus('error')
      setTimeout(() => setSaveStatus('idle'), 3000)
    },
  })

  // Debounced slider values
  const debouncedSliders = useDebounce(sliders, 500)

  // Auto-save when debounced values change
  useEffect(() => {
    // Skip if still loading initial data or if values haven't changed from initial
    if (isLoading || !weightsData) return

    const weights = slidersToWeights(debouncedSliders)

    // Check if weights actually changed
    const currentWeights = weightsData.weights
    const hasChanged =
      Math.abs(weights.medication - currentWeights.medication) > 0.001 ||
      Math.abs(weights.dosage - currentWeights.dosage) > 0.001 ||
      Math.abs(weights.quantity - currentWeights.quantity) > 0.001 ||
      Math.abs(weights.price - currentWeights.price) > 0.001

    if (hasChanged) {
      updateMutation.mutate({
        ...weights,
        reason: 'Updated via UI sliders',
      })
    }
  }, [debouncedSliders])

  const handleSliderChange = useCallback(
    (key: keyof SliderValues, value: number) => {
      setSliders((prev) => ({ ...prev, [key]: value }))
    },
    [],
  )

  // Calculate current weights for display
  const currentWeights = slidersToWeights(sliders)

  if (isLoading) {
    return (
      <div className="glass-card p-6 rounded-xl">
        <div className="flex items-center justify-center gap-2 text-muted-foreground">
          <Loader2 className="h-4 w-4 animate-spin" />
          <span>Loading weights...</span>
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className="glass-card p-6 rounded-xl">
        <div className="flex items-center justify-center gap-2 text-destructive">
          <AlertCircle className="h-4 w-4" />
          <span>Failed to load weights</span>
        </div>
      </div>
    )
  }

  return (
    <div className="glass-card p-6 rounded-xl">
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-2">
          <Settings2 className="h-5 w-5 text-teal" />
          <h3 className="text-lg font-semibold text-foreground">
            Matching Weights
          </h3>
        </div>
        <SaveStatusBadge status={saveStatus} />
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        {/* Medication Name Weight - Most Important */}
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-sm text-muted-foreground">
              Medication Name
            </span>
            <span className="text-sm font-medium text-emerald-400">
              {Math.round(currentWeights.medication * 100)}%
            </span>
          </div>
          <Slider
            value={[sliders.medicationWeight]}
            onValueChange={(v) => handleSliderChange('medicationWeight', v[0])}
            min={50}
            max={100}
            step={1}
            className="slider-emerald"
          />
          <p className="text-xs text-muted-foreground/70">
            Primary matching factor
          </p>
        </div>

        {/* Dosage Strictness */}
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-sm text-muted-foreground">
              Dosage Strictness
            </span>
            <span className="text-sm font-medium text-purple-400">
              {Math.round(currentWeights.dosage * 100)}%
            </span>
          </div>
          <Slider
            value={[sliders.dosageStrictness]}
            onValueChange={(v) => handleSliderChange('dosageStrictness', v[0])}
            max={100}
            step={1}
            className="slider-purple"
          />
          <p className="text-xs text-muted-foreground/70">
            Higher = stricter dosage matching
          </p>
        </div>

        {/* Quantity Tolerance */}
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-sm text-muted-foreground">
              Quantity Tolerance
            </span>
            <span className="text-sm font-medium text-amber">
              {Math.round(currentWeights.quantity * 100)}%
            </span>
          </div>
          <Slider
            value={[sliders.quantityTolerance]}
            onValueChange={(v) => handleSliderChange('quantityTolerance', v[0])}
            max={50}
            step={1}
            className="slider-amber"
          />
          <p className="text-xs text-muted-foreground/70">
            Higher tolerance = less strict
          </p>
        </div>

        {/* Price Flexibility */}
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <span className="text-sm text-muted-foreground">
              Price Flexibility
            </span>
            <span className="text-sm font-medium text-teal">
              {Math.round(currentWeights.price * 100)}%
            </span>
          </div>
          <Slider
            value={[sliders.priceFlexibility]}
            onValueChange={(v) => handleSliderChange('priceFlexibility', v[0])}
            max={50}
            step={1}
            className="slider-teal"
          />
          <p className="text-xs text-muted-foreground/70">
            Higher flexibility = less strict
          </p>
        </div>
      </div>

      {/* Weight Summary */}
      <div className="mt-6 pt-4 border-t border-border/50">
        <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
          <span>Current weights:</span>
          <Badge
            variant="outline"
            className="text-emerald-400 border-emerald-400/30"
          >
            Med: {Math.round(currentWeights.medication * 100)}%
          </Badge>
          <Badge
            variant="outline"
            className="text-purple-400 border-purple-400/30"
          >
            Dose: {Math.round(currentWeights.dosage * 100)}%
          </Badge>
          <Badge variant="outline" className="text-amber border-amber/30">
            Qty: {Math.round(currentWeights.quantity * 100)}%
          </Badge>
          <Badge variant="outline" className="text-teal border-teal/30">
            Price: {Math.round(currentWeights.price * 100)}%
          </Badge>
          <Badge
            variant="outline"
            className="text-muted-foreground border-muted-foreground/30"
          >
            Recency: {Math.round(currentWeights.recency * 100)}%
          </Badge>
        </div>
      </div>
    </div>
  )
}

function SaveStatusBadge({
  status,
}: {
  status: 'idle' | 'saving' | 'saved' | 'error'
}) {
  if (status === 'idle') return null

  return (
    <Badge
      variant="outline"
      className={cn(
        'transition-all duration-200',
        status === 'saving' && 'text-amber border-amber/30 bg-amber/10',
        status === 'saved' &&
          'text-emerald-400 border-emerald-400/30 bg-emerald-400/10',
        status === 'error' &&
          'text-destructive border-destructive/30 bg-destructive/10',
      )}
    >
      {status === 'saving' && (
        <>
          <Loader2 className="h-3 w-3 mr-1 animate-spin" />
          Saving...
        </>
      )}
      {status === 'saved' && (
        <>
          <Check className="h-3 w-3 mr-1" />
          Saved
        </>
      )}
      {status === 'error' && (
        <>
          <AlertCircle className="h-3 w-3 mr-1" />
          Error
        </>
      )}
    </Badge>
  )
}
