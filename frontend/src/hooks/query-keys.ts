// Query Key Factory
// Centralized query key management for TanStack React Query

export const queryKeys = {
  // Review Queue
  reviewQueue: {
    all: ['review-queue'] as const,
    lists: () => [...queryKeys.reviewQueue.all, 'list'] as const,
    list: (params: { limit?: number; offset?: number; status?: string }) =>
      [...queryKeys.reviewQueue.lists(), params] as const,
    details: () => [...queryKeys.reviewQueue.all, 'detail'] as const,
    detail: (id: string) => [...queryKeys.reviewQueue.details(), id] as const,
    stats: () => [...queryKeys.reviewQueue.all, 'stats'] as const,
  },

  // Match Reviews
  matchReviews: {
    all: ['match-reviews'] as const,
    lists: () => [...queryKeys.matchReviews.all, 'list'] as const,
    list: (params: {
      limit?: number
      offset?: number
      status?: string
      minScore?: number
    }) => [...queryKeys.matchReviews.lists(), params] as const,
    details: () => [...queryKeys.matchReviews.all, 'detail'] as const,
    detail: (id: string) => [...queryKeys.matchReviews.details(), id] as const,
    stats: () => [...queryKeys.matchReviews.all, 'stats'] as const,
  },

  // Offers
  offers: {
    all: ['offers'] as const,
    lists: () => [...queryKeys.offers.all, 'list'] as const,
    list: (params: { limit?: number; offset?: number; status?: string }) =>
      [...queryKeys.offers.lists(), params] as const,
    details: () => [...queryKeys.offers.all, 'detail'] as const,
    detail: (id: string) => [...queryKeys.offers.details(), id] as const,
    stats: () => [...queryKeys.offers.all, 'stats'] as const,
  },

  // Requests
  requests: {
    all: ['requests'] as const,
    lists: () => [...queryKeys.requests.all, 'list'] as const,
    list: (params: {
      limit?: number
      offset?: number
      status?: string
      urgency?: string
    }) => [...queryKeys.requests.lists(), params] as const,
    details: () => [...queryKeys.requests.all, 'detail'] as const,
    detail: (id: string) => [...queryKeys.requests.details(), id] as const,
    stats: () => [...queryKeys.requests.all, 'stats'] as const,
  },

  // Groups
  groups: {
    all: ['groups'] as const,
    lists: () => [...queryKeys.groups.all, 'list'] as const,
    list: (params: { limit?: number; offset?: number }) =>
      [...queryKeys.groups.lists(), params] as const,
    details: () => [...queryKeys.groups.all, 'detail'] as const,
    detail: (jid: string) => [...queryKeys.groups.details(), jid] as const,
  },

  // Stats / Dashboard
  stats: {
    all: ['stats'] as const,
    dashboard: () => [...queryKeys.stats.all, 'dashboard'] as const,
    overview: () => [...queryKeys.stats.all, 'overview'] as const,
  },

  // Medications
  medications: {
    all: ['medications'] as const,
    lists: () => [...queryKeys.medications.all, 'list'] as const,
    list: (params: { limit?: number; offset?: number; search?: string }) =>
      [...queryKeys.medications.lists(), params] as const,
    details: () => [...queryKeys.medications.all, 'detail'] as const,
    detail: (id: string) => [...queryKeys.medications.details(), id] as const,
    aliases: (masterId: string) =>
      [...queryKeys.medications.all, 'aliases', masterId] as const,
  },

  // Audit Records
  auditRecords: {
    all: ['audit-records'] as const,
    lists: () => [...queryKeys.auditRecords.all, 'list'] as const,
    list: (params: {
      limit?: number
      sessionId?: string
      minScore?: number
      aiInvolved?: boolean
    }) => [...queryKeys.auditRecords.lists(), params] as const,
    details: () => [...queryKeys.auditRecords.all, 'detail'] as const,
    detail: (matchId: string) =>
      [...queryKeys.auditRecords.details(), matchId] as const,
    session: (sessionId: string) =>
      [...queryKeys.auditRecords.all, 'session', sessionId] as const,
    status: () => [...queryKeys.auditRecords.all, 'status'] as const,
  },

  // Uncertainty Estimation
  uncertainty: {
    all: ['uncertainty'] as const,
    status: () => [...queryKeys.uncertainty.all, 'status'] as const,
    match: (matchId: string) =>
      [...queryKeys.uncertainty.all, 'match', matchId] as const,
  },

  // Participants
  participants: {
    all: ['participants'] as const,
    stats: (id: string) =>
      [...queryKeys.participants.all, 'stats', id] as const,
    byJid: (jid: string) =>
      [...queryKeys.participants.all, 'by-jid', jid] as const,
  },
} as const

// Type helper for query keys
export type QueryKeys = typeof queryKeys
