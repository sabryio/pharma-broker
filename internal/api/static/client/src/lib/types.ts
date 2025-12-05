// API Types for PharmaBroker

export interface Offer {
  id: string
  raw_message_id: string
  source_phone: string
  source_name: string
  source_group: string
  group_name: string
  medication: string
  medication_raw: string
  quantity: number
  unit: string
  price: number
  currency: string
  expiry_date?: string
  batch_number: string
  notes: string
  raw_message: string
  status: 'ACTIVE' | 'MATCHED' | 'EXPIRED'
  created_at: string
  updated_at: string
}

export interface Request {
  id: string
  raw_message_id: string
  source_phone: string
  source_name: string
  source_group: string
  group_name: string
  medication: string
  medication_raw: string
  quantity: number
  unit: string
  max_price: number
  currency: string
  urgent: boolean
  notes: string
  raw_message: string
  status: 'ACTIVE' | 'MATCHED' | 'EXPIRED'
  created_at: string
  updated_at: string
}

export interface Match {
  id: string
  offer_id: string
  request_id: string
  score: number
  status: 'PENDING' | 'CONFIRMED' | 'REJECTED'
  matched_by: string
  matched_at?: string
  created_at: string
  offer?: Offer
  request?: Request
}

export interface Group {
  jid: string
  name: string
  description: string
  monitored: boolean
  message_count: number
  last_message?: string
  added_at: string
}

export interface Stats {
  active_offers: number
  active_requests: number
  pending_matches: number
  confirmed_today: number
}

// AI Analysis types
export interface AnalyzeItem {
  type: 'OFFER' | 'REQUEST' | 'BOTH'
  medication: string
  medication_raw: string
  quantity: number
  unit?: string
  price?: number
  max_price?: number
  currency?: string
  expiry_date?: string
  batch_number?: string
  urgent?: boolean
  notes?: string
}

export interface AnalyzeResult {
  items: AnalyzeItem[]
  raw_json?: string
}

export interface AnalyzeRequest {
  text: string
  source_name?: string
  group_name?: string
}

// Configuration types
export interface AppConfig {
  auto_parse_enabled: boolean
  skip_own_messages: boolean
  match_threshold: number
  batch_size: number
  process_delay_seconds: number
  system_prompt?: string
  response_format?: string
}

export interface ApiResponse<T> {
  success: boolean
  data?: T
  error?: string
  meta?: {
    total: number
    limit: number
    offset: number
  }
}
