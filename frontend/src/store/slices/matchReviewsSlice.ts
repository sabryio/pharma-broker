// Match Reviews Slice
// Selection and navigation state for match reviews

import { createSlice, type PayloadAction } from '@reduxjs/toolkit'

interface LastAction {
  type: 'approved' | 'rejected' | null
  matchId: string | null
  timestamp: number | null
}

interface MatchReviewsState {
  selectedMatchId: string | null
  bulkSelectionIds: string[]
  isBulkMode: boolean
  expandedCardId: string | null
  lastAction: LastAction
}

const initialState: MatchReviewsState = {
  selectedMatchId: null,
  bulkSelectionIds: [],
  isBulkMode: false,
  expandedCardId: null,
  lastAction: { type: null, matchId: null, timestamp: null },
}

export const matchReviewsSlice = createSlice({
  name: 'matchReviews',
  initialState,
  reducers: {
    selectMatch: (state, action: PayloadAction<string>) => {
      state.selectedMatchId = action.payload
    },
    clearSelectedMatch: (state) => {
      state.selectedMatchId = null
    },
    toggleBulkMode: (state) => {
      state.isBulkMode = !state.isBulkMode
      if (!state.isBulkMode) {
        state.bulkSelectionIds = []
      }
    },
    setBulkMode: (state, action: PayloadAction<boolean>) => {
      state.isBulkMode = action.payload
      if (!action.payload) {
        state.bulkSelectionIds = []
      }
    },
    toggleBulkSelection: (state, action: PayloadAction<string>) => {
      const id = action.payload
      const index = state.bulkSelectionIds.indexOf(id)
      if (index === -1) {
        state.bulkSelectionIds.push(id)
      } else {
        state.bulkSelectionIds.splice(index, 1)
      }
    },
    selectAllForBulk: (state, action: PayloadAction<string[]>) => {
      state.bulkSelectionIds = action.payload
    },
    clearBulkSelection: (state) => {
      state.bulkSelectionIds = []
    },
    setExpandedCard: (state, action: PayloadAction<string | null>) => {
      state.expandedCardId = action.payload
    },
    toggleExpandedCard: (state, action: PayloadAction<string>) => {
      state.expandedCardId =
        state.expandedCardId === action.payload ? null : action.payload
    },
    recordAction: (
      state,
      action: PayloadAction<{ type: 'approved' | 'rejected'; matchId: string }>,
    ) => {
      state.lastAction = {
        type: action.payload.type,
        matchId: action.payload.matchId,
        timestamp: Date.now(),
      }
    },
    clearLastAction: (state) => {
      state.lastAction = { type: null, matchId: null, timestamp: null }
    },
  },
})

export const matchReviewsActions = matchReviewsSlice.actions

export default matchReviewsSlice.reducer

// Selectors
export const selectSelectedMatchId = (state: {
  matchReviews: MatchReviewsState
}) => state.matchReviews.selectedMatchId
export const selectBulkSelectionIds = (state: {
  matchReviews: MatchReviewsState
}) => state.matchReviews.bulkSelectionIds
export const selectIsBulkMode = (state: { matchReviews: MatchReviewsState }) =>
  state.matchReviews.isBulkMode
export const selectExpandedCardId = (state: {
  matchReviews: MatchReviewsState
}) => state.matchReviews.expandedCardId
export const selectLastAction = (state: { matchReviews: MatchReviewsState }) =>
  state.matchReviews.lastAction
export const selectBulkSelectionCount = (state: {
  matchReviews: MatchReviewsState
}) => state.matchReviews.bulkSelectionIds.length
