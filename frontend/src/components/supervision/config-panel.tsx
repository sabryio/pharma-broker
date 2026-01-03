import { useState, useEffect } from 'react'
import {
  Settings,
  Save,
  RotateCcw,
  Plus,
  Trash2,
  AlertCircle,
  Clock,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import type { AutoApproveConfig } from '@/schema/supervision'

interface ConfigPanelProps {
  config: AutoApproveConfig
  onSave: (config: AutoApproveConfig) => void
  isSaving?: boolean
  className?: string
}

/**
 * Configuration panel for AI auto-approve settings
 * Requirements: 5.1, 5.2, 5.3, 5.5
 */
export function ConfigPanel({
  config,
  onSave,
  isSaving,
  className,
}: ConfigPanelProps) {
  const [formData, setFormData] = useState<AutoApproveConfig>(config)
  const [newCategory, setNewCategory] = useState('')
  const [newCategoryThreshold, setNewCategoryThreshold] = useState(0.85)
  const [errors, setErrors] = useState<Record<string, string>>({})

  useEffect(() => {
    setFormData(config)
  }, [config])

  const hasChanges = JSON.stringify(formData) !== JSON.stringify(config)

  const validateThreshold = (value: number): boolean => {
    return value >= 0.7 && value <= 0.99
  }

  const handleThresholdChange = (value: number) => {
    if (!validateThreshold(value)) {
      setErrors((prev) => ({
        ...prev,
        confidenceThreshold: 'Threshold must be between 0.70 and 0.99',
      }))
    } else {
      setErrors((prev) => {
        const { confidenceThreshold, ...rest } = prev
        return rest
      })
    }
    setFormData((prev) => ({ ...prev, confidenceThreshold: value }))
  }

  const handleAddCategory = () => {
    if (newCategory.trim() && validateThreshold(newCategoryThreshold)) {
      setFormData((prev) => ({
        ...prev,
        categoryThresholds: {
          ...prev.categoryThresholds,
          [newCategory.trim()]: newCategoryThreshold,
        },
      }))
      setNewCategory('')
      setNewCategoryThreshold(0.85)
    }
  }

  const handleRemoveCategory = (category: string) => {
    setFormData((prev) => {
      const { [category]: _, ...rest } = prev.categoryThresholds
      return { ...prev, categoryThresholds: rest }
    })
  }

  const handleSave = () => {
    if (Object.keys(errors).length === 0) {
      onSave(formData)
    }
  }

  const handleReset = () => {
    setFormData(config)
    setErrors({})
  }

  return (
    <div className={cn('glass-card rounded-xl overflow-hidden', className)}>
      {/* Header */}
      <div className="p-4 border-b border-border flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Settings className="w-5 h-5 text-teal" />
          <h3 className="font-semibold text-foreground">
            Auto-Approve Configuration
          </h3>
        </div>

        <div className="flex items-center gap-2">
          {hasChanges && (
            <button
              onClick={handleReset}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm text-muted-foreground hover:text-foreground hover:bg-secondary/50 transition-colors"
            >
              <RotateCcw className="w-3.5 h-3.5" />
              Reset
            </button>
          )}
          <button
            onClick={handleSave}
            disabled={!hasChanges || Object.keys(errors).length > 0 || isSaving}
            className={cn(
              'flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-sm font-medium transition-colors',
              hasChanges && Object.keys(errors).length === 0
                ? 'bg-teal text-white hover:bg-teal/80'
                : 'bg-secondary text-muted-foreground cursor-not-allowed',
            )}
          >
            <Save className="w-3.5 h-3.5" />
            {isSaving ? 'Saving...' : 'Save Changes'}
          </button>
        </div>
      </div>

      <div className="p-6 space-y-6">
        {/* Global Toggle */}
        <div className="flex items-center justify-between">
          <div>
            <label className="font-medium text-foreground">
              Enable Auto-Approve
            </label>
            <p className="text-sm text-muted-foreground">
              Automatically approve high-confidence matches
            </p>
          </div>
          <button
            onClick={() =>
              setFormData((prev) => ({ ...prev, enabled: !prev.enabled }))
            }
            className={cn(
              'relative w-12 h-6 rounded-full transition-colors',
              formData.enabled ? 'bg-teal' : 'bg-secondary',
            )}
          >
            <span
              className={cn(
                'absolute top-1 w-4 h-4 rounded-full bg-white transition-transform',
                formData.enabled ? 'translate-x-7' : 'translate-x-1',
              )}
            />
          </button>
        </div>

        {/* Confidence Threshold */}
        <div>
          <label className="block font-medium text-foreground mb-2">
            Confidence Threshold
          </label>
          <p className="text-sm text-muted-foreground mb-3">
            Minimum AI confidence required for auto-approval (0.70 - 0.99)
          </p>
          <div className="flex items-center gap-4">
            <input
              type="range"
              min="0.70"
              max="0.99"
              step="0.01"
              value={formData.confidenceThreshold}
              onChange={(e) =>
                handleThresholdChange(parseFloat(e.target.value))
              }
              className="flex-1 h-2 bg-secondary rounded-full appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-teal"
            />
            <div className="w-20">
              <input
                type="number"
                min="0.70"
                max="0.99"
                step="0.01"
                value={formData.confidenceThreshold}
                onChange={(e) =>
                  handleThresholdChange(parseFloat(e.target.value))
                }
                className="w-full px-3 py-1.5 rounded-lg bg-secondary border border-border text-sm text-foreground text-center focus:outline-none focus:ring-2 focus:ring-teal/50"
              />
            </div>
          </div>
          {errors.confidenceThreshold && (
            <p className="mt-2 text-sm text-red-400 flex items-center gap-1">
              <AlertCircle className="w-3.5 h-3.5" />
              {errors.confidenceThreshold}
            </p>
          )}
        </div>

        {/* Category Thresholds */}
        <div>
          <label className="block font-medium text-foreground mb-2">
            Category-Specific Thresholds
          </label>
          <p className="text-sm text-muted-foreground mb-3">
            Override the global threshold for specific medication categories
          </p>

          {/* Existing Categories */}
          {Object.entries(formData.categoryThresholds).length > 0 && (
            <div className="space-y-2 mb-4">
              {Object.entries(formData.categoryThresholds).map(
                ([category, threshold]) => (
                  <div
                    key={category}
                    className="flex items-center gap-3 p-3 rounded-lg bg-secondary/50"
                  >
                    <span className="flex-1 font-medium text-foreground">
                      {category}
                    </span>
                    <input
                      type="number"
                      min="0.70"
                      max="0.99"
                      step="0.01"
                      value={threshold}
                      onChange={(e) =>
                        setFormData((prev) => ({
                          ...prev,
                          categoryThresholds: {
                            ...prev.categoryThresholds,
                            [category]: parseFloat(e.target.value),
                          },
                        }))
                      }
                      className="w-20 px-3 py-1.5 rounded-lg bg-background border border-border text-sm text-foreground text-center focus:outline-none focus:ring-2 focus:ring-teal/50"
                    />
                    <button
                      onClick={() => handleRemoveCategory(category)}
                      className="p-1.5 rounded-lg text-muted-foreground hover:text-red-400 hover:bg-red-400/10 transition-colors"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                ),
              )}
            </div>
          )}

          {/* Add New Category */}
          <div className="flex items-center gap-2">
            <input
              type="text"
              value={newCategory}
              onChange={(e) => setNewCategory(e.target.value)}
              placeholder="Category name..."
              className="flex-1 px-3 py-2 rounded-lg bg-secondary border border-border text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
            />
            <input
              type="number"
              min="0.70"
              max="0.99"
              step="0.01"
              value={newCategoryThreshold}
              onChange={(e) =>
                setNewCategoryThreshold(parseFloat(e.target.value))
              }
              className="w-20 px-3 py-2 rounded-lg bg-secondary border border-border text-sm text-foreground text-center focus:outline-none focus:ring-2 focus:ring-teal/50"
            />
            <button
              onClick={handleAddCategory}
              disabled={!newCategory.trim()}
              className={cn(
                'flex items-center gap-1.5 px-3 py-2 rounded-lg text-sm font-medium transition-colors',
                newCategory.trim()
                  ? 'bg-teal/20 text-teal hover:bg-teal/30'
                  : 'bg-secondary text-muted-foreground cursor-not-allowed',
              )}
            >
              <Plus className="w-4 h-4" />
              Add
            </button>
          </div>
        </div>

        {/* Schedule */}
        <div>
          <label className="block font-medium text-foreground mb-2">
            <div className="flex items-center gap-2">
              <Clock className="w-4 h-4 text-teal" />
              Schedule (Optional)
            </div>
          </label>
          <p className="text-sm text-muted-foreground mb-3">
            Cron expression for when auto-approve should run (leave empty for
            always)
          </p>
          <input
            type="text"
            value={formData.schedule || ''}
            onChange={(e) =>
              setFormData((prev) => ({
                ...prev,
                schedule: e.target.value || null,
              }))
            }
            placeholder="e.g., 0 9-17 * * 1-5 (9am-5pm weekdays)"
            className="w-full px-3 py-2 rounded-lg bg-secondary border border-border text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
          />
        </div>

        {/* Safety Settings */}
        <div className="pt-4 border-t border-border">
          <h4 className="font-medium text-foreground mb-4">Safety Settings</h4>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm text-muted-foreground mb-1.5">
                Override Rate Pause Threshold
              </label>
              <div className="flex items-center gap-2">
                <input
                  type="number"
                  min="0.01"
                  max="0.5"
                  step="0.01"
                  value={formData.overrideRatePauseThreshold}
                  onChange={(e) =>
                    setFormData((prev) => ({
                      ...prev,
                      overrideRatePauseThreshold: parseFloat(e.target.value),
                    }))
                  }
                  className="w-full px-3 py-2 rounded-lg bg-secondary border border-border text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
                />
                <span className="text-sm text-muted-foreground">
                  ({(formData.overrideRatePauseThreshold * 100).toFixed(0)}%)
                </span>
              </div>
            </div>

            <div>
              <label className="block text-sm text-muted-foreground mb-1.5">
                Consecutive Override Limit
              </label>
              <input
                type="number"
                min="1"
                max="20"
                value={formData.consecutiveOverrideLimit}
                onChange={(e) =>
                  setFormData((prev) => ({
                    ...prev,
                    consecutiveOverrideLimit: parseInt(e.target.value),
                  }))
                }
                className="w-full px-3 py-2 rounded-lg bg-secondary border border-border text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
              />
            </div>

            <div>
              <label className="block text-sm text-muted-foreground mb-1.5">
                Override Cooldown (minutes)
              </label>
              <input
                type="number"
                min="1"
                max="1440"
                value={formData.overrideCooldownMins}
                onChange={(e) =>
                  setFormData((prev) => ({
                    ...prev,
                    overrideCooldownMins: parseInt(e.target.value),
                  }))
                }
                className="w-full px-3 py-2 rounded-lg bg-secondary border border-border text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
              />
            </div>

            <div>
              <label className="block text-sm text-muted-foreground mb-1.5">
                Undo Window (minutes)
              </label>
              <input
                type="number"
                min="1"
                max="120"
                value={formData.undoWindowMins}
                onChange={(e) =>
                  setFormData((prev) => ({
                    ...prev,
                    undoWindowMins: parseInt(e.target.value),
                  }))
                }
                className="w-full px-3 py-2 rounded-lg bg-secondary border border-border text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
