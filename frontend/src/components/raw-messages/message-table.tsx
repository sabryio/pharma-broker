// Message Table Component for Raw Messages
// Using TanStack Table for proper table management

import { useMemo } from 'react'
import {
  useReactTable,
  getCoreRowModel,
  flexRender,
  createColumnHelper,
  type RowSelectionState,
  type OnChangeFn,
} from '@tanstack/react-table'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Badge } from '@/components/ui/badge'
import {
  ArrowUpDown,
  ArrowUp,
  ArrowDown,
  MoreHorizontal,
  Eye,
  RefreshCw,
  Trash2,
  Copy,
  ExternalLink,
  MessageSquare,
  Phone,
  Package,
  ShoppingCart,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { StatusBadge } from './status-badge'
import { truncateContent, formatCompactDateTime } from './utils'
import type { RawMessage, SortField, SortOrder } from './types'

const columnHelper = createColumnHelper<RawMessage>()

interface MessageTableProps {
  messages: RawMessage[]
  sortBy: SortField
  sortOrder: SortOrder
  onSort: (field: SortField) => void
  onViewDetailsClick?: (message: RawMessage) => void
  onViewItemsClick?: (message: RawMessage) => void
  selectedId?: string
  // Selection props
  rowSelection?: RowSelectionState
  onRowSelectionChange?: OnChangeFn<RowSelectionState>
  // Action props
  onReprocess?: (message: RawMessage) => void
  onDelete?: (message: RawMessage) => void
  onCopyContent?: (message: RawMessage) => void
  onViewGroup?: (message: RawMessage) => void
  onReply?: (message: RawMessage) => void
}

export function MessageTable({
  messages,
  sortBy,
  sortOrder,
  onSort,
  onViewDetailsClick,
  onViewItemsClick,
  selectedId,
  rowSelection = {},
  onRowSelectionChange,
  onReprocess,
  onDelete,
  onCopyContent,
  onViewGroup,
  onReply,
}: MessageTableProps) {
  const hasSelection = !!onRowSelectionChange
  const hasActions =
    onReprocess || onDelete || onCopyContent || onViewGroup || onReply

  // Sort icon component
  const SortIcon = ({ field }: { field: SortField }) => {
    if (sortBy !== field) {
      return <ArrowUpDown className="w-3 h-3 opacity-40" />
    }
    return sortOrder === 'asc' ? (
      <ArrowUp className="w-3 h-3" />
    ) : (
      <ArrowDown className="w-3 h-3" />
    )
  }

  // Define columns
  const columns = useMemo(
    () => [
      // Selection column
      ...(hasSelection
        ? [
            columnHelper.display({
              id: 'select',
              header: ({ table }) => (
                <Checkbox
                  checked={
                    table.getIsAllPageRowsSelected() ||
                    (table.getIsSomePageRowsSelected() && 'indeterminate')
                  }
                  onCheckedChange={(value) =>
                    table.toggleAllPageRowsSelected(!!value)
                  }
                  aria-label="Select all"
                />
              ),
              cell: ({ row }) => (
                <Checkbox
                  checked={row.getIsSelected()}
                  onCheckedChange={(value) => row.toggleSelected(!!value)}
                  aria-label="Select row"
                  onClick={(e) => e.stopPropagation()}
                />
              ),
              size: 40,
            }),
          ]
        : []),

      // Timestamp column
      columnHelper.accessor('timestamp', {
        header: () => (
          <button
            onClick={() => onSort('timestamp')}
            className="flex items-center gap-1 hover:text-foreground transition-colors"
          >
            Time
            <SortIcon field="timestamp" />
          </button>
        ),
        cell: (info) => (
          <Tooltip>
            <TooltipTrigger className="text-xs text-muted-foreground tabular-nums">
              {formatCompactDateTime(info.getValue())}
            </TooltipTrigger>
            <TooltipContent>
              {new Date(info.getValue()).toLocaleString()}
            </TooltipContent>
          </Tooltip>
        ),
        size: 130,
      }),

      // Content column
      columnHelper.accessor('content', {
        header: 'Content',
        cell: (info) => {
          const content = info.getValue()
          return (
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="text-sm line-clamp-1">
                  {truncateContent(content, 60)}
                </span>
              </TooltipTrigger>
              <TooltipContent
                side="bottom"
                className="max-w-md whitespace-pre-wrap"
              >
                {content.slice(0, 300)}
                {content.length > 300 && '...'}
              </TooltipContent>
            </Tooltip>
          )
        },
      }),

      // Group column
      columnHelper.accessor('groupName', {
        header: 'Group',
        cell: (info) => {
          const groupName = info.getValue()
          const groupJid = info.row.original.groupJid
          return (
            <span className="text-xs text-muted-foreground line-clamp-1">
              {groupName || groupJid?.slice(0, 12)}
            </span>
          )
        },
        size: 140,
      }),

      // Sender column with phone badge
      columnHelper.accessor('participantName', {
        header: 'Sender',
        cell: (info) => {
          const name = info.getValue()
          const jid = info.row.original.participantJid
          const phone = jid?.split('@')[0]

          return (
            <div className="flex flex-col gap-0.5">
              <span className="text-xs text-foreground line-clamp-1">
                {name || 'Unknown'}
              </span>
              {phone && (
                <Badge
                  variant="secondary"
                  className="w-fit px-1.5 py-0 text-[10px] font-normal gap-1"
                >
                  <Phone className="w-2.5 h-2.5" />
                  {phone}
                </Badge>
              )}
            </div>
          )
        },
        size: 150,
      }),

      // Processed Items column
      columnHelper.display({
        id: 'processedItems',
        header: 'Items',
        cell: ({ row }) => {
          const message = row.original
          const offerCount = message.offerCount ?? 0
          const requestCount = message.requestCount ?? 0
          const hasItems = offerCount > 0 || requestCount > 0

          if (!hasItems) {
            return (
              <span className="text-xs text-muted-foreground">—</span>
            )
          }

          return (
            <div
              className="flex flex-wrap gap-1 cursor-pointer"
              onClick={(e) => {
                e.stopPropagation()
                onViewItemsClick?.(message)
              }}
            >
              {offerCount > 0 && (
                <Badge
                  variant="secondary"
                  className="px-1.5 py-0 text-[10px] font-normal gap-1 bg-emerald-500/10 text-emerald-600 hover:bg-emerald-500/20"
                >
                  <Package className="w-2.5 h-2.5" />
                  {offerCount}
                </Badge>
              )}
              {requestCount > 0 && (
                <Badge
                  variant="secondary"
                  className="px-1.5 py-0 text-[10px] font-normal gap-1 bg-blue-500/10 text-blue-600 hover:bg-blue-500/20"
                >
                  <ShoppingCart className="w-2.5 h-2.5" />
                  {requestCount}
                </Badge>
              )}
            </div>
          )
        },
        size: 100,
      }),

      // Status column
      columnHelper.accessor('processedAt', {
        header: () => (
          <button
            onClick={() => onSort('processed_at')}
            className="flex items-center gap-1 hover:text-foreground transition-colors"
          >
            Status
            <SortIcon field="processed_at" />
          </button>
        ),
        cell: (info) => <StatusBadge message={info.row.original} compact />,
        size: 90,
      }),

      // Actions column
      ...(hasActions
        ? [
            columnHelper.display({
              id: 'actions',
              header: '',
              cell: ({ row }) => {
                const message = row.original
                return (
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-7 w-7"
                        onClick={(e) => e.stopPropagation()}
                      >
                        <MoreHorizontal className="h-4 w-4" />
                        <span className="sr-only">Actions</span>
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end" className="w-[160px]">
                      {onViewDetailsClick && (
                        <DropdownMenuItem
                          onClick={() => onViewDetailsClick(message)}
                        >
                          <Eye className="mr-2 h-3.5 w-3.5" />
                          View Details
                        </DropdownMenuItem>
                      )}
                      {onReply && message.participantJid && (
                        <DropdownMenuItem onClick={() => onReply(message)}>
                          <MessageSquare className="mr-2 h-3.5 w-3.5" />
                          Reply to Sender
                        </DropdownMenuItem>
                      )}
                      {onCopyContent && (
                        <DropdownMenuItem
                          onClick={() => onCopyContent(message)}
                        >
                          <Copy className="mr-2 h-3.5 w-3.5" />
                          Copy Content
                        </DropdownMenuItem>
                      )}
                      {onViewGroup && (
                        <DropdownMenuItem onClick={() => onViewGroup(message)}>
                          <ExternalLink className="mr-2 h-3.5 w-3.5" />
                          View Group
                        </DropdownMenuItem>
                      )}
                      {(onReprocess || onDelete) && <DropdownMenuSeparator />}
                      {onReprocess && (
                        <DropdownMenuItem onClick={() => onReprocess(message)}>
                          <RefreshCw className="mr-2 h-3.5 w-3.5" />
                          Reprocess
                        </DropdownMenuItem>
                      )}
                      {onDelete && (
                        <DropdownMenuItem
                          onClick={() => onDelete(message)}
                          className="text-destructive focus:text-destructive"
                        >
                          <Trash2 className="mr-2 h-3.5 w-3.5" />
                          Delete
                        </DropdownMenuItem>
                      )}
                    </DropdownMenuContent>
                  </DropdownMenu>
                )
              },
              size: 50,
            }),
          ]
        : []),
    ],
    [
      hasSelection,
      hasActions,
      sortBy,
      sortOrder,
      onSort,
      onViewDetailsClick,
      onViewItemsClick,
      onReply,
      onCopyContent,
      onViewGroup,
      onReprocess,
      onDelete,
    ],
  )

  // Create table instance
  const table = useReactTable({
    data: messages,
    columns,
    getCoreRowModel: getCoreRowModel(),
    getRowId: (row) => row.id,
    state: {
      rowSelection,
    },
    onRowSelectionChange,
    enableRowSelection: hasSelection,
  })

  return (
    <Table>
      <TableHeader>
        {table.getHeaderGroups().map((headerGroup) => (
          <TableRow key={headerGroup.id} className="hover:bg-transparent">
            {headerGroup.headers.map((header) => (
              <TableHead
                key={header.id}
                style={{
                  width:
                    header.getSize() !== 150 ? header.getSize() : undefined,
                }}
              >
                {header.isPlaceholder
                  ? null
                  : flexRender(
                      header.column.columnDef.header,
                      header.getContext(),
                    )}
              </TableHead>
            ))}
          </TableRow>
        ))}
      </TableHeader>
      <TableBody>
        {table.getRowModel().rows.length === 0 ? (
          <TableRow>
            <TableCell colSpan={columns.length} className="h-24 text-center">
              No messages found.
            </TableCell>
          </TableRow>
        ) : (
          table.getRowModel().rows.map((row) => (
            <TableRow
              key={row.id}
              className={cn(
                selectedId === row.original.id && 'bg-muted/50',
                row.getIsSelected() && 'bg-primary/5',
              )}
              data-state={row.getIsSelected() ? 'selected' : undefined}
            >
              {row.getVisibleCells().map((cell) => (
                <TableCell key={cell.id}>
                  {flexRender(cell.column.columnDef.cell, cell.getContext())}
                </TableCell>
              ))}
            </TableRow>
          ))
        )}
      </TableBody>
    </Table>
  )
}
