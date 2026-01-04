/**
 * Export utilities for raw messages
 * Provides CSV export functionality with proper formatting and download
 */

import type { RawMessage } from '@/schema/raw-message'

// =============================================================================
// Types
// =============================================================================

export interface ExportOptions {
  /** Messages to export */
  messages: RawMessage[]
  /** Optional custom filename (without extension) */
  filename?: string
  /** Whether to include all fields or just essential ones */
  includeAllFields?: boolean
}

export interface CSVColumn {
  header: string
  accessor: (msg: RawMessage) => string
}

// =============================================================================
// CSV Column Definitions
// =============================================================================

/**
 * Essential columns for basic export
 */
const ESSENTIAL_COLUMNS: CSVColumn[] = [
  { header: 'ID', accessor: (msg) => msg.id },
  { header: 'Content', accessor: (msg) => msg.content },
  { header: 'Status', accessor: (msg) => msg.status },
  { header: 'Timestamp', accessor: (msg) => msg.timestamp },
  { header: 'Group Name', accessor: (msg) => msg.groupName ?? '' },
  { header: 'Participant Name', accessor: (msg) => msg.participantName ?? '' },
]

/**
 * All columns for comprehensive export
 */
const ALL_COLUMNS: CSVColumn[] = [
  { header: 'ID', accessor: (msg) => msg.id },
  { header: 'External ID', accessor: (msg) => msg.externalId ?? '' },
  { header: 'Content', accessor: (msg) => msg.content },
  { header: 'Status', accessor: (msg) => msg.status },
  { header: 'Timestamp', accessor: (msg) => msg.timestamp },
  { header: 'Processed At', accessor: (msg) => msg.processedAt ?? '' },
  { header: 'Error', accessor: (msg) => msg.error ?? '' },
  { header: 'Created At', accessor: (msg) => msg.createdAt },
  { header: 'Group ID', accessor: (msg) => msg.groupId },
  { header: 'Group Name', accessor: (msg) => msg.groupName ?? '' },
  { header: 'Group JID', accessor: (msg) => msg.groupJid ?? '' },
  { header: 'Participant ID', accessor: (msg) => msg.participantId },
  { header: 'Participant Name', accessor: (msg) => msg.participantName ?? '' },
  { header: 'Participant JID', accessor: (msg) => msg.participantJid ?? '' },
  { header: 'Reply To ID', accessor: (msg) => msg.replyToId ?? '' },
  { header: 'Reply To Content', accessor: (msg) => msg.replyToContent ?? '' },
  { header: 'Reply To Sender', accessor: (msg) => msg.replyToSender ?? '' },
]

// =============================================================================
// CSV Generation
// =============================================================================

/**
 * Escape a value for CSV format
 * Handles quotes, commas, and newlines
 */
export function escapeCSVValue(value: string): string {
  // If value contains comma, quote, or newline, wrap in quotes and escape internal quotes
  if (
    value.includes(',') ||
    value.includes('"') ||
    value.includes('\n') ||
    value.includes('\r')
  ) {
    return `"${value.replace(/"/g, '""')}"`
  }
  return value
}

/**
 * Generate CSV content from messages
 */
export function generateCSVContent(
  messages: RawMessage[],
  includeAllFields = false,
): string {
  const columns = includeAllFields ? ALL_COLUMNS : ESSENTIAL_COLUMNS

  // Generate header row
  const headerRow = columns.map((col) => escapeCSVValue(col.header)).join(',')

  // Generate data rows
  const dataRows = messages.map((msg) =>
    columns.map((col) => escapeCSVValue(col.accessor(msg))).join(','),
  )

  return [headerRow, ...dataRows].join('\n')
}

// =============================================================================
// File Download
// =============================================================================

/**
 * Generate a timestamp string for filenames
 * Format: YYYY-MM-DD_HH-mm-ss
 */
export function generateTimestamp(): string {
  const now = new Date()
  const year = now.getFullYear()
  const month = String(now.getMonth() + 1).padStart(2, '0')
  const day = String(now.getDate()).padStart(2, '0')
  const hours = String(now.getHours()).padStart(2, '0')
  const minutes = String(now.getMinutes()).padStart(2, '0')
  const seconds = String(now.getSeconds()).padStart(2, '0')

  return `${year}-${month}-${day}_${hours}-${minutes}-${seconds}`
}

/**
 * Trigger a file download in the browser
 */
export function downloadFile(
  content: string,
  filename: string,
  mimeType: string,
): void {
  const blob = new Blob([content], { type: mimeType })
  const url = URL.createObjectURL(blob)

  const link = document.createElement('a')
  link.href = url
  link.download = filename
  link.style.display = 'none'

  document.body.appendChild(link)
  link.click()

  // Cleanup
  document.body.removeChild(link)
  URL.revokeObjectURL(url)
}

// =============================================================================
// Main Export Function
// =============================================================================

/**
 * Export messages to CSV and trigger download
 */
export function exportToCSV(options: ExportOptions): void {
  const { messages, filename, includeAllFields = false } = options

  if (messages.length === 0) {
    throw new Error('No messages to export')
  }

  // Generate CSV content
  const csvContent = generateCSVContent(messages, includeAllFields)

  // Generate filename with timestamp
  const timestamp = generateTimestamp()
  const finalFilename = filename
    ? `${filename}_${timestamp}.csv`
    : `raw-messages_${timestamp}.csv`

  // Trigger download
  downloadFile(csvContent, finalFilename, 'text/csv;charset=utf-8')
}

/**
 * Export selected messages to CSV
 * Convenience function for bulk export action
 */
export function exportSelectedToCSV(
  messages: RawMessage[],
  selectedIds: Set<string>,
): void {
  const selectedMessages = messages.filter((msg) => selectedIds.has(msg.id))

  if (selectedMessages.length === 0) {
    throw new Error('No messages selected for export')
  }

  exportToCSV({
    messages: selectedMessages,
    filename: `raw-messages-selected`,
    includeAllFields: true,
  })
}
