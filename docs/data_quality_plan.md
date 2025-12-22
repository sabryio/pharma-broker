# Data Quality & Maintenance Plan

> **Goal**: Ensure data quality, prevent duplicates, manage cleanup tasks.  
> **Last Updated**: December 22, 2025  
> **Architecture**: Rust Core

## Overview

| Phase | Task                         | Priority | Status      |
| ----- | ---------------------------- | -------- | ----------- |
| 1     | Investigate Duplicate Offers | High     | ✅ Done     |
| 2     | Cleanup Duplicate Offers     | High     | ⏳ Deferred |
| 3     | Prevent Future Duplicates    | High     | ✅ Done     |
| 4     | Activate Audit Logs          | Medium   | ✅ Done     |
| 5     | Activate Match Feedback      | Medium   | ✅ Done     |
| 6     | Activate Weight History      | Low      | ✅ Done     |
| 7     | Audit Log Retention          | Medium   | ✅ Done     |
| 8     | AI Parsing Quality           | Medium   | ✅ Done     |
| 9     | Review Unmapped Medications  | Medium   | 🔜 Ready    |
| 10    | Process Review Queue         | Medium   | 🔜 Ready    |

---

## Data Prevention (Rust Core)

### Duplicate Prevention

**Implementation** (`core/src/ai/pharma_parser.rs`):

- Deduplication check before saving offers/requests
- Configurable dedup window via environment
- Metric: `pharma_duplicates_skipped_total`

### Audit Logging

**Implementation** (`core/crates/db/src/repo/audit_log.rs`):

- All match confirmations/rejections logged
- Entity changes tracked
- Configurable retention period

---

## Janitor Worker (Rust Core)

**Location**: `core/src/worker/janitor.rs`

**Tasks**:

- Clean expired matches
- Purge old audit logs (90 day retention)
- Archive stale offers/requests

---

## Analysis Scripts

**Location**: `analysis/scripts/`

| Script                         | Purpose            | Status   |
| ------------------------------ | ------------------ | -------- |
| `07_ai_parsing_quality.py`     | Parsing metrics    | ✅ Ready |
| `10_investigate_duplicates.py` | Duplicate analysis | ✅ Ready |
| `11_cleanup_duplicates.py`     | Duplicate cleanup  | ✅ Ready |
| `12_review_unmapped.py`        | Fix mappings       | 🔜 Run   |
| `13_process_review_queue.py`   | Clear queue        | 🔜 Run   |
| `14_stale_matches.py`          | Stale analysis     | ✅ Ready |

### Running Scripts

```bash
cd analysis
uv run python scripts/07_ai_parsing_quality.py
```

---

## Current Metrics

| Metric               | Status     |
| -------------------- | ---------- |
| Duplicate prevention | ✅ Active  |
| Audit logs           | ✅ Active  |
| Match feedback       | ✅ Active  |
| Weight history       | ✅ Active  |
| Janitor worker       | ✅ Running |

---

_Last updated: December 22, 2025_
