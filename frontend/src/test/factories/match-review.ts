// Mock Data Factories for Match Reviews
// Type-safe mock data generation using @faker-js/faker

import { faker } from '@faker-js/faker'
import type {
  MatchReviewItem,
  MatchReviewStats,
  OfferSummary,
  RequestSummary,
} from '@/schema/match-review'

export function createMockOfferSummary(
  overrides?: Partial<OfferSummary>,
): OfferSummary {
  return {
    id: faker.string.uuid(),
    product: faker.commerce.productName(),
    source: 'whatsapp',
    sourceGroup: faker.company.name(),
    senderName: faker.person.fullName(),
    senderJid: `${faker.string.numeric(12)}@s.whatsapp.net`,
    rawMessage: faker.lorem.sentence(),
    quantity: faker.number.int({ min: 1, max: 100 }).toString(),
    price: faker.commerce.price({ min: 50, max: 500 }),
    expiry: faker.date.future().toISOString(),
    masterId: faker.helpers.maybe(() => faker.string.uuid()) ?? null,
    medicationAliasId: faker.helpers.maybe(() => faker.string.uuid()) ?? null,
    curationStatus: faker.helpers.arrayElement(['curated', 'pending', null]),
    ...overrides,
  }
}

export function createMockRequestSummary(
  overrides?: Partial<RequestSummary>,
): RequestSummary {
  return {
    id: faker.string.uuid(),
    product: faker.commerce.productName(),
    source: 'whatsapp',
    sourceGroup: faker.company.name(),
    senderName: faker.person.fullName(),
    senderJid: `${faker.string.numeric(12)}@s.whatsapp.net`,
    rawMessage: faker.lorem.sentence(),
    quantity: faker.number.int({ min: 1, max: 50 }).toString(),
    maxPrice: faker.commerce.price({ min: 100, max: 600 }),
    urgency: faker.helpers.arrayElement(['Normal', 'Urgent', 'Critical']),
    masterId: faker.helpers.maybe(() => faker.string.uuid()) ?? null,
    medicationAliasId: faker.helpers.maybe(() => faker.string.uuid()) ?? null,
    curationStatus: faker.helpers.arrayElement(['curated', 'pending', null]),
    ...overrides,
  }
}

export function createMockMatchReviewItem(
  overrides?: Partial<MatchReviewItem>,
): MatchReviewItem {
  return {
    id: faker.string.uuid(),
    confidence: faker.number.float({ min: 0.5, max: 1, fractionDigits: 2 }),
    status: faker.helpers.arrayElement([
      'PENDING',
      'CONFIRMED',
      'REJECTED',
      'EXPIRED',
    ]),
    reasoning: faker.lorem.sentence(),
    issues: faker.helpers.multiple(() => faker.lorem.words(3), {
      count: { min: 0, max: 3 },
    }),
    offer: createMockOfferSummary(),
    request: createMockRequestSummary(),
    createdAt: faker.date.recent().toISOString(),
    confirmedAt:
      faker.helpers.maybe(() => faker.date.recent().toISOString()) ?? null,
    aiConfidence: faker.helpers.maybe(() =>
      faker.number.float({ min: 0.5, max: 1 }),
    ),
    ...overrides,
  }
}

export function createMockMatchReviewStats(
  overrides?: Partial<MatchReviewStats>,
): MatchReviewStats {
  return {
    pending: faker.number.int({ min: 10, max: 100 }),
    confirmedToday: faker.number.int({ min: 5, max: 50 }),
    rejectedToday: faker.number.int({ min: 1, max: 20 }),
    totalPending: faker.number.int({ min: 50, max: 500 }),
    avgConfidence: faker.number.float({
      min: 0.6,
      max: 0.9,
      fractionDigits: 2,
    }),
    uniquePendingOffers: faker.number.int({ min: 10, max: 100 }),
    uniquePendingRequests: faker.number.int({ min: 10, max: 150 }),
    ...overrides,
  }
}

// Generate list of mock items
export function createMockMatchReviewList(
  count: number = 20,
): Array<MatchReviewItem> {
  return faker.helpers.multiple(() => createMockMatchReviewItem(), { count })
}

// Generate paginated response
export function createMockMatchReviewResponse(
  count: number = 20,
  total: number = 100,
  limit: number = 20,
  offset: number = 0,
) {
  return {
    items: createMockMatchReviewList(count),
    total,
    limit,
    offset,
  }
}
