import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { DashboardLayout } from '@/components/layout/dashboard-layout'
import { Card, CardContent } from '@/components/ui/card'
import { SendMessageDialog } from '@/components/ui/send-message-dialog'
import {
  useRawMessages,
  useReprocessMessage,
  useDeleteMessage,
  useBulkReprocessMessages,
  useBulkDeleteMessages,
  useBulkMarkProcessed,
} from '@/hooks/use-raw-messages'
import { useState, useCallback, useEffect, useMemo } from 'react'
import { toast } from 'sonner'
import type { RowSelectionState } from '@tanstack/react-table'
import {
  FilterToolbar,
  MessageTable,
  PaginationBar,
  MessageDetailPanel,
  MessageActionBar,
  EmptyState,
  LoadingSkeleton,
  ConfirmDeleteDialog,
  calculatePagination,
  calculateCanGoNext,
  defaultFilters,
  defaultPagination,
  exportSelectedToCSV,
  type RawMessage,
  type RawMessageFilters,
  type PaginationState,
  type SortField,
} from '@/components/raw-messages'
import { ProcessedItemsDialog } from '@/components/raw-messages/processed-items-dialog'

export const Route = createFileRoute('/raw-messages')({
  component: RawMessages,
})

function RawMessages() {
  const navigate = useNavigate()

  // State
  const [filters, setFilters] = useState<RawMessageFilters>(defaultFilters)
  const [debouncedSearch, setDebouncedSearch] = useState('')
  const [pagination, setPagination] =
    useState<PaginationState>(defaultPagination)
  const [selectedMessage, setSelectedMessage] = useState<RawMessage | null>(
    null,
  )

  // TanStack Table row selection state
  const [rowSelection, setRowSelection] = useState<RowSelectionState>({})

  // Send message dialog state
  const [replyToMessage, setReplyToMessage] = useState<RawMessage | null>(null)

  // Delete confirmation dialog state
  const [deleteDialogState, setDeleteDialogState] = useState<{
    open: boolean
    type: 'single' | 'bulk'
    messageId?: string
  }>({ open: false, type: 'single' })

  // Processed items dialog state
  const [itemsDialogMessage, setItemsDialogMessage] =
    useState<RawMessage | null>(null)

  // Mutation hooks
  const reprocessMutation = useReprocessMessage()
  const deleteMutation = useDeleteMessage()
  const bulkReprocessMutation = useBulkReprocessMessages()
  const bulkDeleteMutation = useBulkDeleteMessages()
  const bulkMarkProcessedMutation = useBulkMarkProcessed()

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

  // Calculate selected count and check if all selected
  const selectedIds = useMemo(() => Object.keys(rowSelection), [rowSelection])
  const selectedCount = selectedIds.length
  const isAllSelected = messages.length > 0 && selectedCount === messages.length

  // Debounce search (300ms)
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedSearch(filters.search)
      setPagination((prev) => ({ ...prev, pageIndex: 0 }))
    }, 300)
    return () => clearTimeout(timer)
  }, [filters.search])

  // Clear selection when data changes
  useEffect(() => {
    setRowSelection({})
  }, [
    pagination.pageIndex,
    pagination.pageSize,
    debouncedSearch,
    filters.status,
  ])

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

  // Selection handlers for action bar
  const handleSelectAll = useCallback(() => {
    if (isAllSelected) {
      setRowSelection({})
    } else {
      const newSelection: RowSelectionState = {}
      messages.forEach((m) => {
        newSelection[m.id] = true
      })
      setRowSelection(newSelection)
    }
  }, [messages, isAllSelected])

  const handleClearSelection = useCallback(() => {
    setRowSelection({})
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

  const handleReprocess = useCallback(
    (message: RawMessage) => {
      reprocessMutation.mutate(message.id, {
        onSuccess: () => {
          toast.success('Message queued for reprocessing')
        },
        onError: (err) => {
          toast.error(`Failed to reprocess: ${err.message}`)
        },
      })
    },
    [reprocessMutation],
  )

  const handleDelete = useCallback((message: RawMessage) => {
    setDeleteDialogState({
      open: true,
      type: 'single',
      messageId: message.id,
    })
  }, [])

  const handleConfirmDelete = useCallback(() => {
    if (deleteDialogState.type === 'single' && deleteDialogState.messageId) {
      deleteMutation.mutate(deleteDialogState.messageId, {
        onSuccess: () => {
          toast.success('Message deleted')
          setDeleteDialogState({ open: false, type: 'single' })
          // Clear selection if deleted message was selected
          if (selectedMessage?.id === deleteDialogState.messageId) {
            setSelectedMessage(null)
          }
        },
        onError: (err) => {
          toast.error(`Failed to delete: ${err.message}`)
        },
      })
    } else if (deleteDialogState.type === 'bulk') {
      bulkDeleteMutation.mutate(selectedIds, {
        onSuccess: (response) => {
          const result = response.data
          if (result) {
            const successCount = result.succeeded.length
            const failCount = result.failed.length
            if (failCount === 0) {
              toast.success(`${successCount} messages deleted`)
            } else {
              toast.warning(
                `${successCount} deleted, ${failCount} failed (may have associated data)`,
              )
            }
          }
          setDeleteDialogState({ open: false, type: 'single' })
          setRowSelection({})
        },
        onError: (err) => {
          toast.error(`Bulk delete failed: ${err.message}`)
        },
      })
    }
  }, [
    deleteDialogState,
    deleteMutation,
    bulkDeleteMutation,
    selectedIds,
    selectedMessage,
  ])

  // Reply handler - opens send message dialog
  const handleReply = useCallback((message: RawMessage) => {
    setReplyToMessage(message)
  }, [])

  // View processed items handler
  const handleViewItems = useCallback((message: RawMessage) => {
    setItemsDialogMessage(message)
  }, [])

  // Bulk action handlers
  const handleBulkReprocess = useCallback(() => {
    bulkReprocessMutation.mutate(selectedIds, {
      onSuccess: (response) => {
        const result = response.data
        if (result) {
          const successCount = result.succeeded.length
          const failCount = result.failed.length
          if (failCount === 0) {
            toast.success(`${successCount} messages queued for reprocessing`)
          } else {
            toast.warning(`${successCount} queued, ${failCount} failed`)
          }
        }
        setRowSelection({})
      },
      onError: (err) => {
        toast.error(`Bulk reprocess failed: ${err.message}`)
      },
    })
  }, [bulkReprocessMutation, selectedIds])

  const handleBulkDelete = useCallback(() => {
    setDeleteDialogState({
      open: true,
      type: 'bulk',
    })
  }, [])

  const handleBulkExport = useCallback(() => {
    try {
      const selectedMessages = messages.filter((m) =>
        selectedIds.includes(m.id),
      )
      exportSelectedToCSV(
        selectedMessages as RawMessage[],
        new Set(selectedIds),
      )
      toast.success(`Exported ${selectedIds.length} messages to CSV`)
    } catch (err) {
      toast.error(
        `Export failed: ${err instanceof Error ? err.message : 'Unknown error'}`,
      )
    }
  }, [messages, selectedIds])

  const handleBulkMarkProcessed = useCallback(() => {
    bulkMarkProcessedMutation.mutate(selectedIds, {
      onSuccess: (response) => {
        const result = response.data
        if (result) {
          const successCount = result.succeeded.length
          const failCount = result.failed.length
          if (failCount === 0) {
            toast.success(`${successCount} messages marked as processed`)
          } else {
            toast.warning(
              `${successCount} marked, ${failCount} failed (may already be processed)`,
            )
          }
        }
        setRowSelection({})
      },
      onError: (err) => {
        toast.error(`Bulk mark processed failed: ${err.message}`)
      },
    })
  }, [bulkMarkProcessedMutation, selectedIds])

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
                onViewDetailsClick={setSelectedMessage}
                onViewItemsClick={handleViewItems}
                selectedId={selectedMessage?.id}
                // TanStack Table row selection
                rowSelection={rowSelection}
                onRowSelectionChange={setRowSelection}
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
          selectedCount={selectedCount}
          totalCount={messages.length}
          onSelectAll={handleSelectAll}
          onClearSelection={handleClearSelection}
          isAllSelected={isAllSelected}
          onBulkReprocess={handleBulkReprocess}
          onBulkDelete={handleBulkDelete}
          onBulkExport={handleBulkExport}
          onBulkMarkProcessed={handleBulkMarkProcessed}
          loading={
            bulkReprocessMutation.isPending ||
            bulkDeleteMutation.isPending ||
            bulkMarkProcessedMutation.isPending
          }
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

        {/* Delete Confirmation Dialog */}
        <ConfirmDeleteDialog
          open={deleteDialogState.open}
          onOpenChange={(open) =>
            setDeleteDialogState((prev) => ({ ...prev, open }))
          }
          onConfirm={handleConfirmDelete}
          title={
            deleteDialogState.type === 'single'
              ? 'Delete Message'
              : 'Delete Selected Messages'
          }
          description={
            deleteDialogState.type === 'single'
              ? 'Are you sure you want to delete this message? Messages with associated offers or requests cannot be deleted.'
              : `Are you sure you want to delete ${selectedCount} selected messages? Messages with associated offers or requests will be skipped.`
          }
          isLoading={deleteMutation.isPending || bulkDeleteMutation.isPending}
          itemCount={
            deleteDialogState.type === 'bulk' ? selectedCount : undefined
          }
        />

        {/* Processed Items Dialog */}
        <ProcessedItemsDialog
          message={itemsDialogMessage}
          onClose={() => setItemsDialogMessage(null)}
        />
      </div>
    </DashboardLayout>
  )
}
