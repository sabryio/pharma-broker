// Raw Messages Hook Unit Tests
// Tests for query key generation and parameter handling
//
// Feature: raw-messages-display
// Validates: Requirements 1.2

import { describe, it, expect } from 'vitest'
import { queryKeys } from '../query-keys'

describe('Raw Messages Query Keys', () => {
  describe('query key generation', () => {
    it('should generate correct base query key', () => {
      const key = queryKeys.rawMessages.all
      expect(key).toEqual(['raw-messages'])
    })

    it('should generate correct list query key with default params', () => {
      const key = queryKeys.rawMessages.list({})
      expect(key).toEqual(['raw-messages', 'list', {}])
    })

    it('should generate correct list query key with all params', () => {
      const params = {
        limit: 20,
        offset: 0,
        search: 'test',
        status: 'processed',
        sort_by: 'timestamp',
        sort_order: 'desc',
        start_date: '2024-01-01',
        end_date: '2024-12-31',
      }
      const key = queryKeys.rawMessages.list(params)
      expect(key).toEqual(['raw-messages', 'list', params])
    })

    it('should generate correct detail query key', () => {
      const id = '123e4567-e89b-12d3-a456-426614174000'
      const key = queryKeys.rawMessages.detail(id)
      expect(key).toEqual(['raw-messages', 'detail', id])
    })

    it('should generate unique keys for different params', () => {
      const key1 = queryKeys.rawMessages.list({ limit: 10, offset: 0 })
      const key2 = queryKeys.rawMessages.list({ limit: 20, offset: 0 })
      const key3 = queryKeys.rawMessages.list({ limit: 10, offset: 10 })

      expect(key1).not.toEqual(key2)
      expect(key1).not.toEqual(key3)
      expect(key2).not.toEqual(key3)
    })

    it('should generate unique keys for different search terms', () => {
      const key1 = queryKeys.rawMessages.list({ search: 'aspirin' })
      const key2 = queryKeys.rawMessages.list({ search: 'paracetamol' })

      expect(key1).not.toEqual(key2)
    })

    it('should generate unique keys for different status filters', () => {
      const key1 = queryKeys.rawMessages.list({ status: 'processed' })
      const key2 = queryKeys.rawMessages.list({ status: 'unprocessed' })
      const key3 = queryKeys.rawMessages.list({ status: 'error' })

      expect(key1).not.toEqual(key2)
      expect(key1).not.toEqual(key3)
      expect(key2).not.toEqual(key3)
    })
  })
})
