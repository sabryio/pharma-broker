import { createFileRoute } from '@tanstack/react-router'
import { DashboardLayout } from '@/components/layout/dashboard-layout'
import { Card, CardContent } from '@/components/ui/card'
import { useRawMessages } from '@/hooks/use-raw-messages'
import { useState, useCallback, useEffect } from 'react'
import {
  FilterToolbar,
  MessageTable,
  PaginationBar,
  MessageDetailPanel,
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
  // State
  const [filters, setFilters] = useState<RawMessageFilters>(defaultFilters)
  const [debouncedSearch, setDebouncedSearch] = useState('')
  const [pagination, setPagination] = useState<PaginationState>(defaultPagination)
  const [selectedMessage, setSelectedMessage] = useState<RawMessage | null>(null)

  // Debounce search (300ms)
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedSearch(filters.search)
      setPagination((prev) => ({ ...prev, pageIndex: 0 }))
    }, 300)
    return () => clearTimeout(timer)
  }, [filters.search])

  // Reset pagination on filter change
  const handleFiltersChange = useCallback((newFilters: RawMessageFilters) => {
    setFilters(newFilters)
    if (newFilters.status !== filters.status ||
        newFilters.startDate !== filters.startDate ||
        newFilters.endDate !== filters.endDate) {
      setPagination((prev) => ({ ...prev, pageIndex: 0 }))
    }
  }, [filters])

  // Handle sort
  const handleSort = useCallback((field: SortField) => {
    setFilters((prev) => ({
      ...prev,
      sortBy: field,
      sortOrder: prev.sortBy === field && prev.sortOrder === 'desc' ? 'asc' : 'desc',
    }))
  }, [])

  // Clear filters
  const clearFilters = useCallback(() => {
    setFilters(defaultFilters)
    setDebouncedSearch('')
    setPagination(defaultPagination)
  }, [])

  // Fetch data
  const { data, isLoading, isError, error, isFetching, refetch } = useRawMessages({
    limit: pagination.pageSize,
    offset: pagination.pageIndex * pagination.pageSize,
    search: debouncedSearch || undefined,
    status: filters.status,
    sort_by: filters.sortBy,
    sort_order: filters.sortOrder,
    start_date: filters.startDate || undefined,
    end_date: filters.endDate || undefined,
  })

  const messages = data?.data || []
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
      </div>
    </DashboardLayout>
  )
}
