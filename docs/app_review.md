# PharmaBroker App Review

> **Purpose**: Define expectations, deliverables, and verification criteria for each development phase.  
> **Last Updated**: December 22, 2025  
> **Architecture**: Go Bridge + Rust Core (Polyglot Microservices)

---

## Executive Summary

PharmaBroker is an AI-powered pharmaceutical trading platform that:

1. Ingests WhatsApp messages from Egyptian trading groups (Go Bridge)
2. Extracts medication offers/requests using AI (Rust Core + Docker Model Runner)
3. Matches supply with demand using 24-module scoring engine (Rust Core)
4. Provides real-time API for operator management

---

## Phase 1: Data Ingestion

### Components

| Component          | Location                               | Purpose                       |
| ------------------ | -------------------------------------- | ----------------------------- |
| `whatsmeow` Client | `bridge/adapters/whatsapp/`            | WhatsApp Web connection       |
| Bridge App         | `bridge/app/bridge.go`                 | Message forwarding to Rust    |
| Reconnector        | `bridge/adapters/whatsapp/`            | Exponential backoff reconnect |
| Deduplicator       | `bridge/deduplicator/`                 | Duplicate message filtering   |
| Rate Limiter       | `bridge/resilience/rate_limiter.go`    | Prevent WhatsApp bans         |
| History Sync       | `bridge/adapters/whatsapp/`            | History sync deduplication    |
| Circuit Breaker    | `bridge/resilience/circuit_breaker.go` | gRPC failure protection       |
| Retry Buffer       | `bridge/adapters/resilience/`          | Failed message retry queue    |
| Group Cache        | `bridge/adapters/grpc/`                | Monitored groups cache        |

### Deliverables

- [x] Connect to WhatsApp Web via QR code
- [x] Monitor configured groups
- [x] Save raw messages via gRPC to Rust Core
- [x] Handle reconnection gracefully
- [x] Rate limiting (100/min, burst 20)
- [x] History sync with deduplication (24h max age, 5min cooldown)
- [x] Circuit breaker for gRPC calls
- [x] Retry buffer for failed messages

### Data Criteria

| Metric               | Target     | Status |
| -------------------- | ---------- | ------ |
| Message capture rate | 100%       | ✅     |
| Connection uptime    | >99%       | ✅     |
| Latency              | <5 seconds | ✅     |

---

## Phase 2: AI Parsing

### Components

| Component     | Location                       | Purpose                   |
| ------------- | ------------------------------ | ------------------------- |
| Parser        | `core/src/ai/pharma_parser.rs` | AI-powered parsing        |
| Token Batcher | `core/src/ai/token_batcher.rs` | Smart batching            |
| Feedback Loop | `core/src/ai/feedback_loop.rs` | Learning from corrections |
| AI Client     | `core/crates/ai-client/`       | Generic AI client library |

### Deliverables

- [x] Extract medications from Arabic text
- [x] Classify as OFFER/REQUEST
- [x] Normalize medication names (Arabic → English)
- [x] Queue low-confidence items for review
- [x] Handle multi-item messages

### Data Criteria

| Metric                | Target | Current |
| --------------------- | ------ | ------- |
| Medication extraction | >95%   | 100% ✅ |
| Unit extraction       | >80%   | 88% ✅  |
| Intent classification | >95%   | ~98% ✅ |

---

## Phase 3: Matching Engine

### Components

| Component   | Location                           | Purpose             |
| ----------- | ---------------------------------- | ------------------- |
| Scorer      | `core/src/matching/scorer.rs`      | Multi-field scoring |
| Learner     | `core/src/matching/learner.rs`     | Weight optimization |
| Calibration | `core/src/matching/calibration.rs` | Platt scaling       |
| Engine      | `core/src/matching/engine.rs`      | Core orchestration  |

### Scoring Weights

| Factor     | Weight | Score Logic               |
| ---------- | ------ | ------------------------- |
| Medication | 40%    | Fuzzy + vector similarity |
| Dosage     | 15%    | Numeric comparison        |
| Quantity   | 20%    | Fulfillment ratio         |
| Price      | 15%    | Budget fit                |
| Recency    | 10%    | Exponential decay         |

### Confidence Bands

| Band    | Score     | Action              |
| ------- | --------- | ------------------- |
| AUTO    | ≥0.90     | Auto-confirm        |
| SUGGEST | 0.70-0.89 | Suggest to operator |
| REVIEW  | 0.50-0.69 | Manual review       |
| NONE    | <0.50     | No match            |

---

## Phase 4: API & Dashboard

### Components

| Component | Location                   | Purpose           |
| --------- | -------------------------- | ----------------- |
| Handlers  | `core/src/api/handlers.rs` | REST endpoints    |
| Routes    | `core/src/api/routes.rs`   | Router config     |
| WebSocket | `core/src/api/mod.rs`      | Real-time updates |

### API Endpoints

| Endpoint                    | Method  | Status |
| --------------------------- | ------- | ------ |
| `/api/offers`               | GET     | ✅     |
| `/api/requests`             | GET     | ✅     |
| `/api/matches`              | GET     | ✅     |
| `/api/matches/{id}/confirm` | POST    | ✅     |
| `/api/matches/{id}/reject`  | POST    | ✅     |
| `/api/review/queue`         | GET     | ✅     |
| `/api/stats`                | GET     | ✅     |
| `/api/groups`               | GET     | ✅     |
| `/api/weights`              | GET/PUT | ✅     |
| `/health`                   | GET     | ✅     |
| `/metrics`                  | GET     | ✅     |

---

## Phase 5: Data Quality & Maintenance

### Components

| Component  | Location                               | Purpose        |
| ---------- | -------------------------------------- | -------------- |
| Janitor    | `core/src/worker/janitor.rs`           | Data cleanup   |
| Audit Repo | `core/crates/db/src/repo/audit_log.rs` | Audit logging  |
| Analysis   | `analysis/scripts/`                    | Python scripts |

### Deliverables

- [x] Duplicate offer prevention
- [x] Stale match detection
- [x] Audit log retention
- [x] Unmapped medication tracking
- [x] Review queue management

### Analysis Scripts

| Script                         | Purpose              | Status   |
| ------------------------------ | -------------------- | -------- |
| `07_ai_parsing_quality.py`     | Parsing metrics      | ✅ Ready |
| `10_investigate_duplicates.py` | Duplicate analysis   | ✅ Ready |
| `11_cleanup_duplicates.py`     | Duplicate cleanup    | ✅ Ready |
| `14_stale_matches.py`          | Stale match analysis | ✅ Ready |

---

## Current Status Summary

| Phase              | Status         | Notes                      |
| ------------------ | -------------- | -------------------------- |
| 1. Data Ingestion  | ✅ Operational | Full resilience stack      |
| 2. AI Parsing      | ✅ Operational | 100% medication extraction |
| 3. Matching Engine | ✅ Operational | 24 modules, 0.82 avg score |
| 4. API             | ✅ Operational | All endpoints working      |
| 5. Data Quality    | ✅ Tools Ready | Run scripts when needed    |

---

## Health Metrics

### Daily Checks

| Check             | Query                                                                           | Expected |
| ----------------- | ------------------------------------------------------------------------------- | -------- |
| Messages ingested | `SELECT COUNT(*) FROM raw_messages WHERE created_at > NOW() - INTERVAL '1 day'` | >0       |
| Offers created    | `SELECT COUNT(*) FROM offers WHERE created_at > NOW() - INTERVAL '1 day'`       | >0       |
| Matches generated | `SELECT COUNT(*) FROM matches WHERE created_at > NOW() - INTERVAL '1 day'`      | >0       |
| Pending matches   | `SELECT COUNT(*) FROM matches WHERE status = 'PENDING'`                         | <1000    |

---

_Last updated: December 22, 2025_
