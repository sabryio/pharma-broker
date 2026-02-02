// Recording Export/Import Property Tests
// Property-based tests for export/import round-trip consistency
//
// Feature: debug-recording-enhancement
// Property 7: Export/Import Round-Trip
// Validates: Requirements 7.1, 7.2

import { describe, it, expect } from 'vitest'
import * as fc from 'fast-check'
import {
  createExportedRecording,
  validateImportedRecording,
  restoreMatchRecording,
  EXPORT_FORMAT_VERSION,
} from '../recording-export'
import type { MatchRecording } from '@/components/debug-recordings/types'

// =============================================================================
// Arbitrary Generators
// =============================================================================

/** Generate a valid recording event */
const recordingEventArb = fc.record({
  type: fc.constantFrom(
    'view',
    'approve',
    'reject',
    'restore',
    'adjust_price',
    'adjust_quantity',
    'adjust_dosage',
    'ai_review',
    'confidence_change',
    'navigate',
    'bulk_select',
    'bulk_action',
    'filter_change',
    'sort_change',
  ),
  label: fc.string({ minLength: 1, maxLength: 50 }),
  description: fc.option(fc.string({ maxLength: 100 }), { nil: undefined }),
  data: fc.option(fc.dictionary(fc.string(), fc.jsonValue()), {
    nil: undefined,
  }),
})

/** Generate adjustment settings */
const adjustmentSettingsArb = fc.record({
  priceFlexibility: fc.float({ min: 0, max: 1, noNaN: true }),
  quantityTolerance: fc.float({ min: 0, max: 1, noNaN: true }),
  dosageStrictness: fc.float({ min: 0, max: 1, noNaN: true }),
})

/** Generate recording metadata */
const recordingMetadataArb = fc.record({
  userAgent: fc.option(fc.string({ maxLength: 200 }), { nil: undefined }),
  sessionId: fc.option(fc.uuid(), { nil: undefined }),
  previousSnapshotId: fc.option(fc.uuid(), { nil: null }),
  scoreBreakdown: fc.option(
    fc.record({
      medicationSimilarity: fc.float({ min: 0, max: 1, noNaN: true }),
      rawSimilarity: fc.float({ min: 0, max: 1, noNaN: true }),
      embeddingSimilarity: fc.option(
        fc.float({ min: 0, max: 1, noNaN: true }),
        { nil: null },
      ),
      dosageMatch: fc.float({ min: 0, max: 1, noNaN: true }),
      quantityMatch: fc.float({ min: 0, max: 1, noNaN: true }),
      priceMatch: fc.option(fc.float({ min: 0, max: 1, noNaN: true }), {
        nil: null,
      }),
      recencyBonus: fc.float({ min: 0, max: 1, noNaN: true }),
      aiLogicScore: fc.option(fc.float({ min: 0, max: 1, noNaN: true }), {
        nil: null,
      }),
      finalScore: fc.float({ min: 0, max: 1, noNaN: true }),
    }),
    { nil: undefined },
  ),
  weights: fc.option(
    fc.record({
      medication: fc.float({ min: 0, max: 1, noNaN: true }),
      raw: fc.float({ min: 0, max: 1, noNaN: true }),
      embedding: fc.float({ min: 0, max: 1, noNaN: true }),
      dosage: fc.float({ min: 0, max: 1, noNaN: true }),
      quantity: fc.float({ min: 0, max: 1, noNaN: true }),
      price: fc.float({ min: 0, max: 1, noNaN: true }),
      recency: fc.float({ min: 0, max: 1, noNaN: true }),
      aiLogic: fc.float({ min: 0, max: 1, noNaN: true }),
    }),
    { nil: undefined },
  ),
})

/** Generate a match review item (simplified for testing) */
const matchReviewArb = fc.record({
  id: fc.uuid(),
  confidence: fc.float({ min: 0, max: 1, noNaN: true }),
  status: fc.constantFrom('PENDING', 'CONFIRMED', 'REJECTED', 'EXPIRED'),
  reasoning: fc.option(fc.string({ maxLength: 200 }), { nil: null }),
  issues: fc.array(fc.string({ maxLength: 50 }), { maxLength: 5 }),
  createdAt: fc.date({ noInvalidDate: true }).map((d) => d.toISOString()),
})

/** Generate offer data */
const offerArb = fc.record({
  id: fc.uuid(),
  product: fc.string({ minLength: 1, maxLength: 100 }),
  medicationRaw: fc.option(fc.string({ maxLength: 100 }), { nil: null }),
  quantity: fc.option(fc.nat().map(String), { nil: null }),
  price: fc.option(fc.float({ min: 0, max: 10000, noNaN: true }).map(String), {
    nil: null,
  }),
})

/** Generate request data */
const requestArb = fc.record({
  id: fc.uuid(),
  product: fc.string({ minLength: 1, maxLength: 100 }),
  medicationRaw: fc.option(fc.string({ maxLength: 100 }), { nil: null }),
  quantity: fc.option(fc.nat().map(String), { nil: null }),
  maxPrice: fc.option(
    fc.float({ min: 0, max: 10000, noNaN: true }).map(String),
    { nil: null },
  ),
})

/** Generate a recording snapshot */
const snapshotArb = fc.record({
  id: fc.uuid(),
  timestamp: fc.date({ noInvalidDate: true }),
  matchReview: matchReviewArb,
  offer: offerArb,
  request: requestArb,
  confidence: fc.float({ min: 0, max: 1, noNaN: true }),
  aiConfidence: fc.option(fc.float({ min: 0, max: 1, noNaN: true }), {
    nil: null,
  }),
  issues: fc.array(fc.string({ maxLength: 100 }), { maxLength: 5 }),
  reasoning: fc.option(fc.string({ maxLength: 200 }), { nil: null }),
  adjustments: adjustmentSettingsArb,
  event: recordingEventArb,
  metadata: recordingMetadataArb,
})

/** Generate a complete MatchRecording */
const matchRecordingArb: fc.Arbitrary<MatchRecording> = fc.record({
  id: fc.uuid(),
  matchId: fc.uuid(),
  startedAt: fc.date({ noInvalidDate: true }),
  endedAt: fc.option(fc.date({ noInvalidDate: true }), { nil: undefined }),
  duration: fc.option(fc.nat({ max: 3600000 }), { nil: undefined }),
  outcome: fc.option(fc.constantFrom('approved', 'rejected', 'pending'), {
    nil: undefined,
  }),
  snapshots: fc.array(snapshotArb, { minLength: 1, maxLength: 10 }),
}) as fc.Arbitrary<MatchRecording>

// =============================================================================
// Property Tests
// =============================================================================

describe('Recording Export/Import', () => {
  /**
   * Property 7: Export/Import Round-Trip
   *
   * For any valid recording export, importing the exported data and then
   * re-exporting SHALL produce an equivalent export (excluding timestamps
   * and generated IDs).
   *
   * Validates: Requirements 7.1, 7.2
   */
  describe('Property 7: Export/Import Round-Trip', () => {
    it('should preserve frontend recording data through export/import cycle', () => {
      fc.assert(
        fc.property(matchRecordingArb, (recording) => {
          // Create export
          const exported = createExportedRecording(recording, null, {
            sessionId: recording.id,
            userAgent: 'test-agent',
            clientVersion: '1.0.0',
          })

          // Simulate real-world JSON serialization (strips undefined values)
          const serialized = JSON.stringify(exported)
          const deserialized = JSON.parse(serialized)

          // Validate the deserialized export
          const validationResult = validateImportedRecording(deserialized)
          expect(validationResult.valid).toBe(true)
          expect(validationResult.data).toBeDefined()

          // Restore the recording
          const restored = restoreMatchRecording(validationResult.data!)

          // Verify key properties are preserved
          expect(restored.id).toBe(recording.id)
          expect(restored.matchId).toBe(recording.matchId)
          expect(restored.snapshots.length).toBe(recording.snapshots.length)
          expect(restored.outcome).toBe(recording.outcome)
          expect(restored.duration).toBe(recording.duration)

          // Verify snapshots are preserved
          for (let i = 0; i < recording.snapshots.length; i++) {
            const original = recording.snapshots[i]
            const restoredSnapshot = restored.snapshots[i]

            expect(restoredSnapshot.id).toBe(original.id)
            expect(restoredSnapshot.confidence).toBe(original.confidence)
            expect(restoredSnapshot.aiConfidence).toBe(original.aiConfidence)
            expect(restoredSnapshot.event.type).toBe(original.event.type)
            expect(restoredSnapshot.event.label).toBe(original.event.label)
            expect(restoredSnapshot.offer.id).toBe(original.offer.id)
            expect(restoredSnapshot.request.id).toBe(original.request.id)
          }
        }),
        { numRuns: 100 },
      )
    })

    it('should produce valid export format with correct version', () => {
      fc.assert(
        fc.property(matchRecordingArb, (recording) => {
          const exported = createExportedRecording(recording, null, {
            sessionId: recording.id,
            userAgent: 'test-agent',
            clientVersion: '1.0.0',
          })

          // Verify export structure
          expect(exported.version).toBe(EXPORT_FORMAT_VERSION)
          expect(exported.exportedAt).toBeDefined()
          expect(new Date(exported.exportedAt).getTime()).not.toBeNaN()
          expect(exported.frontendRecording).toBeDefined()
          expect(exported.metadata).toBeDefined()
          expect(exported.metadata.sessionId).toBe(recording.id)
        }),
        { numRuns: 100 },
      )
    })

    it('should handle recordings with backend data', () => {
      fc.assert(
        fc.property(matchRecordingArb, (recording) => {
          // Create mock backend record
          const mockBackendRecord = {
            record: {
              id: recording.id,
              matchId: recording.matchId,
              offerId: 'offer-123',
              requestId: 'request-456',
              pipelineVersion: '1.0.0',
              offerSnapshot: { product: 'Test Product' },
              requestSnapshot: { product: 'Test Request' },
              weightsSnapshot: { medication: 0.5 },
              configSnapshot: null,
              scoreBreakdown: { finalScore: 0.85 },
              finalScore: 0.85,
              pipelineStages: [],
              aiInvolved: false,
              aiRecord: null,
              resolutionStage: 'exact_match',
              resolutionDetails: null,
              totalLatencyMs: 150,
              createdAt: new Date().toISOString(),
              reviewStatus: null,
              reviewedBy: null,
              reviewedAt: null,
              reviewNotes: null,
              sessionId: recording.id,
              clientMetadata: null,
            },
            replayContext: {
              offer: { id: 'offer-123' },
              request: { id: 'request-456' },
              weights: { medication: 0.5 },
            },
          }

          const exported = createExportedRecording(
            recording,
            mockBackendRecord,
            { sessionId: recording.id },
          )

          // Validate export includes backend data
          expect(exported.backendRecord).toBeDefined()
          expect(exported.backendRecord?.record.matchId).toBe(recording.matchId)
          expect(exported.replayContext).toBeDefined()
          expect(exported.replayContext?.offer).toBeDefined()
        }),
        { numRuns: 50 },
      )
    })
  })

  describe('Validation', () => {
    it('should reject invalid export data', () => {
      const invalidData = [
        null,
        undefined,
        {},
        { version: '1.0.0' }, // Missing required fields
        { version: '1.0.0', exportedAt: 'invalid-date' },
        'not an object',
        123,
      ]

      for (const data of invalidData) {
        const result = validateImportedRecording(data)
        expect(result.valid).toBe(false)
        expect(result.errors.length).toBeGreaterThan(0)
      }
    })

    it('should provide specific error messages for invalid fields', () => {
      const invalidExport = {
        version: '1.0.0',
        exportedAt: new Date().toISOString(),
        frontendRecording: {
          id: 'not-a-uuid', // Invalid UUID format is still a string
          matchId: 123, // Should be string
          startedAt: 'invalid-date',
          snapshots: 'not-an-array', // Should be array
        },
        backendRecord: null,
        replayContext: null,
        metadata: {
          sessionId: 'test',
          userAgent: 'test',
          clientVersion: '1.0.0',
        },
      }

      const result = validateImportedRecording(invalidExport)
      expect(result.valid).toBe(false)
      expect(result.errors.length).toBeGreaterThan(0)

      // Check that errors have field information
      const hasFieldInfo = result.errors.some(
        (e) => e.field && e.field.length > 0,
      )
      expect(hasFieldInfo).toBe(true)
    })
  })
})
