# PharmaBroker: Legacy Go vs Rust Implementation Analysis

Comprehensive phase-by-phase comparison of the complete legacy Go codebase vs the new Rust implementation.

---

## 1. Executive Summary

### Legacy Codebase Size

| Directory       | Files    | Lines (approx) | Description                                   |
| --------------- | -------- | -------------- | --------------------------------------------- |
| `messaging/`    | 14       | ~2,500         | WhatsApp client, reconnector, deduplicator    |
| `parsing/`      | 34       | ~6,000         | AI parsing, confidence, retry, token batching |
| `matching/`     | 17       | ~2,500         | Scorer, learner, scheduler, A/B, warm start   |
| `bot/`          | 25       | ~2,000         | Commands, Telegram, WhatsApp handlers         |
| `api/`          | 51       | ~4,000         | REST handlers, SSE, middleware                |
| `storage/gorm/` | 41       | ~3,500         | Database repositories                         |
| `ai/`           | 9        | ~500           | AI provider adapters                          |
| `notify/`       | 5        | ~400           | Alert notifications                           |
| **TOTAL**       | **~196** | **~21,400**    |                                               |

### Rust Implementation Status

| Component            | Status        | Lines  | Coverage       |
| -------------------- | ------------- | ------ | -------------- |
| **Core gRPC Server** | ✅ Complete   | ~500   | Full           |
| **Matching Engine**  | ✅ Complete   | ~3,500 | 148 tests      |
| **REST API**         | ✅ Complete   | ~800   | Weights, stats |
| **WebSocket**        | ✅ Complete   | ~200   | Events         |
| **AI Client**        | ✅ Complete   | ~300   | Parse, embed   |
| **Repositories**     | ✅ Complete   | ~800   | Postgres       |
| **Metrics**          | ✅ Complete   | ~200   | Prometheus     |
| **WhatsApp Bridge**  | ⚠️ Go Service | -      | Separate       |
| **Bot Commands**     | ❌ Not ported | -      | Future         |

---

## 2. WhatsApp Message Ingestion

### Legacy Go: [messaging/whatsapp/](file:///e:/programming/brand-new/Golang/pharma-broker/legacy/messaging/whatsapp)

| File          | Lines | Key Features                                |
| ------------- | ----- | ------------------------------------------- |
| `manager.go`  | 1,656 | WhatsApp client, QR pairing, event handling |
| `listener.go` | 300   | Message listener interface                  |

**Core Components:**

```go
// manager.go - Core WhatsApp Manager
type Manager struct {
    client          *whatsmeow.Client      // WhatsApp client
    reconnector     *reconnector.Reconnector
    outboundRateLimiter *OutboundRateLimiter
    groupInfoCache  *GroupInfoCache
    orderedQueue    *OrderedMessageQueue
}

// Features:
// - Rate limiting (20/min default, token bucket)
// - Per-group ordered message processing
// - History sync deduplication (5min cooldown)
// - Group info caching (30min TTL)
// - Reconnection with exponential backoff
```

### Current Architecture

```
┌─────────────────┐     gRPC      ┌─────────────────┐
│ Bridge (Go)     │ ────────────► │ Core (Rust)     │
│                 │               │                 │
│ • WhatsApp      │               │ • AI Client     │
│ • Reconnector   │               │ • Matcher       │
│ • Rate Limiter  │               │ • REST API      │
│ • Deduplicator  │               │ • WebSocket     │
└─────────────────┘               └─────────────────┘
```

### Bridge Gap Analysis

| Feature             | Legacy Go | Bridge     | Gap             |
| ------------------- | --------- | ---------- | --------------- |
| WhatsApp connection | ✅        | ✅         | None            |
| Reconnection        | ✅        | ✅         | None            |
| Rate limiting       | ✅ 20/min | ⚠️ Partial | Config needed   |
| Ordered queues      | ✅        | ❌         | Not implemented |
| Group caching       | ✅ 30min  | ⚠️ 5min    | Lower TTL       |
| History sync dedup  | ✅        | ⚠️         | Not verified    |

---

## 3. AI-Powered Arabic Text Parsing

### Legacy Go: [parsing/](file:///e:/programming/brand-new/Golang/pharma-broker/legacy/parsing)

| File                  | Lines | Purpose                           |
| --------------------- | ----- | --------------------------------- |
| `service.go`          | 450   | Parser main service               |
| `processor.go`        | 493   | Batch processing, circuit breaker |
| `retry.go`            | 270   | Retry executor with presets       |
| `token_batcher.go`    | 200   | Token-aware batching for LLM      |
| `embedding.go`        | 100   | Medication embedding cache        |
| `confidence.go`       | 340   | Adaptive thresholds               |
| `smooth_threshold.go` | 470   | Smooth threshold adjustment       |
| `calibration.go`      | 420   | Threshold calibration             |

**Key Features:**

```go
// processor.go - Token-aware AI Batching
func (p *Parser) processBatch(ctx context.Context, batch []*entity.RawMessage) {
    // 1. Split by token limits for LLM
    subBatches := p.tokenBatcher.SplitIntoBatches(batch)

    // 2. Get relevant medication mappings (RAG-lite via FTS)
    mappings := p.getRelevantMappings(ctx, batch)

    // 3. Circuit breaker + retry for AI calls
    results, err := p.aiCircuitBreaker.ExecuteWithContext(ctx, func() {
        return p.retryExecutor.Execute(ctx, "ai_parse", func() {
            return p.aiProvider.ParseMessages(ctx, batch, mappings)
        })
    })

    // 4. Dynamic confidence thresholds
    avgConfidence := p.calculateAvgConfidence(results)
    if avgConfidence < p.GetCurrentStrictThreshold() {
        p.queueForReview(ctx, msg, result)
    }
}
```

### Rust Implementation

| Feature               | Rust Status | Location        |
| --------------------- | ----------- | --------------- |
| AI parsing            | ✅          | `ai/client.rs`  |
| Embedding             | ✅          | `ai/client.rs`  |
| Retry logic           | ✅          | `retry/mod.rs`  |
| Circuit breaker       | ⚠️ Partial  | Not integrated  |
| Token batching        | ❌          | Not implemented |
| Confidence thresholds | ⚠️ Static   | In matcher only |
| Review queue          | ❌          | Not implemented |

### Gap Analysis

| Feature               | Legacy | Rust | Priority |
| --------------------- | ------ | ---- | -------- |
| Token-aware batching  | ✅     | ❌   | Medium   |
| Dynamic thresholds    | ✅     | ❌   | Low      |
| Review queue          | ✅     | ❌   | Medium   |
| Circuit breaker       | ✅     | ⚠️   | High     |
| FTS medication lookup | ✅     | ❌   | Low      |

---

## 4. Intelligent Matching Engine

### Legacy Go: [matching/](file:///e:/programming/brand-new/Golang/pharma-broker/legacy/matching)

| File            | Lines | Purpose                  |
| --------------- | ----- | ------------------------ |
| `scorer.go`     | 432   | Multi-field scoring      |
| `learner.go`    | 325   | Adaptive weight learning |
| `scheduler.go`  | 452   | Cron-based learning jobs |
| `warm_start.go` | 429   | Bayesian prior blending  |
| `abtest.go`     | 328   | A/B test framework       |
| `interface.go`  | 118   | Types and interfaces     |

### Rust Implementation: ✅ 100% Parity

| Module          | Lines | Tests | Status      |
| --------------- | ----- | ----- | ----------- |
| `scorer.rs`     | 360   | 50+   | ✅ Complete |
| `learner.rs`    | 815   | 13    | ✅ Complete |
| `scheduler.rs`  | 624   | 13    | ✅ Complete |
| `warm_start.rs` | 400   | 10    | ✅ Complete |
| `abtest.rs`     | 440   | 9     | ✅ Complete |
| `engine.rs`     | 450   | 7     | ✅ **NEW**  |
| `dosage.rs`     | 315   | 40+   | ✅ Complete |

**See [matching_comparison_analysis.md](file:///C:/Users/Work%20Pc/.gemini/antigravity/brain/fba0345d-1fe0-4846-86ce-f703febc0fc4/matching_comparison_analysis.md) for detailed algorithm comparison.**

---

## 5. Confidence-Based Actions

### Legacy Go: [parsing/auto_action.go](file:///e:/programming/brand-new/Golang/pharma-broker/legacy/parsing/auto_action.go)

```go
type AutoActionConfig struct {
    AutoBandAction    ActionType // AUTO → auto-confirm
    SuggestBandAction ActionType // SUGGEST → notify operator
    ReviewBandAction  ActionType // REVIEW → queue for review
    AutoConfirmEnabled bool
    MinScoreForAutoConfirm float64 // 0.9
}

// DetermineAction decides based on confidence band
func (h *AutoActionHandler) DetermineAction(score *MatchScore) MatchActionResult {
    switch score.Confidence {
    case ConfidenceAuto:
        if score.Total >= h.config.MinScoreForAutoConfirm {
            return AUTO_CONFIRM
        }
    case ConfidenceSuggest:
        return SUGGEST_TO_OPERATOR
    case ConfidenceReview:
        return QUEUE_FOR_REVIEW
    default:
        return IGNORE
    }
}
```

### Rust Implementation

| Feature                | Status | Notes                 |
| ---------------------- | ------ | --------------------- |
| Confidence bands       | ✅     | `ConfidenceBand` enum |
| Auto-confirm logic     | ⚠️     | Thresholds only       |
| Operator notifications | ❌     | Not implemented       |
| Review queue           | ❌     | Not implemented       |
| Action config          | ❌     | Static behavior       |

### Gap: Future Phase

```rust
// Proposed: src/matching/actions.rs
pub enum MatchAction {
    AutoConfirm,
    SuggestToOperator,
    QueueForReview,
    Ignore,
}

pub struct AutoActionHandler {
    config: AutoActionConfig,
    notifier: Box<dyn MatchNotifier>,
}
```

---

## 6. Multi-Platform Bot Commands

### Legacy Go: [bot/commands/](file:///e:/programming/brand-new/Golang/pharma-broker/legacy/bot/commands)

| Command      | File           | Lines | Purpose         |
| ------------ | -------------- | ----- | --------------- |
| `/confirm`   | `confirm.go`   | 110   | Confirm match   |
| `/reject`    | `reject.go`    | 107   | Reject match    |
| `/pending`   | `pending.go`   | 100   | List pending    |
| `/offers`    | `offers.go`    | 85    | List offers     |
| `/requests`  | `requests.go`  | 90    | List requests   |
| `/dashboard` | `dashboard.go` | 100   | Show stats      |
| `/groups`    | `groups.go`    | 80    | List groups     |
| `/help`      | `help.go`      | 80    | Show help       |
| `/start`     | `start.go`     | 110   | Welcome + setup |
| `/status`    | `status.go`    | 70    | System status   |

**Core Pattern:**

```go
// confirm.go - Bot Command Handler
func (c *ConfirmCommand) Handle(ctx context.Context, cmd *Command, msg *Message) Response {
    matchID := cmd.Args[0]
    match := c.findMatchByPartialID(ctx, matchID)

    c.matchRepo.UpdateStatus(ctx, match.ID, StatusConfirmed, "bot:"+sender)
    c.audit.Log(ctx, AuditMatchConfirmed, match.ID, "Confirmed via bot")

    return Response{Text: "✅ Match confirmed!"}
}
```

### Rust Implementation: ❌ Not Ported

**Reason:** Bot commands remain in Go Bridge. Calls gRPC for data.

**Future Option:** REST API endpoints for bot frontend.

---

## 7. Real-time Updates

### Legacy Go: [api/sse/](file:///e:/programming/brand-new/Golang/pharma-broker/legacy/api/sse)

| File               | Lines | Purpose                  |
| ------------------ | ----- | ------------------------ |
| `sse.go`           | 308   | SSE hub with heartbeat   |
| `sequenced.go`     | 160   | Sequenced event delivery |
| `subscription.go`  | 200   | Topic subscriptions      |
| `auth.go`          | 330   | Token-based auth         |
| `client_health.go` | 230   | Client liveness tracking |

**Core Features:**

```go
// sse.go - SSE Hub
type SSEHub struct {
    clients     map[chan SSEEvent]bool
    maxClients  int  // 100 default
    heartbeat   30 * time.Second
}

func (h *SSEHub) BroadcastNewOffer(offerID, medication string)
func (h *SSEHub) BroadcastNewRequest(requestID, medication string)
func (h *SSEHub) BroadcastNewMatch(matchID string, score float64)
```

### Rust Implementation: ✅ WebSocket

| Feature            | Legacy SSE | Rust WS   | Status         |
| ------------------ | ---------- | --------- | -------------- |
| New offer events   | ✅         | ✅        | Complete       |
| New request events | ✅         | ✅        | Complete       |
| New match events   | ✅         | ✅        | Complete       |
| Heartbeat          | ✅ 30s     | ⚠️ Manual | Needs impl     |
| Client limit       | ✅ 100     | ❌        | Needs impl     |
| Subscriptions      | ✅ Topics  | ❌        | Single channel |
| Auth               | ✅ Token   | ❌        | Open           |

---

## 8. Adaptive Learning

### Legacy Go: [matching/](file:///e:/programming/brand-new/Golang/pharma-broker/legacy/matching) (learner + scheduler + warm_start + abtest)

**Complete Learning Pipeline:**

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│ Feedback    │ ──► │ Correlations │ ──► │ New Weights │
│ (confirm/   │     │ per factor   │     │ + bounds    │
│  reject)    │     └──────────────┘     └─────────────┘
└─────────────┘              │                   │
                    ┌────────▼────────┐    ┌─────▼─────┐
                    │ Warm Start      │ ◄──│ Normalize │
                    │ (Bayesian       │    │ sum = 1.0 │
                    │  blending)      │    └───────────┘
                    └────────┬────────┘
                    ┌────────▼────────┐
                    │ A/B Test        │
                    │ (if active)     │
                    └────────┬────────┘
                    ┌────────▼────────┐
                    │ Scheduler       │
                    │ (auto-apply?)   │
                    └─────────────────┘
```

### Rust Implementation: ✅ Complete

All components ported with 148 tests. See Phase 4 for details.

**New Additions:**

- Unified `MatchingEngine` orchestrator
- 8 REST API endpoints for weight management
- Scheduler env var configuration

---

## 9. Cross-Cutting Concerns

### 9.1 Metrics

| Legacy                | Rust                | Status   |
| --------------------- | ------------------- | -------- |
| Prometheus counters   | ✅ `metrics/mod.rs` | Complete |
| Message processing    | ✅                  | Complete |
| Match queue depth     | ✅                  | Complete |
| AI latency            | ⚠️                  | Partial  |
| Circuit breaker state | ❌                  | Not impl |

### 9.2 Database

| Repository      | Legacy | Rust | Status |
| --------------- | ------ | ---- | ------ |
| Offers          | GORM   | sqlx | ✅     |
| Requests        | GORM   | sqlx | ✅     |
| Matches         | GORM   | sqlx | ✅     |
| Groups          | GORM   | sqlx | ✅     |
| RawMessages     | GORM   | sqlx | ✅     |
| FeedbackRecords | GORM   | ❌   | Gap    |
| WeightHistory   | GORM   | ❌   | Gap    |
| ReviewQueue     | GORM   | ❌   | Gap    |

### 9.3 Configuration

| Source          | Legacy         | Rust        |
| --------------- | -------------- | ----------- |
| YAML config     | ✅ config.yaml | ❌ Env only |
| Env overrides   | ✅             | ✅          |
| Runtime updates | ✅             | ⚠️ Partial  |

### 9.4 Logging

| Feature         | Legacy  | Rust    |
| --------------- | ------- | ------- |
| Structured logs | zerolog | tracing |
| Trace IDs       | ✅      | ✅      |
| Log levels      | ✅      | ✅      |

---

## Summary: Feature Parity Matrix

| Functionality             | Legacy Lines  | Rust Lines    | Status        | Gap Priority |
| ------------------------- | ------------- | ------------- | ------------- | ------------ |
| **1. WhatsApp Ingestion** | ~2,500        | Go Bridge     | ⚠️ Separate   | Low          |
| **2. AI Parsing**         | ~6,000        | ~300          | ⚠️ Core only  | Medium       |
| **3. Matching Engine**    | ~2,500        | ~3,500        | ✅ Complete   | None         |
| **4. Confidence Actions** | ~600          | ⚠️ Partial    | ⚠️ Thresholds | Medium       |
| **5. Bot Commands**       | ~2,000        | Go Bridge     | ❌            | Low          |
| **6. Real-time Updates**  | ~800          | ~200          | ✅ WebSocket  | Low          |
| **7. Adaptive Learning**  | (in matching) | (in matching) | ✅ Complete   | None         |
| **8. Cross-Cutting**      | ~4,000        | ~1,000        | ⚠️ Partial    | Medium       |

---

## Recommended Next Phases

1. **Feedback Recording** - Add `/confirm` and `/reject` API endpoints
2. **Feedback Repository** - `FeedbackRecord` table for learning
3. **Circuit Breaker** - Integrate into AI client
4. **Token Batching** - For large message batches
5. **Review Queue** - Low-confidence message handling
6. **WebSocket Auth** - Token-based authentication
