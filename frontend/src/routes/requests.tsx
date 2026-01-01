import { createFileRoute } from '@tanstack/react-router'
import { DashboardLayout } from '@/components/layout/dashboard-layout'
import { Badge } from '@/components/ui/badge'
import {
  Flame,
  Filter,
  Plus,
  ChevronLeft,
  ChevronRight,
  Loader2,
  AlertCircle,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { useRequests } from '@/hooks/use-offers-requests'
import { useState, useMemo } from 'react'
import {
  useReactTable,
  getCoreRowModel,
  flexRender,
  createColumnHelper,
} from '@tanstack/react-table'
import type {
  Request,
  RequestStatus,
  UrgencyLevel,
} from '@/schema/offer-request'

export const Route = createFileRoute('/requests')({
  component: Requests,
})

const columnHelper = createColumnHelper<Request>()

const columns = [
  columnHelper.accessor('medication', {
    header: 'Medication Needed',
    cell: (info) => (
      <span className="font-medium text-foreground">{info.getValue()}</span>
    ),
  }),
  columnHelper.accessor('quantity', {
    header: 'Quantity Requested',
    cell: (info) => {
      const qty = info.getValue()
      const unit = info.row.original.unit
      return (
        <span className="text-muted-foreground">
          {qty ? `${qty} ${unit || 'units'}` : '-'}
        </span>
      )
    },
  }),
  columnHelper.accessor('max_price', {
    header: 'Max Price',
    cell: (info) => {
      const price = info.getValue()
      const currency = info.row.original.currency || 'EGP'
      return (
        <span className="font-medium text-amber">
          {price ? `${price} ${currency}` : '-'}
        </span>
      )
    },
  }),
  columnHelper.accessor('urgency_level', {
    header: 'Urgent',
    cell: (info) => {
      const urgency = info.getValue() as UrgencyLevel
      return urgency !== 'Normal' ? (
        <Flame className="w-5 h-5 text-amber" />
      ) : null
    },
  }),
  columnHelper.accessor('status', {
    header: 'Status',
    cell: (info) => {
      const status = info.getValue() as RequestStatus
      return (
        <Badge
          variant="outline"
          className={cn(
            'font-medium',
            status === 'Active'
              ? 'border-amber/50 text-amber bg-amber/10'
              : status === 'Matched'
                ? 'border-teal/50 text-teal bg-teal/10'
                : 'border-muted-foreground/50 text-muted-foreground bg-muted/10',
          )}
        >
          {status}
        </Badge>
      )
    },
  }),
]

function Requests() {
  const [pagination, setPagination] = useState({ pageIndex: 0, pageSize: 10 })

  const { data, isLoading, isError, error } = useRequests({
    limit: pagination.pageSize,
    offset: pagination.pageIndex * pagination.pageSize,
  })

  const requests = useMemo(() => data?.data || [], [data])
  const totalCount = data?.meta?.total || 0
  const pageCount = Math.ceil(totalCount / pagination.pageSize)

  const table = useReactTable({
    data: requests,
    columns,
    getCoreRowModel: getCoreRowModel(),
    manualPagination: true,
    pageCount,
    state: { pagination },
    onPaginationChange: setPagination,
  })

  return (
    <DashboardLayout>
      <div className="space-y-6">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold text-foreground">
              Requests Management
            </h1>
            <p className="text-muted-foreground">
              Track and manage medication requests
              {totalCount > 0 && (
                <span className="ml-2 text-amber">({totalCount} total)</span>
              )}
            </p>
          </div>
          <button className="flex items-center gap-2 px-4 py-2 rounded-lg bg-amber text-primary-foreground font-medium hover:bg-amber/90 transition-colors">
            <Plus className="w-4 h-4" />
            New Request
          </button>
        </div>

        {/* Filters */}
        <div className="flex items-center gap-3">
          <button className="px-4 py-2 rounded-full bg-amber text-primary-foreground text-sm font-medium">
            All
          </button>
          <button className="px-4 py-2 rounded-full bg-secondary text-foreground text-sm font-medium hover:bg-secondary/80 transition-colors">
            Open
          </button>
          <button className="px-4 py-2 rounded-full bg-secondary text-foreground text-sm font-medium hover:bg-secondary/80 transition-colors">
            Fulfilled
          </button>
          <button className="px-4 py-2 rounded-full bg-secondary text-foreground text-sm font-medium hover:bg-secondary/80 transition-colors">
            Urgent
          </button>
          <button className="ml-auto flex items-center gap-2 px-3 py-2 rounded-lg bg-secondary text-muted-foreground text-sm hover:text-foreground transition-colors">
            <Filter className="w-4 h-4" />
            Filter
          </button>
        </div>

        {/* Table */}
        <div className="glass-card rounded-xl overflow-hidden">
          {isLoading ? (
            <div className="flex items-center justify-center h-64">
              <Loader2 className="w-8 h-8 text-amber animate-spin" />
            </div>
          ) : isError ? (
            <div className="flex flex-col items-center justify-center h-64 gap-3">
              <AlertCircle className="w-10 h-10 text-destructive" />
              <p className="text-muted-foreground">
                {error?.message || 'Failed to load requests'}
              </p>
              <p className="text-xs text-muted-foreground">
                Make sure the backend is running on port 8081
              </p>
            </div>
          ) : requests.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-64 gap-3">
              <p className="text-muted-foreground">No requests found</p>
            </div>
          ) : (
            <table className="w-full">
              <thead>
                {table.getHeaderGroups().map((headerGroup) => (
                  <tr key={headerGroup.id} className="border-b border-border">
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
          )}

          {/* Pagination */}
          {!isLoading && !isError && requests.length > 0 && (
            <div className="flex items-center justify-between px-6 py-4 border-t border-border">
              <div className="text-sm text-muted-foreground">
                Showing {pagination.pageIndex * pagination.pageSize + 1} -{' '}
                {Math.min(
                  (pagination.pageIndex + 1) * pagination.pageSize,
                  totalCount,
                )}{' '}
                of {totalCount}
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => table.previousPage()}
                  disabled={!table.getCanPreviousPage()}
                  className="p-2 rounded-lg bg-secondary text-foreground disabled:opacity-50 disabled:cursor-not-allowed hover:bg-secondary/80 transition-colors"
                >
                  <ChevronLeft className="w-4 h-4" />
                </button>
                <span className="text-sm text-muted-foreground px-2">
                  Page {pagination.pageIndex + 1} of {pageCount || 1}
                </span>
                <button
                  onClick={() => table.nextPage()}
                  disabled={!table.getCanNextPage()}
                  className="p-2 rounded-lg bg-secondary text-foreground disabled:opacity-50 disabled:cursor-not-allowed hover:bg-secondary/80 transition-colors"
                >
                  <ChevronRight className="w-4 h-4" />
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </DashboardLayout>
  )
}
