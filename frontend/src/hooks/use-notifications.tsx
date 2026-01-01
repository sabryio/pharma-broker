import { useState, useEffect, useCallback } from 'react'
import { toast } from 'sonner'

export interface NotificationSettings {
  // High priority review settings
  highPriorityEnabled: boolean
  highPriorityThreshold: number // Confidence below this is high priority (e.g., 60%)

  // Approval rate alert settings
  approvalRateEnabled: boolean
  approvalRateThreshold: number // Alert when rate drops below this (e.g., 70%)

  // Notification channels
  browserNotificationsEnabled: boolean
  inAppNotificationsEnabled: boolean
  emailNotificationsEnabled: boolean // For future backend integration
  emailAddress: string

  // Quiet hours
  quietHoursEnabled: boolean
  quietHoursStart: string // "22:00"
  quietHoursEnd: string // "08:00"
}

export interface NotificationHistoryEntry {
  id: string
  type: 'high_priority' | 'approval_rate' | 'custom'
  title: string
  message: string
  timestamp: Date
  read: boolean
  channels: ('browser' | 'in_app' | 'email')[]
  metadata?: {
    productName?: string
    confidence?: number
    approvalRate?: number
  }
}

const DEFAULT_SETTINGS: NotificationSettings = {
  highPriorityEnabled: true,
  highPriorityThreshold: 60,
  approvalRateEnabled: true,
  approvalRateThreshold: 70,
  browserNotificationsEnabled: false,
  inAppNotificationsEnabled: true,
  emailNotificationsEnabled: false,
  emailAddress: '',
  quietHoursEnabled: false,
  quietHoursStart: '22:00',
  quietHoursEnd: '08:00',
}

const STORAGE_KEY = 'pharmabroker-notification-settings'
const HISTORY_STORAGE_KEY = 'pharmabroker-notification-history'

export function useNotifications() {
  const [settings, setSettings] = useState<NotificationSettings>(() => {
    const stored = localStorage.getItem(STORAGE_KEY)
    return stored
      ? { ...DEFAULT_SETTINGS, ...JSON.parse(stored) }
      : DEFAULT_SETTINGS
  })

  const [notificationHistory, setNotificationHistory] = useState<
    NotificationHistoryEntry[]
  >(() => {
    const stored = localStorage.getItem(HISTORY_STORAGE_KEY)
    if (stored) {
      const parsed = JSON.parse(stored)
      return parsed.map((entry: NotificationHistoryEntry) => ({
        ...entry,
        timestamp: new Date(entry.timestamp),
      }))
    }
    return []
  })

  const [browserPermission, setBrowserPermission] =
    useState<NotificationPermission>('default')

  useEffect(() => {
    if ('Notification' in window) {
      setBrowserPermission(Notification.permission)
    }
  }, [])

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings))
  }, [settings])

  useEffect(() => {
    localStorage.setItem(
      HISTORY_STORAGE_KEY,
      JSON.stringify(notificationHistory),
    )
  }, [notificationHistory])

  const updateSettings = useCallback(
    (updates: Partial<NotificationSettings>) => {
      setSettings((prev) => ({ ...prev, ...updates }))
    },
    [],
  )

  const resetSettings = useCallback(() => {
    setSettings(DEFAULT_SETTINGS)
  }, [])

  const addToHistory = useCallback(
    (entry: Omit<NotificationHistoryEntry, 'id' | 'timestamp' | 'read'>) => {
      const newEntry: NotificationHistoryEntry = {
        ...entry,
        id: `notif-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
        timestamp: new Date(),
        read: false,
      }
      setNotificationHistory((prev) => [newEntry, ...prev].slice(0, 100)) // Keep last 100
      return newEntry
    },
    [],
  )

  const markAsRead = useCallback((id: string) => {
    setNotificationHistory((prev) =>
      prev.map((entry) => (entry.id === id ? { ...entry, read: true } : entry)),
    )
  }, [])

  const markAllAsRead = useCallback(() => {
    setNotificationHistory((prev) =>
      prev.map((entry) => ({ ...entry, read: true })),
    )
  }, [])

  const clearHistory = useCallback(() => {
    setNotificationHistory([])
  }, [])

  const deleteNotification = useCallback((id: string) => {
    setNotificationHistory((prev) => prev.filter((entry) => entry.id !== id))
  }, [])

  const unreadCount = notificationHistory.filter((n) => !n.read).length

  const requestBrowserPermission = useCallback(async () => {
    if (!('Notification' in window)) {
      toast.error('Browser notifications not supported', {
        description: "Your browser doesn't support push notifications.",
      })
      return false
    }

    const permission = await Notification.requestPermission()
    setBrowserPermission(permission)

    if (permission === 'granted') {
      updateSettings({ browserNotificationsEnabled: true })
      toast.success('Notifications enabled', {
        description:
          "You'll now receive browser notifications for important alerts.",
      })
      return true
    } else {
      toast.error('Permission denied', {
        description: 'Browser notifications were not enabled.',
      })
      return false
    }
  }, [updateSettings])

  const isInQuietHours = useCallback(() => {
    if (!settings.quietHoursEnabled) return false

    const now = new Date()
    const currentMinutes = now.getHours() * 60 + now.getMinutes()

    const [startHour, startMin] = settings.quietHoursStart
      .split(':')
      .map(Number)
    const [endHour, endMin] = settings.quietHoursEnd.split(':').map(Number)

    const startMinutes = startHour * 60 + startMin
    const endMinutes = endHour * 60 + endMin

    // Handle overnight quiet hours (e.g., 22:00 - 08:00)
    if (startMinutes > endMinutes) {
      return currentMinutes >= startMinutes || currentMinutes <= endMinutes
    }

    return currentMinutes >= startMinutes && currentMinutes <= endMinutes
  }, [
    settings.quietHoursEnabled,
    settings.quietHoursStart,
    settings.quietHoursEnd,
  ])

  const sendBrowserNotification = useCallback(
    (title: string, body: string, tag?: string) => {
      if (
        !settings.browserNotificationsEnabled ||
        browserPermission !== 'granted'
      )
        return false
      if (isInQuietHours()) return false

      try {
        new Notification(title, {
          body,
          icon: '/favicon.ico',
          tag: tag || 'pharmabroker-notification',
          requireInteraction: true,
        })
        return true
      } catch (error) {
        console.error('Failed to send browser notification:', error)
        return false
      }
    },
    [settings.browserNotificationsEnabled, browserPermission, isInQuietHours],
  )

  const sendInAppNotification = useCallback(
    (
      title: string,
      description: string,
      variant?: 'default' | 'destructive',
    ) => {
      if (!settings.inAppNotificationsEnabled) return false
      if (isInQuietHours()) return false

      if (variant === 'destructive') {
        toast.error(title, {
          description,
          duration: 8000,
        })
      } else {
        toast.success(title, {
          description,
          duration: 8000,
        })
      }

      return true
    },
    [settings.inAppNotificationsEnabled, isInQuietHours],
  )

  const notifyHighPriorityReview = useCallback(
    (productName: string, confidence: number) => {
      if (!settings.highPriorityEnabled) return null
      if (confidence >= settings.highPriorityThreshold) return null

      const title = 'High Priority Review'
      const message = `${productName} requires attention (${confidence}% confidence)`
      const channels: ('browser' | 'in_app')[] = []

      if (
        sendBrowserNotification(title, message, `high-priority-${productName}`)
      ) {
        channels.push('browser')
      }
      if (sendInAppNotification(title, message, 'destructive')) {
        channels.push('in_app')
      }

      if (channels.length > 0) {
        return addToHistory({
          type: 'high_priority',
          title,
          message,
          channels,
          metadata: { productName, confidence },
        })
      }
      return null
    },
    [
      settings.highPriorityEnabled,
      settings.highPriorityThreshold,
      sendBrowserNotification,
      sendInAppNotification,
      addToHistory,
    ],
  )

  const notifyLowApprovalRate = useCallback(
    (currentRate: number) => {
      if (!settings.approvalRateEnabled) return null
      if (currentRate >= settings.approvalRateThreshold) return null

      const title = 'Approval Rate Alert'
      const message = `Approval rate has dropped to ${currentRate.toFixed(1)}% (below ${settings.approvalRateThreshold}% threshold)`
      const channels: ('browser' | 'in_app')[] = []

      if (sendBrowserNotification(title, message, 'approval-rate-alert')) {
        channels.push('browser')
      }
      if (sendInAppNotification(title, message, 'destructive')) {
        channels.push('in_app')
      }

      if (channels.length > 0) {
        return addToHistory({
          type: 'approval_rate',
          title,
          message,
          channels,
          metadata: { approvalRate: currentRate },
        })
      }
      return null
    },
    [
      settings.approvalRateEnabled,
      settings.approvalRateThreshold,
      sendBrowserNotification,
      sendInAppNotification,
      addToHistory,
    ],
  )

  return {
    settings,
    updateSettings,
    resetSettings,
    browserPermission,
    requestBrowserPermission,
    notifyHighPriorityReview,
    notifyLowApprovalRate,
    sendInAppNotification,
    sendBrowserNotification,
    isInQuietHours,
    // History
    notificationHistory,
    unreadCount,
    markAsRead,
    markAllAsRead,
    clearHistory,
    deleteNotification,
  }
}
