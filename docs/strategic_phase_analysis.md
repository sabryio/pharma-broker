# PharmaBroker Strategic Phase Analysis

Comprehensive strategic assessment of each phase with functional analysis, pros/cons, performance, production readiness, testing strategy, and recommendations.

---

## Phase 1: Executive Summary

### 1.1 Phase Overview

The system is a pharmaceutical medication matching platform that:

- Ingests WhatsApp messages containing medication offers/requests
- Uses AI (LLM) to parse Arabic text into structured data
- Matches offers with requests using multi-field scoring
- Provides operator interfaces via bot commands and REST API
- Learns from feedback to improve matching over time

**Current State:**

- Legacy Go: ~21,400 lines across 196 files
- Rust Core: ~6,500 lines with 148 tests
- Architecture: Go Bridge (WhatsApp) → gRPC → Rust Core

### 1.2 Functional Assessment

| Area            | Complete | Missing                         | Priority |
| --------------- | -------- | ------------------------------- | -------- |
| Core matching   | ✅       | -                               | -        |
| AI integration  | ✅       | Token batching, circuit breaker | High     |
| Learning system | ✅       | Feedback persistence            | High     |
| WebSocket       | ✅       | Auth, heartbeat                 | Medium   |
| Bot commands    | ❌       | All commands                    | Low      |

### 1.3 Pros and Cons

**Pros:**

- Rust core provides memory safety and performance
- Matching engine has 100% feature parity with 148 tests
- Clear separation: Bridge (Go) handles WhatsApp, Core (Rust) handles business logic
- Unified MatchingEngine orchestrates all components

**Cons:**

- Split architecture requires gRPC overhead
- Bot commands duplicated in Go (not leveraging Rust core)
- Missing feedback persistence prevents production learning
- No YAML config support in Rust

### 1.4 Performance Considerations

- **Bottleneck:** AI parsing is the slowest path (~500ms-2s per message)
- **Recommendation:** Implement token-aware batching to maximize throughput
- **Concern:** WebSocket lacks client limits, could cause memory exhaustion

### 1.5 Production Readiness

| Criterion         | Status        | Risk            |
| ----------------- | ------------- | --------------- |
| Core stability    | ✅ High       | Low             |
| Error handling    | ⚠️ Partial    | Medium          |
| Monitoring        | ✅ Prometheus | Low             |
| Graceful shutdown | ⚠️            | Medium          |
| Data persistence  | ⚠️            | High (feedback) |

### 1.6 Testing Strategy

```
Priority 1: E2E Integration
- Docker Compose with all services
- Message flow: WhatsApp → Bridge → Core → DB
- Verify match creation and WebSocket events

Priority 2: Load Testing
- 100+ concurrent messages
- Measure AI latency under load
- WebSocket connection limits

Priority 3: Chaos Testing
- AI service unavailable
- Database connection drops
- Bridge disconnection recovery
```

### 1.7 Recommendations

1. **High Priority:** Add FeedbackRecord repository for production learning
2. **High Priority:** Integrate circuit breaker into AI client
3. **Medium:** Add health check endpoints with deep checks
4. **Low:** Consider single-language deployment (full Rust)

### 1.8 Implementation Tasks

#### Task 1.1: Deep Health Checks

- [ ] **Step 1:** Create `core/src/api/health.rs` with three endpoints
  - `/health` - Basic liveness (return 200)
  - `/health/ready` - Check DB connection via `sqlx::query("SELECT 1")`
  - `/health/live` - Check AI gateway reachability
- [ ] **Step 2:** Add timeout (5s) for each health check
- [ ] **Step 3:** Return JSON with component statuses
- [ ] **Test:** `cargo test health_` - verify each endpoint behavior

#### Task 1.2: Graceful Shutdown

- [ ] **Step 1:** Add `tokio::signal::ctrl_c()` handler in `main.rs`
- [ ] **Step 2:** Implement shutdown broadcast channel
- [ ] **Step 3:** Ensure gRPC server and HTTP server stop accepting new requests
- [ ] **Step 4:** Add 10-second drain timeout for in-flight requests
- [ ] **Test:** `cargo test shutdown_` - verify clean termination

#### Task 1.3: E2E Docker Compose Test

- [ ] **Step 1:** Create `tests/e2e/docker-compose.test.yml`
- [ ] **Step 2:** Add test script that:
  1. Starts all services
  2. Sends test message via gRPC
  3. Verifies offer/request created in DB
  4. Checks WebSocket event received
- [ ] **Test:** `./scripts/e2e-test.sh` - full integration validation

---

## Phase 2: WhatsApp Message Ingestion

### 2.1 Phase Overview

WhatsApp integration via `whatsmeow` library with:

- QR code pairing
- Message event handling
- Reconnection with exponential backoff
- Rate limiting (20 msgs/min)
- Per-group ordered processing

**Current State:** Go Bridge service, communicates via gRPC to Rust Core.

### 2.2 Functional Assessment

| Feature         | Legacy           | Bridge  | Gap                          |
| --------------- | ---------------- | ------- | ---------------------------- |
| WhatsApp client | ✅               | ✅      | None                         |
| Reconnection    | ✅ Exponential   | ✅      | Config alignment             |
| Rate limiting   | ✅ 20/min        | ⚠️      | Verify implementation        |
| Ordered queues  | ✅ Per-group     | ❌      | **Critical for correctness** |
| History dedup   | ✅ 5min cooldown | ⚠️      | Not verified                 |
| Group cache     | ✅ 30min TTL     | ⚠️ 5min | Lower TTL                    |

### 2.3 Pros and Cons

**Pros:**

- Proven whatsmeow library
- PostgreSQL session storage (no SQLite)
- Separate service allows independent scaling
- Metrics and health endpoints

**Cons:**

- Ordered queue not implemented (messages may be processed out of order)
- Rate limiter config not verified
- No admin alerts on connection failure
- History sync may reprocess old messages

### 2.4 Performance Considerations

- **Token bucket rate limiter:** 20/min with burst of 5 is appropriate for WhatsApp
- **Ordered queues:** Critical for reply-context messages; without them, a reply may be processed before its parent
- **Session storage:** PostgreSQL is slower than SQLite but more reliable

**Recommendation:** Implement ordered per-group queue with 100-message buffer.

### 2.5 Production Readiness

| Criterion            | Status | Action                           |
| -------------------- | ------ | -------------------------------- |
| Connection stability | ⚠️     | Verify reconnector in production |
| Message ordering     | ❌     | Implement ordered queue          |
| Rate limiting        | ⚠️     | Verify config matches legacy     |
| Monitoring           | ⚠️     | Add connection state metrics     |
| Alerting             | ❌     | Add Slack/Telegram alerts        |

### 2.6 Testing Strategy

```yaml
Unit Tests:
  - Rate limiter token bucket algorithm
  - Reconnector backoff timing
  - Message deduplication

Integration Tests:
  - Connect with test WhatsApp account
  - Send message and verify gRPC forwarding
  - Simulate disconnection and reconnection

Load Tests:
  - 50 messages/minute for 1 hour
  - Monitor memory and goroutine count
  - Verify no message loss
```

### 2.7 Recommendations

1. **Critical:** Implement per-group ordered message queue
2. **High:** Add connection state metrics (connected, reconnecting, failed)
3. **Medium:** Verify rate limiter matches legacy config
4. **Low:** Add admin alerting for disconnections

### 2.8 Implementation Tasks

#### Task 2.1: Per-Group Ordered Message Queue (Critical)

- [ ] **Step 1:** Create `OrderedMessageQueue` struct in `bridge/pkg/queue/ordered.go`
  - Map of group JID → buffered channel (100 messages)
  - One goroutine per group processes messages in order
- [ ] **Step 2:** Modify `handleMessage` to enqueue instead of immediate gRPC call
- [ ] **Step 3:** Add graceful shutdown (drain queues before exit)
- [ ] **Step 4:** Add metrics: queue depth per group, processing latency
- [ ] **Test:** `go test ./pkg/queue/... -run TestOrderedQueue`
  - Verify messages for same group processed sequentially
  - Verify different groups processed concurrently

#### Task 2.2: Connection State Metrics

- [ ] **Step 1:** Add Prometheus gauge `whatsapp_connection_state`
  - Labels: state (connected, connecting, reconnecting, failed)
- [ ] **Step 2:** Update gauge on each state transition
- [ ] **Step 3:** Add `whatsapp_reconnect_total` counter
- [ ] **Test:** Simulate disconnect, verify metrics update

#### Task 2.3: Rate Limiter Verification

- [ ] **Step 1:** Review `OutboundRateLimiter` config against legacy
  - Verify: 20/min, burst 5
- [ ] **Step 2:** Add config via environment variables
- [ ] **Step 3:** Add metrics: `rate_limiter_wait_seconds`, `rate_limiter_dropped_total`
- [ ] **Test:** Send 30 messages in 1 minute, verify rate limiting

#### Task 2.4: Admin Alerting

- [ ] **Step 1:** Create `AlertNotifier` interface in `bridge/pkg/notify/`
- [ ] **Step 2:** Implement `TelegramNotifier` for critical alerts
- [ ] **Step 3:** Send alert on: connection failed, max reconnects reached
- [ ] **Test:** Trigger disconnect, verify Telegram message sent

---

## Phase 3: AI-Powered Arabic Text Parsing

### 3.1 Phase Overview

AI parsing pipeline that:

- Receives raw WhatsApp messages
- Calls LLM API to extract structured medication data
- Handles Arabic text, dosages, prices
- Uses dynamic confidence thresholds
- Queues low-confidence results for review

**Current State:** Basic AI client in Rust; missing advanced features.

### 3.2 Functional Assessment

| Feature            | Legacy | Rust | Gap               |
| ------------------ | ------ | ---- | ----------------- |
| AI parsing         | ✅     | ✅   | None              |
| Embedding          | ✅     | ✅   | None              |
| Retry logic        | ✅ 3x  | ✅   | None              |
| Circuit breaker    | ✅     | ❌   | **High priority** |
| Token batching     | ✅     | ❌   | Medium priority   |
| Dynamic thresholds | ✅     | ❌   | Low               |
| Review queue       | ✅     | ❌   | Medium            |
| FTS mapping        | ✅     | ❌   | Low               |

### 3.3 Pros and Cons

**Pros:**

- Clean async AI client interface
- Retry logic with configurable attempts
- Embedding support for semantic matching
- Gateway (TypeScript) provides OpenAI-compatible API

**Cons:**

- No circuit breaker = cascading failures if AI down
- No token batching = inefficient for large batches
- No review queue = low-confidence data may be lost
- Static confidence = no adaptive learning

### 3.4 Performance Considerations

- **AI Latency:** 500ms-2s per call; batch to reduce round trips
- **Token limits:** LLM has context limits; token batcher prevents truncation
- **Circuit breaker:** Prevents overload during AI outages

**Bottleneck Analysis:**

```
Current: 1 message → 1 AI call → 500ms
Target:  10 messages → 1 AI call → 600ms (10x throughput)
```

### 3.5 Production Readiness

| Criterion       | Status                | Risk   |
| --------------- | --------------------- | ------ |
| AI availability | ⚠️ No circuit breaker | High   |
| Error handling  | ✅ Retry logic        | Low    |
| Throughput      | ⚠️ No batching        | Medium |
| Cost control    | ⚠️ No token tracking  | Medium |
| Data quality    | ⚠️ No review queue    | Medium |

### 3.6 Testing Strategy

```yaml
Unit Tests:
  - Parse response structure validation
  - Retry on transient errors
  - Timeout handling

Integration Tests:
  - Real AI gateway calls with test messages
  - Arabic text parsing accuracy
  - Embedding generation and similarity

Failure Tests:
  - AI gateway timeout (5s, 30s)
  - AI gateway 500 errors
  - Malformed AI responses
  - Circuit breaker trips and recovers
```

### 3.7 Recommendations

1. **Critical:** Implement circuit breaker (failsafe for AI outages)
2. **High:** Add token-aware batching for throughput
3. **Medium:** Add review queue for low-confidence results
4. **Low:** Add AI cost/token tracking metrics

### 3.8 Implementation Tasks

#### Task 3.1: Circuit Breaker (Critical)

- [ ] **Step 1:** Add `failsafe-rs` or implement custom circuit breaker in `core/src/ai/circuit_breaker.rs`
  - States: Closed, Open, HalfOpen
  - Config: failure_threshold=5, recovery_timeout=30s
- [ ] **Step 2:** Wrap `AiClient::parse()` with circuit breaker
- [ ] **Step 3:** Add Prometheus metrics: `circuit_breaker_state`, `circuit_breaker_failures_total`
- [ ] **Step 4:** Return cached/fallback response when circuit is open
- [ ] **Test:** `cargo test circuit_breaker_`
  - Verify circuit opens after 5 failures
  - Verify circuit recovers after timeout
  - Verify fallback response returned when open

#### Task 3.2: Token-Aware Batching

- [ ] **Step 1:** Create `core/src/ai/token_batcher.rs`
  - Estimate tokens per message (~50 avg)
  - Split batch if total > 4000 tokens
- [ ] **Step 2:** Integrate with `process_message` in gRPC server
- [ ] **Step 3:** Add metrics: `ai_batch_size`, `ai_tokens_used`
- [ ] **Test:** `cargo test token_batcher_`
  - Verify large batch splits correctly
  - Verify small batch stays intact

#### Task 3.3: Review Queue

- [ ] **Step 1:** Create `review_queue` table in PostgreSQL
  - Fields: id, raw_message_id, ai_result, confidence, reason, status
- [ ] **Step 2:** Create `ReviewQueueRepository` in `core/src/repository/`
- [ ] **Step 3:** Queue messages with avg_confidence < 0.5
- [ ] **Step 4:** Add API endpoint `GET /api/review-queue`
- [ ] **Test:** `cargo test review_queue_`

#### Task 3.4: AI Latency Metrics

- [ ] **Step 1:** Add histogram `ai_parse_duration_seconds`
- [ ] **Step 2:** Add counter `ai_parse_total` with labels: status (success, error, circuit_open)
- [ ] **Step 3:** Add gauge `ai_batch_pending`
- [ ] **Test:** Verify metrics appear at `/metrics`

---

## Phase 4: Intelligent Matching Engine

### 4.1 Phase Overview

Multi-field scoring engine with:

- 5 scoring factors (medication, dosage, quantity, price, recency)
- Configurable weights per factor
- Confidence bands (AUTO, SUGGEST, REVIEW, NONE)
- Medication gate (reject mismatched medications)

**Current State:** ✅ 100% feature parity with legacy, 148 tests.

### 4.2 Functional Assessment

| Component      | Lines | Tests | Status      |
| -------------- | ----- | ----- | ----------- |
| Scorer         | 360   | 50+   | ✅ Complete |
| Learner        | 815   | 13    | ✅ Complete |
| Scheduler      | 624   | 13    | ✅ Complete |
| Warm Start     | 400   | 10    | ✅ Complete |
| A/B Testing    | 440   | 9     | ✅ Complete |
| Unified Engine | 450   | 7     | ✅ **NEW**  |

**Enhancement:** Unified `MatchingEngine` orchestrates all components.

### 4.3 Pros and Cons

**Pros:**

- All algorithms ported with identical formulas
- 148 comprehensive tests
- Async-first design with tokio
- REST API for weight management
- Type-safe with compile-time guarantees

**Cons:**

- Repository traits not connected to real database
- No feedback persistence = learning disabled in production
- Scheduler runs but can't persist learned weights

### 4.4 Performance Considerations

- **Scoring:** O(1) per match, highly efficient
- **Learning:** O(n) correlation calculation, runs daily
- **A/B Testing:** O(1) deterministic user assignment
- **Warm Start:** O(1) Bayesian blending

**No performance concerns** — matching is the fastest component.

### 4.5 Production Readiness

| Criterion             | Status | Notes           |
| --------------------- | ------ | --------------- |
| Algorithm correctness | ✅     | 148 tests       |
| Thread safety         | ✅     | RwLock patterns |
| Configuration         | ✅     | Env vars        |
| Persistence           | ❌     | Missing repos   |
| Monitoring            | ⚠️     | Basic metrics   |

### 4.6 Testing Strategy

```yaml
Existing Tests (148):
  - Quantity scoring: 12 cases
  - Price scoring: 8 cases
  - Confidence bands: 9 cases
  - Weight learning: 13 cases
  - Scheduler: 13 cases
  - Warm start: 10 cases
  - A/B testing: 9 cases

Additional Tests Needed:
  - End-to-end: Offer + Request → Match
  - Database integration: Save/load weights
  - Feedback loop: Confirm → Learn → Apply
```

### 4.7 Recommendations

1. **Critical:** Implement FeedbackRecordRepository
2. **Critical:** Implement WeightHistoryRepository
3. **High:** Add E2E matching tests with real database
4. **Medium:** Add metrics for match quality (confirmation rate)

### 4.8 Implementation Tasks

#### Task 4.1: FeedbackRecordRepository (Critical)

- [ ] **Step 1:** Create table migration in `migrations/`
  ```sql
  CREATE TABLE feedback_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    match_id UUID NOT NULL REFERENCES matches(id),
    user_id TEXT NOT NULL,
    confirmed BOOLEAN NOT NULL,
    medication_score FLOAT NOT NULL,
    dosage_score FLOAT NOT NULL,
    quantity_score FLOAT NOT NULL,
    price_score FLOAT NOT NULL,
    recency_score FLOAT NOT NULL,
    total_score FLOAT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
  );
  ```
- [ ] **Step 2:** Create `FeedbackRecordRepository` trait in `core/src/repository/feedback.rs`
- [ ] **Step 3:** Implement `PostgresFeedbackRepository`
- [ ] **Step 4:** Add `get_stats()` method for learner
- [ ] **Test:** `cargo test feedback_repo_` - CRUD + aggregation

#### Task 4.2: WeightHistoryRepository (Critical)

- [ ] **Step 1:** Create table migration
  ```sql
  CREATE TABLE weight_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    medication_weight FLOAT NOT NULL,
    dosage_weight FLOAT NOT NULL,
    quantity_weight FLOAT NOT NULL,
    price_weight FLOAT NOT NULL,
    recency_weight FLOAT NOT NULL,
    source TEXT NOT NULL,
    sample_count INT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
  );
  ```
- [ ] **Step 2:** Create `WeightHistoryRepository` trait
- [ ] **Step 3:** Implement `PostgresWeightHistoryRepository`
- [ ] **Step 4:** Add `get_current()` and `get_history(limit)` methods
- [ ] **Test:** `cargo test weight_history_` - save/load/rollback

#### Task 4.3: E2E Matching Test

- [ ] **Step 1:** Create `tests/matching_e2e.rs`
- [ ] **Step 2:** Test flow: Create offer → Create request → Trigger match → Verify saved
- [ ] **Step 3:** Test flow: Confirm match → Record feedback → Verify in DB
- [ ] **Test:** `cargo test --test matching_e2e` with real Postgres

#### Task 4.4: Match Quality Metrics

- [ ] **Step 1:** Add gauge `matching_confirmation_rate`
- [ ] **Step 2:** Add histogram `matching_score_distribution`
- [ ] **Step 3:** Add counter `matching_confidence_band_total` with labels
- [ ] **Test:** Verify metrics at `/metrics`

---

## Phase 5: Confidence-Based Actions

### 5.1 Phase Overview

Automatic actions based on match confidence:

- AUTO (≥0.9): Auto-confirm and notify
- SUGGEST (0.7-0.9): Suggest to operator
- REVIEW (0.5-0.7): Queue for review
- NONE (<0.5): Ignore

**Current State:** Confidence bands exist; action handling incomplete.

### 5.2 Functional Assessment

| Feature          | Legacy | Rust | Gap             |
| ---------------- | ------ | ---- | --------------- |
| Confidence bands | ✅     | ✅   | None            |
| Auto-confirm     | ✅     | ⚠️   | Logic only      |
| Operator notify  | ✅     | ❌   | Not implemented |
| Review queue     | ✅     | ❌   | Not implemented |
| Config runtime   | ✅     | ❌   | Static          |

### 5.3 Pros and Cons

**Pros:**

- Clean ConfidenceBand enum
- Thresholds configurable
- Scoring logic complete

**Cons:**

- No action handler for confidence decisions
- No notification system
- No audit logging
- No operator dashboard integration

### 5.4 Performance Considerations

- **Minimal impact:** Action determination is O(1)
- **Notification bottleneck:** Async notification prevents blocking

### 5.5 Production Readiness

| Criterion     | Status | Risk              |
| ------------- | ------ | ----------------- |
| Core logic    | ✅     | Low               |
| Auto-confirm  | ⚠️     | Medium            |
| Notifications | ❌     | High              |
| Audit trail   | ❌     | High (compliance) |

### 5.6 Testing Strategy

```yaml
Unit Tests:
  - ActionType for each confidence band
  - Config overrides
  - Threshold boundaries (0.899 vs 0.90)

Integration Tests:
  - Auto-confirm updates match status
  - Notification sent for SUGGEST
  - Review queue populated for REVIEW
```

### 5.7 Recommendations

1. **High:** Implement AutoActionHandler in Rust
2. **High:** Add MatchNotifier trait with implementations
3. **Medium:** Add audit logging for all actions
4. **Low:** Add runtime config updates via API

### 5.8 Implementation Tasks

#### Task 5.1: AutoActionHandler

- [ ] **Step 1:** Create `core/src/matching/actions.rs`
  - `MatchAction` enum: AutoConfirm, SuggestToOperator, QueueForReview, Ignore
  - `AutoActionConfig` struct with configurable thresholds
- [ ] **Step 2:** Implement `determine_action(score: &MatchScore) -> MatchAction`
- [ ] **Step 3:** Integrate into `MatchingEngine::score_match()` return
- [ ] **Test:** `cargo test auto_action_` - boundary cases (0.899 vs 0.90)

#### Task 5.2: MatchNotifier Trait

- [ ] **Step 1:** Create `core/src/notify/mod.rs` with trait
  ```rust
  #[async_trait]
  pub trait MatchNotifier: Send + Sync {
      async fn notify_new_match(&self, match_id: &str, score: f64) -> Result<()>;
      async fn notify_suggested(&self, match_id: &str) -> Result<()>;
  }
  ```
- [ ] **Step 2:** Implement `WebSocketNotifier`
- [ ] **Step 3:** Implement `TelegramNotifier` (future)
- [ ] **Test:** `cargo test notifier_` - mock implementations

#### Task 5.3: Audit Logging

- [ ] **Step 1:** Create `audit_logs` table
  ```sql
  CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    action TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id UUID NOT NULL,
    actor TEXT NOT NULL,
    details JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
  );
  ```
- [ ] **Step 2:** Create `AuditLogger` trait and implementation
- [ ] **Step 3:** Log on: match_confirmed, match_rejected, weights_updated
- [ ] **Test:** `cargo test audit_` - verify logs persisted

---

## Phase 6: Multi-Platform Bot Commands

### 6.1 Phase Overview

Bot commands for WhatsApp/Telegram operators:

- `/confirm <id>` - Confirm match
- `/reject <id>` - Reject match
- `/pending` - List pending matches
- `/offers` - List offers
- `/requests` - List requests
- `/dashboard` - Show stats

**Current State:** Remains in Go Bridge; not ported to Rust.

### 6.2 Functional Assessment

| Command    | Go Bridge | Rust API | Gap          |
| ---------- | --------- | -------- | ------------ |
| /confirm   | ✅        | ❌       | API endpoint |
| /reject    | ✅        | ❌       | API endpoint |
| /pending   | ✅ (gRPC) | ⚠️       | Partial      |
| /offers    | ✅ (gRPC) | ✅       | None         |
| /requests  | ✅ (gRPC) | ✅       | None         |
| /dashboard | ✅ (gRPC) | ✅       | None         |

### 6.3 Pros and Cons

**Pros:**

- Commands work via gRPC calls to Rust core
- Audit logging in Go
- Arabic support in responses

**Cons:**

- Duplicate logic in Go and Rust
- No feedback recording in Rust
- Bot-only access (no web operator UI)

### 6.4 Performance Considerations

- **No concerns:** Bot commands are low-frequency
- **Latency:** gRPC adds ~5ms overhead, acceptable

### 6.5 Production Readiness

| Criterion      | Status |
| -------------- | ------ |
| Functionality  | ✅     |
| Error handling | ✅     |
| Audit logging  | ✅     |
| Rate limiting  | ⚠️     |

### 6.6 Testing Strategy

```yaml
Integration Tests:
  - Send /confirm via WhatsApp
  - Verify match status updated
  - Verify audit log created

Load Tests:
  - 100 concurrent /pending calls
  - Measure response latency
```

### 6.7 Recommendations

1. **High:** Add `/confirm` REST endpoint in Rust (for feedback learning)
2. **High:** Add `/reject` REST endpoint in Rust
3. **Medium:** Add rate limiting per user
4. **Low:** Consider web operator dashboard

### 6.8 Implementation Tasks

#### Task 6.1: Confirm REST Endpoint (Critical for Learning)

- [ ] **Step 1:** Create `POST /api/matches/{id}/confirm` endpoint
  - Request body: `{ "user_id": "...", "notes": "..." }`
- [ ] **Step 2:** Update match status to `CONFIRMED`
- [ ] **Step 3:** Record feedback to `FeedbackRecordRepository`
- [ ] **Step 4:** Broadcast WebSocket event `match_confirmed`
- [ ] **Test:** `cargo test confirm_endpoint_` - status update + feedback saved

#### Task 6.2: Reject REST Endpoint

- [ ] **Step 1:** Create `POST /api/matches/{id}/reject` endpoint
  - Request body: `{ "user_id": "...", "reason": "..." }`
- [ ] **Step 2:** Update match status to `REJECTED`
- [ ] **Step 3:** Record negative feedback
- [ ] **Step 4:** Broadcast WebSocket event `match_rejected`
- [ ] **Test:** `cargo test reject_endpoint_`

#### Task 6.3: Rate Limiting

- [ ] **Step 1:** Add tower middleware for rate limiting per IP
- [ ] **Step 2:** Configure: 100 req/min per IP
- [ ] **Step 3:** Add header `X-RateLimit-Remaining`
- [ ] **Test:** Send 150 requests, verify 429 after 100

---

## Phase 7: Real-time Updates

### 7.1 Phase Overview

Real-time event delivery to frontends:

- New offer created
- New request created
- New match found
- Match status changed

**Current State:** WebSocket in Rust; SSE in Legacy Go API.

### 7.2 Functional Assessment

| Feature       | Legacy SSE | Rust WS | Gap            |
| ------------- | ---------- | ------- | -------------- |
| New offer     | ✅         | ✅      | None           |
| New request   | ✅         | ✅      | None           |
| New match     | ✅         | ✅      | None           |
| Heartbeat     | ✅ 30s     | ❌      | Needs impl     |
| Client limit  | ✅ 100     | ❌      | Needs impl     |
| Auth          | ✅ Token   | ❌      | Open           |
| Subscriptions | ✅ Topics  | ❌      | Single channel |

### 7.3 Pros and Cons

**Pros:**

- WebSocket is more efficient than SSE
- Tokio-based async handling
- Broadcast channel pattern

**Cons:**

- No heartbeat = stale connections undetected
- No client limit = potential DoS
- No auth = anyone can connect
- No topics = all events to all clients

### 7.4 Performance Considerations

- **Memory:** Each connection holds a channel buffer
- **Without limits:** 1000+ connections could exhaust memory
- **Heartbeat:** Detects dead connections, allows cleanup

### 7.5 Production Readiness

| Criterion          | Status | Risk   |
| ------------------ | ------ | ------ |
| Core functionality | ✅     | Low    |
| Security           | ❌     | High   |
| Scalability        | ❌     | Medium |
| Reliability        | ⚠️     | Medium |

### 7.6 Testing Strategy

```yaml
Unit Tests:
  - Broadcast to multiple clients
  - Client disconnect cleanup

Integration Tests:
  - Connect WebSocket
  - Trigger offer creation
  - Verify event received

Load Tests:
  - 100 concurrent WebSocket connections
  - Broadcast 1000 events/minute
  - Monitor memory usage

Security Tests:
  - Connect without token → should fail
  - Test connection limits
```

### 7.7 Recommendations

1. **Critical:** Add token-based authentication
2. **High:** Implement client connection limit (100)
3. **High:** Add 30-second heartbeat with pong check
4. **Medium:** Add topic subscriptions

### 7.8 Implementation Tasks

#### Task 7.1: WebSocket Authentication (Critical)

- [ ] **Step 1:** Require token in query param: `ws://host/ws?token=xxx`
- [ ] **Step 2:** Validate token against database/cache
- [ ] **Step 3:** Reject connection with 401 if invalid
- [ ] **Step 4:** Add user_id to connection metadata for logging
- [ ] **Test:** `cargo test ws_auth_` - valid token connects, invalid rejected

#### Task 7.2: Connection Limit

- [ ] **Step 1:** Add `AtomicUsize` counter for active connections
- [ ] **Step 2:** Reject with 503 if count >= 100
- [ ] **Step 3:** Add metric `websocket_connections_active`
- [ ] **Test:** Connect 101 clients, verify 101st rejected

#### Task 7.3: Heartbeat with Pong Check

- [ ] **Step 1:** Send ping every 30 seconds per connection
- [ ] **Step 2:** Expect pong within 10 seconds
- [ ] **Step 3:** Disconnect stale clients (no pong)
- [ ] **Step 4:** Add metric `websocket_stale_connections_closed`
- [ ] **Test:** Connect, disable pong, verify disconnection after 40s

#### Task 7.4: Topic Subscriptions

- [ ] **Step 1:** Allow subscribe message: `{"type": "subscribe", "topics": ["offers", "matches"]}`
- [ ] **Step 2:** Only send events matching subscribed topics
- [ ] **Step 3:** Default: all topics
- [ ] **Test:** Subscribe to `matches` only, verify no `offers` events

---

## Phase 8: Adaptive Learning

### 8.1 Phase Overview

Continuous improvement based on feedback:

1. Collect confirm/reject feedback
2. Calculate factor correlations
3. Adjust weights using learning rate
4. Apply constraints (min/max)
5. Normalize to sum = 1.0
6. Apply via scheduler or A/B test

**Current State:** ✅ All algorithms ported; missing persistence.

### 8.2 Functional Assessment

| Component         | Status | Missing |
| ----------------- | ------ | ------- |
| Correlation calc  | ✅     | -       |
| Weight adjustment | ✅     | -       |
| Constraints       | ✅     | -       |
| Normalization     | ✅     | -       |
| Scheduler         | ✅     | -       |
| Warm Start        | ✅     | -       |
| A/B Testing       | ✅     | -       |
| **Persistence**   | ❌     | Repos   |

### 8.3 Pros and Cons

**Pros:**

- All algorithms implemented and tested
- Unified MatchingEngine orchestrates learning
- REST API for weight management
- A/B testing with statistical significance

**Cons:**

- FeedbackRecordRepository not implemented
- WeightHistoryRepository not implemented
- Learning scheduler runs but can't persist
- No feedback recording from bot/API

### 8.4 Performance Considerations

- **Learning job:** O(n) where n = feedback count; runs daily
- **Impact:** ~1s for 10,000 feedback records
- **No production concern:** Background job, non-blocking

### 8.5 Production Readiness

| Criterion             | Status      | Risk         |
| --------------------- | ----------- | ------------ |
| Algorithm correctness | ✅          | Low          |
| Test coverage         | ✅ 52 tests | Low          |
| Persistence           | ❌          | **Critical** |
| Scheduler             | ✅          | Low          |
| Rollback              | ⚠️          | Medium       |

### 8.6 Testing Strategy

```yaml
Existing Tests (52):
  - Learner: 13 tests
  - Scheduler: 13 tests
  - Warm Start: 10 tests
  - A/B Test: 9 tests
  - Engine: 7 tests

Additional Tests:
  - End-to-end: Feedback → Learn → Apply
  - Persistence: Save/load weights
  - Rollback: Revert to previous weights
  - A/B significance: Chi-square validation
```

### 8.7 Recommendations

1. **Critical:** Implement FeedbackRecordRepository
2. **Critical:** Implement WeightHistoryRepository
3. **High:** Add feedback recording API endpoints
4. **Medium:** Add learning job metrics
5. **Low:** Add rollback via API

### 8.8 Implementation Tasks

#### Task 8.1: Connect Repositories to Engine

- [ ] **Step 1:** Inject `FeedbackRecordRepository` into `MatchingEngine`
- [ ] **Step 2:** Inject `WeightHistoryRepository` into `MatchingEngine`
- [ ] **Step 3:** Update `create_matching_engine()` to accept repos
- [ ] **Step 4:** Modify `main.rs` to pass real implementations
- [ ] **Test:** `cargo test engine_persistence_` - weights persist after restart

#### Task 8.2: End-to-End Learning Test

- [ ] **Step 1:** Create `tests/learning_e2e.rs`
- [ ] **Step 2:** Test flow:
  1. Record 50 confirm + 50 reject feedback
  2. Trigger scheduler job
  3. Verify weights updated in DB
  4. Restart engine, verify weights loaded
- [ ] **Test:** `cargo test --test learning_e2e`

#### Task 8.3: Learning Job Metrics

- [ ] **Step 1:** Add histogram `learning_job_duration_seconds`
- [ ] **Step 2:** Add counter `learning_job_runs_total` with labels (success, failed, skipped)
- [ ] **Step 3:** Add gauge `learning_sample_count`
- [ ] **Test:** Run scheduler, verify metrics updated

#### Task 8.4: Rollback API

- [ ] **Step 1:** Create `POST /api/weights/rollback` endpoint
- [ ] **Step 2:** Load previous weights from history
- [ ] **Step 3:** Apply to engine and save
- [ ] **Step 4:** Add audit log entry
- [ ] **Test:** `cargo test weights_rollback_`

---

## Phase 9: Cross-Cutting Concerns

### 9.1 Phase Overview

Infrastructure supporting all phases:

- Metrics (Prometheus)
- Logging (tracing)
- Configuration (env vars)
- Database (PostgreSQL/sqlx)
- Health checks

### 9.2 Functional Assessment

| Area               | Status | Gap                         |
| ------------------ | ------ | --------------------------- |
| Prometheus metrics | ✅     | Circuit breaker, AI latency |
| Structured logging | ✅     | -                           |
| Trace IDs          | ✅     | -                           |
| Health endpoints   | ✅     | Deep checks                 |
| Database repos     | ⚠️     | 3 missing                   |
| Configuration      | ⚠️     | No YAML                     |

### 9.3 Pros and Cons

**Pros:**

- Good observability foundation
- sqlx compile-time query checking
- tokio async runtime
- Docker Compose ready

**Cons:**

- Missing 3 critical repositories
- No YAML config (env vars only)
- Circuit breaker metrics missing
- No graceful shutdown handling

### 9.4 Performance Considerations

- **Database:** Connection pool tuning needed for production
- **Logging:** Consider async log flushing for high throughput
- **Metrics:** Minimal overhead with prometheus crate

### 9.5 Production Readiness

| Criterion         | Status | Risk   |
| ----------------- | ------ | ------ |
| Observability     | ✅     | Low    |
| Database          | ⚠️     | Medium |
| Configuration     | ⚠️     | Low    |
| Graceful shutdown | ⚠️     | Medium |
| Secret management | ❌     | High   |

### 9.6 Testing Strategy

```yaml
Health Check Tests:
  - /health returns 200
  - /health/ready checks DB
  - /health/live checks process

Database Tests:
  - Connection pool exhaustion
  - Transaction rollback on error
  - Concurrent access patterns

Configuration Tests:
  - Missing required env vars → clear error
  - Invalid values → validation error
```

### 9.7 Recommendations

1. **Critical:** Implement missing repositories
2. **High:** Add deep health checks (DB, AI, gRPC)
3. **High:** Add graceful shutdown handlers
4. **Medium:** Add YAML config support
5. **Medium:** Add secret management (vault/K8s secrets)

### 9.8 Implementation Tasks

#### Task 9.1: Missing Repositories

- [ ] **Step 1:** Implement `FeedbackRecordRepository` (see Task 4.1)
- [ ] **Step 2:** Implement `WeightHistoryRepository` (see Task 4.2)
- [ ] **Step 3:** Implement `ReviewQueueRepository` (see Task 3.3)
- [ ] **Test:** All 3 repos have CRUD + aggregation tests

#### Task 9.2: Deep Health Checks

- [ ] **Step 1:** `/health/ready` → Check DB: `SELECT 1`
- [ ] **Step 2:** `/health/ready` → Check AI: ping gateway
- [ ] **Step 3:** `/health/ready` → Check gRPC: reflection call
- [ ] **Step 4:** Return JSON with component-level status
- [ ] **Test:** Disable DB, verify health returns degraded

#### Task 9.3: Graceful Shutdown

- [ ] **Step 1:** Handle SIGTERM/SIGINT in `main.rs`
- [ ] **Step 2:** Stop accepting new requests
- [ ] **Step 3:** Wait for in-flight requests (10s timeout)
- [ ] **Step 4:** Close WebSocket connections gracefully
- [ ] **Step 5:** Flush logs and metrics
- [ ] **Test:** Send SIGTERM during request, verify request completes

#### Task 9.4: YAML Configuration

- [ ] **Step 1:** Add `config` crate with multiple sources
- [ ] **Step 2:** Load from `config.yaml` with env var overrides
- [ ] **Step 3:** Validate config at startup
- [ ] **Step 4:** Add `--config` CLI flag
- [ ] **Test:** Load from YAML, override with env var

#### Task 9.5: Secret Management

- [ ] **Step 1:** Support reading secrets from `/run/secrets/` (Docker)
- [ ] **Step 2:** Add placeholder for Vault integration
- [ ] **Step 3:** Never log secrets (redact in tracing)
- [ ] **Test:** Set DATABASE_URL via secret file, verify loaded

---

## Summary: Priority Matrix

| Priority     | Phase | Action                              |
| ------------ | ----- | ----------------------------------- |
| **Critical** | 8     | Implement FeedbackRecordRepository  |
| **Critical** | 8     | Implement WeightHistoryRepository   |
| **Critical** | 3     | Add circuit breaker to AI client    |
| **Critical** | 7     | Add WebSocket authentication        |
| **High**     | 2     | Implement per-group ordered queue   |
| **High**     | 5     | Implement AutoActionHandler         |
| **High**     | 6     | Add /confirm and /reject endpoints  |
| **High**     | 7     | Add connection limits and heartbeat |
| **Medium**   | 3     | Token-aware batching                |
| **Medium**   | 5     | Review queue for low-confidence     |
| **Medium**   | 9     | YAML config support                 |
| **Low**      | 6     | Web operator dashboard              |
| **Low**      | 3     | Dynamic confidence thresholds       |

---

## Appendix: File Reference

| Phase | Key Files                       | Lines  |
| ----- | ------------------------------- | ------ |
| 2     | `messaging/whatsapp/manager.go` | 1,656  |
| 3     | `parsing/processor.go`          | 493    |
| 3     | `parsing/retry.go`              | 270    |
| 4     | `matching/scorer.rs`            | 360    |
| 4     | `matching/learner.rs`           | 815    |
| 4     | `matching/engine.rs`            | 450    |
| 5     | `parsing/auto_action.go`        | 268    |
| 6     | `bot/commands/*.go`             | ~1,000 |
| 7     | `api/sse/sse.go`                | 308    |
| 8     | `matching/scheduler.rs`         | 624    |
| 9     | `core/src/grpc/server.rs`       | ~500   |
