// Review Queue Slice
// Selection and view state for review queue

import { createSlice, type PayloadAction } from '@reduxjs/toolkit'

type ViewMode = 'list' | 'detail' | 'bulk'

interface AdjustmentSettings {
  priceFlexibility: number
  quantityTolerance: number
  dosageStrictness: number
}

interface ReviewQueueState {
  currentItemId: string | null
  currentIndex: number
  selectedIds: string[]
  viewMode: ViewMode
  adjustments: AdjustmentSettings
}

const initialState: ReviewQueueState = {
  currentItemId: null,
  currentIndex: 0,
  selectedIds: [],
  viewMode: 'list',
  adjustments: {
    priceFlexibility: 50,
    quantityTolerance: 50,
    dosageStrictness: 50,
  },
}

export const reviewQueueSlice = createSlice({
  name: 'reviewQueue',
  initialState,
  reducers: {
    setCurrentItem: (
      state,
      action: PayloadAction<{ id: string; index: number }>,
    ) => {
      state.currentItemId = action.payload.id
      state.currentIndex = action.payload.index
    },
    clearCurrentItem: (state) => {
      state.currentItemId = null
      state.currentIndex = 0
    },
    setViewMode: (state, action: PayloadAction<ViewMode>) => {
      state.viewMode = action.payload
    },
    toggleSelection: (state, action: PayloadAction<string>) => {
      const id = action.payload
      const index = state.selectedIds.indexOf(id)
      if (index === -1) {
        state.selectedIds.push(id)
      } else {
        state.selectedIds.splice(index, 1)
      }
    },
    selectAll: (state, action: PayloadAction<string[]>) => {
      state.selectedIds = action.payload
    },
    clearSelection: (state) => {
      state.selectedIds = []
    },
    setAdjustments: (
      state,
      action: PayloadAction<Partial<AdjustmentSettings>>,
    ) => {
      state.adjustments = { ...state.adjustments, ...action.payload }
    },
    resetAdjustments: (state) => {
      state.adjustments = initialState.adjustments
    },
    navigateToNext: (state, action: PayloadAction<string[]>) => {
      const ids = action.payload
      const currentIdx = state.currentItemId
        ? ids.indexOf(state.currentItemId)
        : -1
      if (currentIdx < ids.length - 1) {
        state.currentIndex = currentIdx + 1
        state.currentItemId = ids[currentIdx + 1]
      }
    },
    navigateToPrevious: (state, action: PayloadAction<string[]>) => {
      const ids = action.payload
      const currentIdx = state.currentItemId
        ? ids.indexOf(state.currentItemId)
        : -1
      if (currentIdx > 0) {
        state.currentIndex = currentIdx - 1
        state.currentItemId = ids[currentIdx - 1]
      }
    },
  },
})

export const reviewQueueActions = reviewQueueSlice.actions

export default reviewQueueSlice.reducer

// Selectors
export const selectCurrentItemId = (state: { reviewQueue: ReviewQueueState }) =>
  state.reviewQueue.currentItemId
export const selectCurrentIndex = (state: { reviewQueue: ReviewQueueState }) =>
  state.reviewQueue.currentIndex
export const selectSelectedIds = (state: { reviewQueue: ReviewQueueState }) =>
  state.reviewQueue.selectedIds
export const selectViewMode = (state: { reviewQueue: ReviewQueueState }) =>
  state.reviewQueue.viewMode
export const selectAdjustments = (state: { reviewQueue: ReviewQueueState }) =>
  state.reviewQueue.adjustments
export const selectSelectionCount = (state: {
  reviewQueue: ReviewQueueState
}) => state.reviewQueue.selectedIds.length
