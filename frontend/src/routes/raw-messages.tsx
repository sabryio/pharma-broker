import { createFileRoute } from '@tanstack/react-router'
import { DashboardLayout } from '@/components/layout/dashboard-layout'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from '@/components/ui/sheet'
import {
  Search,
  ChevronLeft,
  ChevronRight,
  Loader2,
  AlertCircle,
  ArrowUpDown,
  ArrowUp,
  ArrowDown,
  MessageSquare,
  RefreshCw,
  Calendar,
  X,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { useRawMessages } from '@/hooks/use-raw-messages'
import { useState, useMemo, useCallback, useEffect } from 'react'
import {
  useReactTable,
  getCoreRowModel,
  flexRender,
  createColumnHelper,
} from '@tanstack/react-table'
import type {
  RawMessage,
  ProcessingStatus,
  SortField,
  SortOrder,
} from '@/schema/raw-message'

export const Route = createFileRoute('/raw-messages')({
  component: RawMessages,
})

// Helper function to truncate content (Property 1: Content Truncation)
export function truncateContent(
  content: string,
  maxLength: number = 100,
): string {
  if (content.length <= maxLength) {
    return content
  }
  return content.slice(0, maxLength) + '...'
}

// Helper function to calculate pagination metadata (Property 2 & 3)
export function calculatePagination(
  total: number,
  pageSize: number,
  offset: number,
) {
  const totalPages = Math.ceil(total / pageSize) || 1
  const currentPage = Math.floor(offset / pageSize) + 1
  return { totalPages, currentPage }
}

export function calculateCanGoNext(
  total: number,
  limit: number,
  offset: number,
): boolean {
  return offset + limit < total
}

// Format timestamp to relative time
function formatRelativeTime(timestamp: string): string {
  const date = new Date(timestamp)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMins = Math.floor(diffMs / 60000)
  const diffHours = Math.floor(diffMs / 3600000)
  const diffDays = Math.floor(diffMs / 86400000)

  if (diffMins < 1) return 'Just now'
  if (diffMins < 60) return `${diffMins}m ago`
  if (diffHours < 24) return `${diffHours}h ago`
  if (diffDays < 7) return `${diffDays}d ago`
  return date.toLocaleDateString()
}

// Get status badge variant
function getStatusBadge(message: RawMessage) {
  if (message.error) {
    return (
      <Badge
        variant="outline"
        className="border-destructive/50 text-destructive bg-destructive/10"
      >
        Error
      </Badge>
    )
  }
  if (message.processed_at) {
    return (
      <Badge
        variant="outline"
        className="border-emerald/50 text-emerald bg-emerald/10"
      >
        Processed
      </Badge>
    )
  }
  return (
    <Badge variant="outline" className="border-amber/50 text-amber bg-amber/10">
      Pending
    </Badge>
  )
}

function RawMessages() {
  // Pagination state
  const [pagination, setPagination] = useState({ pageIndex: 0, pageSize: 20 })

  // Filter state
  const [searchInput, setSearchInput] = useState('')
  const [debouncedSearch, setDebouncedSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState<ProcessingStatus>('all')
  const [startDate, setStartDate] = useState<string>('')
  const [endDate, setEndDate] = useState<string>('')

  // Sort state
  const [sortBy, setSortBy] = useState<SortField>('timestamp')
  const [sortOrder, setSortOrder] = useState<SortOrder>('desc')

  // Detail panel state
  const [selectedMessage, setSelectedMessage] = useState<RawMessage | null>(
    null,
  )

  // Debounce search input (300ms)
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedSearch(searchInput)
      // Reset pagination when search changes
      setPagination((prev) => ({ ...prev, pageIndex: 0 }))
    }, 300)
    return () => clearTimeout(timer)
  }, [searchInput])

  // Reset pagination when filters change
  const handleStatusChange = useCallback((value: string) => {
    setStatusFilter(value as ProcessingStatus)
    setPagination((prev) => ({ ...prev, pageIndex: 0 }))
  }, [])

  const handleDateChange = useCallback(
    (type: 'start' | 'end', value: string) => {
      if (type === 'start') {
        setStartDate(value)
      } else {
        setEndDate(value)
      }
      setPagination((prev) => ({ ...prev, pageIndex: 0 }))
    },
    [],
  )

  // Handle sort toggle
  const handleSort = useCallback(
    (field: SortField) => {
      if (sortBy === field) {
        setSortOrder((prev) => (prev === 'asc' ? 'desc' : 'asc'))
      } else {
        setSortBy(field)
        setSortOrder('desc')
      }
    },
    [sortBy],
  )

  // Clear all filters
  const clearFilters = useCallback(() => {
    setSearchInput('')
    setDebouncedSearch('')
    setStatusFilter('all')
    setStartDate('')
    setEndDate('')
    setPagination({ pageIndex: 0, pageSize: 20 })
  }, [])

  // Fetch data
  const { data, isLoading, isError, error, isFetching, refetch } =
    useRawMessages({
      limit: pagination.pageSize,
      offset: pagination.pageIndex * pagination.pageSize,
      search: debouncedSearch || undefined,
      status: statusFilter,
      sort_by: sortBy,
      sort_order: sortOrder,
      start_date: startDate || undefined,
      end_date: endDate || undefined,
    })

  const messages = useMemo(() => data?.data || [], [data])
  const totalCount = data?.meta?.total || 0
  const { totalPages, currentPage } = calculatePagination(
    totalCount,
    pagination.pageSize,
    pagination.pageIndex * pagination.pageSize,
  )
  const canGoNext = calculateCanGoNext(
    totalCount,
    pagination.pageSize,
    pagination.pageIndex * pagination.pageSize,
  )
  const canGoPrev = pagination.pageIndex > 0

  const columnHelper = createColumnHelper<RawMessage>()

  const columns = useMemo(
    () => [
      columnHelper.accessor('timestamp', {
        header: () => (
          <button
            onClick={() => handleSort('timestamp')}
            className="flex items-center gap-1 hover:text-foreground transition-colors"
          >
            Timestamp
            {sortBy === 'timestamp' ? (
              sortOrder === 'asc' ? (
                <ArrowUp className="w-3 h-3" />
              ) : (
                <ArrowDown className="w-3 h-3" />
              )
            ) : (
              <ArrowUpDown className="w-3 h-3 opacity-50" />
            )}
          </button>
        ),
        cell: (info) => (
          <span className="text-muted-foreground whitespace-nowrap">
            {formatRelativeTime(info.getValue())}
          </span>
        ),
      }),
      columnHelper.accessor('content', {
        header: 'Content',
        cell: (info) => (
          <span className="text-foreground">
            {truncateContent(info.getValue(), 100)}
          </span>
        ),
      }),
      columnHelper.accessor('group_name', {
        header: 'Group',
        cell: (info) => (
          <span className="text-muted-foreground">
            {info.getValue() || info.row.original.group_jid}
          </span>
        ),
      }),
      columnHelper.accessor('participant_name', {
        header: 'Participant',
        cell: (info) => (
          <span className="text-muted-foreground">
            {info.getValue() || info.row.original.participant_jid}
          </span>
        ),
      }),
      columnHelper.accessor('processed_at', {
        header: () => (
          <button
            onClick={() => handleSort('processed_at')}
            className="flex items-center gap-1 hover:text-foreground transition-colors"
          >
            Status
            {sortBy === 'processed_at' ? (
              sortOrder === 'asc' ? (
                <ArrowUp className="w-3 h-3" />
              ) : (
                <ArrowDown className="w-3 h-3" />
              )
            ) : (
              <ArrowUpDown className="w-3 h-3 opacity-50" />
            )}
          </button>
        ),
        cell: (info) => getStatusBadge(info.row.original),
      }),
    ],
    [columnHelper, handleSort, sortBy, sortOrder],
  )

  const table = useReactTable({
    data: messages,
    columns,
    getCoreRowModel: getCoreRowModel(),
    manualPagination: true,
    pageCount: totalPages,
    state: { pagination },
    onPaginationChange: setPagination,
  })

  const hasActiveFilters =
    debouncedSearch || statusFilter !== 'all' || startDate || endDate

  return (
    <DashboardLayout>
      <div className="space-y-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold text-foreground">Raw Messages</h1>
            <p className="text-muted-foreground">
              View incoming WhatsApp messages before parsing
              {totalCount > 0 && (
                <span className="ml-2 text-teal">({totalCount} total)</span>
              )}
            </p>
          </div>
          <Button
            variant="outline"
            size="sm"
            onClick={() => refetch()}
            disabled={isFetching}
            className="gap-2"
          >
            <RefreshCw
              className={cn('w-4 h-4', isFetching && 'animate-spin')}
            />
            Refresh
          </Button>
        </div>

        {/* Filters */}
        <div className="flex flex-wrap items-center gap-3">
          {/* Search */}
          <div className="relative flex-1 min-w-[200px] max-w-sm">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
            <Input
              placeholder="Search messages..."
              value={searchInput}
              onChange={(e) => setSearchInput(e.target.value)}
              className="pl-9"
            />
          </div>

          {/* Status Filter */}
          <Select value={statusFilter} onValueChange={handleStatusChange}>
            <SelectTrigger className="w-[140px]">
              <SelectValue placeholder="Status" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All Status</SelectItem>
              <SelectItem value="processed">Processed</SelectItem>
              <SelectItem value="unprocessed">Pending</SelectItem>
              <SelectItem value="error">Error</SelectItem>
            </SelectContent>
          </Select>

          {/* Date Range - Hidden on mobile */}
          <div className="hidden sm:flex items-center gap-2">
            <div className="relative">
              <Calendar className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
              <Input
                type="date"
                value={startDate}
                onChange={(e) => handleDateChange('start', e.target.value)}
                className="pl-9 w-[150px]"
                placeholder="Start date"
              />
            </div>
            <span className="text-muted-foreground">to</span>
            <Input
              type="date"
              value={endDate}
              onChange={(e) => handleDateChange('end', e.target.value)}
              className="w-[150px]"
              placeholder="End date"
            />
          </div>

          {/* Page Size - Hidden on mobile */}
          <Select
            value={String(pagination.pageSize)}
            onValueChange={(value) =>
              setPagination({ pageIndex: 0, pageSize: Number(value) })
            }
          >
            <SelectTrigger className="hidden sm:flex w-[100px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="10">10 / page</SelectItem>
              <SelectItem value="20">20 / page</SelectItem>
              <SelectItem value="50">50 / page</SelectItem>
            </SelectContent>
          </Select>

          {/* Clear Filters */}
          {hasActiveFilters && (
            <Button
              variant="ghost"
              size="sm"
              onClick={clearFilters}
              className="gap-1"
            >
              <X className="w-4 h-4" />
              <span className="hidden sm:inline">Clear</span>
            </Button>
          )}
        </div>

        {/* Table */}
        <div className="glass-card rounded-xl overflow-hidden">
          {isLoading ? (
            <div className="flex items-center justify-center h-64">
              <Loader2 className="w-8 h-8 text-teal animate-spin" />
            </div>
          ) : isError ? (
            <div className="flex flex-col items-center justify-center h-64 gap-3">
              <AlertCircle className="w-10 h-10 text-destructive" />
              <p className="text-muted-foreground">
                {error?.message || 'Failed to load messages'}
              </p>
              <Button variant="outline" size="sm" onClick={() => refetch()}>
                Try Again
              </Button>
            </div>
          ) : messages.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-64 gap-3">
              <MessageSquare className="w-10 h-10 text-muted-foreground" />
              <p className="text-muted-foreground">
                {hasActiveFilters
                  ? 'No messages match your filters'
                  : 'No messages found'}
              </p>
              {hasActiveFilters && (
                <Button variant="outline" size="sm" onClick={clearFilters}>
                  Clear Filters
                </Button>
              )}
            </div>
          ) : (
            <>
              {/* Subtle loading indicator during refresh */}
              {isFetching && !isLoading && (
                <div className="absolute top-0 left-0 right-0 h-1 bg-teal/20">
                  <div
                    className="h-full bg-teal animate-pulse"
                    style={{ width: '30%' }}
                  />
                </div>
              )}

              {/* Desktop Table View */}
              <div className="hidden md:block">
                <table className="w-full">
                  <thead>
                    {table.getHeaderGroups().map((headerGroup) => (
                      <tr
                        key={headerGroup.id}
                        className="border-b border-border"
                      >
                        {headerGroup.headers.map((header) => (
                          <th
                            key={header.id}
                            className="text-left px-6 py-4 text-sm font-medium text-muted-foreground"
                          >
                            {header.isPlaceholder
                              ? null
                              : flexRender(
                                  header.column.columnDef.header,
                                  header.getContext(),
                                )}
                          </th>
                        ))}
                      </tr>
                    ))}
                  </thead>
                  <tbody>
                    {table.getRowModel().rows.map((row, index) => (
                      <tr
                        key={row.id}
                        onClick={() => setSelectedMessage(row.original)}
                        className={cn(
                          'border-b border-border/50 hover:bg-secondary/30 transition-colors cursor-pointer animate-fade-in',
                        )}
                        style={{ animationDelay: `${index * 30}ms` }}
                      >
                        {row.getVisibleCells().map((cell) => (
                          <td key={cell.id} className="px-6 py-4 text-sm">
                            {flexRender(
                              cell.column.columnDef.cell,
                              cell.getContext(),
                            )}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>

              {/* Mobile Card View */}
              <div className="md:hidden divide-y divide-border">
                {messages.map((message, index) => (
                  <div
                    key={message.id}
                    onClick={() => setSelectedMessage(message)}
                    className={cn(
                      'p-4 hover:bg-secondary/30 transition-colors cursor-pointer animate-fade-in',
                    )}
                    style={{ animationDelay: `${index * 30}ms` }}
                  >
                    <div className="flex items-start justify-between gap-3 mb-2">
                      <span className="text-xs text-muted-foreground">
                        {formatRelativeTime(message.timestamp)}
                      </span>
                      {getStatusBadge(message)}
                    </div>
                    <p className="text-sm text-foreground mb-2">
                      {truncateContent(message.content, 80)}
                    </p>
                    <div className="flex items-center gap-2 text-xs text-muted-foreground">
                      <span>{message.group_name || message.group_jid}</span>
                      <span>•</span>
                      <span>
                        {message.participant_name || message.participant_jid}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            </>
          )}

          {/* Pagination */}
          {!isLoading && !isError && messages.length > 0 && (
            <div className="flex flex-col sm:flex-row items-center justify-between gap-3 px-4 sm:px-6 py-4 border-t border-border">
              <div className="text-sm text-muted-foreground">
                Showing {pagination.pageIndex * pagination.pageSize + 1} -{' '}
                {Math.min(
                  (pagination.pageIndex + 1) * pagination.pageSize,
                  totalCount,
                )}{' '}
                of {totalCount}
              </div>
              <div className="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() =>
                    setPagination((prev) => ({
                      ...prev,
                      pageIndex: prev.pageIndex - 1,
                    }))
                  }
                  disabled={!canGoPrev}
                  className="min-w-[44px] min-h-[44px] sm:min-w-0 sm:min-h-0"
                >
                  <ChevronLeft className="w-4 h-4" />
                </Button>
                <span className="text-sm text-muted-foreground px-2">
                  Page {currentPage} of {totalPages}
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() =>
                    setPagination((prev) => ({
                      ...prev,
                      pageIndex: prev.pageIndex + 1,
                    }))
                  }
                  disabled={!canGoNext}
                  className="min-w-[44px] min-h-[44px] sm:min-w-0 sm:min-h-0"
                >
                  <ChevronRight className="w-4 h-4" />
                </Button>
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Detail Panel */}
      <Sheet
        open={!!selectedMessage}
        onOpenChange={(open) => !open && setSelectedMessage(null)}
      >
        <SheetContent className="w-full sm:max-w-lg overflow-y-auto">
          {selectedMessage && (
            <>
              <SheetHeader>
                <SheetTitle>Message Details</SheetTitle>
                <SheetDescription>
                  {formatRelativeTime(selectedMessage.timestamp)}
                </SheetDescription>
              </SheetHeader>

              <div className="space-y-6 mt-6">
                {/* Status */}
                <div>
                  <h4 className="text-sm font-medium text-muted-foreground mb-2">
                    Status
                  </h4>
                  {getStatusBadge(selectedMessage)}
                </div>

                {/* Full Content */}
                <div>
                  <h4 className="text-sm font-medium text-muted-foreground mb-2">
                    Content
                  </h4>
                  <p className="text-foreground whitespace-pre-wrap bg-secondary/30 p-3 rounded-lg">
                    {selectedMessage.content}
                  </p>
                </div>

                {/* Reply Context */}
                {selectedMessage.reply_to_id && (
                  <div>
                    <h4 className="text-sm font-medium text-muted-foreground mb-2">
                      Reply To
                    </h4>
                    <div className="bg-secondary/30 p-3 rounded-lg border-l-2 border-teal">
                      {selectedMessage.reply_to_sender && (
                        <p className="text-sm text-teal font-medium mb-1">
                          {selectedMessage.reply_to_sender}
                        </p>
                      )}
                      <p className="text-muted-foreground text-sm">
                        {selectedMessage.reply_to_content ||
                          'Original message not available'}
                      </p>
                    </div>
                  </div>
                )}

                {/* Metadata */}
                <div>
                  <h4 className="text-sm font-medium text-muted-foreground mb-2">
                    Metadata
                  </h4>
                  <dl className="space-y-2 text-sm">
                    <div className="flex justify-between">
                      <dt className="text-muted-foreground">ID</dt>
                      <dd className="text-foreground font-mono text-xs">
                        {selectedMessage.id}
                      </dd>
                    </div>
                    {selectedMessage.external_id && (
                      <div className="flex justify-between">
                        <dt className="text-muted-foreground">External ID</dt>
                        <dd className="text-foreground font-mono text-xs">
                          {selectedMessage.external_id}
                        </dd>
                      </div>
                    )}
                    <div className="flex justify-between">
                      <dt className="text-muted-foreground">Timestamp</dt>
                      <dd className="text-foreground">
                        {new Date(selectedMessage.timestamp).toLocaleString()}
                      </dd>
                    </div>
                    <div className="flex justify-between">
                      <dt className="text-muted-foreground">Created At</dt>
                      <dd className="text-foreground">
                        {new Date(selectedMessage.created_at).toLocaleString()}
                      </dd>
                    </div>
                    {selectedMessage.processed_at && (
                      <div className="flex justify-between">
                        <dt className="text-muted-foreground">Processed At</dt>
                        <dd className="text-foreground">
                          {new Date(
                            selectedMessage.processed_at,
                          ).toLocaleString()}
                        </dd>
                      </div>
                    )}
                  </dl>
                </div>

                {/* Error */}
                {selectedMessage.error && (
                  <div>
                    <h4 className="text-sm font-medium text-destructive mb-2">
                      Error
                    </h4>
                    <p className="text-destructive bg-destructive/10 p-3 rounded-lg text-sm">
                      {selectedMessage.error}
                    </p>
                  </div>
                )}

                {/* Group & Participant Links */}
                <div>
                  <h4 className="text-sm font-medium text-muted-foreground mb-2">
                    Source
                  </h4>
                  <dl className="space-y-2 text-sm">
                    <div className="flex justify-between items-center">
                      <dt className="text-muted-foreground">Group</dt>
                      <dd>
                        <a
                          href={`/groups?id=${selectedMessage.group_id}`}
                          className="text-teal hover:underline"
                        >
                          {selectedMessage.group_name ||
                            selectedMessage.group_jid}
                        </a>
                      </dd>
                    </div>
                    <div className="flex justify-between items-center">
                      <dt className="text-muted-foreground">Participant</dt>
                      <dd className="text-foreground">
                        {selectedMessage.participant_name ||
                          selectedMessage.participant_jid}
                      </dd>
                    </div>
                  </dl>
                </div>
              </div>
            </>
          )}
        </SheetContent>
      </Sheet>
    </DashboardLayout>
  )
}
