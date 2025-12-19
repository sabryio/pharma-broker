# Data Quality & Table Activation Plan

> **Goal**: Fix 106 duplicate offers and activate unused tables for production readiness.

## Overview

| Phase | Task                         | Priority | Est. Time |
| ----- | ---------------------------- | -------- | --------- |
| 1     | Investigate Duplicate Offers | High     | 30 min    |
| 2     | Cleanup Duplicate Offers     | High     | 1 hour    |
| 3     | Prevent Future Duplicates    | High     | Done ✅   |
| 4     | Activate Audit Logs          | Medium   | 2 hours   |
| 5     | Activate Match Feedback      | Medium   | 2 hours   |
| 6     | Activate Weight History      | Low      | 1 hour    |

---

## Phase 1: Investigate Duplicate Offers

### 1.1 Identify Duplicates

```sql
-- Find duplicate offers (same sender, medication, within 10 min)
SELECT
    source_phone,
    medication,
    COUNT(*) as count,
    MIN(created_at) as first_seen,
    MAX(created_at) as last_seen,
    EXTRACT(EPOCH FROM (MAX(created_at) - MIN(created_at)))/60 as span_minutes
FROM offers
GROUP BY source_phone, LOWER(medication)
HAVING COUNT(*) > 1
ORDER BY count DESC
LIMIT 20;
```

### 1.2 Analyze Root Cause

```sql
-- Check if duplicates are from different groups (cross-posts)
SELECT
    o.source_phone,
    o.medication,
    o.source_group,
    o.group_name,
    o.created_at,
    rm.group_jid
FROM offers o
JOIN raw_messages rm ON o.raw_message_id = rm.id
WHERE (o.source_phone, LOWER(o.medication)) IN (
    SELECT source_phone, LOWER(medication)
    FROM offers
    GROUP BY source_phone, LOWER(medication)
    HAVING COUNT(*) > 1
)
ORDER BY o.source_phone, o.medication, o.created_at;
```

### 1.3 Document Findings

- [ ] Record number of duplicates per category
- [ ] Identify if cross-posts or true duplicates
- [ ] Note time windows between duplicates

---

## Phase 2: Cleanup Duplicate Offers

### 2.1 Backup Before Cleanup

```sql
-- Create backup table
CREATE TABLE offers_backup_YYYYMMDD AS
SELECT * FROM offers;

-- Verify backup
SELECT COUNT(*) FROM offers_backup_YYYYMMDD;
```

### 2.2 Identify Offers to Keep (Oldest per group)

```sql
-- Find IDs to KEEP (oldest offer per sender+medication)
WITH ranked AS (
    SELECT
        id,
        source_phone,
        medication,
        ROW_NUMBER() OVER (
            PARTITION BY source_phone, LOWER(medication)
            ORDER BY created_at ASC
        ) as rn
    FROM offers
    WHERE status = 'ACTIVE'
)
SELECT id FROM ranked WHERE rn = 1;
```

### 2.3 Mark Duplicates as EXPIRED

```sql
-- Mark duplicates as EXPIRED (preserves data)
WITH to_expire AS (
    SELECT id FROM (
        SELECT
            id,
            ROW_NUMBER() OVER (
                PARTITION BY source_phone, LOWER(medication)
                ORDER BY created_at ASC
            ) as rn
        FROM offers
        WHERE status = 'ACTIVE'
    ) ranked
    WHERE rn > 1
)
UPDATE offers
SET status = 'EXPIRED', updated_at = NOW()
WHERE id IN (SELECT id FROM to_expire);

-- Verify cleanup
SELECT status, COUNT(*) FROM offers GROUP BY status;
```

### 2.4 Verification

- [ ] Run duplicate query again (should return 0)
- [ ] Verify active offer count is reasonable
- [ ] Test that matching still works

---

## Phase 3: Prevent Future Duplicates ✅

**Already implemented:**

- `FindRecentDuplicate()` method in OfferRepo
- Dedup check in `processor.go` before saving
- Configurable via `parser.dedup_window` (default: 10m)
- Set to `0` to disable

---

## Phase 4: Activate Audit Logs

### 4.1 Current State

The `audit_logs` table exists but is empty. The `AuditRepository` interface exists.

### 4.2 Key Events to Log

| Event             | When                        | Priority |
| ----------------- | --------------------------- | -------- |
| `MATCH_CONFIRMED` | Match confirmed via API/bot | High     |
| `MATCH_REJECTED`  | Match rejected via API/bot  | High     |
| `WEIGHTS_APPLIED` | Scoring weights updated     | Medium   |
| `CONFIG_CHANGED`  | System config modified      | Medium   |
| `OFFER_EXPIRED`   | Offer manually expired      | Low      |

### 4.3 Implementation Steps

1. **Verify AuditRepository is injected** into handlers:

   ```go
   // In match_handler.go after confirmation:
   if h.auditRepo != nil {
       h.auditRepo.Log(ctx, entity.AuditMatchConfirmed, id, "Confirmed by "+req.MatchedBy)
   }
   ```

2. **Add audit calls** in:

   - [ ] `ConfirmMatchGin` handler
   - [ ] `RejectMatchGin` handler
   - [ ] `UpdateWeights` handler
   - [ ] `UpdateConfig` handler

3. **Verify logs are written**:
   ```sql
   SELECT * FROM audit_logs ORDER BY created_at DESC LIMIT 10;
   ```

---

## Phase 5: Activate Match Feedback

### 5.1 Purpose

Collect operator feedback on match quality to enable adaptive learning.

### 5.2 Current State

- `match_feedback` table exists
- `FeedbackRepository` interface defined
- `RecordFeedback` method available

### 5.3 Implementation Steps

1. **Add feedback endpoint** (if not exists):

   ```
   POST /api/matches/{id}/feedback
   {
     "decision": "CONFIRMED|REJECTED",
     "reason": "optional reason",
     "operator_id": "operator123"
   }
   ```

2. **Record feedback on confirm/reject**:

   ```go
   // After successful confirmation:
   h.feedbackRepo.RecordFeedback(ctx, &entity.MatchFeedback{
       MatchID:    id,
       Decision:   "CONFIRMED",
       OperatorID: req.MatchedBy,
       Reason:     req.Notes,
   })
   ```

3. **Verify feedback is recorded**:
   ```sql
   SELECT * FROM match_feedback ORDER BY created_at DESC LIMIT 10;
   ```

---

## Phase 6: Activate Weight History

### 6.1 Purpose

Track changes to scoring weights for rollback and analysis.

### 6.2 Current State

- `weight_history` table exists
- `WeightLearner.ApplyWeights` accepts notes

### 6.3 Implementation Steps

1. **Verify weight history is logged** on apply:

   ```go
   // In WeightLearner.ApplyWeights - should already save to weight_history
   ```

2. **Test weight update flow**:

   ```bash
   # Update weights via API
   curl -X POST http://localhost:8080/api/weights/manual \
     -H "Content-Type: application/json" \
     -d '{"weights": {...}, "notes": "Test update"}'
   ```

3. **Verify history is recorded**:
   ```sql
   SELECT * FROM weight_history ORDER BY applied_at DESC LIMIT 10;
   ```

---

## Verification Checklist

### After Phase 2 (Cleanup)

- [ ] `SELECT COUNT(*) FROM offers WHERE status = 'ACTIVE'` ≤ original - duplicates
- [ ] Duplicate query returns 0 rows
- [ ] Matching engine still finds matches

### After Phases 4-6 (Activation)

- [ ] Confirm a match → check `audit_logs`
- [ ] Confirm a match → check `match_feedback`
- [ ] Update weights → check `weight_history`

---

## Rollback Procedures

### Restore Offers from Backup

```sql
-- If something went wrong
DELETE FROM offers;
INSERT INTO offers SELECT * FROM offers_backup_YYYYMMDD;
```

### Disable Deduplication

```yaml
# config.yaml
parser:
  dedup_window: 0 # Disables dedup
```

---

## Success Metrics

| Metric                 | Before | Target |
| ---------------------- | ------ | ------ |
| Duplicate offers       | 106    | 0      |
| Audit logs (7 days)    | 0      | > 100  |
| Match feedback records | 0      | > 50   |
| Weight history entries | 0      | ≥ 1    |
