import axios from 'axios'

// API base URL - uses Vite env var or falls back to localhost
const API_BASE_URL = import.meta.env.VITE_API_URL || 'http://localhost:8082'

// Create axios instance with default config
export const apiClient = axios.create({
  baseURL: API_BASE_URL,
  timeout: 30000,
  headers: {
    'Content-Type': 'application/json',
  },
})

// Response interceptor for error handling
apiClient.interceptors.response.use(
  (response) => response,
  (error) => {
    // Log errors in development
    if (import.meta.env.DEV) {
      console.error('[API Error]', error.response?.data || error.message)
    }

    // Transform error for consistent handling
    const message =
      error.response?.data?.error ||
      error.response?.data?.message ||
      error.message ||
      'An unexpected error occurred'

    return Promise.reject(new Error(message))
  },
)

export default apiClient
