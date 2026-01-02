// Filters Slice
// Shared filter and sort state for lists

import { createSlice, type PayloadAction } from '@reduxjs/toolkit'

type SortDirection = 'asc' | 'desc'

interface FilterState {
  // Review Queue filters
  reviewQueue: {
    status: 'pending' | 'approved' | 'rejected' | 'all'
    searchQuery: string
    sortBy: 'date' | 'confidence'
    sortDirection: SortDirection
  }
  // Match Reviews filters
  matchReviews: {
    status: 'PENDING' | 'CONFIRMED' | 'REJECTED' | 'all'
    minConfidence: number
    searchQuery: string
    sortBy: 'date' | 'confidence' | 'status'
    sortDirection: SortDirection
  }
  // Offers filters
  offers: {
    status: 'Active' | 'Matched' | 'Expired' | 'all'
    searchQuery: string
  }
  // Requests filters
  requests: {
    status: 'Active' | 'Matched' | 'Expired' | 'all'
    urgency: 'Normal' | 'Urgent' | 'Critical' | 'all'
    searchQuery: string
  }
}

const initialState: FilterState = {
  reviewQueue: {
    status: 'pending',
    searchQuery: '',
    sortBy: 'date',
    sortDirection: 'desc',
  },
  matchReviews: {
    status: 'PENDING',
    minConfidence: 0,
    searchQuery: '',
    sortBy: 'date',
    sortDirection: 'desc',
  },
  offers: { status: 'Active', searchQuery: '' },
  requests: { status: 'Active', urgency: 'all', searchQuery: '' },
}

export const filtersSlice = createSlice({
  name: 'filters',
  initialState,
  reducers: {
    setReviewQueueFilter: (
      state,
      action: PayloadAction<Partial<FilterState['reviewQueue']>>,
    ) => {
      state.reviewQueue = { ...state.reviewQueue, ...action.payload }
    },
    setMatchReviewsFilter: (
      state,
      action: PayloadAction<Partial<FilterState['matchReviews']>>,
    ) => {
      state.matchReviews = { ...state.matchReviews, ...action.payload }
    },
    setOffersFilter: (
      state,
      action: PayloadAction<Partial<FilterState['offers']>>,
    ) => {
      state.offers = { ...state.offers, ...action.payload }
    },
    setRequestsFilter: (
      state,
      action: PayloadAction<Partial<FilterState['requests']>>,
    ) => {
      state.requests = { ...state.requests, ...action.payload }
    },
    resetFilters: (state, action: PayloadAction<keyof FilterState>) => {
      const key = action.payload
      if (key === 'reviewQueue') {
        state.reviewQueue = initialState.reviewQueue
      } else if (key === 'matchReviews') {
        state.matchReviews = initialState.matchReviews
      } else if (key === 'offers') {
        state.offers = initialState.offers
      } else if (key === 'requests') {
        state.requests = initialState.requests
      }
    },
    resetAllFilters: () => initialState,
  },
})

export const filterActions = filtersSlice.actions

export default filtersSlice.reducer

// Selectors
export const selectReviewQueueFilters = (state: { filters: FilterState }) =>
  state.filters.reviewQueue
export const selectMatchReviewsFilters = (state: { filters: FilterState }) =>
  state.filters.matchReviews
export const selectOffersFilters = (state: { filters: FilterState }) =>
  state.filters.offers
export const selectRequestsFilters = (state: { filters: FilterState }) =>
  state.filters.requests
