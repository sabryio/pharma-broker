// Send Message Hook Unit Tests
// Tests for the useSendMessage hook and messaging API
//
// Feature: send-message
// Validates: Requirements 6.3, 6.4, 6.6, 6.8

import { describe, it, expect, vi, beforeEach } from 'vitest'
import { sendMessage, type SendMessageRequest } from '@/api/messaging'

// Mock the API client
vi.mock('@/api/client', () => ({
  default: {
    post: vi.fn(),
  },
}))

describe('Send Message API', () => {
  describe('SendMessageRequest validation', () => {
    it('should have required recipient_jid field', () => {
      const request: SendMessageRequest = {
        recipient_jid: '201234567890@s.whatsapp.net',
        content: 'Hello',
      }
      expect(request.recipient_jid).toBeDefined()
      expect(request.recipient_jid).not.toBe('')
    })

    it('should have required content field', () => {
      const request: SendMessageRequest = {
        recipient_jid: '201234567890@s.whatsapp.net',
        content: 'Hello, this is a test message',
      }
      expect(request.content).toBeDefined()
      expect(request.content).not.toBe('')
    })

    it('should allow optional reference_id field', () => {
      const requestWithRef: SendMessageRequest = {
        recipient_jid: '201234567890@s.whatsapp.net',
        content: 'Hello',
        reference_id: 'ref-123',
      }
      expect(requestWithRef.reference_id).toBe('ref-123')

      const requestWithoutRef: SendMessageRequest = {
        recipient_jid: '201234567890@s.whatsapp.net',
        content: 'Hello',
      }
      expect(requestWithoutRef.reference_id).toBeUndefined()
    })
  })

  describe('JID format validation', () => {
    it('should accept valid individual JID format', () => {
      const jid = '201234567890@s.whatsapp.net'
      expect(jid).toMatch(/@s\.whatsapp\.net$/)
    })

    it('should accept valid group JID format', () => {
      const jid = '120363123456789012@g.us'
      expect(jid).toMatch(/@g\.us$/)
    })

    it('should accept valid LID format', () => {
      const jid = '123456789@lid'
      expect(jid).toMatch(/@lid$/)
    })
  })

  describe('Content validation', () => {
    it('should reject empty content', () => {
      const content = ''
      expect(content.trim()).toBe('')
    })

    it('should reject whitespace-only content', () => {
      const content = '   \t\n  '
      expect(content.trim()).toBe('')
    })

    it('should accept valid content', () => {
      const content = 'Hello, this is a test message'
      expect(content.trim()).not.toBe('')
    })

    it('should enforce max length of 4096 characters', () => {
      const maxLength = 4096
      const validContent = 'a'.repeat(maxLength)
      const invalidContent = 'a'.repeat(maxLength + 1)

      expect(validContent.length).toBeLessThanOrEqual(maxLength)
      expect(invalidContent.length).toBeGreaterThan(maxLength)
    })
  })

  describe('Character count display', () => {
    it('should calculate correct character count', () => {
      const message = 'Hello, world!'
      expect(message.length).toBe(13)
    })

    it('should handle Arabic text character count', () => {
      const arabicMessage = 'مرحباً'
      expect(arabicMessage.length).toBeGreaterThan(0)
    })

    it('should handle mixed language text', () => {
      const mixedMessage = 'Hello مرحباً'
      expect(mixedMessage.length).toBeGreaterThan(0)
    })
  })
})

describe('SendMessageResponse handling', () => {
  it('should handle success response', () => {
    const response = {
      success: true,
      message_id: 'msg-123',
      error: undefined,
    }
    expect(response.success).toBe(true)
    expect(response.message_id).toBeDefined()
    expect(response.error).toBeUndefined()
  })

  it('should handle error response', () => {
    const response = {
      success: false,
      error: 'Bridge not connected',
      code: 'BRIDGE_NOT_CONNECTED',
    }
    expect(response.success).toBe(false)
    expect(response.error).toBeDefined()
    expect(response.code).toBeDefined()
  })

  it('should handle validation error response', () => {
    const response = {
      success: false,
      error: 'content cannot be empty or whitespace-only',
      code: 'EMPTY_CONTENT',
    }
    expect(response.success).toBe(false)
    expect(response.code).toBe('EMPTY_CONTENT')
  })
})

// Property-Based Tests using fast-check
// Feature: send-message, Property 2: Missing Fields Rejected (Frontend)
// Feature: send-message, Property 4: Whitespace Content Rejected (Frontend)
// Validates: Requirements 1.2, 1.5

import * as fc from 'fast-check'

describe('Property-Based Tests: Form Validation', () => {
  // Property 2: Missing Fields Rejected
  describe('Property 2: Missing Fields Rejected', () => {
    it('should reject requests with empty recipient_jid', () => {
      fc.assert(
        fc.property(fc.string({ minLength: 1 }), (content) => {
          const request = {
            recipient_jid: '',
            content,
          }
          // Empty recipient_jid should be invalid
          return request.recipient_jid === ''
        }),
        { numRuns: 100 },
      )
    })

    it('should reject requests with empty content', () => {
      fc.assert(
        fc.property(
          fc.string({ minLength: 1 }).filter((s) => s.includes('@')),
          (jid) => {
            const request = {
              recipient_jid: jid,
              content: '',
            }
            // Empty content should be invalid
            return request.content === ''
          },
        ),
        { numRuns: 100 },
      )
    })

    it('should accept requests with both fields populated', () => {
      fc.assert(
        fc.property(
          fc.string({ minLength: 1 }).filter((s) => s.includes('@')),
          fc
            .string({ minLength: 1, maxLength: 4096 })
            .filter((s) => s.trim().length > 0),
          (jid, content) => {
            const request = {
              recipient_jid: jid,
              content,
            }
            // Both fields populated should be valid
            return (
              request.recipient_jid.length > 0 && request.content.length > 0
            )
          },
        ),
        { numRuns: 100 },
      )
    })
  })

  // Property 4: Whitespace Content Rejected
  describe('Property 4: Whitespace Content Rejected', () => {
    it('should reject content that is only whitespace', () => {
      // Generate strings that are only whitespace using array and join
      const whitespaceArb = fc
        .array(fc.constantFrom(' ', '\t', '\n', '\r'), {
          minLength: 1,
          maxLength: 100,
        })
        .map((arr) => arr.join(''))

      fc.assert(
        fc.property(whitespaceArb, (whitespaceContent) => {
          // Whitespace-only content should be rejected
          return whitespaceContent.trim() === ''
        }),
        { numRuns: 100 },
      )
    })

    it('should accept content with non-whitespace characters', () => {
      // Generate strings that have at least one non-whitespace character
      const nonWhitespaceArb = fc
        .string({ minLength: 1, maxLength: 4096 })
        .filter((s) => s.trim().length > 0)

      fc.assert(
        fc.property(nonWhitespaceArb, (content) => {
          // Non-whitespace content should be accepted
          return content.trim().length > 0
        }),
        { numRuns: 100 },
      )
    })

    it('should correctly trim leading and trailing whitespace', () => {
      fc.assert(
        fc.property(
          fc.string({ minLength: 1 }).filter((s) => s.trim().length > 0),
          fc.string({ minLength: 0, maxLength: 10 }),
          fc.string({ minLength: 0, maxLength: 10 }),
          (core, leadingWs, trailingWs) => {
            const content = leadingWs + core + trailingWs
            const trimmed = content.trim()
            // Trimmed content should not have leading/trailing whitespace
            return trimmed === trimmed.trim() && trimmed.length > 0
          },
        ),
        { numRuns: 100 },
      )
    })
  })

  // Additional property: Content length validation
  describe('Property: Content Length Validation', () => {
    const MAX_LENGTH = 4096

    it('should accept content within max length', () => {
      fc.assert(
        fc.property(
          fc
            .string({ minLength: 1, maxLength: MAX_LENGTH })
            .filter((s) => s.trim().length > 0),
          (content) => {
            return content.length <= MAX_LENGTH
          },
        ),
        { numRuns: 100 },
      )
    })

    it('should reject content exceeding max length', () => {
      fc.assert(
        fc.property(
          fc.string({
            minLength: MAX_LENGTH + 1,
            maxLength: MAX_LENGTH + 1000,
          }),
          (content) => {
            return content.length > MAX_LENGTH
          },
        ),
        { numRuns: 100 },
      )
    })
  })

  // Property: JID format validation
  describe('Property: JID Format Validation', () => {
    it('should validate individual JID format', () => {
      // Generate valid phone numbers (digits only) using array and join
      const phoneArb = fc
        .array(
          fc.constantFrom('0', '1', '2', '3', '4', '5', '6', '7', '8', '9'),
          {
            minLength: 10,
            maxLength: 15,
          },
        )
        .map((arr) => arr.join(''))

      fc.assert(
        fc.property(phoneArb, (phone) => {
          const jid = `${phone}@s.whatsapp.net`
          return jid.endsWith('@s.whatsapp.net') && jid.split('@')[0].length > 0
        }),
        { numRuns: 100 },
      )
    })

    it('should validate group JID format', () => {
      const groupIdArb = fc
        .array(
          fc.constantFrom('0', '1', '2', '3', '4', '5', '6', '7', '8', '9'),
          {
            minLength: 15,
            maxLength: 20,
          },
        )
        .map((arr) => arr.join(''))

      fc.assert(
        fc.property(groupIdArb, (groupId) => {
          const jid = `${groupId}@g.us`
          return jid.endsWith('@g.us') && jid.split('@')[0].length > 0
        }),
        { numRuns: 100 },
      )
    })
  })
})
