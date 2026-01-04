// Recording Export/Import Hook
// Provides functionality for exporting and importing debug recordings
//
// Feature: debug-recording-enhancement
// Implements: Requirements 7.1, 7.2, 7.4, 7.5

import { useState, useCallback } from 'react'
import { useAppSelector, useAppDispatch } from '@/store/hooks'
import {
  recordingsActions,
  selectRecordingsMap,
} from '@/store/slices/recordingsSlice'
import type { AuditRecordDetail } from '@/hooks/use-audit-records'
import {
  createExportedRecording,
  downloadRecordingAsJson,
  readRecordingFromFile,
  restoreMatchRecording,
  validateImportedRecording,
  type ExportedRecording,
  type ImportValidationResult,
  type ExportMetadata,
} from '@/schema/recording-export'

const API_BASE = import.meta.env.VITE_API_URL || 'http://localhost:8081'

// =============================================================================
// Types
// =============================================================================

export interface ExportOptions {
  includeBackendRecord?: boolean
  metadata?: Partial<ExportMetadata>
}

export interface UseRecordingExportReturn {
  /** Export a single recording by match ID */
  exportRecording: (matchId: string, options?: ExportOptions) => Promise<void>
  /** Export multiple recordings */
  exportMultipleRecordings: (
    matchIds: string[],
    options?: ExportOptions,
  ) => Promise<void>
  /** Import a recording from a file */
  importFromFile: (file: File) => Promise<ImportValidationResult>
  /** Import a recording from JSON data */
  importFromJson: (data: unknown) => ImportValidationResult
  /** Whether an export is in progress */
  isExporting: boolean
  /** Whether an import is in progress */
  isImporting: boolean
  /** Last export error */
  exportError: string | null
  /** Last import error */
  importError: string | null
  /** Clear errors */
  clearErrors: () => void
}

// =============================================================================
// API Functions
// =============================================================================

/**
 * Fetch backend audit record for a match
 * Returns null if not found or on error
 */
async function fetchBackendAuditRecord(
  matchId: string,
): Promise<AuditRecordDetail | null> {
  try {
    const response = await fetch(`${API_BASE}/api/audit-records/${matchId}`)
    if (!response.ok) {
      if (response.status === 404) return null
      throw new Error(`Failed to fetch audit record: ${response.statusText}`)
    }
    return response.json()
  } catch (err) {
    console.warn(`Failed to fetch backend audit record for ${matchId}:`, err)
    return null
  }
}

// =============================================================================
// Hook Implementation
// =============================================================================

/**
 * Hook for exporting and importing debug recordings
 *
 * Implements: Requirements 7.1, 7.2, 7.4, 7.5
 *
 * @example
 * ```tsx
 * const { exportRecording, importFromFile, isExporting } = useRecordingExport()
 *
 * // Export a recording
 * await exportRecording('match-123', { includeBackendRecord: true })
 *
 * // Import from file
 * const result = await importFromFile(file)
 * if (result.valid) {
 *   console.log('Imported successfully')
 * } else {
 *   console.error('Validation errors:', result.errors)
 * }
 * ```
 */
export function useRecordingExport(): UseRecordingExportReturn {
  const dispatch = useAppDispatch()
  const recordings = useAppSelector(selectRecordingsMap)

  const [isExporting, setIsExporting] = useState(false)
  const [isImporting, setIsImporting] = useState(false)
  const [exportError, setExportError] = useState<string | null>(null)
  const [importError, setImportError] = useState<string | null>(null)

  const clearErrors = useCallback(() => {
    setExportError(null)
    setImportError(null)
  }, [])

  /**
   * Export a single recording by match ID
   * Fetches backend audit record if requested and combines with frontend data
   *
   * Implements: Requirements 7.1, 7.4
   */
  const exportRecording = useCallback(
    async (matchId: string, options: ExportOptions = {}): Promise<void> => {
      const { includeBackendRecord = true, metadata = {} } = options

      setIsExporting(true)
      setExportError(null)

      try {
        // Get frontend recording
        const frontendRecording = recordings[matchId]
        if (!frontendRecording) {
          throw new Error(`Recording not found for match ID: ${matchId}`)
        }

        // Fetch backend audit record if requested
        let backendRecord: AuditRecordDetail | null = null
        if (includeBackendRecord) {
          backendRecord = await fetchBackendAuditRecord(matchId)
        }

        // Create export data
        const exportData = createExportedRecording(
          frontendRecording,
          backendRecord,
          metadata,
        )

        // Download as JSON file
        downloadRecordingAsJson(exportData)
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Export failed'
        setExportError(message)
        throw err
      } finally {
        setIsExporting(false)
      }
    },
    [recordings],
  )

  /**
   * Export multiple recordings as a single archive
   *
   * Implements: Requirements 7.4
   */
  const exportMultipleRecordings = useCallback(
    async (matchIds: string[], options: ExportOptions = {}): Promise<void> => {
      const { includeBackendRecord = true, metadata = {} } = options

      setIsExporting(true)
      setExportError(null)

      try {
        const exports: ExportedRecording[] = []

        for (const matchId of matchIds) {
          const frontendRecording = recordings[matchId]
          if (!frontendRecording) continue

          let backendRecord: AuditRecordDetail | null = null
          if (includeBackendRecord) {
            backendRecord = await fetchBackendAuditRecord(matchId)
          }

          exports.push(
            createExportedRecording(frontendRecording, backendRecord, metadata),
          )
        }

        if (exports.length === 0) {
          throw new Error('No recordings found to export')
        }

        // Create archive with metadata
        const archive = {
          version: '1.0.0',
          exportedAt: new Date().toISOString(),
          count: exports.length,
          recordings: exports,
        }

        // Download as JSON
        const json = JSON.stringify(archive, null, 2)
        const blob = new Blob([json], { type: 'application/json' })
        const url = URL.createObjectURL(blob)

        const link = document.createElement('a')
        link.href = url
        link.download = `recordings-archive-${Date.now()}.json`
        document.body.appendChild(link)
        link.click()
        document.body.removeChild(link)
        URL.revokeObjectURL(url)
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Export failed'
        setExportError(message)
        throw err
      } finally {
        setIsExporting(false)
      }
    },
    [recordings],
  )

  /**
   * Import a recording from a file
   * Validates the file content and restores to Redux store
   *
   * Implements: Requirements 7.2, 7.5
   */
  const importFromFile = useCallback(
    async (file: File): Promise<ImportValidationResult> => {
      setIsImporting(true)
      setImportError(null)

      try {
        // Read and validate file
        const result = await readRecordingFromFile(file)

        if (!result.valid || !result.data) {
          setImportError(result.errors[0]?.message ?? 'Invalid file format')
          return result
        }

        // Restore recording to Redux store
        const restoredRecording = restoreMatchRecording(result.data)
        dispatch(
          recordingsActions.importRecordings({
            [restoredRecording.id]: restoredRecording,
          }),
        )

        return result
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Import failed'
        setImportError(message)
        return {
          valid: false,
          errors: [{ field: 'file', message }],
        }
      } finally {
        setIsImporting(false)
      }
    },
    [dispatch],
  )

  /**
   * Import a recording from JSON data (already parsed)
   * Validates and restores to Redux store
   *
   * Implements: Requirements 7.2, 7.5
   */
  const importFromJson = useCallback(
    (data: unknown): ImportValidationResult => {
      setImportError(null)

      const result = validateImportedRecording(data)

      if (!result.valid || !result.data) {
        setImportError(result.errors[0]?.message ?? 'Invalid data format')
        return result
      }

      // Restore recording to Redux store
      const restoredRecording = restoreMatchRecording(result.data)
      dispatch(
        recordingsActions.importRecordings({
          [restoredRecording.id]: restoredRecording,
        }),
      )

      return result
    },
    [dispatch],
  )

  return {
    exportRecording,
    exportMultipleRecordings,
    importFromFile,
    importFromJson,
    isExporting,
    isImporting,
    exportError,
    importError,
    clearErrors,
  }
}
