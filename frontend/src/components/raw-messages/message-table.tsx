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
import { ArrowUpDown, ArrowUp, ArrowDown } from 'lucide-react'
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
}

export function MessageTable({
  messages,
  sortBy,
  sortOrder,
  onSort,
  onRowClick,
  selectedId,
}: MessageTableProps) {
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
          <TableHead className="w-[140px]">
            <button
              onClick={() => onSort('timestamp')}
              className="flex items-center gap-1 hover:text-foreground transition-colors"
            >
              Time
              <SortIcon field="timestamp" />
            </button>
          </TableHead>
          <TableHead>Content</TableHead>
          <TableHead className="w-[160px]">Group</TableHead>
          <TableHead className="w-[140px]">Sender</TableHead>
          <TableHead className="w-[100px]">
            <button
              onClick={() => onSort('processed_at')}
              className="flex items-center gap-1 hover:text-foreground transition-colors"
            >
              Status
              <SortIcon field="processed_at" />
            </button>
          </TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {messages.map((message) => (
          <TableRow
            key={message.id}
            onClick={() => onRowClick(message)}
            className={cn(
              'cursor-pointer',
              selectedId === message.id && 'bg-muted/50',
            )}
          >
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
            <TableCell>
              <Tooltip>
                <TooltipTrigger asChild>
                  <span className="text-sm line-clamp-1">
                    {truncateContent(message.content, 80)}
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
            <TableCell className="text-xs text-muted-foreground">
              <span className="line-clamp-1">
                {message.groupName || message.groupJid}
              </span>
            </TableCell>
            <TableCell className="text-xs text-muted-foreground">
              <span className="line-clamp-1">
                {message.participantName || message.participantJid}
              </span>
            </TableCell>
            <TableCell>
              <StatusBadge message={message} compact />
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}
