import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import type { Offer, Request, Match, Group, Stats } from './types'

const API_BASE = '/api'

// Fetch functions
async function fetchJson<T>(url: string, options?: RequestInit): Promise<T> {
  const res = await fetch(url, options)
  const json = await res.json()
  if (!json.success && json.error) {
    throw new Error(json.error)
  }
  return json.data
}

// Offers
export function useOffers(query?: string) {
  return useQuery({
    queryKey: ['offers', query],
    queryFn: () => {
      const url = query
        ? `${API_BASE}/offers?q=${encodeURIComponent(query)}`
        : `${API_BASE}/offers`
      return fetchJson<Offer[]>(url)
    },
  })
}

// Requests
export function useRequests(query?: string) {
  return useQuery({
    queryKey: ['requests', query],
    queryFn: () => {
      const url = query
        ? `${API_BASE}/requests?q=${encodeURIComponent(query)}`
        : `${API_BASE}/requests`
      return fetchJson<Request[]>(url)
    },
  })
}

// Matches
export function useMatches() {
  return useQuery({
    queryKey: ['matches'],
    queryFn: () => fetchJson<Match[]>(`${API_BASE}/matches`),
  })
}

// Stats
export function useStats() {
  return useQuery({
    queryKey: ['stats'],
    queryFn: () => fetchJson<Stats>(`${API_BASE}/stats`),
    refetchInterval: 30000,
  })
}

// Groups
export function useGroups() {
  return useQuery({
    queryKey: ['groups'],
    queryFn: () => fetchJson<Group[]>(`${API_BASE}/groups`),
  })
}

// Mutations
export function useConfirmMatch() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (matchId: string) => {
      return fetchJson(`${API_BASE}/matches/${matchId}/confirm`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ matched_by: 'operator' }),
      })
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['matches'] })
      queryClient.invalidateQueries({ queryKey: ['offers'] })
      queryClient.invalidateQueries({ queryKey: ['requests'] })
      queryClient.invalidateQueries({ queryKey: ['stats'] })
    },
  })
}

export function useRejectMatch() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (matchId: string) => {
      return fetchJson(`${API_BASE}/matches/${matchId}/reject`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ matched_by: 'operator' }),
      })
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['matches'] })
      queryClient.invalidateQueries({ queryKey: ['stats'] })
    },
  })
}

export function useSyncGroups() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async () => {
      return fetchJson<Group[]>(`${API_BASE}/groups/sync`, { method: 'POST' })
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['groups'] })
    },
  })
}

export function useToggleGroup() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async ({
      jid,
      monitored,
    }: {
      jid: string
      monitored: boolean
    }) => {
      return fetchJson(`${API_BASE}/groups/${encodeURIComponent(jid)}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ monitored }),
      })
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['groups'] })
    },
  })
}
