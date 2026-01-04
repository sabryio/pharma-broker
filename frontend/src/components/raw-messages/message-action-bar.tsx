// Message Action Bar Component
// Floating action bar specific to raw messages bulk operations

import { RefreshCw, Trash2, Download, CheckCircle } from 'lucide-react'
import {
  FloatingActionBar,
  type ActionConfig,
} from '@/components/ui/floating-action-bar'

interface MessageActionBarProps {
  selectedCount: number
  totalCount: number
  onSelectAll: () => void
  onClearSelection: () => void
  isAllSelected: boolean
  onBulkReprocess: () => void
  onBulkDelete: () => void
  onBulkExport?: () => void
  onBulkMarkProcessed?: () => void
  loading?: boolean
}

export function MessageActionBar({
  selectedCount,
  totalCount,
  onSelectAll,
  onClearSelection,
  isAllSelected,
  onBulkReprocess,
  onBulkDelete,
  onBulkExport,
  onBulkMarkProcessed,
  loading = false,
}: MessageActionBarProps) {
  const actions: ActionConfig[] = [
    {
      id: 'reprocess',
      label: 'Reprocess',
      icon: <RefreshCw className="w-4 h-4" />,
      variant: 'warning',
      shortcut: 'R',
      onClick: onBulkReprocess,
    },
    ...(onBulkMarkProcessed
      ? [
          {
            id: 'mark-processed',
            label: 'Mark Processed',
            icon: <CheckCircle className="w-4 h-4" />,
            variant: 'success' as const,
            shortcut: 'P',
            onClick: onBulkMarkProcessed,
          },
        ]
      : []),
    ...(onBulkExport
      ? [
          {
            id: 'export',
            label: 'Export',
            icon: <Download className="w-4 h-4" />,
            variant: 'default' as const,
            shortcut: 'E',
            onClick: onBulkExport,
          },
        ]
      : []),
    {
      id: 'delete',
      label: 'Delete',
      icon: <Trash2 className="w-4 h-4" />,
      variant: 'destructive',
      shortcut: '⌫',
      onClick: onBulkDelete,
    },
  ]

  return (
    <FloatingActionBar
      selectedCount={selectedCount}
      totalCount={totalCount}
      actions={actions}
      onSelectAll={onSelectAll}
      onClearSelection={onClearSelection}
      isAllSelected={isAllSelected}
      loading={loading}
      position="bottom"
      showKeyboardHints
    />
  )
}
