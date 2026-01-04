// Debug Recordings Route
// Professional debugging dashboard for match recordings and pipeline analysis
// Migrated to use Redux for state management

import { createFileRoute } from '@tanstack/react-router'
import { useCallback, useMemo, useEffect } from 'react'
import { toast } from 'sonner'
import {
  Video,
  VideoOff,
  GitBranch,
  Layers,
  Clock,
  Upload,
  Trash2,
  Search,
  Filter,
  RefreshCw,
  CheckCircle,
  XCircle,
  Database,
  Activity,
  Eye,
  BarChart3,
  Target,
  Gauge,
  Zap,
  FileJson,
  Settings2,
} from 'lucide-react'

import { DashboardLayout } from '@/components/layout/dashboard-layout'
import {
  RecordingCard,
  RecordingPlayback,
  PipelineViewer,
  convertToPipelineRecording,
  StatsCard,
  CircularProgress,
  ProgressBar,
  AuditRecordsViewer,
  PerformanceAnalyticsView,
  type MatchRecording,
  type PipelineRecording,
} from '@/components/debug-recordings'
import { usePipelineVisualization } from '@/hooks/use-audit-records'
import { cn } from '@/lib/utils'
import { Input } from '@/components/ui/input'
import { useAppSelector, useRecordingsActions } from '@/store'
import {
  selectRecordingsArray,
  selectIsRecording,
  selectSelectedRecordingId,
} from '@/store/slices'

import { useState } from 'react'

export const Route = createFileRoute('/debug-recordings')({
  component: DebugRecordings,
})

type ViewMode = 'overview' | 'recordings' | 'pipeline' | 'analytics' | 'audit'
type SortBy = 'date' | 'duration' | 'snapshots' | 'confidence'
type FilterOutcome = 'all' | 'approved' | 'rejected' | 'pending'

function formatDuration(ms: number): string {
  const seconds = Math.floor(ms / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  if (hours > 0) return `${hours}h ${minutes % 60}m`
  if (minutes > 0) return `${minutes}m ${seconds % 60}s`
  return `${seconds}s`
}

// ============================================================================
// View Mode Tabs
// ============================================================================

function ViewModeTabs({
  activeMode,
  onModeChange,
  recordingCount,
}: {
  activeMode: ViewMode
  onModeChange: (mode: ViewMode) => void
  recordingCount: number
}) {
  const tabs: { id: ViewMode; label: string; icon: React.ElementType }[] = [
    { id: 'overview', label: 'Overview', icon: BarChart3 },
    { id: 'recordings', label: 'Recordings', icon: Video },
    { id: 'pipeline', label: 'Pipeline', icon: GitBranch },
    { id: 'analytics', label: 'Analytics', icon: Activity },
    { id: 'audit', label: 'Audit Records', icon: Database },
  ]

  return (
    <div className="flex items-center gap-1.5 p-1.5 rounded-2xl bg-secondary/30 border border-border/50 backdrop-blur-sm">
      {tabs.map((tab) => (
        <button
          key={tab.id}
          onClick={() => onModeChange(tab.id)}
          className={cn(
            'flex items-center gap-2 px-4 py-2.5 rounded-xl text-sm font-medium transition-all duration-300',
            activeMode === tab.id
              ? 'bg-gradient-to-r from-teal-500 to-emerald-500 text-white shadow-lg shadow-teal-500/25'
              : 'text-muted-foreground hover:text-foreground hover:bg-secondary/50',
          )}
        >
          <tab.icon className="w-4 h-4" />
          {tab.label}
          {tab.id === 'recordings' && recordingCount > 0 && (
            <span
              className={cn(
                'px-1.5 py-0.5 rounded-full text-[10px] font-bold min-w-[20px] text-center',
                activeMode === tab.id
                  ? 'bg-white/20'
                  : 'bg-teal-500/20 text-teal-400',
              )}
            >
              {recordingCount}
            </span>
          )}
        </button>
      ))}
    </div>
  )
}

// ============================================================================
// Main Component
// ============================================================================

function DebugRecordings() {
  const actions = useRecordingsActions()

  // Redux state
  const recordings = useAppSelector(selectRecordingsArray)
  const isRecording = useAppSelector(selectIsRecording)
  const viewMode = useAppSelector((state) => state.recordings.viewMode)
  const selectedRecordingId = useAppSelector(selectSelectedRecordingId)
  const recordingsMap = useAppSelector((state) => state.recordings.recordings)

  // Local UI state (not persisted)
  const [playbackRecording, setPlaybackRecording] =
    useState<MatchRecording | null>(null)
  const [pipelineRecording, setPipelineRecording] =
    useState<PipelineRecording | null>(null)
  const [pipelineMatchId, setPipelineMatchId] = useState<string | undefined>(undefined)
  const [searchQuery, setSearchQuery] = useState('')
  const [sortBy, setSortBy] = useState<SortBy>('date')
  const [filterOutcome, setFilterOutcome] = useState<FilterOutcome>('all')

  // Fetch pipeline visualization from backend
  const { data: pipelineData, refetch: refetchPipeline, isLoading: isPipelineLoading } = usePipelineVisualization(pipelineMatchId)

  // Filter and sort recordings
  const filteredRecordings = useMemo(() => {
    let result = [...recordings]

    if (searchQuery) {
      const query = searchQuery.toLowerCase()
      result = result.filter(
        (r) =>
          r.matchId.toLowerCase().includes(query) ||
          r.snapshots.some(
            (s) =>
              s.offer.product.toLowerCase().includes(query) ||
              s.request.product.toLowerCase().includes(query),
          ),
      )
    }

    if (filterOutcome !== 'all') {
      result = result.filter((r) => r.outcome === filterOutcome)
    }

    result.sort((a, b) => {
      switch (sortBy) {
        case 'date':
          return (
            new Date(b.startedAt).getTime() - new Date(a.startedAt).getTime()
          )
        case 'duration':
          return (b.duration || 0) - (a.duration || 0)
        case 'snapshots':
          return b.snapshots.length - a.snapshots.length
        case 'confidence':
          const avgA =
            a.snapshots.reduce((acc, s) => acc + s.confidence, 0) /
            (a.snapshots.length || 1)
          const avgB =
            b.snapshots.reduce((acc, s) => acc + s.confidence, 0) /
            (b.snapshots.length || 1)
          return avgB - avgA
        default:
          return 0
      }
    })

    return result
  }, [recordings, searchQuery, sortBy, filterOutcome])

  // Convert pipeline data when it arrives from backend
  useEffect(() => {
    if (pipelineData) {
      const converted = convertToPipelineRecording(pipelineData)
      setPipelineRecording(converted)
    }
  }, [pipelineData])

  // Stats calculations
  const stats = useMemo(() => {
    const total = recordings.length
    const approved = recordings.filter((r) => r.outcome === 'approved').length
    const rejected = recordings.filter((r) => r.outcome === 'rejected').length
    const pending = recordings.filter(
      (r) => !r.outcome || r.outcome === 'pending',
    ).length
    const avgDuration =
      recordings.length > 0
        ? recordings.reduce((acc, r) => acc + (r.duration || 0), 0) /
          recordings.length
        : 0
    const avgSnapshots =
      recordings.length > 0
        ? recordings.reduce((acc, r) => acc + r.snapshots.length, 0) /
          recordings.length
        : 0
    const avgConfidence =
      recordings.length > 0
        ? recordings.reduce((acc, r) => {
            const recAvg =
              r.snapshots.reduce((a, s) => a + s.confidence, 0) /
              (r.snapshots.length || 1)
            return acc + recAvg
          }, 0) / recordings.length
        : 0
    const totalEvents = recordings.reduce(
      (acc, r) => acc + r.snapshots.length,
      0,
    )

    return {
      total,
      approved,
      rejected,
      pending,
      avgDuration,
      avgSnapshots,
      avgConfidence,
      totalEvents,
    }
  }, [recordings])

  // Sparkline data
  const sparklineData = useMemo(() => {
    return recordings
      .slice(0, 20)
      .map((r) => r.snapshots.length)
      .reverse()
  }, [recordings])

  // Export a single recording
  const exportRecording = useCallback(
    (matchId: string): string | null => {
      const recording = recordingsMap[matchId]
      if (!recording) return null
      return JSON.stringify(recording, null, 2)
    },
    [recordingsMap],
  )

  const handleExport = useCallback(
    (recording: MatchRecording) => {
      const json = exportRecording(recording.matchId)
      if (json) {
        const blob = new Blob([json], { type: 'application/json' })
        const url = URL.createObjectURL(blob)
        const a = document.createElement('a')
        a.href = url
        a.download = `recording-${recording.matchId.slice(0, 8)}.json`
        a.click()
        URL.revokeObjectURL(url)
        toast.success('Recording exported', {
          description: `Match #${recording.matchId.slice(0, 8)}`,
        })
      }
    },
    [exportRecording],
  )

  const handleExportAll = useCallback(() => {
    const json = JSON.stringify(recordingsMap, null, 2)
    const blob = new Blob([json], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `all-recordings-${new Date().toISOString().slice(0, 10)}.json`
    a.click()
    URL.revokeObjectURL(url)
    toast.success('All recordings exported', {
      description: `${recordings.length} recordings`,
    })
  }, [recordingsMap, recordings.length])

  const handleDelete = useCallback(
    (recording: MatchRecording) => {
      actions.deleteRecording(recording.matchId)
      toast.success('Recording deleted', {
        description: `Match #${recording.matchId.slice(0, 8)}`,
      })
    },
    [actions],
  )

  const handleClearAll = useCallback(() => {
    actions.clearAllRecordings()
    toast.success('All recordings cleared')
  }, [actions])

  const handleViewPipeline = useCallback(
    (recording: MatchRecording) => {
      // Set the match ID to trigger fetching real pipeline data from backend
      setPipelineMatchId(recording.matchId)
      actions.setViewMode('pipeline')
    },
    [actions],
  )

  const handleImport = useCallback(() => {
    const input = document.createElement('input')
    input.type = 'file'
    input.accept = '.json'
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0]
      if (file) {
        try {
          const text = await file.text()
          const data = JSON.parse(text)

          // Check if it's a single recording or multiple
          if (data.matchId && data.snapshots) {
            // Single recording - wrap it
            actions.importRecordings({ [data.matchId]: data })
            toast.success('Recording imported')
          } else {
            // Multiple recordings
            const count = Object.keys(data).length
            actions.importRecordings(data)
            toast.success('Recordings imported', {
              description: `${count} recordings loaded`,
            })
          }
        } catch {
          toast.error('Import failed', { description: 'Invalid JSON file' })
        }
      }
    }
    input.click()
  }, [actions])

  const handleViewModeChange = useCallback(
    (mode: ViewMode) => {
      actions.setViewMode(mode)
    },
    [actions],
  )

  const handleSelectRecording = useCallback(
    (matchId: string) => {
      actions.selectRecording(matchId)
    },
    [actions],
  )

  return (
    <DashboardLayout>
      <div className="min-h-screen bg-gradient-to-br from-background via-background to-secondary/20">
        {/* Header */}
        <div className="sticky top-0 z-40 backdrop-blur-xl bg-background/80 border-b border-border/50">
          <div className="max-w-[1800px] mx-auto px-6 py-4">
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center gap-4">
                <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-violet-500/30 to-purple-500/30 flex items-center justify-center shadow-lg shadow-violet-500/20">
                  <Layers className="w-7 h-7 text-violet-400" />
                </div>
                <div>
                  <h1 className="text-2xl font-bold text-foreground flex items-center gap-3">
                    Debug Recordings
                    {isRecording && (
                      <span className="flex items-center gap-2 px-3 py-1 rounded-full bg-red-500/20 text-red-400 text-sm font-medium animate-pulse">
                        <span className="w-2 h-2 rounded-full bg-red-500" />
                        Recording
                      </span>
                    )}
                  </h1>
                  <p className="text-sm text-muted-foreground">
                    Analyze match recordings and pipeline execution traces
                  </p>
                </div>
              </div>

              <div className="flex items-center gap-2">
                <button
                  onClick={handleImport}
                  className="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-secondary/50 hover:bg-secondary text-muted-foreground hover:text-foreground transition-all hover:scale-105"
                >
                  <Upload className="w-4 h-4" />
                  Import
                </button>
                {recordings.length > 0 && (
                  <button
                    onClick={handleExportAll}
                    className="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-teal-500/20 hover:bg-teal-500/30 text-teal-400 transition-all hover:scale-105"
                  >
                    <FileJson className="w-4 h-4" />
                    Export All
                  </button>
                )}
                <button
                  onClick={handleClearAll}
                  disabled={recordings.length === 0}
                  className="flex items-center gap-2 px-4 py-2.5 rounded-xl bg-red-500/10 hover:bg-red-500/20 text-red-400 transition-all hover:scale-105 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:scale-100"
                >
                  <Trash2 className="w-4 h-4" />
                  Clear All
                </button>
              </div>
            </div>

            <ViewModeTabs
              activeMode={viewMode}
              onModeChange={handleViewModeChange}
              recordingCount={recordings.length}
            />
          </div>
        </div>

        {/* Content */}
        <div className="max-w-[1800px] mx-auto px-6 py-6">
          {/* Overview Mode */}
          {viewMode === 'overview' && (
            <div className="space-y-6">
              {/* Stats Grid */}
              <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
                <StatsCard
                  icon={Database}
                  label="Total Recordings"
                  value={stats.total}
                  trend={12}
                  trendLabel="vs last week"
                  color="violet"
                  sparklineData={sparklineData}
                />
                <StatsCard
                  icon={CheckCircle}
                  label="Approved"
                  value={stats.approved}
                  trend={8}
                  color="emerald"
                />
                <StatsCard
                  icon={XCircle}
                  label="Rejected"
                  value={stats.rejected}
                  trend={-5}
                  color="red"
                />
                <StatsCard
                  icon={Clock}
                  label="Avg Duration"
                  value={formatDuration(stats.avgDuration)}
                  color="amber"
                />
              </div>

              {/* Secondary Stats */}
              <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div className="p-6 rounded-2xl bg-gradient-to-br from-secondary/50 to-secondary/20 border border-border/50 backdrop-blur-sm">
                  <div className="flex items-center justify-between mb-4">
                    <h3 className="text-sm font-semibold text-foreground">
                      Confidence Distribution
                    </h3>
                    <Gauge className="w-5 h-5 text-muted-foreground" />
                  </div>
                  <div className="flex items-center justify-center">
                    <CircularProgress
                      value={stats.avgConfidence}
                      max={100}
                      size={140}
                      strokeWidth={10}
                      color="teal"
                      label="Avg Confidence"
                      sublabel={`${stats.total} recordings`}
                    />
                  </div>
                </div>

                <div className="p-6 rounded-2xl bg-gradient-to-br from-secondary/50 to-secondary/20 border border-border/50 backdrop-blur-sm">
                  <div className="flex items-center justify-between mb-4">
                    <h3 className="text-sm font-semibold text-foreground">
                      Outcome Breakdown
                    </h3>
                    <Target className="w-5 h-5 text-muted-foreground" />
                  </div>
                  <div className="space-y-4">
                    <ProgressBar
                      value={stats.approved}
                      max={stats.total || 1}
                      label="Approved"
                      valueLabel={stats.approved}
                      color="emerald"
                    />
                    <ProgressBar
                      value={stats.rejected}
                      max={stats.total || 1}
                      label="Rejected"
                      valueLabel={stats.rejected}
                      color="red"
                    />
                    <ProgressBar
                      value={stats.pending}
                      max={stats.total || 1}
                      label="Pending"
                      valueLabel={stats.pending}
                      color="amber"
                    />
                  </div>
                </div>

                <div className="p-6 rounded-2xl bg-gradient-to-br from-secondary/50 to-secondary/20 border border-border/50 backdrop-blur-sm">
                  <div className="flex items-center justify-between mb-4">
                    <h3 className="text-sm font-semibold text-foreground">
                      Quick Stats
                    </h3>
                    <Zap className="w-5 h-5 text-muted-foreground" />
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    <div className="p-3 rounded-xl bg-background/50 text-center border border-border/20">
                      <p className="text-2xl font-bold text-foreground">
                        {stats.avgSnapshots.toFixed(1)}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        Avg Snapshots
                      </p>
                    </div>
                    <div className="p-3 rounded-xl bg-background/50 text-center border border-border/20">
                      <p className="text-2xl font-bold text-foreground">
                        {
                          recordings.filter((r) => r.snapshots.length > 5)
                            .length
                        }
                      </p>
                      <p className="text-xs text-muted-foreground">Complex</p>
                    </div>
                    <div className="p-3 rounded-xl bg-background/50 text-center border border-border/20">
                      <p className="text-2xl font-bold text-teal-400">
                        {stats.avgConfidence.toFixed(0)}%
                      </p>
                      <p className="text-xs text-muted-foreground">
                        Confidence
                      </p>
                    </div>
                    <div className="p-3 rounded-xl bg-background/50 text-center border border-border/20">
                      <p className="text-2xl font-bold text-violet-400">
                        {stats.totalEvents}
                      </p>
                      <p className="text-xs text-muted-foreground">Events</p>
                    </div>
                  </div>
                </div>
              </div>

              {/* Recent Recordings */}
              <div className="p-6 rounded-2xl bg-gradient-to-br from-secondary/50 to-secondary/20 border border-border/50 backdrop-blur-sm">
                <div className="flex items-center justify-between mb-4">
                  <h3 className="text-lg font-semibold text-foreground">
                    Recent Recordings
                  </h3>
                  <button
                    onClick={() => handleViewModeChange('recordings')}
                    className="text-sm text-teal-400 hover:text-teal-300 transition-colors font-medium"
                  >
                    View All →
                  </button>
                </div>
                {recordings.length === 0 ? (
                  <div className="text-center py-16">
                    <VideoOff className="w-16 h-16 text-muted-foreground/20 mx-auto mb-4" />
                    <h3 className="text-lg font-semibold text-foreground mb-2">
                      No Recordings Yet
                    </h3>
                    <p className="text-sm text-muted-foreground max-w-md mx-auto">
                      Recordings will appear here when you review matches in the
                      Review Queue.
                    </p>
                  </div>
                ) : (
                  <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-4">
                    {recordings.slice(0, 6).map((recording) => (
                      <RecordingCard
                        key={recording.matchId}
                        recording={recording}
                        isSelected={selectedRecordingId === recording.matchId}
                        onSelect={() =>
                          handleSelectRecording(recording.matchId)
                        }
                        onPlay={() => setPlaybackRecording(recording)}
                        onExport={() => handleExport(recording)}
                        onDelete={() => handleDelete(recording)}
                        onViewPipeline={() => handleViewPipeline(recording)}
                      />
                    ))}
                  </div>
                )}
              </div>
            </div>
          )}

          {/* Recordings Mode */}
          {viewMode === 'recordings' && (
            <div className="space-y-6">
              {/* Search and Filters */}
              <div className="flex flex-wrap items-center gap-4 p-4 rounded-2xl bg-secondary/30 border border-border/50 backdrop-blur-sm">
                <div className="relative flex-1 min-w-[200px]">
                  <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
                  <Input
                    placeholder="Search by match ID or product..."
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    className="pl-10 bg-background/50 border-border/50"
                  />
                </div>

                <div className="flex items-center gap-2">
                  <Filter className="w-4 h-4 text-muted-foreground" />
                  <select
                    value={filterOutcome}
                    onChange={(e) =>
                      setFilterOutcome(e.target.value as FilterOutcome)
                    }
                    className="px-3 py-2 rounded-lg bg-background/50 border border-border/50 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-teal-500/50"
                  >
                    <option value="all">All Outcomes</option>
                    <option value="approved">Approved</option>
                    <option value="rejected">Rejected</option>
                    <option value="pending">Pending</option>
                  </select>
                </div>

                <div className="flex items-center gap-2">
                  <Settings2 className="w-4 h-4 text-muted-foreground" />
                  <select
                    value={sortBy}
                    onChange={(e) => setSortBy(e.target.value as SortBy)}
                    className="px-3 py-2 rounded-lg bg-background/50 border border-border/50 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-teal-500/50"
                  >
                    <option value="date">Sort by Date</option>
                    <option value="duration">Sort by Duration</option>
                    <option value="snapshots">Sort by Snapshots</option>
                    <option value="confidence">Sort by Confidence</option>
                  </select>
                </div>

                <button
                  onClick={() => {
                    setSearchQuery('')
                    setFilterOutcome('all')
                    setSortBy('date')
                  }}
                  className="p-2.5 rounded-lg bg-secondary/50 hover:bg-secondary text-muted-foreground hover:text-foreground transition-all hover:scale-105"
                  title="Reset filters"
                >
                  <RefreshCw className="w-4 h-4" />
                </button>
              </div>

              {/* Results count */}
              <div className="flex items-center justify-between px-1">
                <p className="text-sm text-muted-foreground">
                  Showing{' '}
                  <span className="text-foreground font-semibold">
                    {filteredRecordings.length}
                  </span>{' '}
                  of{' '}
                  <span className="text-foreground font-semibold">
                    {recordings.length}
                  </span>{' '}
                  recordings
                </p>
              </div>

              {/* Recordings Grid */}
              {filteredRecordings.length === 0 ? (
                <div className="text-center py-20">
                  <Eye className="w-16 h-16 text-muted-foreground/20 mx-auto mb-4" />
                  <h3 className="text-lg font-semibold text-foreground mb-2">
                    No Recordings Found
                  </h3>
                  <p className="text-sm text-muted-foreground">
                    {searchQuery || filterOutcome !== 'all'
                      ? 'Try adjusting your search or filters'
                      : 'Start reviewing matches to create recordings'}
                  </p>
                </div>
              ) : (
                <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-4">
                  {filteredRecordings.map((recording) => (
                    <RecordingCard
                      key={recording.matchId}
                      recording={recording}
                      isSelected={selectedRecordingId === recording.matchId}
                      onSelect={() => handleSelectRecording(recording.matchId)}
                      onPlay={() => setPlaybackRecording(recording)}
                      onExport={() => handleExport(recording)}
                      onDelete={() => handleDelete(recording)}
                      onViewPipeline={() => handleViewPipeline(recording)}
                    />
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Pipeline Mode */}
          {viewMode === 'pipeline' && (
            <div className="space-y-6">
              {isPipelineLoading ? (
                <div className="text-center py-20">
                  <RefreshCw className="w-16 h-16 text-muted-foreground/20 mx-auto mb-4 animate-spin" />
                  <h3 className="text-lg font-semibold text-foreground mb-2">
                    Loading Pipeline Data
                  </h3>
                  <p className="text-sm text-muted-foreground">
                    Fetching pipeline visualization from backend...
                  </p>
                </div>
              ) : pipelineRecording ? (
                <PipelineViewer
                  recording={pipelineRecording}
                  onClose={() => {
                    setPipelineRecording(null)
                    setPipelineMatchId(undefined)
                  }}
                  onRefresh={() => {
                    // Refetch pipeline data from backend
                    refetchPipeline()
                  }}
                  onExport={() => {
                    const blob = new Blob(
                      [JSON.stringify(pipelineRecording, null, 2)],
                      { type: 'application/json' },
                    )
                    const url = URL.createObjectURL(blob)
                    const a = document.createElement('a')
                    a.href = url
                    a.download = `pipeline-${pipelineRecording.matchId.slice(0, 8)}.json`
                    a.click()
                    URL.revokeObjectURL(url)
                    toast.success('Pipeline exported')
                  }}
                />
              ) : (
                <div className="text-center py-20">
                  <GitBranch className="w-16 h-16 text-muted-foreground/20 mx-auto mb-4" />
                  <h3 className="text-lg font-semibold text-foreground mb-2">
                    No Pipeline Selected
                  </h3>
                  <p className="text-sm text-muted-foreground mb-6 max-w-md mx-auto">
                    Select a recording from the Recordings tab and click the
                    pipeline icon to view its execution trace.
                  </p>
                  <button
                    onClick={() => handleViewModeChange('recordings')}
                    className="px-5 py-2.5 rounded-xl bg-gradient-to-r from-teal-500 to-emerald-500 text-white font-medium shadow-lg shadow-teal-500/25 hover:shadow-xl hover:scale-105 transition-all"
                  >
                    Browse Recordings
                  </button>
                </div>
              )}
            </div>
          )}

          {/* Analytics Mode */}
          {viewMode === 'analytics' && <PerformanceAnalyticsView />}

          {/* Audit Records Mode */}
          {viewMode === 'audit' && <AuditRecordsViewer />}
        </div>

        {/* Playback Modal */}
        {playbackRecording && (
          <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-sm">
            <div className="w-full max-w-4xl animate-in fade-in zoom-in-95 duration-200">
              <RecordingPlayback
                recording={playbackRecording}
                onClose={() => setPlaybackRecording(null)}
                onExport={() => handleExport(playbackRecording)}
                onDelete={() => {
                  handleDelete(playbackRecording)
                  setPlaybackRecording(null)
                }}
              />
            </div>
          </div>
        )}
      </div>
    </DashboardLayout>
  )
}
