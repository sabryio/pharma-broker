# Investigation: AI Supervision Flow Issues

## Executive Summary

This document outlines the investigation into why the AI Supervision features (Live Feed, Audit Logs) are not functioning as expected. The investigation revealed that while the core logic and WebSocket event structures exist, they are not fully integrated into the production matching engine and background workers, and the persistent storage layer is missing for these specific event types.

## Problem Description

The AI Supervision dashboard on the frontend displays empty statistics or a non-functional live feed. Historical audit logs for AI decisions (approvals, blocks, overrides) are not appearing in the table view despite successful matching operations and manual overrides in the UI.

## Root Cause Analysis

### 1. Hardcoded API Returns

The `MatchingEngine` implementation for retrieving supervision audit logs was found to be a stub.

- **File:** `core/src/matching/engine.rs`
- **Current State:** `get_supervision_audit_log` is hardcoded to return `Ok(Vec::new())` with a comment stating it will be connected later.
- **Impact:** The frontend always receives zero records when querying historical AI decisions.

### 2. Lack of Production Integration for `SupervisionAuditTrail`

The `SupervisionAuditTrail` module (in `core/src/matching/supervision_audit.rs`) provides the necessary logic for logging AI events, but it is not instantiated or used in the production `MatchingEngine`.

- **Finding:** Grep results show `SupervisionAuditTrail::new` is only used in unit tests.
- **Impact:** AI decisions made by the system are not being recorded to any in-memory or persistent store.

### 3. Worker Integration Gaps

The `AutoApproveWorker` (in `core/src/worker/auto_approve_worker.rs`) correctly broadcasts WebSocket events for real-time updates but does not call any auditing service to record these decisions permanently.

- **Impact:** Events are "fire-and-forget" over WebSockets; if the frontend isn't listening at the exact moment an event occurs, the data is lost forever.

### 4. Database Persistence Missing

While `SeaOrmMatchAuditRecordRepo` exists for "Debug Recordings" (Match Audit Records), there is no dedicated database repository or SeaORM entity for the extended `SupervisionAuditEntry` type used by the AI supervision system.

- **Impact:** System restarts clear all supervision "history," as no database table is serving this data.

## Technical Findings

### Supervision Events

The system defines several critical event types that are currently not being persisted:

- `AutoApproved`: High-confidence AI matches.
- `QueuedForReview`: Borderline matches.
- `Blocked`: Matches rejected by safety guardrails.
- `Overridden`: Human intervention on AI decisions.
- `Undo`: Reversing an approval.

### WebSocket Mechanism

- **File:** `core/src/ws/mod.rs`
- **Status:** Correctly defines `WsEvent` variants for all supervision types. This explains why real-time "flashes" might work if the user is currently on the dashboard, but history remains empty.

## Proposed Resolution

1. **Instantiate `SupervisionAuditTrail`**: Add to `MatchingEngine` and initialize it during application bootstrap in `main.rs`.
2. **Hook into Worker**: Modify `AutoApproveWorker::handle_auto_approve_result` to log results to the `SupervisionAuditTrail`.
3. **Implement Retrieval**: Connect `MatchingEngine::get_supervision_audit_log` to the repository managed by `SupervisionAuditTrail`.
4. **Permanent Storage**: (Optional/Follow-up) Define a SeaORM entity for supervision audits to ensure cross-restart persistence, or leverage the existing `AuditLog` table for general events.
