import { toast } from 'sonner'

/**
 * Toast utility for consistent notifications across the app.
 * Uses Sonner for reliable, accessible toasts with RTL support.
 */

// Success notifications
export const showSuccess = (message: string, description?: string) => {
  toast.success(message, {
    description,
    position: 'bottom-left',
    duration: 3000,
  })
}

// Error notifications
export const showError = (message: string, description?: string) => {
  toast.error(message, {
    description,
    position: 'bottom-left',
    duration: 5000,
  })
}

// Info notifications
export const showInfo = (message: string, description?: string) => {
  toast.info(message, {
    description,
    position: 'bottom-left',
    duration: 4000,
  })
}

// Warning notifications
export const showWarning = (message: string, description?: string) => {
  toast.warning(message, {
    description,
    position: 'bottom-left',
    duration: 4000,
  })
}

// Loading toast with promise
export const showLoading = <T>(
  promise: Promise<T>,
  messages: {
    loading: string
    success: string
    error: string
  },
) => {
  return toast.promise(promise, {
    loading: messages.loading,
    success: messages.success,
    error: messages.error,
    position: 'bottom-left',
  })
}

// Action reminder toast (important - stays longer)
export const showReminder = (
  message: string,
  action?: {
    label: string
    onClick: () => void
  },
) => {
  toast.info(message, {
    position: 'bottom-left',
    duration: 10000, // 10 seconds for important reminders
    action: action
      ? {
          label: action.label,
          onClick: action.onClick,
        }
      : undefined,
  })
}

// Dismissible notification for tasks
export const showTask = (
  message: string,
  options?: {
    description?: string
    onDismiss?: () => void
  },
) => {
  toast(message, {
    description: options?.description,
    position: 'bottom-left',
    duration: Infinity, // Stays until manually dismissed
    dismissible: true,
    onDismiss: options?.onDismiss,
  })
}

// Export toast for custom usage
export { toast }
