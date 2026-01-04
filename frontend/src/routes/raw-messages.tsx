import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { DashboardLayout } from '@/components/layout/dashboard-layout'
import { Card, CardContent } from '@/components/ui/card'
import { SendMessageDialog } from '@/components/ui/send-message-dialog'
import { useRawMessages } from '@/hooks/use-raw-messages'
import { useState, useCallback, useEffect, useMemo } from 'react'
import { toast } from 'sonner'
import {
  FilterToolbar,
  MessageTable,
  PaginationBar,
  MessageDetailPanel,
  MessageActionBar,
  EmptyState,
  LoadingSkeleton,
  calculatePagination,
  calculateCanGoNext,
  defaultFilters,
  defaultPagination,
  type RawMessage,
  type RawMessageFilters,
  type PaginationState,
  type SortField,
} from '@/components/raw-messages'

export const Route = createFileRoute('/raw-messages')({
  component: RawMessages,
})

function RawMessages() {
  const navigate = useNavigate()

  // State
  const [filters, setFilters] = useState<RawMessageFilters>(defaultFilters)
  const [debouncedSearch, setDebouncedSearch] = useState('')
  const [pagination, setPagination] = useState<PaginationState>(defaultPagination)
  const [selectedMessage, setSelectedMessage] = useState<RawMessage | null>(null)
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())

  // Send message dialog state
  const [replyToMessage, setReplyToMessage] = useState<RawMessage | null>(null)

  // Fetch data first (before callbacks that depend on it)
  const { data, isLoading, isError, error, isFetching, refetch } =
    useRawMessages({
      limit: pagination.pageSize,
      offset: pagination.pageIndex * pagination.pageSize,
      search: debouncedSearch || undefined,
      status: filters.status,
      sort_by: filters.sortBy,
      sort_order: filters.sortOrder,
      start_date: filters.startDate || undefined,
      end_date: filters.endDate || undefined,
    })

  // Derived data
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

  const hasActiveFilters =
    debouncedSearch ||
    filters.status !== 'all' ||
    filters.startDate ||
    filters.endDate

  const isAllSelected = useMemo(() => {
    if (messages.length === 0) return false
    return messages.every((m) => selectedIds.has(m.id))
  }, [messages, selectedIds])

  // Debounce search (300ms)
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedSearch(filters.search)
      setPagination((prev) => ({ ...prev, pageIndex: 0 }))
    }, 300)
    return () => clearTimeout(timer)
  }, [filters.search])

  // Reset pagination on filter change
  const handleFiltersChange = useCallback(
    (newFilters: RawMessageFilters) => {
      setFilters(newFilters)
      if (
        newFilters.status !== filters.status ||
        newFilters.startDate !== filters.startDate ||
        newFilters.endDate !== filters.endDate
      ) {
        setPagination((prev) => ({ ...prev, pageIndex: 0 }))
      }
    },
    [filters],
  )

  // Handle sort
  const handleSort = useCallback((field: SortField) => {
    setFilters((prev) => ({
      ...prev,
      sortBy: field,
      sortOrder:
        prev.sortBy === field && prev.sortOrder === 'desc' ? 'asc' : 'desc',
    }))
  }, [])

  // Clear filters
  const clearFilters = useCallback(() => {
    setFilters(defaultFilters)
    setDebouncedSearch('')
    setPagination(defaultPagination)
  }, [])

  // Selection handlers
  const handleToggleSelect = useCallback((id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return next
    })
  }, [])

  const handleSelectAll = useCallback(() => {
    setSelectedIds((prev) => {
      if (prev.size === messages.length && messages.length > 0) {
        return new Set()
      }
      return new Set(messages.map((m) => m.id))
    })
  }, [messages])

  const handleClearSelection = useCallback(() => {
    setSelectedIds(new Set())
  }, [])

  // Action handlers
  const handleCopyContent = useCallback((message: RawMessage) => {
    navigator.clipboard.writeText(message.content)
    toast.success('Content copied to clipboard')
  }, [])

  const handleViewGroup = useCallback(
    (message: RawMessage) => {
      navigate({ to: '/groups', search: { id: message.groupId } })
    },
    [navigate],
  )

  const handleReprocess = useCallback((message: RawMessage) => {
    // TODO: Implement reprocess API call
    toast.info(`Reprocessing message ${message.id.slice(0, 8)}...`)
  }, [])

  const handleDelete = useCallback((message: RawMessage) => {
    // TODO: Implement delete API call
    toast.info(`Delete message ${message.id.slice(0, 8)}`)
  }, [])

  // Reply handler - opens send message dialog
  const handleReply = useCallback((message: RawMessage) => {
    setReplyToMessage(message)
  }, [])

  // Bulk action handlers
  const handleBulkReprocess = useCallback(() => {
    // TODO: Implement bulk reprocess API call
    toast.info(`Reprocessing ${selectedIds.size} messages...`)
    setSelectedIds(new Set())
  }, [selectedIds.size])

  const handleBulkDelete = useCallback(() => {
    // TODO: Implement bulk delete API call
    toast.info(`Deleting ${selectedIds.size} messages...`)
    setSelectedIds(new Set())
  }, [selectedIds.size])

  const handleBulkExport = useCallback(() => {
    // TODO: Implement export functionality
    toast.info(`Exporting ${selectedIds.size} messages...`)
  }, [selectedIds.size])

  const handleBulkMarkProcessed = useCallback(() => {
    // TODO: Implement mark as processed API call
    toast.info(`Marking ${selectedIds.size} messages as processed...`)
    setSelectedIds(new Set())
  }, [selectedIds.size])

  return (
    <DashboardLayout>
      <div className="h-full flex flex-col gap-3 p-4">
        {/* Header - Compact */}
        <div className="flex items-center justify-between">
          <h1 className="text-lg font-semibold">Raw Messages</h1>
        </div>

        {/* Main Content Card */}
        <Card className="flex-1 flex flex-col overflow-hidden py-0 gap-0">
          {/* Filter Toolbar */}
          <div className="border-b p-2">
            <FilterToolbar
              filters={filters}
              onFiltersChange={handleFiltersChange}
              onRefresh={() => refetch()}
              isRefreshing={isFetching}
              totalCount={totalCount}
            />
          </div>

          {/* Table Content */}
          <CardContent className="flex-1 overflow-auto p-0 relative">
            {/* Loading overlay for refetch */}
            {isFetching && !isLoading && (
              <div className="absolute top-0 left-0 right-0 h-0.5 bg-primary/20 overflow-hidden z-10">
                <div className="h-full w-1/3 bg-primary animate-pulse" />
              </div>
            )}

            {isLoading ? (
              <LoadingSkeleton rows={pagination.pageSize} />
            ) : isError ? (
              <EmptyState
                type="error"
                errorMessage={error?.message}
                onRetry={() => refetch()}
              />
            ) : messages.length === 0 ? (
              <EmptyState
                type={hasActiveFilters ? 'no-results' : 'no-data'}
                onClearFilters={hasActiveFilters ? clearFilters : undefined}
              />
            ) : (
              <MessageTable
                messages={messages}
                sortBy={filters.sortBy}
                sortOrder={filters.sortOrder}
                onSort={handleSort}
                onRowClick={setSelectedMessage}
                selectedId={selectedMessage?.id}
                // Selection
                selectedIds={selectedIds}
                onToggleSelect={handleToggleSelect}
                onSelectAll={handleSelectAll}
                isAllSelected={isAllSelected}
                // Actions
                onReprocess={handleReprocess}
                onDelete={handleDelete}
                onCopyContent={handleCopyContent}
                onViewGroup={handleViewGroup}
                onReply={handleReply}
              />
            )}
          </CardContent>

          {/* Pagination */}
          {!isLoading && !isError && messages.length > 0 && (
            <PaginationBar
              pagination={pagination}
              onPaginationChange={setPagination}
              totalCount={totalCount}
              totalPages={totalPages}
              currentPage={currentPage}
              canGoPrev={canGoPrev}
              canGoNext={canGoNext}
            />
          )}
        </Card>

        {/* Detail Panel */}
        <MessageDetailPanel
          message={selectedMessage}
          onClose={() => setSelectedMessage(null)}
        />

        {/* Floating Action Bar for Bulk Operations */}
        <MessageActionBar
          selectedCount={selectedIds.size}
          totalCount={messages.length}
          onSelectAll={handleSelectAll}
          onClearSelection={handleClearSelection}
          isAllSelected={isAllSelected}
          onBulkReprocess={handleBulkReprocess}
          onBulkDelete={handleBulkDelete}
          onBulkExport={handleBulkExport}
          onBulkMarkProcessed={handleBulkMarkProcessed}
        />

        {/* Send Message Dialog */}
        {replyToMessage && replyToMessage.participantJid && (
          <SendMessageDialog
            open={!!replyToMessage}
            onOpenChange={(open) => !open && setReplyToMessage(null)}
            recipientJid={replyToMessage.participantJid}
            recipientName={replyToMessage.participantName || undefined}
            context={`Reply to message from ${replyToMessage.participantName || replyToMessage.participantJid?.split('@')[0]}`}
            accentColor="teal"
            onSuccess={() => {
              toast.success('Reply sent successfully')
              setReplyToMessage(null)
            }}
          />
        )}
      </div>
    </DashboardLayout>
  )
}
