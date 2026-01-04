# Investigation: Debug Recordings Visibility Issue

## Executive Summary

This document outlines the investigation into why debug recordings (Match Audit Records) were not appearing as expected on the frontend. The investigation revealed a disconnect between the recording infrastructure and the core matching engine, as well as a lack of database persistence in the retrieval API.

## Problem Description

Users reported that debug recordings were either missing or disappeared after a backend restart. On the frontend, the "Audit Records Viewer" showed zero records or only a small subset of recent activity, despite matching operations occurring in the background.

## Root Cause Analysis

### 1. Missing Audit Calls in Matching Engine

The `MatchingEngine` (located in `core/src/matching/engine.rs`) was found to have an `audit_recorder` field, but its scoring methods (`score_match`, `score_match_ai`) did not actually call `self.audit_recorder.record()`.

- **Impact:** Match events were never being captured by the recording system.

### 2. Lack of Persistent Recorder Integration

The `MatchingEngine` initialized a standard in-memory `AuditRecorder` using `AuditRecorder::from_env()`. While a `PersistentAuditRecorder` (in `core/src/matching/persistent_audit.rs`) existed, it was not being utilized by the engine.

- **Impact:** Any records captured (if the calls were present) would be lost upon application restart and would be limited by a fixed-size in-memory buffer.

### 3. API Retrieval Limitations

The backend API endpoint for fetching session records (`/api/audit-records/session/:sessionId`) explicitly implemented only the in-memory retrieval logic. A `TODO` comment in `core/src/api/audit_records.rs` confirmed that database integration was pending.

- **Impact:** Even if records were persisted to the database by other means, the frontend remained unable to fetch them.

## Technical Findings

### Backend Components

- **`AuditRecorder`**: Manages a volatile `VecDeque` of records in memory.
- **`PersistentAuditRecorder`**: Designed to wrap a recorder and flush records to a database repository asynchronously.
- **`MatchAuditRecordRepository`**: Provides the interface for DB persistence, implemented via SeaORM.

### Frontend Components

- **`useRecording` Hook**: Correctly generates a `sessionId` and uses it to tag snapshots.
- **Audit Viewer**: Calls `/api/audit-records/session/{sessionId}` to correlate local snapshots with backend audit data.

## Proposed Resolution

The following steps have been planned to resolve the issue:

1. **Integrate `PersistentAuditRecorder`**: Update `MatchingEngine` to use the persistent implementation of the audit recorder.
2. **Enable Auditing Calls**: Insert `recorder.record()` calls into the `MatchingEngine` scoring logic to capture `MatchAuditRecord` objects.
3. **Enhance API Endpoints**: Update the audit record API to query both the in-memory buffer and the database repository, merging and deduplicating the results.
