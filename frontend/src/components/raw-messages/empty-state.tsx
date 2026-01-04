// Empty State Component for Raw Messages
import { Button } from '@/components/ui/button'
import { MessageSquare, Search, AlertCircle, RefreshCw } from 'lucide-react'

interface EmptyStateProps {
  type: 'no-data' | 'no-results' | 'error'
  errorMessage?: string
  onClearFilters?: () => void
  onRetry?: () => void
}

export function EmptyState({
  type,
  errorMessage,
  onClearFilters,
  onRetry,
}: EmptyStateProps) {
  if (type === 'error') {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-center">
        <div className="w-10 h-10 rounded-full bg-destructive/10 flex items-center justify-center mb-3">
          <AlertCircle className="w-5 h-5 text-destructive" />
        </div>
        <p className="text-sm font-medium mb-1">Failed to load messages</p>
        <p className="text-xs text-muted-foreground mb-4">
          {errorMessage || 'An unexpected error occurred'}
        </p>
        {onRetry && (
          <Button variant="outline" size="sm" onClick={onRetry}>
            <RefreshCw className="w-3.5 h-3.5 mr-1.5" />
            Try Again
          </Button>
        )}
      </div>
    )
  }

  if (type === 'no-results') {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-center">
        <div className="w-10 h-10 rounded-full bg-muted flex items-center justify-center mb-3">
          <Search className="w-5 h-5 text-muted-foreground" />
        </div>
        <p className="text-sm font-medium mb-1">No matching messages</p>
        <p className="text-xs text-muted-foreground mb-4">
          Try adjusting your filters
        </p>
        {onClearFilters && (
          <Button variant="outline" size="sm" onClick={onClearFilters}>
            Clear Filters
          </Button>
        )}
      </div>
    )
  }

  return (
    <div className="flex flex-col items-center justify-center py-12 text-center">
      <div className="w-10 h-10 rounded-full bg-muted flex items-center justify-center mb-3">
        <MessageSquare className="w-5 h-5 text-muted-foreground" />
      </div>
      <p className="text-sm font-medium mb-1">No messages yet</p>
      <p className="text-xs text-muted-foreground">
        Messages will appear here when received
      </p>
    </div>
  )
}
