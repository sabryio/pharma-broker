# Data Quality & Table Activation Plan

> **Goal**: Fix duplicates, activate unused tables, improve AI parsing quality.
> **Last Updated**: 2025-12-19

## Overview

| Phase | Task                         | Priority | Status                 |
| ----- | ---------------------------- | -------- | ---------------------- |
| 1     | Investigate Duplicate Offers | High     | ✅ Done                |
| 2     | Cleanup Duplicate Offers     | High     | ⏳ Dev mode (deferred) |
| 3     | Prevent Future Duplicates    | High     | ✅ Done                |
| 4     | Activate Audit Logs          | Medium   | ✅ Done                |
| 5     | Activate Match Feedback      | Medium   | ✅ Done                |
| 6     | Activate Weight History      | Low      | ✅ Done                |
| 7     | Audit Log Retention          | Medium   | ✅ Done                |
| 8     | AI Parsing Quality           | Medium   | ✅ Done                |
| 9     | Review Unmapped Medications  | Medium   | 🔜 Ready               |
| 10    | Process Review Queue         | Medium   | 🔜 Ready               |

---

## Phase 1: Investigate Duplicate Offers ✅

**Script**: `analysis/scripts/10_investigate_duplicates.py`

**Findings** (2025-12-19):

- 3,394 duplicate groups identified (way more than initial 106 estimate)
- Most are cross-posts (same message to multiple groups)
- Average duplication rate: ~2.4x

---

## Phase 2: Cleanup Duplicate Offers ⏳

**Script**: `analysis/scripts/11_cleanup_duplicates.py`

**Status**: Deferred (dev mode - cleanup not critical while prevention is active)

---

## Phase 3: Prevent Future Duplicates ✅

**Implementation**:

- `FindRecentDuplicate()` method in OfferRepo
- Dedup check in `processor.go` before saving
- Configurable via `parser.dedup_window` (default: 10m)
- Set to `0` to disable
- Metric: `pharma_duplicates_skipped_total`

---

## Phase 4: Activate Audit Logs ✅

**Status**: Already active via `logAudit()` calls in MatchHandler

**Events logged**:

- `MATCH_CONFIRMED` - Match confirmed
- `MATCH_REJECTED` - Match rejected

---

## Phase 5: Activate Match Feedback ✅

**Implementation** (2025-12-19):

- Added `feedbackRepo` to MatchHandler
- Added `recordFeedback()` helper function
- Auto-records on confirm/reject

**Data captured**:

- Match ID, Operator ID, Decision
- Original score, Confidence band
- Notes/reason

---

## Phase 6: Activate Weight History ✅

**Status**: Already active via `WeightLearner.ApplyWeights()`

- Saves to `weight_history` table on weight changes
- Includes metrics, source, notes

---

## Phase 7: Audit Log Retention ✅

**Implementation** (2025-12-19):

- `DeleteOlderThan()` method in AuditRepository
- Janitor extended to clean audit logs daily
- Config: `database.audit_retention_days` (default: 90)

---

## Phase 8: AI Parsing Quality ✅

**Analysis Script**: `analysis/scripts/07_ai_parsing_quality.py`

**Findings** (2025-12-19):
| Metric | Offers | Requests |
|--------|--------|----------|
| Medication | 100% | 100% |
| Unit | 88% | 81% |
| Quantity | 32% | 17% |
| Price | 31% | 0% |

> Note: Low price extraction is expected - most messages don't include prices.

**Prompt Enhancement** (2025-12-19):

- Updated `ai/prompts/templates.go`
- Added stricter quantity/price extraction rules
- More Arabic number words and price patterns

---

## Phase 9: Review Unmapped Medications 🔜

**Script**: `analysis/scripts/12_review_unmapped.py`

**Issue**: Wrong Arabic→English mappings detected:

- ريبلسيس → Revolade (should be Rybelsus)
- ميوكوستا → Mycostatin (should be Mucosta)
- سيبراليكس → Ceftriaxone (should be Cipralex)

**Action**: Run script to review and fix mappings

---

## Phase 10: Process Review Queue 🔜

**Script**: `analysis/scripts/13_process_review_queue.py`

**Status**: 172 items pending review (0.0 avg confidence)

---

## Phase 11: Stale Matches 🟢

**Script**: `analysis/scripts/14_stale_matches.py`

**Status** (2025-12-19): Healthy!

- 2,337 pending matches (all 1-3 days old)
- No stale matches (>7 days)
- No cleanup needed

---

## Success Metrics

| Metric                | Before | Current | Target |
| --------------------- | ------ | ------- | ------ |
| Duplicate prevention  | ❌     | ✅      | ✅     |
| Audit logs active     | ❌     | ✅      | ✅     |
| Match feedback active | ❌     | ✅      | ✅     |
| Weight history active | ✅     | ✅      | ✅     |
| Audit retention       | ❌     | ✅      | ✅     |
| Unmapped reviewed     | ❌     | 🔜      | ✅     |
| Review queue cleared  | ❌     | 🔜      | ✅     |
