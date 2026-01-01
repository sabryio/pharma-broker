# Requirements Document

## Introduction

This feature adds medication curation capabilities to the existing review queue UI (`frontend/src/routes/review-queue.tsx`). Users will be able to review AI-parsed medication names, approve/reject them, link aliases to master medications, and create new master medications when needed. Changes will propagate to all related tables (offers, requests, medication_aliases) through the existing backend curation API.

## Glossary

- **Review_Queue**: The existing UI component for reviewing low-confidence AI matches
- **Medication_Alias**: A parsed medication name that may have variations (e.g., "Panadol 500mg", "بنادول")
- **Master_Medication**: The canonical/normalized medication record with standardized name, strength, and metadata
- **Curation_Status**: The state of an alias: PENDING, APPROVED, or REJECTED
- **Curation_API**: The existing REST API at `/api/curation/*` providing stats, aliases, suggestions, and approval endpoints
- **AI_Suggestion**: A recommended master medication match generated via semantic or fuzzy search

## Requirements

### Requirement 1: Display Curation Statistics

**User Story:** As a reviewer, I want to see medication curation statistics, so that I can understand the overall curation progress and workload.

#### Acceptance Criteria

1. WHEN the review queue page loads, THE Curation_Stats_Panel SHALL display total aliases, pending count, approved count, and curation percentage
2. WHEN curation actions are performed, THE Curation_Stats_Panel SHALL refresh to reflect updated statistics
3. THE Curation_Stats_Panel SHALL fetch data from the `/api/curation/stats` endpoint

### Requirement 2: List Pending Medication Aliases

**User Story:** As a reviewer, I want to see a list of pending medication aliases, so that I can review and curate them.

#### Acceptance Criteria

1. WHEN the curation tab is selected, THE Alias_List SHALL display pending medication aliases with pagination
2. THE Alias_List SHALL show alias name, occurrence count, first seen date, and AI suggestion confidence for each item
3. WHEN a user scrolls or clicks pagination, THE Alias_List SHALL load additional aliases from `/api/curation/aliases`
4. THE Alias_List SHALL support filtering by curation status (pending, approved, all)

### Requirement 3: View AI Suggestions for Aliases

**User Story:** As a reviewer, I want to see AI-generated suggestions for matching master medications, so that I can quickly approve correct mappings.

#### Acceptance Criteria

1. WHEN a user selects an alias, THE Suggestion_Panel SHALL display up to 5 master medication suggestions
2. THE Suggestion_Panel SHALL show suggestion name, confidence score, and source (semantic/fuzzy) for each suggestion
3. WHEN suggestions are displayed, THE Suggestion_Panel SHALL sort them by confidence score descending
4. IF no suggestions are found, THEN THE Suggestion_Panel SHALL display a "No matches found" message with option to create new master

### Requirement 4: Approve Alias Mapping

**User Story:** As a reviewer, I want to approve an alias and link it to a master medication, so that the mapping is saved and applied to all related records.

#### Acceptance Criteria

1. WHEN a user clicks approve on a suggestion, THE System SHALL call `/api/curation/approve` with alias_id, master_id, and operator_id
2. WHEN approval succeeds, THE System SHALL update the alias status to APPROVED in the UI
3. WHEN approval succeeds, THE System SHALL remove the alias from the pending list
4. WHEN approval fails, THE System SHALL display an error toast with the failure reason
5. THE System SHALL support keyboard shortcut (Enter) to approve the top suggestion

### Requirement 5: Create New Master Medication

**User Story:** As a reviewer, I want to create a new master medication when no suitable match exists, so that I can properly categorize new medications.

#### Acceptance Criteria

1. WHEN a user clicks "Create New Master", THE Create_Master_Dialog SHALL open with a form
2. THE Create_Master_Dialog SHALL require canonical name and optionally accept Arabic name, strength, active ingredient, and manufacturer
3. WHEN the form is submitted, THE System SHALL call `/api/curation/master` to create the master medication
4. WHEN creation succeeds, THE System SHALL automatically link the current alias to the new master
5. IF creation fails due to duplicate name, THEN THE System SHALL display an appropriate error message

### Requirement 6: Keyboard Navigation Support

**User Story:** As a power user, I want to use keyboard shortcuts for curation actions, so that I can review medications efficiently.

#### Acceptance Criteria

1. WHEN in curation mode, THE System SHALL support arrow keys (↑↓) to navigate between aliases
2. WHEN viewing suggestions, THE System SHALL support number keys (1-5) to select a suggestion
3. WHEN a suggestion is selected, THE System SHALL support Enter to approve and Escape to cancel
4. THE System SHALL support Tab to switch between match review and curation tabs
5. WHEN keyboard shortcuts are available, THE System SHALL display a keyboard hint panel

### Requirement 7: Integration with Existing Review Queue

**User Story:** As a reviewer, I want curation to be integrated into the existing review queue workflow, so that I can handle both match reviews and medication curation in one place.

#### Acceptance Criteria

1. THE Review_Queue page SHALL have a tab or toggle to switch between "Match Review" and "Medication Curation" modes
2. WHEN switching modes, THE System SHALL preserve the state of the other mode
3. THE System SHALL share the same layout and styling as the existing review queue components
4. WHEN a match contains an uncurated medication, THE System SHALL display a visual indicator

### Requirement 8: Bulk Curation Actions

**User Story:** As a reviewer, I want to approve multiple aliases at once when they have high-confidence suggestions, so that I can process the queue faster.

#### Acceptance Criteria

1. WHEN in bulk mode, THE System SHALL allow selecting multiple aliases with checkboxes
2. THE System SHALL only allow bulk approval for aliases with suggestions above 90% confidence
3. WHEN bulk approve is clicked, THE System SHALL process all selected aliases sequentially
4. THE System SHALL display progress and results of bulk operations
5. IF any bulk operation fails, THEN THE System SHALL continue with remaining items and report failures at the end
