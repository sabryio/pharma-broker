// Filter and Sort Utility Functions for Matches
// These functions are exported for property-based testing

import type { MatchReviewItem, MatchStatus } from '@/schema/match-review'
import type { FilterState } from './filter-panel'

// Confidence band thresholds
export const CONFIDENCE_BANDS = {
  high: { min: 80, max: 100 },
  medium: { min: 50, max: 79 },
  low: { min: 0, max: 49 },
} as const

/**
 * Filter matches by status
 * Returns all matches if status is 'all', otherwise only matches with matching status
 */
export function filterByStatus(
  matches: MatchReviewItem[],
  status: 'all' | MatchStatus,
): MatchReviewItem[] {
  if (status === 'all') return matches
  return matches.filter((match) => match.status === status)
}

/**
 * Filter matches by confidence threshold
 * Returns matches with confidence >= minConfidence and <= maxConfidence
 */
export function filterByConfidenceThreshold(
  matches: MatchReviewItem[],
  minConfidence: number,
  maxConfidence: number,
): MatchReviewItem[] {
  return matches.filter(
    (match) =>
      match.confidence >= minConfidence && match.confidence <= maxConfidence,
  )
}

/**
 * Filter matches by medication search term
 * Returns matches where offer or request product contains the search term (case-insensitive)
 */
export function filterByMedicationSearch(
  matches: MatchReviewItem[],
  searchTerm: string,
): MatchReviewItem[] {
  if (!searchTerm.trim()) return matches
  const search = searchTerm.toLowerCase()
  return matches.filter(
    (match) =>
      match.offer.product.toLowerCase().includes(search) ||
      match.request.product.toLowerCase().includes(search),
  )
}

/**
 * Filter matches by confidence band
 * Returns matches within the specified confidence band range
 */
export function filterByConfidenceBand(
  matches: MatchReviewItem[],
  band: 'all' | 'high' | 'medium' | 'low',
): MatchReviewItem[] {
  if (band === 'all') return matches
  const { min, max } = CONFIDENCE_BANDS[band]
  return matches.filter(
    (match) => match.confidence >= min && match.confidence <= max,
  )
}

/**
 * Apply all filters to matches
 * Returns the filtered matches and the count
 */
export function applyFilters(
  matches: MatchReviewItem[],
  filters: FilterState,
): MatchReviewItem[] {
  let result = [...matches]

  // Status filter
  result = filterByStatus(result, filters.status)

  // Confidence band filter
  result = filterByConfidenceBand(result, filters.confidenceBand)

  // Confidence range filter
  result = filterByConfidenceThreshold(
    result,
    filters.minConfidence,
    filters.maxConfidence,
  )

  // Medication search filter
  result = filterByMedicationSearch(result, filters.medicationSearch)

  return result
}

/**
 * Sort matches by confidence
 * Returns matches sorted by confidence score
 */
export function sortByConfidence(
  matches: MatchReviewItem[],
  order: 'asc' | 'desc',
): MatchReviewItem[] {
  return [...matches].sort((a, b) => {
    const comparison = a.confidence - b.confidence
    return order === 'desc' ? -comparison : comparison
  })
}

/**
 * Sort matches by date
 * Returns matches sorted by creation date
 */
export function sortByDate(
  matches: MatchReviewItem[],
  order: 'asc' | 'desc',
): MatchReviewItem[] {
  return [...matches].sort((a, b) => {
    const comparison =
      new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
    return order === 'desc' ? -comparison : comparison
  })
}

/**
 * Sort matches by medication name
 * Returns matches sorted alphabetically by offer product name
 */
export function sortByMedication(
  matches: MatchReviewItem[],
  order: 'asc' | 'desc',
): MatchReviewItem[] {
  return [...matches].sort((a, b) => {
    const comparison = a.offer.product.localeCompare(b.offer.product)
    return order === 'desc' ? -comparison : comparison
  })
}

/**
 * Apply sorting to matches
 * Returns the sorted matches
 */
export function applySorting(
  matches: MatchReviewItem[],
  sortBy: 'confidence' | 'date' | 'medication',
  sortOrder: 'asc' | 'desc',
): MatchReviewItem[] {
  switch (sortBy) {
    case 'confidence':
      return sortByConfidence(matches, sortOrder)
    case 'date':
      return sortByDate(matches, sortOrder)
    case 'medication':
      return sortByMedication(matches, sortOrder)
    default:
      return matches
  }
}

/**
 * Apply all filters and sorting to matches
 * Returns the filtered and sorted matches
 */
export function applyFiltersAndSorting(
  matches: MatchReviewItem[],
  filters: FilterState,
): MatchReviewItem[] {
  const filtered = applyFilters(matches, filters)
  return applySorting(filtered, filters.sortBy, filters.sortOrder)
}
