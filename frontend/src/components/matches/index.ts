// Matches Components Index
// Export all matches-related components

export {
  StatsDashboard,
  calculateAverageConfidence,
  calculateConfidenceBands,
} from './stats-dashboard'
export {
  FilterPanel,
  defaultFilterState,
  CONFIDENCE_BANDS,
  type FilterState,
} from './filter-panel'
export {
  filterByStatus,
  filterByConfidenceThreshold,
  filterByMedicationSearch,
  filterByConfidenceBand,
  applyFilters,
  sortByConfidence,
  sortByDate,
  sortByMedication,
  applySorting,
  applyFiltersAndSorting,
} from './filter-utils'
export { MatchCard, getConfidenceColor, getStatusColor } from './match-card'
export { MatchDetail } from './match-detail'
export { MatchesList } from './matches-list'
export {
  ConfirmActionDialog,
  type ConfirmActionDialogProps,
} from './confirm-action-dialog'
export { showUndoToast, useUndoToast, type UndoToastProps } from './undo-toast'
