import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'
import {
  AlertCircle,
  Calendar,
  Flame,
  Loader2,
  Plus,
  RefreshCw,
  Search,
  Trash2,
  X,
} from 'lucide-react'

import { DashboardLayout } from '@/components/layout/dashboard-layout'
import { Switch } from '@/components/ui/switch'
import { cn } from '@/lib/utils'
import {
  useCreatePriorityMedication,
  useDeletePriorityMedication,
  usePriorityMedications,
  useUpdatePriorityMedication,
} from '@/hooks/use-priority-medications'
import {
  PRIORITY_CONFIG,
  PriorityLevel,
  type CreatePriorityRequest,
  type PriorityMedication,
} from '@/schema/priority-medications'

export const Route = createFileRoute('/priority-medications')({
  component: PriorityMedications,
})

function PriorityMedications() {
  const { data, isLoading, error, refetch } = usePriorityMedications()
  const createMutation = useCreatePriorityMedication()
  const updateMutation = useUpdatePriorityMedication()
  const deleteMutation = useDeletePriorityMedication()

  const [searchQuery, setSearchQuery] = useState('')
  const [filterLevel, setFilterLevel] = useState<PriorityLevel | 'all'>('all')
  const [filterActive, setFilterActive] = useState<
    'all' | 'active' | 'inactive'
  >('all')
  const [showCreateForm, setShowCreateForm] = useState(false)
  const [editingId, setEditingId] = useState<string | null>(null)

  const priorities = data?.priorities ?? []

  // Filter priorities
  const filteredPriorities = priorities.filter((p) => {
    const matchesSearch =
      searchQuery === '' ||
      p.medicationName.toLowerCase().includes(searchQuery.toLowerCase()) ||
      p.medicationNameAr?.toLowerCase().includes(searchQuery.toLowerCase())

    const matchesLevel =
      filterLevel === 'all' || p.priorityLevel === filterLevel

    const isCurrentlyActive =
      p.active &&
      new Date(p.activeFrom) <= new Date() &&
      (!p.activeUntil || new Date(p.activeUntil) > new Date())

    const matchesActive =
      filterActive === 'all' ||
      (filterActive === 'active' && isCurrentlyActive) ||
      (filterActive === 'inactive' && !isCurrentlyActive)

    return matchesSearch && matchesLevel && matchesActive
  })

  const activeCount = priorities.filter((p) => {
    const isCurrentlyActive =
      p.active &&
      new Date(p.activeFrom) <= new Date() &&
      (!p.activeUntil || new Date(p.activeUntil) > new Date())
    return isCurrentlyActive
  }).length

  const criticalCount = priorities.filter(
    (p) => p.priorityLevel === PriorityLevel.CRITICAL,
  ).length

  const toggleActive = (priority: PriorityMedication) => {
    updateMutation.mutate({
      id: priority.id,
      request: { active: !priority.active },
    })
  }

  const handleDelete = (id: string, name: string) => {
    if (window.confirm(`Delete priority for "${name}"?`)) {
      deleteMutation.mutate(id)
    }
  }

  if (isLoading) {
    return (
      <DashboardLayout>
        <div className="flex items-center justify-center min-h-[400px]">
          <div className="flex flex-col items-center gap-4">
            <Loader2 className="w-8 h-8 text-teal animate-spin" />
            <p className="text-muted-foreground">
              Loading priority medications...
            </p>
          </div>
        </div>
      </DashboardLayout>
    )
  }

  if (error) {
    return (
      <DashboardLayout>
        <div className="flex items-center justify-center min-h-[400px]">
          <div className="flex flex-col items-center gap-4 text-center">
            <div className="p-4 rounded-full bg-red-500/10">
              <AlertCircle className="w-8 h-8 text-red-400" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-foreground mb-1">
                Failed to load priority medications
              </h2>
              <p className="text-muted-foreground text-sm mb-4">
                {error.message}
              </p>
              <button
                onClick={() => refetch()}
                className="flex items-center gap-2 px-4 py-2 rounded-lg bg-teal hover:bg-teal/80 text-white transition-colors mx-auto"
              >
                <RefreshCw className="w-4 h-4" />
                Retry
              </button>
            </div>
          </div>
        </div>
      </DashboardLayout>
    )
  }

  return (
    <DashboardLayout>
      <div className="space-y-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold text-foreground flex items-center gap-2">
              <Flame className="w-7 h-7 text-orange-400" />
              Priority Medications
            </h1>
            <p className="text-muted-foreground">
              Fast-track critical medications for immediate processing
            </p>
          </div>
          <button
            onClick={() => setShowCreateForm(true)}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-linear-to-r from-orange-500/20 to-red-500/20 border border-orange-500/30 hover:border-orange-500/50 text-orange-400 hover:text-orange-300 font-medium transition-all duration-300 hover:scale-105 active:scale-95"
          >
            <Plus className="w-4 h-4" />
            Add Priority
          </button>
        </div>

        {/* Stats */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="glass-card p-5 rounded-xl border-l-4 border-teal">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-muted-foreground mb-1">
                  Total Priorities
                </p>
                <p className="text-3xl font-bold text-foreground">
                  {priorities.length}
                </p>
              </div>
              <div className="p-3 rounded-lg bg-teal/20">
                <Flame className="w-6 h-6 text-teal" />
              </div>
            </div>
          </div>

          <div className="glass-card p-5 rounded-xl border-l-4 border-emerald">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-muted-foreground mb-1">
                  Currently Active
                </p>
                <p className="text-3xl font-bold text-emerald">{activeCount}</p>
              </div>
              <div className="p-3 rounded-lg bg-emerald/20">
                <Calendar className="w-6 h-6 text-emerald" />
              </div>
            </div>
          </div>

          <div className="glass-card p-5 rounded-xl border-l-4 border-red-500">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-muted-foreground mb-1">
                  Critical Level
                </p>
                <p className="text-3xl font-bold text-red-400">
                  {criticalCount}
                </p>
              </div>
              <div className="p-3 rounded-lg bg-red-500/20">
                <AlertCircle className="w-6 h-6 text-red-400" />
              </div>
            </div>
          </div>
        </div>

        {/* Filters */}
        <div className="glass-card p-4 rounded-xl space-y-4">
          <div className="flex flex-col md:flex-row gap-4">
            {/* Search */}
            <div className="flex-1 relative">
              <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
              <input
                type="text"
                placeholder="Search medications..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full pl-10 pr-4 py-2 bg-background border border-border rounded-lg text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
              />
            </div>

            {/* Priority Level Filter */}
            <select
              value={filterLevel}
              onChange={(e) =>
                setFilterLevel(e.target.value as PriorityLevel | 'all')
              }
              className="px-4 py-2 bg-background border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
            >
              <option value="all">All Levels</option>
              <option value={PriorityLevel.CRITICAL}>Critical</option>
              <option value={PriorityLevel.URGENT}>Urgent</option>
              <option value={PriorityLevel.HIGH}>High</option>
              <option value={PriorityLevel.NORMAL}>Normal</option>
              <option value={PriorityLevel.LOW}>Low</option>
            </select>

            {/* Active Filter */}
            <select
              value={filterActive}
              onChange={(e) =>
                setFilterActive(e.target.value as 'all' | 'active' | 'inactive')
              }
              className="px-4 py-2 bg-background border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
            >
              <option value="all">All Status</option>
              <option value="active">Active Only</option>
              <option value="inactive">Inactive Only</option>
            </select>
          </div>

          {(searchQuery || filterLevel !== 'all' || filterActive !== 'all') && (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <span>
                Showing {filteredPriorities.length} of {priorities.length}{' '}
                priorities
              </span>
              <button
                onClick={() => {
                  setSearchQuery('')
                  setFilterLevel('all')
                  setFilterActive('all')
                }}
                className="text-teal hover:text-teal/80 transition-colors"
              >
                Clear filters
              </button>
            </div>
          )}
        </div>

        {/* Priority List */}
        {filteredPriorities.length === 0 ? (
          <div className="glass-card p-12 rounded-xl text-center">
            <Flame className="w-12 h-12 mx-auto mb-4 text-muted-foreground opacity-50" />
            <p className="text-muted-foreground">
              {searchQuery || filterLevel !== 'all' || filterActive !== 'all'
                ? 'No priorities match your filters'
                : 'No priority medications configured'}
            </p>
            <p className="text-sm text-muted-foreground mt-1">
              {searchQuery || filterLevel !== 'all' || filterActive !== 'all'
                ? 'Try adjusting your search or filters'
                : 'Add medications to fast-track critical items'}
            </p>
          </div>
        ) : (
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {filteredPriorities.map((priority, index) => (
              <PriorityCard
                key={priority.id}
                priority={priority}
                index={index}
                onToggleActive={toggleActive}
                onDelete={handleDelete}
                onEdit={setEditingId}
                isUpdating={updateMutation.isPending}
                isDeleting={deleteMutation.isPending}
              />
            ))}
          </div>
        )}

        {/* Create/Edit Form Modal */}
        {(showCreateForm || editingId) && (
          <PriorityFormModal
            priority={
              editingId ? priorities.find((p) => p.id === editingId) : undefined
            }
            onClose={() => {
              setShowCreateForm(false)
              setEditingId(null)
            }}
            onSubmit={(data) => {
              if (editingId) {
                updateMutation.mutate({ id: editingId, request: data })
              } else {
                createMutation.mutate(data)
              }
              setShowCreateForm(false)
              setEditingId(null)
            }}
            isSubmitting={createMutation.isPending || updateMutation.isPending}
          />
        )}
      </div>
    </DashboardLayout>
  )
}

// Priority Card Component
function PriorityCard({
  priority,
  index,
  onToggleActive,
  onDelete,
  onEdit,
  isUpdating,
  isDeleting,
}: {
  priority: PriorityMedication
  index: number
  onToggleActive: (priority: PriorityMedication) => void
  onDelete: (id: string, name: string) => void
  onEdit: (id: string) => void
  isUpdating: boolean
  isDeleting: boolean
}) {
  const config = PRIORITY_CONFIG[priority.priorityLevel]
  const isCurrentlyActive =
    priority.active &&
    new Date(priority.activeFrom) <= new Date() &&
    (!priority.activeUntil || new Date(priority.activeUntil) > new Date())

  const formatDate = (dateStr: string) => {
    return new Date(dateStr).toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    })
  }

  return (
    <div
      className={cn(
        'glass-card p-5 rounded-xl transition-all duration-300 hover:border-teal/50 animate-fade-in',
        isCurrentlyActive && 'border-l-4',
        isCurrentlyActive && config.border.replace('border-', 'border-l-'),
      )}
      style={{ animationDelay: `${index * 50}ms` }}
    >
      <div className="flex items-start justify-between mb-4">
        <div className="flex-1">
          <div className="flex items-center gap-2 mb-2">
            <h3 className="text-lg font-semibold text-foreground">
              {priority.medicationName}
            </h3>
            <span
              className={cn(
                'px-2 py-0.5 rounded text-xs font-bold',
                config.bg,
                config.color,
                config.border,
                'border',
              )}
            >
              {config.label}
            </span>
          </div>
          {priority.medicationNameAr && (
            <p className="text-sm text-muted-foreground mb-2">
              {priority.medicationNameAr}
            </p>
          )}
          {priority.reason && (
            <p className="text-sm text-muted-foreground italic">
              {priority.reason}
            </p>
          )}
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => onEdit(priority.id)}
            disabled={isUpdating}
            className="p-2 rounded-lg hover:bg-secondary transition-colors text-muted-foreground hover:text-foreground"
            title="Edit"
          >
            <svg
              className="w-4 h-4"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"
              />
            </svg>
          </button>
          <button
            onClick={() => onDelete(priority.id, priority.medicationName)}
            disabled={isDeleting}
            className="p-2 rounded-lg hover:bg-red-500/20 transition-colors text-muted-foreground hover:text-red-400"
            title="Delete"
          >
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      </div>

      <div className="space-y-3 pt-4 border-t border-border">
        <div className="flex items-center justify-between text-sm">
          <span className="text-muted-foreground">Priority Score:</span>
          <span className={cn('font-bold', config.color)}>{config.score}</span>
        </div>

        <div className="flex items-center justify-between text-sm">
          <span className="text-muted-foreground">Active From:</span>
          <span className="text-foreground">
            {formatDate(priority.activeFrom)}
          </span>
        </div>

        {priority.activeUntil && (
          <div className="flex items-center justify-between text-sm">
            <span className="text-muted-foreground">Expires:</span>
            <span className="text-foreground">
              {formatDate(priority.activeUntil)}
            </span>
          </div>
        )}

        <div className="flex items-center justify-between pt-2">
          <span className="text-sm text-muted-foreground">Status</span>
          <div className="flex items-center gap-2">
            <span
              className={cn(
                'text-xs font-medium',
                isCurrentlyActive ? 'text-emerald' : 'text-muted-foreground',
              )}
            >
              {isCurrentlyActive ? 'Active' : 'Inactive'}
            </span>
            <Switch
              checked={priority.active}
              onCheckedChange={() => onToggleActive(priority)}
              disabled={isUpdating}
              className="data-[state=checked]:bg-emerald"
            />
          </div>
        </div>
      </div>
    </div>
  )
}

// Priority Form Modal Component
function PriorityFormModal({
  priority,
  onClose,
  onSubmit,
  isSubmitting,
}: {
  priority?: PriorityMedication
  onClose: () => void
  onSubmit: (data: CreatePriorityRequest) => void
  isSubmitting: boolean
}) {
  const [formData, setFormData] = useState<CreatePriorityRequest>({
    medicationName: priority?.medicationName ?? '',
    medicationNameAr: priority?.medicationNameAr ?? undefined,
    priorityLevel: priority?.priorityLevel ?? PriorityLevel.NORMAL,
    reason: priority?.reason ?? undefined,
    active: priority?.active ?? true,
    activeFrom: priority?.activeFrom
      ? new Date(priority.activeFrom).toISOString().split('T')[0]
      : new Date().toISOString().split('T')[0],
    activeUntil: priority?.activeUntil
      ? new Date(priority.activeUntil).toISOString().split('T')[0]
      : undefined,
  })

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmit(formData)
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm animate-fade-in">
      <div className="glass-card-enhanced p-6 rounded-2xl max-w-lg w-full max-h-[90vh] overflow-y-auto animate-scale-in">
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-xl font-bold text-foreground">
            {priority ? 'Edit Priority Medication' : 'Add Priority Medication'}
          </h2>
          <button
            onClick={onClose}
            className="p-2 rounded-lg hover:bg-secondary transition-colors text-muted-foreground hover:text-foreground"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-sm font-medium text-foreground mb-2">
              Medication Name *
            </label>
            <input
              type="text"
              required
              value={formData.medicationName}
              onChange={(e) =>
                setFormData({ ...formData, medicationName: e.target.value })
              }
              className="w-full px-4 py-2 bg-background border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
              placeholder="e.g., Insulin"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-foreground mb-2">
              Arabic Name (Optional)
            </label>
            <input
              type="text"
              value={formData.medicationNameAr ?? ''}
              onChange={(e) =>
                setFormData({
                  ...formData,
                  medicationNameAr: e.target.value || undefined,
                })
              }
              className="w-full px-4 py-2 bg-background border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
              placeholder="e.g., انسولين"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-foreground mb-2">
              Priority Level *
            </label>
            <select
              required
              value={formData.priorityLevel}
              onChange={(e) =>
                setFormData({
                  ...formData,
                  priorityLevel: e.target.value as PriorityLevel,
                })
              }
              className="w-full px-4 py-2 bg-background border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
            >
              {Object.entries(PRIORITY_CONFIG).map(([level, config]) => (
                <option key={level} value={level}>
                  {config.label} (Score: {config.score})
                </option>
              ))}
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-foreground mb-2">
              Reason (Optional)
            </label>
            <textarea
              value={formData.reason ?? ''}
              onChange={(e) =>
                setFormData({
                  ...formData,
                  reason: e.target.value || undefined,
                })
              }
              rows={3}
              className="w-full px-4 py-2 bg-background border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-teal/50 resize-none"
              placeholder="e.g., Life-saving medication - critical priority"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm font-medium text-foreground mb-2">
                Active From *
              </label>
              <input
                type="date"
                required
                value={formData.activeFrom}
                onChange={(e) =>
                  setFormData({ ...formData, activeFrom: e.target.value })
                }
                className="w-full px-4 py-2 bg-background border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-foreground mb-2">
                Active Until (Optional)
              </label>
              <input
                type="date"
                value={formData.activeUntil ?? ''}
                onChange={(e) =>
                  setFormData({
                    ...formData,
                    activeUntil: e.target.value || undefined,
                  })
                }
                className="w-full px-4 py-2 bg-background border border-border rounded-lg text-foreground focus:outline-none focus:ring-2 focus:ring-teal/50"
              />
            </div>
          </div>

          <div className="flex items-center justify-between pt-2">
            <label className="text-sm font-medium text-foreground">
              Active
            </label>
            <Switch
              checked={formData.active ?? true}
              onCheckedChange={(checked) =>
                setFormData({ ...formData, active: checked })
              }
              className="data-[state=checked]:bg-emerald"
            />
          </div>

          <div className="flex gap-3 pt-4">
            <button
              type="button"
              onClick={onClose}
              disabled={isSubmitting}
              className="flex-1 px-4 py-2 rounded-lg bg-secondary hover:bg-secondary/80 text-foreground font-medium transition-colors disabled:opacity-50"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSubmitting}
              className="flex-1 px-4 py-2 rounded-lg bg-linear-to-r from-orange-500/20 to-red-500/20 border border-orange-500/30 hover:border-orange-500/50 text-orange-400 font-medium transition-all duration-300 disabled:opacity-50 flex items-center justify-center gap-2"
            >
              {isSubmitting ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" />
                  Saving...
                </>
              ) : (
                <>{priority ? 'Update' : 'Create'}</>
              )}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
