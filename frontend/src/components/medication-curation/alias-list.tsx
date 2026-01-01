import React from 'react'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  ChevronLeft,
  ChevronRight,
  Hash,
  Calendar,
  Sparkles,
  Check,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import type { MedicationAlias } from '@/schema/curation'

interface AliasListProps {
  aliases: MedicationAlias[]
  selectedId: string | null
  onSelect: (alias: MedicationAlias) => void
  total: number
  pageSize: number
  currentPage: number
  onPageChange: (page: number) => void
  isBulkMode?: boolean
  bulkSelectedIds?: Set<string>
  onToggleBulk?: (id: string) => void
}

export const AliasList: React.FC<AliasListProps> = ({
  aliases,
  selectedId,
  onSelect,
  total,
  pageSize,
  currentPage,
  onPageChange,
  isBulkMode,
  bulkSelectedIds,
  onToggleBulk,
}) => {
  const totalPages = Math.ceil(total / pageSize)

  return (
    <div className="space-y-4">
      <div className="rounded-xl border border-white/5 bg-black/20 overflow-hidden">
        <Table>
          <TableHeader className="bg-white/2">
            <TableRow className="border-white/5 hover:bg-transparent">
              {isBulkMode && (
                <TableHead className="w-[40px] px-2 text-center">
                  <div className="w-4 h-4 rounded border border-white/20 mx-auto opacity-40" />
                </TableHead>
              )}
              <TableHead className="text-xs font-bold uppercase tracking-wider text-muted-foreground">
                Alias Name
              </TableHead>
              <TableHead className="text-xs font-bold uppercase tracking-wider text-muted-foreground w-[100px]">
                Count
              </TableHead>
              <TableHead className="text-xs font-bold uppercase tracking-wider text-muted-foreground w-[150px]">
                Confidence
              </TableHead>
              <TableHead className="text-xs font-bold uppercase tracking-wider text-muted-foreground w-[150px]">
                First Seen
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {aliases.length > 0 ? (
              aliases.map((alias) => (
                <TableRow
                  key={alias.id}
                  className={cn(
                    'border-white/5 cursor-pointer transition-colors',
                    selectedId === alias.id
                      ? 'bg-teal/10 hover:bg-teal/20'
                      : 'hover:bg-white/2',
                  )}
                  onClick={() =>
                    isBulkMode ? onToggleBulk?.(alias.id) : onSelect(alias)
                  }
                >
                  {isBulkMode && (
                    <TableCell className="px-2 text-center">
                      <div
                        className={cn(
                          'w-4 h-4 rounded border transition-all mx-auto flex items-center justify-center',
                          bulkSelectedIds?.has(alias.id)
                            ? 'bg-teal border-teal'
                            : 'border-white/20 bg-white/5',
                        )}
                      >
                        {bulkSelectedIds?.has(alias.id) && (
                          <Check className="w-3 h-3 text-white" />
                        )}
                      </div>
                    </TableCell>
                  )}
                  <TableCell className="font-medium">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-mono">
                        {alias.aliasName}
                      </span>
                      {alias.curationStatus === 'Approved' && (
                        <Badge className="bg-emerald/20 text-emerald border-none h-4 px-1.5 text-[9px]">
                          Curated
                        </Badge>
                      )}
                    </div>
                  </TableCell>
                  <TableCell>
                    <div className="flex items-center gap-1.5 text-muted-foreground">
                      <Hash className="w-3 h-3" />
                      <span className="text-xs font-mono">
                        {alias.occurrenceCount}
                      </span>
                    </div>
                  </TableCell>
                  <TableCell>
                    {alias.aiSuggestionConfidence ? (
                      <div className="flex items-center gap-1.5">
                        <Sparkles
                          className={cn(
                            'w-3 h-3',
                            alias.aiSuggestionConfidence > 0.9
                              ? 'text-teal'
                              : 'text-amber-400',
                          )}
                        />
                        <span className="text-xs font-mono">
                          {Math.round(alias.aiSuggestionConfidence * 100)}%
                        </span>
                      </div>
                    ) : (
                      <span className="text-xs text-muted-foreground italic">
                        N/A
                      </span>
                    )}
                  </TableCell>
                  <TableCell>
                    <div className="flex items-center gap-1.5 text-muted-foreground">
                      <Calendar className="w-3 h-3" />
                      <span className="text-[10px] font-mono whitespace-nowrap">
                        {alias.firstSeenAt
                          ? new Date(alias.firstSeenAt).toLocaleDateString()
                          : 'Jan 1, 2026'}
                      </span>
                    </div>
                  </TableCell>
                </TableRow>
              ))
            ) : (
              <TableRow>
                <TableCell
                  colSpan={4}
                  className="h-32 text-center text-muted-foreground italic"
                >
                  No pending aliases found
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </div>

      {totalPages > 1 && (
        <div className="flex items-center justify-between px-2">
          <p className="text-xs text-muted-foreground">
            Showing <span className="font-mono">{aliases.length}</span> of{' '}
            <span className="font-mono">{total}</span> aliases
          </p>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              className="h-8 w-8 p-0 bg-transparent border-white/10"
              disabled={currentPage === 0}
              onClick={() => onPageChange(currentPage - 1)}
            >
              <ChevronLeft className="h-4 w-4" />
            </Button>
            <span className="text-xs font-mono">
              {currentPage + 1} / {totalPages}
            </span>
            <Button
              variant="outline"
              size="sm"
              className="h-8 w-8 p-0 bg-transparent border-white/10"
              disabled={currentPage >= totalPages - 1}
              onClick={() => onPageChange(currentPage + 1)}
            >
              <ChevronRight className="h-4 w-4" />
            </Button>
          </div>
        </div>
      )}
    </div>
  )
}
