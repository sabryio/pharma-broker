# PharmaBroker Enhancement Roadmap

This document outlines planned enhancements based on the comprehensive app review.
Each phase contains specific tasks that can be tracked for implementation.

---

## Phase 1: Code Quality & Testing (Priority: HIGH)

**Timeline:** 1-2 weeks  
**Goal:** Improve maintainability and test coverage

### 1.1 Split Large Files

- [ ] **Split `parser.go`** (1150 lines → 3 files)

  - [ ] Create `internal/ai/parser_core.go` - Main parser struct and initialization
  - [ ] Create `internal/ai/parser_matching.go` - Matching worker loop
  - [ ] Create `internal/ai/parser_helpers.go` - Utility functions (createOffer, createRequest)
  - [ ] Update imports and run tests

- [ ] **Split `handlers.go`** (915 lines → 4 files)
  - [ ] Create `internal/api/handlers_offers.go` - Offer endpoints
  - [ ] Create `internal/api/handlers_requests.go` - Request endpoints
  - [ ] Create `internal/api/handlers_matches.go` - Match endpoints
  - [ ] Create `internal/api/handlers_config.go` - Config & stats endpoints
  - [ ] Verify all routes still work

### 1.2 Increase Test Coverage

| Package  | Current | Target | Tasks         |
| -------- | ------- | ------ | ------------- |
| ai       | 27%     | 50%    | Add 15+ tests |
| whatsapp | 23%     | 50%    | Add 10+ tests |
| api      | ~30%    | 60%    | Add 20+ tests |

- [ ] Add AI parsing integration tests with mock LLM responses
- [ ] Add WhatsApp listener edge case tests
- [ ] Add API endpoint validation tests
- [ ] Add concurrent access tests for matching

---

## Phase 2: Production Features (Priority: MEDIUM)

**Timeline:** 2-3 weeks  
**Goal:** Prepare for production deployment

### 2.1 Dashboard Authentication

- [ ] Choose auth strategy (JWT / session-based)
- [ ] Create `internal/auth/` package
- [ ] Add user model to database
- [ ] Implement login/logout endpoints
- [ ] Add auth middleware to protected routes
- [ ] Update frontend to handle auth flow

### 2.2 Cursor-Based Pagination

- [ ] Replace offset pagination with cursor-based
- [ ] Update `/api/offers` with `?cursor=xxx&limit=50`
- [ ] Update `/api/requests` pagination
- [ ] Update `/api/matches` pagination
- [ ] Add pagination metadata to responses

### 2.3 Full-Text Search API

- [ ] Create `/api/search` endpoint
- [ ] Support medication name search (FTS5)
- [ ] Support group/sender filtering
- [ ] Support date range filtering
- [ ] Add search to dashboard

### 2.4 Bulk Operations

- [ ] Add `POST /api/matches/bulk-confirm` endpoint
- [ ] Add `POST /api/matches/bulk-reject` endpoint
- [ ] Update dashboard with bulk action UI
- [ ] Add audit logging for bulk actions

---

## Phase 3: Scale & Performance (Priority: LOW)

**Timeline:** 1-2 weeks  
**Goal:** Handle 1000+ messages/hour

### 3.1 Database Optimization

- [ ] Analyze slow queries with EXPLAIN
- [ ] Add composite indexes for common queries:
  - `offers(status, medication, created_at)`
  - `requests(status, medication, created_at)`
  - `matches(status, score, created_at)`
- [ ] Optimize FTS5 configuration
- [ ] Add database connection pooling tuning

### 3.2 Load Testing

- [ ] Create load test script (simulate 100 concurrent messages)
- [ ] Benchmark AI parsing throughput
- [ ] Benchmark matching engine throughput
- [ ] Identify and fix bottlenecks
- [ ] Document performance baseline

### 3.3 Prometheus Dashboards

- [ ] Create Grafana dashboard template
- [ ] Add key metrics:
  - Messages processed/hour
  - AI parse latency (p50, p95, p99)
  - Match score distribution
  - Error rates by component

---

## Phase 4: Additional Features (Priority: OPTIONAL)

### 4.1 WhatsApp Notifications

- [ ] Add notification via WhatsApp (reuse existing whatsapp package)
- [ ] Create match notification template
- [ ] Add user preference for notification channel

### 4.2 Webhook Support

- [ ] Add webhook configuration to settings
- [ ] Send webhooks on new matches
- [ ] Send webhooks on confirmations
- [ ] Document webhook payload format

### 4.3 PDF Export

- [ ] Add PDF report generation
- [ ] Include charts/graphs in reports
- [ ] Add to scheduled reports

### 4.4 Archival Policy

- [ ] Add auto-archive after X days config
- [ ] Create archive tables
- [ ] Add janitor job for archival
- [ ] Add archived data export

---

## Security Enhancements

### Already Implemented ✅

- [x] GORM parameterized queries (SQL injection safe)
- [x] Environment variables for secrets
- [x] Bot command phone whitelist

### To Implement

- [ ] Dashboard authentication (Phase 2.1)
- [ ] Request rate limiting per IP
- [ ] Input validation middleware
- [ ] HTTPS enforcement in production
- [ ] Content Security Policy headers

---

## How to Use This Document

1. Pick a phase to work on
2. Create a Git branch: `feature/phase-1-testing`
3. Check off tasks as you complete them
4. Create PRs with references to specific tasks
5. Update this document with any new discoveries

---

_Last Updated: December 2024_
