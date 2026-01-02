// UI Slice
// Global UI state for sidebar, modals, theme, and toasts

import { createSlice, type PayloadAction } from '@reduxjs/toolkit'

interface ToastItem {
  id: string
  message: string
  type: 'success' | 'error' | 'info' | 'warning'
}

interface UiState {
  sidebarCollapsed: boolean
  activeModal: string | null
  theme: 'light' | 'dark' | 'system'
  toastQueue: ToastItem[]
}

const initialState: UiState = {
  sidebarCollapsed: false,
  activeModal: null,
  theme: 'system',
  toastQueue: [],
}

export const uiSlice = createSlice({
  name: 'ui',
  initialState,
  reducers: {
    toggleSidebar: (state) => {
      state.sidebarCollapsed = !state.sidebarCollapsed
    },
    setSidebarCollapsed: (state, action: PayloadAction<boolean>) => {
      state.sidebarCollapsed = action.payload
    },
    openModal: (state, action: PayloadAction<string>) => {
      state.activeModal = action.payload
    },
    closeModal: (state) => {
      state.activeModal = null
    },
    setTheme: (state, action: PayloadAction<'light' | 'dark' | 'system'>) => {
      state.theme = action.payload
    },
    addToast: (state, action: PayloadAction<Omit<ToastItem, 'id'>>) => {
      state.toastQueue.push({
        ...action.payload,
        id: `toast-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
      })
    },
    removeToast: (state, action: PayloadAction<string>) => {
      state.toastQueue = state.toastQueue.filter((t) => t.id !== action.payload)
    },
    clearToasts: (state) => {
      state.toastQueue = []
    },
  },
})

export const uiActions = uiSlice.actions

export default uiSlice.reducer

// Selectors
export const selectSidebarCollapsed = (state: { ui: UiState }) =>
  state.ui.sidebarCollapsed
export const selectActiveModal = (state: { ui: UiState }) =>
  state.ui.activeModal
export const selectTheme = (state: { ui: UiState }) => state.ui.theme
export const selectToasts = (state: { ui: UiState }) => state.ui.toastQueue
