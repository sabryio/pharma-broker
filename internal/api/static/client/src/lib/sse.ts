import { useEffect, useCallback, useRef } from 'react'
import { useQueryClient } from '@tanstack/react-query'

export function useSSE(onStatusChange?: (connected: boolean) => void) {
  const queryClient = useQueryClient()
  const eventSourceRef = useRef<EventSource | null>(null)
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const connect = useCallback(() => {
    if (eventSourceRef.current) {
      eventSourceRef.current.close()
    }

    const es = new EventSource('/api/events')
    eventSourceRef.current = es

    es.addEventListener('connected', () => {
      onStatusChange?.(true)
    })

    es.addEventListener('new_offer', () => {
      queryClient.invalidateQueries({ queryKey: ['offers'] })
      queryClient.invalidateQueries({ queryKey: ['stats'] })
    })

    es.addEventListener('new_request', () => {
      queryClient.invalidateQueries({ queryKey: ['requests'] })
      queryClient.invalidateQueries({ queryKey: ['stats'] })
    })

    es.addEventListener('new_match', () => {
      queryClient.invalidateQueries({ queryKey: ['matches'] })
      queryClient.invalidateQueries({ queryKey: ['stats'] })
    })

    es.addEventListener('match_confirmed', () => {
      queryClient.invalidateQueries({ queryKey: ['matches'] })
      queryClient.invalidateQueries({ queryKey: ['offers'] })
      queryClient.invalidateQueries({ queryKey: ['requests'] })
      queryClient.invalidateQueries({ queryKey: ['stats'] })
    })

    es.addEventListener('heartbeat', () => {
      onStatusChange?.(true)
    })

    es.onerror = () => {
      onStatusChange?.(false)
      es.close()
      reconnectTimeoutRef.current = setTimeout(connect, 5000)
    }
  }, [queryClient, onStatusChange])

  useEffect(() => {
    connect()
    return () => {
      eventSourceRef.current?.close()
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current)
      }
    }
  }, [connect])
}

export function timeAgo(dateStr?: string): string {
  if (!dateStr) return ''
  const date = new Date(dateStr)
  const now = new Date()
  const diff = Math.floor((now.getTime() - date.getTime()) / 1000)

  if (diff < 60) return 'just now'
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  return `${Math.floor(diff / 86400)}d ago`
}
