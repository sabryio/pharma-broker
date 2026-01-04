// Pagination Bar Component for Raw Messages
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight } from 'lucide-react'
import type { PaginationState } from './types'

interface PaginationBarProps {
  pagination: PaginationState
  onPaginationChange: (pagination: PaginationState) => void
  totalCount: number
  totalPages: number
  currentPage: number
  canGoPrev: boolean
  canGoNext: boolean
}

export function PaginationBar({
  pagination,
  onPaginationChange,
  totalCount,
  totalPages,
  currentPage,
  canGoPrev,
  canGoNext,
}: PaginationBarProps) {
  const startItem = pagination.pageIndex * pagination.pageSize + 1
  const endItem = Math.min(
    (pagination.pageIndex + 1) * pagination.pageSize,
    totalCount,
  )

  return (
    <div className="flex items-center justify-between px-3 py-2 border-t bg-muted/20">
      <div className="flex items-center gap-2">
        <span className="text-xs text-muted-foreground">Rows:</span>
        <Select
          value={String(pagination.pageSize)}
          onValueChange={(value) =>
            onPaginationChange({ pageIndex: 0, pageSize: Number(value) })
          }
        >
          <SelectTrigger className="h-7 w-[70px] text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="10">10</SelectItem>
            <SelectItem value="25">25</SelectItem>
            <SelectItem value="50">50</SelectItem>
            <SelectItem value="100">100</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div className="flex items-center gap-1">
        <span className="text-xs text-muted-foreground tabular-nums mr-2">
          {startItem}-{endItem} of {totalCount.toLocaleString()}
        </span>

        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={() => onPaginationChange({ ...pagination, pageIndex: 0 })}
          disabled={!canGoPrev}
        >
          <ChevronsLeft className="w-3.5 h-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={() =>
            onPaginationChange({
              ...pagination,
              pageIndex: pagination.pageIndex - 1,
            })
          }
          disabled={!canGoPrev}
        >
          <ChevronLeft className="w-3.5 h-3.5" />
        </Button>

        <span className="text-xs text-muted-foreground px-2 tabular-nums">
          {currentPage} / {totalPages}
        </span>

        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={() =>
            onPaginationChange({
              ...pagination,
              pageIndex: pagination.pageIndex + 1,
            })
          }
          disabled={!canGoNext}
        >
          <ChevronRight className="w-3.5 h-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={() =>
            onPaginationChange({ ...pagination, pageIndex: totalPages - 1 })
          }
          disabled={!canGoNext}
        >
          <ChevronsRight className="w-3.5 h-3.5" />
        </Button>
      </div>
    </div>
  )
}
