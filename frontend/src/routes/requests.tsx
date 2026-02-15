import { createFileRoute } from '@tanstack/react-router'
import { DashboardLayout } from '@/components/layout/dashboard-layout'
import { Badge } from '@/components/ui/badge'
import { MatchCountInline } from '@/components/ui/match-count-badge'
import { ReclassifyDialog } from '@/components/ui/reclassify-dialog'
import {
  Flame,
  Filter,
  Plus,
  ChevronLeft,
  ChevronRight,
  Loader2,
  AlertCircle,
  ArrowRightLeft,
  MoreHorizontal,
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
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Button } from '@/components/ui/button'
import type {
  Request,
  RequestStatus,
  UrgencyLevel,
} from '@/schema/offer-request'

export const Route = createFileRoute('/requests')({
  component: Requests,
})

function Requests() {
  const [pagination, setPagination] = useState({ pageIndex: 0, pageSize: 10 })
  const [reclassifyItem, setReclassifyItem] = useState<{
    id: string
    medication: string
    medicationRaw: string
  } | null>(null)

  const { data, isLoading, isError, error } = useRequests({
    limit: pagination.pageSize,
    offset: pagination.pageIndex * pagination.pageSize,
  })

  const requests = useMemo(() => data?.data || [], [data])
  const totalCount = data?.meta?.total || 0
  const pageCount = Math.ceil(totalCount / pagination.pageSize)

  const columnHelper = createColumnHelper<Request>()

  const columns = useMemo(
    () => [
      columnHelper.accessor('medication', {
        header: 'Medication Needed',
        cell: (info) => (
          <span className="font-medium text-foreground">{info.getValue()}</span>
        ),
      }),
      columnHelper.accessor('confirmed_match_count', {
        header: 'Matches',
        cell: (info) => (
          <MatchCountInline count={info.getValue() ?? 0} variant="request" />
        ),
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
      columnHelper.display({
        id: 'actions',
        header: '',
        cell: (info) => {
          const request = info.row.original
          return (
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="ghost" size="sm" className="h-8 w-8 p-0">
                  <MoreHorizontal className="h-4 w-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end">
                <DropdownMenuItem
                  onClick={() =>
                    setReclassifyItem({
                      id: request.id,
                      medication: request.medication,
                      medicationRaw: request.medication_raw,
                    })
                  }
                  className="gap-2 text-emerald"
                >
                  <ArrowRightLeft className="w-4 h-4" />
                  Convert to Offer
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          )
        },
      }),
    ],
    [columnHelper],
  )

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
                Make sure the backend is running on port 8082
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
                      'border-b border-border/50 hover:bg-secondary/30 transition-colors animate-fade-in',
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

      {/* Reclassify Dialog */}
      <ReclassifyDialog
        open={!!reclassifyItem}
        onOpenChange={(open) => !open && setReclassifyItem(null)}
        itemId={reclassifyItem?.id ?? ''}
        itemType="request"
        medication={reclassifyItem?.medication ?? ''}
        medicationRaw={reclassifyItem?.medicationRaw}
        onSuccess={() => setReclassifyItem(null)}
      />
    </DashboardLayout>
  )
}
