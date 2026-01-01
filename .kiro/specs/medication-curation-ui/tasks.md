# Implementation Plan: Medication Curation UI

## Overview

This implementation adds medication curation capabilities to the existing review queue page. The tasks are organized to build incrementally: schemas first, then API client, hooks, components, and finally integration with the existing review queue.

## Tasks

- [ ] 1. Create Zod schemas and TypeScript types
  - [ ] 1.1 Create `frontend/src/schema/curation.ts` with all Zod schemas
    - Define CurationStatusSchema, CurationStatsSchema, MedicationAliasSchema
    - Define MasterSuggestionSchema, AliasListResponseSchema, SuggestionResponseSchema
    - Define CreateMasterRequestSchema, ApproveAliasRequestSchema
    - Export inferred TypeScript types
    - _Requirements: 2.2, 3.2, 5.2_

- [ ] 2. Create API client functions
  - [ ] 2.1 Create `frontend/src/api/curation.ts` with API functions
    - Implement getCurationStats() calling `/api/curation/stats`
    - Implement getAliases() calling `/api/curation/aliases` with pagination
    - Implement getSuggestions() calling `/api/curation/suggestions`
    - Implement createMaster() calling POST `/api/curation/master`
    - Implement approveAlias() calling POST `/api/curation/approve`
    - Use Zod schemas for response validation
    - _Requirements: 1.3, 2.3, 3.1, 4.1, 5.3_

- [ ] 3. Create React Query hooks
  - [ ] 3.1 Create `frontend/src/hooks/use-curation.ts` with query hooks
    - Implement useCurationStats() with 30s stale time and refetch interval
    - Implement useAliases() with pagination support and keepPreviousData
    - Implement useSuggestions() with enabled flag based on selected alias
    - _Requirements: 1.1, 1.2, 2.1, 3.1_

  - [ ] 3.2 Add mutation hooks with optimistic updates
    - Implement useApproveAlias() with optimistic removal from pending list
    - Implement useCreateMaster() with success callback for auto-linking
    - Implement useBulkApprove() for sequential processing with progress
    - _Requirements: 4.1, 4.2, 4.3, 5.3, 5.4, 8.3_

  - [ ] 3.3 Write property test for approval state changes
    - **Property 6: Approval State Changes**
    - **Validates: Requirements 4.2, 4.3**

- [ ] 4. Checkpoint - Verify API integration
  - Ensure all API functions work with backend
  - Ensure hooks properly cache and invalidate data
  - Ask the user if questions arise

- [ ] 5. Create CurationStats component
  - [ ] 5.1 Create `frontend/src/components/medication-curation/curation-stats.tsx`
    - Display total aliases, pending count, approved count, curation percentage
    - Use existing card styling from review queue
    - Handle loading and error states
    - _Requirements: 1.1, 1.2_

  - [ ] 5.2 Write property test for stats rendering
    - **Property 1: Stats Display and Refresh**
    - **Validates: Requirements 1.1, 1.2**

- [ ] 6. Create AliasList component
  - [ ] 6.1 Create `frontend/src/components/medication-curation/alias-list.tsx`
    - Render list of aliases with name, occurrence count, first seen, confidence
    - Support selection with visual highlight
    - Implement pagination controls
    - Support filtering by status (pending, approved, all)
    - _Requirements: 2.1, 2.2, 2.3, 2.4_

  - [ ] 6.2 Write property test for alias list rendering
    - **Property 2: Alias List Rendering**
    - **Validates: Requirements 2.2**

  - [ ] 6.3 Write property test for alias filtering
    - **Property 3: Alias Filtering**
    - **Validates: Requirements 2.4**

- [ ] 7. Create SuggestionPanel component
  - [ ] 7.1 Create `frontend/src/components/medication-curation/suggestion-panel.tsx`
    - Display up to 5 suggestions sorted by confidence
    - Show name, confidence percentage, source badge (semantic/fuzzy)
    - Support selection with keyboard (1-5 keys)
    - Show "No matches found" with create button when empty
    - Approve button for selected suggestion
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 4.5_

  - [ ] 7.2 Write property test for suggestions sorting
    - **Property 4: Suggestions Sorting**
    - **Validates: Requirements 3.3**

  - [ ] 7.3 Write property test for suggestion panel rendering
    - **Property 5: Suggestion Panel Rendering**
    - **Validates: Requirements 3.2**

- [ ] 8. Create CreateMasterDialog component
  - [ ] 8.1 Create `frontend/src/components/medication-curation/create-master-dialog.tsx`
    - Form with required name field
    - Optional fields: Arabic name, strength, active ingredient, manufacturer
    - Form validation using Zod schema
    - Submit handler calling createMaster mutation
    - Auto-link to current alias on success
    - Error handling for duplicate names
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5_

  - [ ] 8.2 Write property test for form validation
    - **Property 8: Create Master Form Validation**
    - **Validates: Requirements 5.2**

- [ ] 9. Checkpoint - Verify individual components
  - Test each component in isolation
  - Ensure all tests pass
  - Ask the user if questions arise

- [ ] 10. Create CurationMode container component
  - [ ] 10.1 Create `frontend/src/components/medication-curation/curation-mode.tsx`
    - Compose CurationStats, AliasList, SuggestionPanel, CreateMasterDialog
    - Manage selected alias state
    - Handle keyboard navigation (↑↓ for aliases, 1-5 for suggestions)
    - Handle Enter to approve, Escape to cancel
    - Display keyboard hints panel
    - _Requirements: 6.1, 6.2, 6.3, 6.5_

  - [ ] 10.2 Write property test for keyboard navigation
    - **Property 9: Keyboard Navigation**
    - **Validates: Requirements 6.1, 6.2**

- [ ] 11. Create barrel export
  - [ ] 11.1 Create `frontend/src/components/medication-curation/index.ts`
    - Export all components and types
    - _Requirements: N/A (code organization)_

- [ ] 12. Add bulk curation support
  - [ ] 12.1 Add bulk selection mode to AliasList
    - Add checkbox column when in bulk mode
    - Track selected alias IDs
    - Select all / deselect all functionality
    - _Requirements: 8.1_

  - [ ] 12.2 Add bulk approval logic to CurationMode
    - Validate all selected have 90%+ confidence suggestions
    - Process approvals sequentially
    - Display progress indicator
    - Report successes and failures
    - _Requirements: 8.2, 8.3, 8.4, 8.5_

  - [ ] 12.3 Write property test for bulk approval validation
    - **Property 13: Bulk Approval Validation**
    - **Validates: Requirements 8.2**

  - [ ] 12.4 Write property test for bulk operation resilience
    - **Property 14: Bulk Operation Resilience**
    - **Validates: Requirements 8.5**

- [ ] 13. Integrate with review-queue.tsx
  - [ ] 13.1 Add mode toggle to review-queue.tsx
    - Add tab/toggle for "Match Review" and "Medication Curation" modes
    - Preserve state when switching modes
    - Support Tab key to switch modes
    - _Requirements: 7.1, 7.2, 6.4_

  - [ ] 13.2 Add uncurated medication indicator to match cards
    - Check medication_curated flag on offers/requests
    - Display visual indicator (icon/badge) for uncurated items
    - _Requirements: 7.4_

  - [ ] 13.3 Write property test for mode state preservation
    - **Property 10: Mode State Preservation**
    - **Validates: Requirements 7.2**

  - [ ] 13.4 Write property test for uncurated indicator
    - **Property 11: Uncurated Medication Indicator**
    - **Validates: Requirements 7.4**

- [ ] 14. Final checkpoint - Full integration testing
  - Ensure all components work together
  - Ensure all tests pass
  - Verify keyboard shortcuts work end-to-end
  - Ask the user if questions arise

## Notes

- All tasks including property-based tests are required for comprehensive coverage
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation
- Property tests validate universal correctness properties using fast-check
- Unit tests validate specific examples and edge cases
- The implementation follows existing patterns from `use-review-queue.ts` and `review-queue.tsx`
