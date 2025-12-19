# PharmaBroker Comprehensive App Review

> **Purpose**: Define expectations, deliverables, and verification criteria for each development phase.
> **Date**: 2025-12-19

---

## Executive Summary

PharmaBroker is an AI-powered pharmaceutical trading platform that:

1. Ingests WhatsApp messages from Egyptian trading groups
2. Extracts medication offers/requests using AI (Arabic → structured data)
3. Matches supply with demand using 5-dimensional scoring
4. Provides real-time dashboard for operator management

---

## Phase 1: Data Ingestion

### Components

| Component          | Location                | Purpose                      |
| ------------------ | ----------------------- | ---------------------------- |
| `whatsmeow` Client | `messaging/`            | WhatsApp Web connection      |
| Manager            | `messaging/manager.go`  | Authentication, reconnection |
| Listener           | `messaging/listener.go` | Message capture              |

### Deliverables

- [x] Connect to WhatsApp Web via QR code
- [x] Monitor configured groups
- [x] Save raw messages to `raw_messages` table
- [x] Handle reconnection gracefully

### Data Criteria

| Metric               | Target     | How to Measure                          |
| -------------------- | ---------- | --------------------------------------- |
| Message capture rate | 100%       | Compare group message count vs DB count |
| Connection uptime    | >99%       | Monitor reconnection events             |
| Latency              | <5 seconds | Timestamp diff: message → DB            |

### Verification

```sql
-- Check daily ingestion
SELECT DATE(created_at) as day, COUNT(*)
FROM raw_messages
GROUP BY DATE(created_at) ORDER BY day DESC LIMIT 7;
```

---

## Phase 2: AI Parsing

### Components

| Component     | Location                   | Purpose           |
| ------------- | -------------------------- | ----------------- |
| Parser        | `parsing/processor.go`     | Batch processing  |
| AI Provider   | `ai/`                      | Gemini/Docker LLM |
| Prompts       | `ai/prompts/templates.go`  | System prompts    |
| Token Batcher | `parsing/token_batcher.go` | Smart batching    |

### Deliverables

- [x] Extract medications from Arabic text
- [x] Classify as OFFER/REQUEST
- [x] Normalize medication names (Arabic → English)
- [x] Queue low-confidence items for review
- [x] Handle multi-item messages

### Data Criteria

| Metric                 | Target  | Current | Script                     |
| ---------------------- | ------- | ------- | -------------------------- |
| Medication extraction  | >95%    | 100% ✅ | `07_ai_parsing_quality.py` |
| Unit extraction        | >80%    | 88% ✅  | -                          |
| Intent classification  | >95%    | ~98%    | Manual review              |
| Low-confidence routing | Working | ✅      | Check `review_queue`       |

### Verification

```bash
uv run python scripts/07_ai_parsing_quality.py
```

---

## Phase 3: Matching Engine

### Components

| Component | Location              | Purpose              |
| --------- | --------------------- | -------------------- |
| Scorer    | `matching/scorer.go`  | Multi-field scoring  |
| Weights   | `matching/weights.go` | Configurable weights |
| Learner   | `matching/learner.go` | Adaptive learning    |

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

### Data Criteria

| Metric                | Target | How to Measure             |
| --------------------- | ------ | -------------------------- |
| Match accuracy        | >90%   | Feedback confirmation rate |
| False positives       | <5%    | Rejection rate             |
| Avg score (confirmed) | >0.85  | Query `match_feedback`     |
| Processing latency    | <30s   | Offer→Match timing         |

### Verification

```sql
-- Check matching effectiveness
SELECT
    DATE(created_at) as day,
    COUNT(*) as matches,
    AVG(score) as avg_score,
    SUM(CASE WHEN status='CONFIRMED' THEN 1 ELSE 0 END) as confirmed
FROM matches
GROUP BY DATE(created_at) ORDER BY day DESC LIMIT 7;
```

---

## Phase 4: API & Dashboard

### Components

| Component  | Location          | Purpose             |
| ---------- | ----------------- | ------------------- |
| Handlers   | `api/handlers/`   | REST endpoints      |
| SSE Hub    | `api/sse/`        | Real-time updates   |
| Middleware | `api/middleware/` | Auth, CORS, logging |

### API Endpoints

| Endpoint                    | Method   | Status |
| --------------------------- | -------- | ------ |
| `/api/offers`               | GET      | ✅     |
| `/api/requests`             | GET      | ✅     |
| `/api/matches`              | GET/POST | ✅     |
| `/api/matches/{id}/confirm` | POST     | ✅     |
| `/api/matches/{id}/reject`  | POST     | ✅     |
| `/api/review/queue`         | GET      | ✅     |
| `/api/stats`                | GET      | ✅     |
| `/api/sse`                  | GET      | ✅     |

### Data Criteria

| Metric            | Target | How to Measure    |
| ----------------- | ------ | ----------------- |
| API response time | <200ms | Monitoring/logs   |
| SSE delivery      | <1s    | Dashboard latency |
| Error rate        | <0.1%  | Error logs        |

---

## Phase 5: Data Quality & Maintenance

### Components

| Component  | Location                     | Purpose        |
| ---------- | ---------------------------- | -------------- |
| Janitor    | `storage/janitor/`           | Data cleanup   |
| Audit Repo | `storage/gorm/audit_repo.go` | Audit logging  |
| Analysis   | `analysis/scripts/`          | Python scripts |

### Deliverables

- [x] Duplicate offer prevention
- [x] Stale match detection
- [x] Audit log retention (90 days)
- [x] Unmapped medication tracking
- [x] Review queue management

### Analysis Scripts

| Script                         | Purpose              | Status   |
| ------------------------------ | -------------------- | -------- |
| `07_ai_parsing_quality.py`     | Parsing metrics      | ✅ Ready |
| `10_investigate_duplicates.py` | Duplicate analysis   | ✅ Ready |
| `11_cleanup_duplicates.py`     | Duplicate cleanup    | ✅ Ready |
| `12_review_unmapped.py`        | Fix bad mappings     | 🔜 Run   |
| `13_process_review_queue.py`   | Clear review queue   | 🔜 Run   |
| `14_stale_matches.py`          | Stale match analysis | ✅ Ready |

---

## Phase 6: Bot Integration

### Components

| Component   | Location          | Purpose                     |
| ----------- | ----------------- | --------------------------- |
| Bot Handler | `bot/handler.go`  | Command processing          |
| Commands    | `bot/commands.go` | `/status`, `/pending`, etc. |

### Commands

| Command         | Purpose              | Status |
| --------------- | -------------------- | ------ |
| `/status`       | System stats         | ✅     |
| `/pending`      | List pending matches | ✅     |
| `/confirm <id>` | Confirm match        | ✅     |
| `/reject <id>`  | Reject match         | ✅     |
| `/help`         | Show commands        | ✅     |

---

## Health Metrics Dashboard

### Daily Checks

| Check             | Query/Script                                                                    | Expected      |
| ----------------- | ------------------------------------------------------------------------------- | ------------- |
| Messages ingested | `SELECT COUNT(*) FROM raw_messages WHERE created_at > NOW() - INTERVAL '1 day'` | >0            |
| Offers created    | `SELECT COUNT(*) FROM offers WHERE created_at > NOW() - INTERVAL '1 day'`       | >0            |
| Matches generated | `SELECT COUNT(*) FROM matches WHERE created_at > NOW() - INTERVAL '1 day'`      | >0            |
| Pending matches   | `SELECT COUNT(*) FROM matches WHERE status = 'PENDING'`                         | <1000         |
| Review queue      | `SELECT COUNT(*) FROM review_queue WHERE status = 'PENDING'`                    | <500          |
| Stale items       | `14_stale_matches.py`                                                           | 0 over 7 days |

### Weekly Checks

| Check              | Script/Query                                                                           | Expected             |
| ------------------ | -------------------------------------------------------------------------------------- | -------------------- |
| AI parsing quality | `07_ai_parsing_quality.py`                                                             | >95% medication      |
| Duplicate rate     | `10_investigate_duplicates.py`                                                         | Stable or decreasing |
| Confirmation rate  | `SELECT COUNT(CASE WHEN status='CONFIRMED' THEN 1 END)::float / COUNT(*) FROM matches` | >70%                 |

---

## Current Status Summary

| Phase              | Status         | Notes                      |
| ------------------ | -------------- | -------------------------- |
| 1. Data Ingestion  | ✅ Operational | WhatsApp connected         |
| 2. AI Parsing      | ✅ Operational | 100% medication extraction |
| 3. Matching Engine | ✅ Operational | 0.82 avg score             |
| 4. API & Dashboard | ✅ Operational | SSE working                |
| 5. Data Quality    | ✅ Tools Ready | Run scripts when needed    |
| 6. Bot Integration | ✅ Operational | Commands active            |

---

## Recommended Next Actions

| Priority | Action                                             | Effort  |
| -------- | -------------------------------------------------- | ------- |
| 1        | Run `12_review_unmapped.py` (fix 45 mappings)      | 30 min  |
| 2        | Run `13_process_review_queue.py` (clear 172 items) | 1 hour  |
| 3        | Monitor `pharma_duplicates_skipped_total` metric   | Ongoing |
| 4        | Review confirmation rate weekly                    | Ongoing |
