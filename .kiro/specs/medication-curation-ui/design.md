# Design Document: Medication Curation UI

## Overview

This design adds medication curation capabilities to the existing review queue page (`frontend/src/routes/review-queue.tsx`). The implementation follows the existing patterns in the codebase: React Query for data fetching, Zod for schema validation, and the existing component library for UI consistency.

The feature integrates with the existing backend curation API (`/api/curation/*`) which provides endpoints for statistics, alias listing, suggestions, master creation, and approval workflows.

## Architecture

```mermaid
graph TB
    subgraph Frontend
        RQ[review-queue.tsx] --> MC[MedicationCuration Component]
        MC --> SL[AliasList]
        MC --> SP[SuggestionPanel]
        MC --> CS[CurationStats]
        MC --> CMD[CreateMasterDialog]

        SL --> UCH[useCurationHooks]
        SP --> UCH
        CS --> UCH
        CMD --> UCH

        UCH --> API[curation.ts API]
    end

    subgraph Backend
        API --> BE[/api/curation/*]
        BE --> DB[(Database)]
    end
```

## Components and Interfaces

### New Files to Create

```
frontend/src/
├── api/
│   └── curation.ts              # API client functions
├── schema/
│   └── curation.ts              # Zod schemas and types
├── hooks/
│   └── use-curation.ts          # React Query hooks
└── components/
    └── medication-curation/
        ├── index.ts             # Barrel export
        ├── types.ts             # Component-specific types
        ├── curation-stats.tsx   # Statistics panel
        ├── alias-list.tsx       # Pending aliases list
        ├── suggestion-panel.tsx # AI suggestions display
        ├── create-master-dialog.tsx # New master form
        └── curation-mode.tsx    # Main container component
```

### API Client Interface

```typescript
// frontend/src/api/curation.ts

interface CurationStats {
  totalAliases: number;
  pendingCount: number;
  approvedCount: number;
  rejectedCount: number;
  curationPercentage: number;
}

interface MedicationAlias {
  id: string;
  aliasName: string;
  aliasNameNormalized: string;
  masterMedicationId: string | null;
  aiSuggestionConfidence: number | null;
  curationStatus: "pending" | "approved" | "rejected";
  curatedBy: string | null;
  curatedAt: string | null;
  occurrenceCount: number;
  firstSeenAt: string;
  lastSeenAt: string;
}

interface MasterSuggestion {
  id: string;
  name: string;
  confidence: number;
  source: "semantic" | "fuzzy";
}

interface CreateMasterRequest {
  name: string;
  nameAr?: string;
  activeIngredient?: string;
  strength?: string;
  manufacturer?: string;
}

interface ApproveAliasRequest {
  aliasId: string;
  masterId: string;
  operatorId: string;
}

// API Functions
async function getCurationStats(): Promise<CurationStats>;
async function getAliases(params: {
  limit?: number;
  offset?: number;
  status?: string;
}): Promise<{ aliases: MedicationAlias[]; total: number }>;
async function getSuggestions(
  name: string,
  limit?: number
): Promise<{ suggestions: MasterSuggestion[] }>;
async function createMaster(
  data: CreateMasterRequest
): Promise<MedicationMaster>;
async function approveAlias(data: ApproveAliasRequest): Promise<void>;
```

### React Query Hooks Interface

```typescript
// frontend/src/hooks/use-curation.ts

function useCurationStats(): UseQueryResult<CurationStats>;
function useAliases(params: AliasParams): UseQueryResult<AliasListResponse>;
function useSuggestions(aliasName: string): UseQueryResult<SuggestionResponse>;
function useCreateMaster(): UseMutationResult<
  MedicationMaster,
  Error,
  CreateMasterRequest
>;
function useApproveAlias(): UseMutationResult<void, Error, ApproveAliasRequest>;
function useBulkApprove(): UseMutationResult<
  BulkResult,
  Error,
  ApproveAliasRequest[]
>;
```

### Component Props Interfaces

```typescript
// CurationStats
interface CurationStatsProps {
  stats: CurationStats | undefined;
  isLoading: boolean;
}

// AliasList
interface AliasListProps {
  aliases: MedicationAlias[];
  selectedId: string | null;
  onSelect: (alias: MedicationAlias) => void;
  onPageChange: (page: number) => void;
  total: number;
  pageSize: number;
  currentPage: number;
  isLoading: boolean;
}

// SuggestionPanel
interface SuggestionPanelProps {
  alias: MedicationAlias | null;
  suggestions: MasterSuggestion[];
  isLoading: boolean;
  onApprove: (masterId: string) => void;
  onCreateNew: () => void;
  selectedIndex: number;
  onSelectIndex: (index: number) => void;
}

// CreateMasterDialog
interface CreateMasterDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  aliasName: string;
  onCreated: (master: MedicationMaster) => void;
}
```

## Data Models

### Zod Schemas

```typescript
// frontend/src/schema/curation.ts

import { z } from "zod";

export const CurationStatusSchema = z.enum(["pending", "approved", "rejected"]);

export const CurationStatsSchema = z.object({
  totalAliases: z.number(),
  pendingCount: z.number(),
  approvedCount: z.number(),
  rejectedCount: z.number(),
  curationPercentage: z.number(),
});

export const MedicationAliasSchema = z.object({
  id: z.string().uuid(),
  aliasName: z.string(),
  aliasNameNormalized: z.string(),
  masterMedicationId: z.string().uuid().nullable(),
  aiSuggestionConfidence: z.number().nullable(),
  curationStatus: CurationStatusSchema,
  curatedBy: z.string().nullable(),
  curatedAt: z.string().nullable(),
  occurrenceCount: z.number(),
  firstSeenAt: z.string(),
  lastSeenAt: z.string(),
});

export const MasterSuggestionSchema = z.object({
  id: z.string().uuid(),
  name: z.string(),
  confidence: z.number().min(0).max(1),
  source: z.enum(["semantic", "fuzzy"]),
});

export const AliasListResponseSchema = z.object({
  aliases: z.array(MedicationAliasSchema),
  total: z.number(),
});

export const SuggestionResponseSchema = z.object({
  suggestions: z.array(MasterSuggestionSchema),
});

export const CreateMasterRequestSchema = z.object({
  name: z.string().min(1, "Name is required"),
  nameAr: z.string().optional(),
  activeIngredient: z.string().optional(),
  strength: z.string().optional(),
  manufacturer: z.string().optional(),
});

export const ApproveAliasRequestSchema = z.object({
  aliasId: z.string().uuid(),
  masterId: z.string().uuid(),
  operatorId: z.string(),
});
```

## Correctness Properties

_A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees._

### Property 1: Stats Display and Refresh

_For any_ curation statistics response from the API, the CurationStats component SHALL render all four metrics (total, pending, approved, percentage) and _for any_ successful curation action, the stats query SHALL be invalidated to trigger a refresh.

**Validates: Requirements 1.1, 1.2**

### Property 2: Alias List Rendering

_For any_ list of medication aliases, the AliasList component SHALL render each alias with its name, occurrence count, first seen date, and AI suggestion confidence (if available).

**Validates: Requirements 2.2**

### Property 3: Alias Filtering

_For any_ filter status selection (pending, approved, all), the API call SHALL include the correct status parameter and the returned aliases SHALL match the selected filter.

**Validates: Requirements 2.4**

### Property 4: Suggestions Sorting

_For any_ list of master medication suggestions, they SHALL be sorted by confidence score in descending order (highest confidence first).

**Validates: Requirements 3.3**

### Property 5: Suggestion Panel Rendering

_For any_ master suggestion, the SuggestionPanel SHALL display the suggestion name, confidence score as percentage, and source indicator (semantic/fuzzy).

**Validates: Requirements 3.2**

### Property 6: Approval State Changes

_For any_ successful alias approval, the alias SHALL be removed from the pending list AND the alias status SHALL be updated to APPROVED in any cached data.

**Validates: Requirements 4.2, 4.3**

### Property 7: Approval API Call

_For any_ approval action, the API call SHALL include the correct alias_id, master_id, and operator_id parameters.

**Validates: Requirements 4.1**

### Property 8: Create Master Form Validation

_For any_ CreateMasterDialog submission, the canonical name field SHALL be required (non-empty) and optional fields (Arabic name, strength, ingredient, manufacturer) SHALL be accepted without validation errors.

**Validates: Requirements 5.2**

### Property 9: Keyboard Navigation

_For any_ keyboard navigation in curation mode, arrow keys (↑↓) SHALL change the selected alias index within bounds, and number keys (1-5) SHALL select the corresponding suggestion if it exists.

**Validates: Requirements 6.1, 6.2**

### Property 10: Mode State Preservation

_For any_ mode switch between "Match Review" and "Medication Curation", the state of the previous mode (selected items, scroll position, filters) SHALL be preserved when switching back.

**Validates: Requirements 7.2**

### Property 11: Uncurated Medication Indicator

_For any_ match review item where the medication has `medication_curated = false` or `master_medication_id = null`, a visual indicator SHALL be displayed.

**Validates: Requirements 7.4**

### Property 12: Bulk Selection

_For any_ bulk mode selection, multiple aliases SHALL be selectable via checkboxes, and the selection state SHALL be maintained across pagination.

**Validates: Requirements 8.1**

### Property 13: Bulk Approval Validation

_For any_ bulk approval operation, only aliases with at least one suggestion above 90% confidence SHALL be eligible, and the bulk approve button SHALL be disabled if any selected alias lacks a high-confidence suggestion.

**Validates: Requirements 8.2**

### Property 14: Bulk Operation Resilience

_For any_ bulk approval operation where some items fail, the operation SHALL continue processing remaining items and SHALL report both successes and failures at completion.

**Validates: Requirements 8.5**

## Error Handling

### API Error Handling

```typescript
// All API calls use the existing error interceptor in client.ts
// Additional error handling for curation-specific errors:

interface CurationError {
  code:
    | "DUPLICATE_MASTER"
    | "ALIAS_NOT_FOUND"
    | "MASTER_NOT_FOUND"
    | "ALREADY_APPROVED";
  message: string;
}

// Error display via toast notifications (using sonner)
function handleCurationError(error: CurationError) {
  switch (error.code) {
    case "DUPLICATE_MASTER":
      toast.error("Master medication already exists", {
        description: error.message,
      });
      break;
    case "ALIAS_NOT_FOUND":
      toast.error("Alias not found", {
        description: "The alias may have been deleted",
      });
      break;
    case "ALREADY_APPROVED":
      toast.warning("Already approved", {
        description: "This alias was already curated",
      });
      break;
    default:
      toast.error("Curation error", { description: error.message });
  }
}
```

### Optimistic Updates

```typescript
// Approval mutation with optimistic update
const useApproveAlias = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: approveAlias,
    onMutate: async (variables) => {
      await queryClient.cancelQueries({ queryKey: ["curation", "aliases"] });

      const previousAliases = queryClient.getQueryData(["curation", "aliases"]);

      // Optimistically remove from pending list
      queryClient.setQueriesData(
        { queryKey: ["curation", "aliases"] },
        (old) => ({
          ...old,
          aliases: old.aliases.filter((a) => a.id !== variables.aliasId),
          total: old.total - 1,
        })
      );

      // Optimistically update stats
      queryClient.setQueryData(["curation", "stats"], (old) => ({
        ...old,
        pendingCount: old.pendingCount - 1,
        approvedCount: old.approvedCount + 1,
        curationPercentage: ((old.approvedCount + 1) / old.totalAliases) * 100,
      }));

      return { previousAliases };
    },
    onError: (err, variables, context) => {
      queryClient.setQueriesData(
        { queryKey: ["curation", "aliases"] },
        context.previousAliases
      );
      handleCurationError(err);
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["curation"] });
    },
  });
};
```

## Testing Strategy

### Unit Tests

Unit tests will verify specific examples and edge cases:

1. **Component rendering tests**: Verify components render correctly with mock data
2. **Form validation tests**: Verify CreateMasterDialog validates required fields
3. **Error state tests**: Verify error messages display correctly
4. **Empty state tests**: Verify "No suggestions found" message appears when appropriate
5. **Keyboard shortcut tests**: Verify specific key combinations trigger correct actions

### Property-Based Tests

Property-based tests will use `fast-check` to verify universal properties across many generated inputs:

1. **Stats rendering property**: For any valid stats object, all metrics are displayed
2. **Alias list rendering property**: For any list of aliases, all required fields are shown
3. **Sorting property**: For any list of suggestions, they are sorted by confidence descending
4. **Filter property**: For any filter selection, API is called with correct parameter
5. **Approval state property**: For any successful approval, alias is removed from pending
6. **Bulk validation property**: For any selection, only high-confidence items are bulk-approvable

### Test Configuration

- Property tests: minimum 100 iterations per property
- Test framework: Vitest with React Testing Library
- Property testing library: fast-check
- Each property test tagged with: `**Feature: medication-curation-ui, Property N: {property_text}**`

### Test File Structure

```
frontend/src/
├── api/
│   └── __tests__/
│       └── curation.test.ts
├── hooks/
│   └── __tests__/
│       └── use-curation.test.ts
└── components/
    └── medication-curation/
        └── __tests__/
            ├── curation-stats.test.tsx
            ├── alias-list.test.tsx
            ├── suggestion-panel.test.tsx
            └── curation-mode.test.tsx
```
