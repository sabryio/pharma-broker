// Raw Messages Utility Functions

/**
 * Truncates content to a maximum length with ellipsis
 * Property 1: Content Truncation - ensures content.length <= maxLength + 3
 */
export function truncateContent(
  content: string,
  maxLength: number = 100,
): string {
  if (content.length <= maxLength) {
    return content
  }
  return content.slice(0, maxLength) + '...'
}

/**
 * Calculates pagination metadata
 * Property 2 & 3: Pagination calculations
 */
export function calculatePagination(
  total: number,
  pageSize: number,
  offset: number,
) {
  const totalPages = Math.ceil(total / pageSize) || 1
  const currentPage = Math.floor(offset / pageSize) + 1
  return { totalPages, currentPage }
}

export function calculateCanGoNext(
  total: number,
  limit: number,
  offset: number,
): boolean {
  return offset + limit < total
}

/**
 * Formats timestamp to relative time string
 */
export function formatRelativeTime(timestamp: string): string {
  const date = new Date(timestamp)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffMins = Math.floor(diffMs / 60000)
  const diffHours = Math.floor(diffMs / 3600000)
  const diffDays = Math.floor(diffMs / 86400000)

  if (diffMins < 1) return 'Just now'
  if (diffMins < 60) return `${diffMins}m ago`
  if (diffHours < 24) return `${diffHours}h ago`
  if (diffDays < 7) return `${diffDays}d ago`
  return date.toLocaleDateString()
}

/**
 * Formats timestamp to compact datetime string
 */
export function formatCompactDateTime(timestamp: string): string {
  const date = new Date(timestamp)
  return date.toLocaleString('en-US', {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}
