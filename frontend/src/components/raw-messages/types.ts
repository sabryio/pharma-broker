// Raw Messages Component Types
import type {
  RawMessage,
  ProcessingStatus,
  SortField,
  SortOrder,
} from '@/schema/raw-message'

export type { RawMessage, ProcessingStatus, SortField, SortOrder }

export interface RawMessageFilters {
  search: string
  status: ProcessingStatus
  startDate: string
  endDate: string
  sortBy: SortField
  sortOrder: SortOrder
}

export const defaultFilters: RawMessageFilters = {
  search: '',
  status: 'all',
  startDate: '',
  endDate: '',
  sortBy: 'timestamp',
  sortOrder: 'desc',
}

export interface PaginationState {
  pageIndex: number
  pageSize: number
}

export const defaultPagination: PaginationState = {
  pageIndex: 0,
  pageSize: 25,
}
