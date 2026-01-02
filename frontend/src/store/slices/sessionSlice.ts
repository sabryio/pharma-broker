// Session Slice
// User session and preferences (placeholder for auth)

import { createSlice, type PayloadAction } from '@reduxjs/toolkit'

interface SessionPreferences {
  defaultPageSize: number
  autoRefreshInterval: number // ms, 0 = disabled
  showConfidenceColors: boolean
  compactMode: boolean
}

interface SessionState {
  userId: string
  userName: string | null
  isAuthenticated: boolean
  preferences: SessionPreferences
}

// Placeholder user until auth is implemented
const PLACEHOLDER_USER_ID = '00000000-0000-4000-8000-000000000001'

const initialState: SessionState = {
  userId: PLACEHOLDER_USER_ID,
  userName: null,
  isAuthenticated: false,
  preferences: {
    defaultPageSize: 20,
    autoRefreshInterval: 30000,
    showConfidenceColors: true,
    compactMode: false,
  },
}

export const sessionSlice = createSlice({
  name: 'session',
  initialState,
  reducers: {
    setUser: (
      state,
      action: PayloadAction<{ userId: string; userName: string }>,
    ) => {
      state.userId = action.payload.userId
      state.userName = action.payload.userName
      state.isAuthenticated = true
    },
    clearSession: () => initialState,
    updatePreferences: (
      state,
      action: PayloadAction<Partial<SessionPreferences>>,
    ) => {
      state.preferences = { ...state.preferences, ...action.payload }
    },
    setPageSize: (state, action: PayloadAction<number>) => {
      state.preferences.defaultPageSize = action.payload
    },
    setAutoRefresh: (state, action: PayloadAction<number>) => {
      state.preferences.autoRefreshInterval = action.payload
    },
    toggleCompactMode: (state) => {
      state.preferences.compactMode = !state.preferences.compactMode
    },
  },
})

export const sessionActions = sessionSlice.actions

export default sessionSlice.reducer

// Selectors
export const selectUserId = (state: { session: SessionState }) =>
  state.session.userId
export const selectUserName = (state: { session: SessionState }) =>
  state.session.userName
export const selectIsAuthenticated = (state: { session: SessionState }) =>
  state.session.isAuthenticated
export const selectPreferences = (state: { session: SessionState }) =>
  state.session.preferences
export const selectPageSize = (state: { session: SessionState }) =>
  state.session.preferences.defaultPageSize
