// Message Table Component for Raw Messages
import { useMemo } from 'react'
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
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { StatusBadge } from './status-badge'
import { truncateContent, formatCompactDateTime } from './utils'
import type { RawMessage, SortField, SortOrder } from './types'

interface MessageTableProps {
  messages: RawMessage[]
  sortBy: SortField
  sortOrder: SortOrder
  onSort: (field: SortField) => void
  onRowClick: (message: RawMessage) => void
  selectedId?: string
  // Selection props
  selectedIds?: Set<string>
  onToggleSelect?: (id: string) => void
  onSelectAll?: () => void
  isAllSelected?: boolean
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
  onRowClick,
  selectedId,
  selectedIds,
  onToggleSelect,
  onSelectAll,
  isAllSelected,
  onReprocess,
  onDelete,
  onCopyContent,
  onViewGroup,
  onReply,
}: MessageTableProps) {
  const hasSelection = !!selectedIds && !!onToggleSelect
  const hasActions = onReprocess || onDelete || onCopyContent || onViewGroup || onReply

  const SortIcon = useMemo(() => {
    return ({ field }: { field: SortField }) => {
      if (sortBy !== field) {
        return <ArrowUpDown className="w-3 h-3 opacity-40" />
      }
      return sortOrder === 'asc' ? (
        <ArrowUp className="w-3 h-3" />
      ) : (
        <ArrowDown className="w-3 h-3" />
      )
    }
  }, [sortBy, sortOrder])

  return (
    <Table>
      <TableHeader>
        <TableRow className="hover:bg-transparent">
          {/* Checkbox Column */}
          {hasSelection && (
            <TableHead className="w-[40px]">
              <Checkbox
                checked={isAllSelected}
                onCheckedChange={() => onSelectAll?.()}
                aria-label="Select all"
              />
            </TableHead>
          )}
          <TableHead className="w-[130px]">
            <button
              onClick={() => onSort('timestamp')}
              className="flex items-center gap-1 hover:text-foreground transition-colors"
            >
              Time
              <SortIcon field="timestamp" />
            </button>
          </TableHead>
          <TableHead>Content</TableHead>
          <TableHead className="w-[140px]">Group</TableHead>
          <TableHead className="w-[140px]">Sender</TableHead>
          <TableHead className="w-[90px]">
            <button
              onClick={() => onSort('processed_at')}
              className="flex items-center gap-1 hover:text-foreground transition-colors"
            >
              Status
              <SortIcon field="processed_at" />
            </button>
          </TableHead>
          {/* Actions Column */}
          {hasActions && <TableHead className="w-[50px]" />}
        </TableRow>
      </TableHeader>
      <TableBody>
        {messages.map((message) => {
          const isSelected = selectedIds?.has(message.id)

          return (
            <TableRow
              key={message.id}
              onClick={() => onRowClick(message)}
              className={cn(
                'cursor-pointer',
                selectedId === message.id && 'bg-muted/50',
                isSelected && 'bg-primary/5',
              )}
            >
              {/* Checkbox */}
              {hasSelection && (
                <TableCell onClick={(e) => e.stopPropagation()}>
                  <Checkbox
                    checked={isSelected}
                    onCheckedChange={() => onToggleSelect?.(message.id)}
                    aria-label={`Select message ${message.id}`}
                  />
                </TableCell>
              )}

              {/* Timestamp */}
              <TableCell className="text-xs text-muted-foreground tabular-nums">
                <Tooltip>
                  <TooltipTrigger>
                    {formatCompactDateTime(message.timestamp)}
                  </TooltipTrigger>
                  <TooltipContent>
                    {new Date(message.timestamp).toLocaleString()}
                  </TooltipContent>
                </Tooltip>
              </TableCell>

              {/* Content */}
              <TableCell>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <span className="text-sm line-clamp-1">
                      {truncateContent(message.content, 60)}
                    </span>
                  </TooltipTrigger>
                  <TooltipContent
                    side="bottom"
                    className="max-w-md whitespace-pre-wrap"
                  >
                    {message.content.slice(0, 300)}
                    {message.content.length > 300 && '...'}
                  </TooltipContent>
                </Tooltip>
              </TableCell>

              {/* Group */}
              <TableCell className="text-xs text-muted-foreground">
                <span className="line-clamp-1">
                  {message.groupName || message.groupJid?.slice(0, 12)}
                </span>
              </TableCell>

              {/* Sender */}
              <TableCell>
                <div className="flex flex-col gap-0.5">
                  <span className="text-xs text-foreground line-clamp-1">
                    {message.participantName || 'Unknown'}
                  </span>
                  {message.participantJid && (
                    <Badge
                      variant="secondary"
                      className="w-fit px-1.5 py-0 text-[10px] font-normal gap-1"
                    >
                      <Phone className="w-2.5 h-2.5" />
                      {message.participantJid.split('@')[0]}
                    </Badge>
                  )}
                </div>
              </TableCell>

              {/* Status */}
              <TableCell>
                <StatusBadge message={message} compact />
              </TableCell>

              {/* Actions */}
              {hasActions && (
                <TableCell onClick={(e) => e.stopPropagation()}>
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-7 w-7"
                      >
                        <MoreHorizontal className="h-4 w-4" />
                        <span className="sr-only">Actions</span>
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end" className="w-[160px]">
                      <DropdownMenuItem onClick={() => onRowClick(message)}>
                        <Eye className="mr-2 h-3.5 w-3.5" />
                        View Details
                      </DropdownMenuItem>
                      {onReply && message.participantJid && (
                        <DropdownMenuItem onClick={() => onReply(message)}>
                          <MessageSquare className="mr-2 h-3.5 w-3.5" />
                          Reply to Sender
                        </DropdownMenuItem>
                      )}
                      {onCopyContent && (
                        <DropdownMenuItem onClick={() => onCopyContent(message)}>
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
                </TableCell>
              )}
            </TableRow>
          )
        })}
      </TableBody>
    </Table>
  )
}
