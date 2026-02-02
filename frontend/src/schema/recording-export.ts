// Recording Export/Import Schema
// Unified format for exporting and importing debug recordings with backend data
//
// Feature: debug-recording-enhancement
// Implements: Requirements 7.1, 7.2, 7.5

import { z } from 'zod'
import type { MatchRecording } from '@/components/debug-recordings/types'
import type { AuditRecordDetail } from '@/hooks/use-audit-records'

// =============================================================================
// Export Format Version
// =============================================================================

export const EXPORT_FORMAT_VERSION = '1.0.0'

// =============================================================================
// Replay Context Schema
// =============================================================================

export const ReplayContextSchema = z.object({
  offer: z.record(z.string(), z.unknown()),
  request: z.record(z.string(), z.unknown()),
  weights: z.record(z.string(), z.unknown()),
  config: z.record(z.string(), z.unknown()).nullable().optional(),
})

export type ReplayContext = z.infer<typeof ReplayContextSchema>

// =============================================================================
// Export Metadata Schema
// =============================================================================

export const ExportMetadataSchema = z.object({
  sessionId: z.string(),
  userAgent: z.string(),
  clientVersion: z.string(),
  exportedBy: z.string().optional(),
  tags: z.array(z.string()).optional(),
  notes: z.string().optional(),
})

export type ExportMetadata = z.infer<typeof ExportMetadataSchema>

// =============================================================================
// Recording Event Schema (for validation)
// =============================================================================

export const RecordingEventSchema = z.object({
  type: z.string(),
  label: z.string(),
  description: z.string().optional(),
  data: z.record(z.string(), z.unknown()).optional(),
})

// =============================================================================
// Recording Snapshot Schema (for validation)
// =============================================================================

export const RecordingSnapshotSchema = z.object({
  id: z.string(),
  timestamp: z.union([z.string(), z.date()]),
  matchReview: z.record(z.string(), z.unknown()),
  offer: z.object({
    id: z.string(),
    product: z.string(),
    medicationRaw: z.string().nullable().optional(),
    quantity: z.string().nullable(),
    price: z.string().nullable().optional(),
  }),
  request: z.object({
    id: z.string(),
    product: z.string(),
    medicationRaw: z.string().nullable().optional(),
    quantity: z.string().nullable(),
    maxPrice: z.string().nullable().optional(),
  }),
  confidence: z.number(),
  aiConfidence: z.number().nullable(),
  issues: z.array(z.string()),
  reasoning: z.string().nullable().optional(),
  adjustments: z.object({
    priceFlexibility: z.number(),
    quantityTolerance: z.number(),
    dosageStrictness: z.number(),
  }),
  event: RecordingEventSchema,
  metadata: z.object({
    userAgent: z.string().optional(),
    sessionId: z.string().optional(),
    previousSnapshotId: z.string().nullable().optional(),
    scoreBreakdown: z.record(z.string(), z.number().nullable()).optional(),
    weights: z.record(z.string(), z.number().nullable()).optional(),
  }),
})

// =============================================================================
// Frontend Recording Schema (for validation)
// =============================================================================

export const FrontendRecordingSchema = z.object({
  id: z.string(),
  matchId: z.string(),
  startedAt: z.union([z.string(), z.date()]),
  endedAt: z.union([z.string(), z.date()]).optional(),
  duration: z.number().optional(),
  outcome: z.enum(['approved', 'rejected', 'pending']).optional(),
  snapshots: z.array(RecordingSnapshotSchema),
})

// =============================================================================
// Backend Audit Record Schema (for validation)
// =============================================================================

export const BackendAuditRecordSchema = z.object({
  record: z.object({
    id: z.string(),
    matchId: z.string(),
    offerId: z.string(),
    requestId: z.string(),
    pipelineVersion: z.string(),
    offerSnapshot: z.record(z.string(), z.unknown()),
    requestSnapshot: z.record(z.string(), z.unknown()),
    weightsSnapshot: z.record(z.string(), z.unknown()),
    configSnapshot: z.record(z.string(), z.unknown()).nullable(),
    scoreBreakdown: z.record(z.string(), z.unknown()),
    finalScore: z.number(),
    pipelineStages: z.array(
      z.object({
        stage: z.string(),
        startedAt: z.string(),
        durationMs: z.number(),
        candidatesIn: z.number(),
        candidatesOut: z.number(),
        details: z.record(z.string(), z.unknown()).nullable(),
      }),
    ),
    aiInvolved: z.boolean(),
    aiRecord: z
      .object({
        model: z.string(),
        promptTokens: z.number().nullable(),
        completionTokens: z.number().nullable(),
        latencyMs: z.number(),
        response: z.record(z.string(), z.unknown()),
      })
      .nullable(),
    resolutionStage: z.string(),
    resolutionDetails: z.record(z.string(), z.unknown()).nullable(),
    totalLatencyMs: z.number(),
    createdAt: z.string(),
    reviewStatus: z.string().nullable(),
    reviewedBy: z.string().nullable(),
    reviewedAt: z.string().nullable(),
    reviewNotes: z.string().nullable(),
    sessionId: z.string().nullable(),
    clientMetadata: z.record(z.string(), z.unknown()).nullable(),
  }),
  replayContext: ReplayContextSchema.nullable(),
})

// =============================================================================
// Exported Recording Schema (Complete Export Format)
// =============================================================================

export const ExportedRecordingSchema = z.object({
  version: z.string(),
  exportedAt: z.string(),
  frontendRecording: FrontendRecordingSchema,
  backendRecord: BackendAuditRecordSchema.nullable(),
  replayContext: ReplayContextSchema.nullable(),
  metadata: ExportMetadataSchema,
})

export type ExportedRecording = z.infer<typeof ExportedRecordingSchema>

// =============================================================================
// Import Validation Result
// =============================================================================

export interface ImportValidationError {
  field: string
  message: string
  path?: string[]
}

export interface ImportValidationResult {
  valid: boolean
  errors: ImportValidationError[]
  data?: ExportedRecording
}

// =============================================================================
// Validation Functions
// =============================================================================

/**
 * Validate an imported recording against the schema
 * Returns detailed validation errors for invalid imports
 *
 * Implements: Requirements 7.2, 7.5
 */
export function validateImportedRecording(
  data: unknown,
): ImportValidationResult {
  const result = ExportedRecordingSchema.safeParse(data)

  if (result.success) {
    return {
      valid: true,
      errors: [],
      data: result.data,
    }
  }

  // Convert Zod errors to our format
  const errors: ImportValidationError[] = result.error.issues.map((issue) => ({
    field: issue.path.join('.') || 'root',
    message: issue.message,
    path: issue.path.map(String),
  }))

  return {
    valid: false,
    errors,
  }
}

// =============================================================================
// Export Helper Functions
// =============================================================================

/**
 * Create an ExportedRecording from frontend and backend data
 *
 * Implements: Requirements 7.1
 */
export function createExportedRecording(
  frontendRecording: MatchRecording,
  backendRecord: AuditRecordDetail | null,
  metadata: Partial<ExportMetadata>,
): ExportedRecording {
  // Build replay context from backend record if available
  const replayContext: ReplayContext | null = backendRecord?.replayContext
    ? {
        offer: backendRecord.replayContext.offer,
        request: backendRecord.replayContext.request,
        weights: backendRecord.replayContext.weights,
        config: null,
      }
    : null

  // Serialize dates to ISO strings for export
  const serializedRecording = {
    ...frontendRecording,
    startedAt:
      frontendRecording.startedAt instanceof Date
        ? frontendRecording.startedAt.toISOString()
        : frontendRecording.startedAt,
    endedAt:
      frontendRecording.endedAt instanceof Date
        ? frontendRecording.endedAt.toISOString()
        : frontendRecording.endedAt,
    snapshots: frontendRecording.snapshots.map((snapshot) => ({
      ...snapshot,
      timestamp:
        snapshot.timestamp instanceof Date
          ? snapshot.timestamp.toISOString()
          : snapshot.timestamp,
    })),
  }

  return {
    version: EXPORT_FORMAT_VERSION,
    exportedAt: new Date().toISOString(),
    frontendRecording:
      serializedRecording as ExportedRecording['frontendRecording'],
    backendRecord: backendRecord,
    replayContext,
    metadata: {
      sessionId: metadata.sessionId ?? frontendRecording.id,
      userAgent: metadata.userAgent ?? navigator.userAgent,
      clientVersion: metadata.clientVersion ?? '1.0.0',
      exportedBy: metadata.exportedBy,
      tags: metadata.tags,
      notes: metadata.notes,
    },
  }
}

// =============================================================================
// Import Helper Functions
// =============================================================================

/**
 * Parse and restore a MatchRecording from exported data
 * Converts ISO date strings back to Date objects
 *
 * Implements: Requirements 7.2
 */
export function restoreMatchRecording(
  exported: ExportedRecording,
): MatchRecording {
  const { frontendRecording } = exported

  return {
    ...frontendRecording,
    startedAt: new Date(frontendRecording.startedAt),
    endedAt: frontendRecording.endedAt
      ? new Date(frontendRecording.endedAt)
      : undefined,
    snapshots: frontendRecording.snapshots.map((snapshot) => ({
      ...snapshot,
      timestamp: new Date(snapshot.timestamp as string),
      // Ensure all required fields are present
      matchReview:
        snapshot.matchReview as MatchRecording['snapshots'][0]['matchReview'],
    })),
  } as MatchRecording
}

/**
 * Generate a downloadable JSON file from exported recording
 *
 * Implements: Requirements 7.1, 7.4
 */
export function downloadRecordingAsJson(
  recording: ExportedRecording,
  filename?: string,
): void {
  const json = JSON.stringify(recording, null, 2)
  const blob = new Blob([json], { type: 'application/json' })
  const url = URL.createObjectURL(blob)

  const link = document.createElement('a')
  link.href = url
  link.download =
    filename ??
    `recording-${recording.frontendRecording.matchId}-${Date.now()}.json`
  document.body.appendChild(link)
  link.click()
  document.body.removeChild(link)
  URL.revokeObjectURL(url)
}

/**
 * Read a JSON file and parse it as an ExportedRecording
 * Returns validation result with errors if invalid
 *
 * Implements: Requirements 7.2, 7.5
 */
export async function readRecordingFromFile(
  file: File,
): Promise<ImportValidationResult> {
  return new Promise((resolve) => {
    const reader = new FileReader()

    reader.onload = (event) => {
      try {
        const text = event.target?.result as string
        const data = JSON.parse(text)
        resolve(validateImportedRecording(data))
      } catch (err) {
        resolve({
          valid: false,
          errors: [
            {
              field: 'file',
              message:
                err instanceof Error
                  ? err.message
                  : 'Failed to parse JSON file',
            },
          ],
        })
      }
    }

    reader.onerror = () => {
      resolve({
        valid: false,
        errors: [
          {
            field: 'file',
            message: 'Failed to read file',
          },
        ],
      })
    }

    reader.readAsText(file)
  })
}
