# Go → Rust/TypeScript Migration Plan

> **Objective**: Migrate PharmaBroker from Go monolith to Rust core + TypeScript gateway
> **Timeline**: 8 weeks | **Test Coverage**: Maintain 100% feature parity

---

## User Review Required

> [!IMPORTANT]
> This plan involves significant architectural changes. Key decisions requiring approval:
>
> 1. **Database**: Keep PostgreSQL (same schema) or redesign?
> 2. **API Contract**: Keep REST endpoints identical for backward compatibility?
> 3. **Deployment**: Docker Compose first, then Kubernetes?

---

## SOLID Principles Application

| Principle                 | Go Current                | Rust Implementation         |
| ------------------------- | ------------------------- | --------------------------- |
| **S**ingle Responsibility | ✅ Handlers are focused   | Each handler = one file     |
| **O**pen/Closed           | ⚠️ Some hardcoded weights | Trait-based extension       |
| **L**iskov Substitution   | ✅ Interface segregation  | Trait implementations       |
| **I**nterface Segregation | ✅ Reader/Writer split    | Separate traits per concern |
| **D**ependency Inversion  | ✅ Interface injection    | Trait objects / generics    |

---

## Phase 1: Foundation (Week 1-2)

### 1.1 Project Scaffolding

**Verification**: Read Go files to understand structure

| Go Source                         | Verify                                                        | Rust Target                |
| --------------------------------- | ------------------------------------------------------------- | -------------------------- |
| `domain/entity/entity.go`         | ✅ 6 structs: RawMessage, Offer, Request, Match, Group, Stats | `src/domain/*.rs`          |
| `domain/repository/repository.go` | ✅ 15 interfaces with reader/writer pattern                   | `src/repository/traits.rs` |

**Tasks**:

```bash
# Create Rust workspace
cargo new pharma-core --lib
cd pharma-core

# Add dependencies
cargo add tokio --features full
cargo add axum tower tower-http
cargo add sqlx --features runtime-tokio,postgres,chrono,uuid
cargo add serde --features derive
cargo add serde_json chrono uuid
```

**Rust Domain Types** (from `entity.go:8-51`):

```rust
// src/domain/types.rs
#[derive(Debug, Clone, PartialEq, sqlx::Type)]
pub enum MessageType { Offer, Request, Both, Unknown }

#[derive(Debug, Clone, PartialEq, sqlx::Type)]
pub enum ItemStatus { Active, Matched, Expired, Archived }

#[derive(Debug, Clone, PartialEq, sqlx::Type)]
pub enum MatchStatus { Pending, Confirmed, Rejected }
```

**Test Verification**:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_type_serialization() {
        assert_eq!(serde_json::to_string(&MessageType::Offer).unwrap(), "\"Offer\"");
    }
}
```

---

### 1.2 Entity Migration

**Source Files to Verify**:

- `domain/entity/entity.go:54-71` → RawMessage
- `domain/entity/entity.go:86-106` → Offer
- `domain/entity/entity.go:109-128` → Request
- `domain/entity/entity.go:131-142` → Match

**Rust Implementation**:

```rust
// src/domain/offer.rs
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Offer {
    pub id: String,
    pub raw_message_id: String,
    pub source_phone: String,
    pub medication: String,
    pub medication_raw: String,
    pub quantity: f64,
    pub unit: Option<String>,
    pub price: f64,
    pub status: ItemStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

**Verification Checklist**:

- [ ] All 20 fields from Go Offer struct present
- [ ] All 18 fields from Go Request struct present
- [ ] All 10 fields from Go Match struct present
- [ ] JSON serialization matches Go output

---

## Phase 2: Repository Layer (Week 2-3)

### 2.1 Repository Traits

**Source**: `domain/repository/repository.go:13-186`

**Go Interface → Rust Trait Mapping**:

| Go Interface      | Methods   | Rust Trait             |
| ----------------- | --------- | ---------------------- |
| `OfferReader`     | 5 methods | `OfferReadRepository`  |
| `OfferWriter`     | 2 methods | `OfferWriteRepository` |
| `MatchReader`     | 6 methods | `MatchReadRepository`  |
| `AuditRepository` | 3 methods | `AuditRepository`      |

**Rust Trait Example**:

```rust
#[async_trait]
pub trait OfferReadRepository: Send + Sync {
    async fn get_by_id(&self, id: &str) -> Result<Option<Offer>>;
    async fn get_active(&self, limit: i64, offset: i64) -> Result<Vec<Offer>>;
    async fn search(&self, query: &str, limit: i64, offset: i64) -> Result<Vec<Offer>>;
    async fn count_active(&self) -> Result<i64>;
    async fn find_recent_duplicate(
        &self,
        sender_phone: &str,
        medication: &str,
        within: Duration,
    ) -> Result<Option<Offer>>;
}
```

**Verification**: Run existing Go tests, then port each test case

---

### 2.2 PostgreSQL Implementation

**Tasks**:

1. Create `src/repository/postgres/mod.rs`
2. Implement each trait for `PgPool`
3. Port SQL queries from `storage/gorm/*.go`

**Test Strategy** (from Go mock pattern):

```rust
// tests/repository/offer_test.rs
#[sqlx::test]
async fn test_save_and_get_offer(pool: PgPool) {
    let repo = PostgresOfferRepo::new(pool);
    let offer = Offer { id: "test-1".into(), ..Default::default() };

    repo.save(&offer).await.unwrap();
    let found = repo.get_by_id("test-1").await.unwrap();

    assert_eq!(found.unwrap().id, "test-1");
}
```

---

## Phase 3: Matching Engine (Week 3-4)

### 3.1 Scorer Migration

**Source Files**:

- `matching/scorer.go` (432 lines) → `src/matching/scorer.rs`
- `matching/scorer_test.go` (439 lines) → tests in same file

**Test Cases to Port** (from `scorer_test.go`):

| Go Test                       | Line    | Rust Equivalent |
| ----------------------------- | ------- | --------------- |
| `TestQuantityScore`           | 11-53   | 15 test cases   |
| `TestPriceScore`              | 56-100  | 15 test cases   |
| `TestRecencyScore`            | 102-130 | 8 test cases    |
| `TestGetConfidenceBand`       | 158-187 | 12 test cases   |
| `TestScoreMatch`              | 189-241 | 1 integration   |
| `TestScoreMatch_PartialMatch` | 243-290 | 1 integration   |
| `TestScoreMatch_NoMatch`      | 292-319 | 1 integration   |
| `TestUpdateWeights`           | 321-337 | 1 unit          |
| `TestCustomWeights`           | 361-394 | 1 unit          |
| Benchmarks                    | 396-438 | 4 benchmarks    |

**Rust Test Example** (ported from Go):

```rust
// src/matching/scorer.rs
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(10.0, 10.0, 1.0)]           // exact match
    #[case(20.0, 10.0, 1.0)]           // offer exceeds request
    #[case(9.0, 10.0, 1.0)]            // 90% (within tolerance)
    #[case(8.9, 10.0, 0.89)]           // 89% (below tolerance)
    #[case(5.0, 10.0, 0.5)]            // 50% fulfillment
    #[case(0.0, 10.0, 0.0)]            // zero offer
    fn test_quantity_score(#[case] offer: f64, #[case] request: f64, #[case] expected: f64) {
        let scorer = Scorer::default();
        let result = scorer.quantity_score(offer, request);
        assert!((result - expected).abs() < 0.01);
    }
}
```

**Verification**:

```bash
# Run Go tests first
go test ./matching/... -v

# Run Rust tests
cargo test matching:: -- --nocapture
```

---

### 3.2 Learner & Scheduler Migration

**Source Files**:

- `matching/learner.go` (340 lines) → `src/matching/learner.rs`
- `matching/scheduler.go` (380 lines) → `src/matching/scheduler.rs`

**Tests to Port**:

- `learner_test.go`: 8 tests
- `scheduler_test.go`: 9 tests

---

## Phase 4: API Layer (Week 4-5)

### 4.1 REST Handlers

**Source**: `api/handlers/match_handler.go`

**Verified Endpoints**:
| Go Handler | Method | Rust Handler |
|------------|--------|--------------|
| `GetMatchesGin` | GET /api/matches | `get_matches` |
| `ConfirmMatchGin` | POST /api/matches/:id/confirm | `confirm_match` |
| `RejectMatchGin` | POST /api/matches/:id/reject | `reject_match` |
| `ExportMatchesCSVGin` | GET /api/matches/export | `export_csv` |

**Rust Implementation**:

```rust
// src/api/handlers/matches.rs
pub async fn confirm_match(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ConfirmRequest>,
) -> Result<Json<Value>, AppError> {
    let match_entity = state.match_repo.get_by_id(&id).await?
        .ok_or(AppError::NotFound("Match not found"))?;

    state.match_repo.update_status(&id, MatchStatus::Confirmed, &req.matched_by).await?;
    state.offer_repo.update_status(&match_entity.offer_id, ItemStatus::Matched).await?;

    // Audit & feedback (mirrors Go implementation)
    state.audit_repo.log(AuditAction::MatchConfirmed, &id, &format!("By: {}", req.matched_by)).await?;

    Ok(Json(json!({"status": "confirmed"})))
}
```

**Test Verification** (from `match_handler_test.go`):

```rust
#[tokio::test]
async fn test_confirm_match() {
    let app = test_app().await;

    let response = app
        .oneshot(Request::post("/api/matches/test-id/confirm")
            .json(&json!({"matched_by": "operator1"})))
        .await;

    assert_eq!(response.status(), StatusCode::OK);
}
```

---

## Phase 5: Go WhatsApp Bridge (Week 5-6) ✅

### 5.1 Minimal Go Service

**Status: Complete**

The Go bridge is fully implemented with comprehensive resilience features:

**Core Components**:

- `bridge/main.go` - Main bridge with WhatsApp connection
- `bridge/proto/` - gRPC client for Rust core communication

**Resilience Components** (32 tests):

- `bridge/resilience/circuit_breaker.go` - Prevent cascading failures
- `bridge/resilience/retry_buffer.go` - Queue failed messages
- `bridge/resilience/rate_limiter.go` - Token bucket (20/min, burst 5) - 9 tests
- `bridge/historysync/handler.go` - History sync deduplication - 9 tests
- `bridge/deduplicator/deduplicator.go` - Message deduplication - 10 tests
- `bridge/reconnector/reconnector.go` - Exponential backoff - 10 tests
- `bridge/cache/group_cache.go` - Monitored groups cache

**Health Endpoint** (`/health` on port 5050):

- Circuit breaker state
- Retry buffer size
- Deduplicator stats
- Rate limiter stats
- History sync stats

---

## Phase 6: TypeScript Gateway (Week 6-7)

### 6.1 AI SDK Integration

**New TypeScript Service**:

```typescript
// gateway/src/routes/parse.ts
import { streamText } from "ai";
import { openai } from "@ai-sdk/openai";

export async function parseMessage(content: string) {
  const result = await streamText({
    model: openai("gpt-4-turbo"),
    system: PHARMA_PROMPT,
    prompt: content,
  });

  // Forward to Rust core
  const response = await fetch("http://rust-core:8080/api/internal/parse", {
    method: "POST",
    body: JSON.stringify({ parsed: result }),
  });
}
```

---

## Phase 7: Integration Testing (Week 7-8)

### 7.1 E2E Test Suite

**Port from Go**:

- `matching/e2e_test.go` → `tests/e2e/matching.rs`

```rust
#[tokio::test]
async fn test_full_matching_flow() {
    // 1. Create offer
    let offer = create_test_offer().await;

    // 2. Create matching request
    let request = create_test_request().await;

    // 3. Wait for matching to run
    tokio::time::sleep(Duration::from_secs(2)).await;

    // 4. Verify match created
    let matches = get_pending_matches().await;
    assert!(matches.iter().any(|m| m.offer_id == offer.id));
}
```

---

## Verification Checklist

### Per-Phase Verification

| Phase         | Go Tests      | Rust Tests         | Status |
| ------------- | ------------- | ------------------ | ------ |
| 1. Domain     | N/A (structs) | Type serialization | ⬜     |
| 2. Repository | Mock tests    | sqlx::test         | ⬜     |
| 3. Matching   | 37 tests      | rstest + criterion | ⬜     |
| 4. API        | 32 tests      | axum::test         | ⬜     |
| 5. Bridge     | 32 tests ✅   | gRPC integration   | ✅     |
| 6. Gateway    | N/A           | Vitest             | ⬜     |
| 7. E2E        | 5 tests       | Full stack         | ⬜     |

### Test Count Summary

| Go Package      | Test Count   | Rust Equivalent     |
| --------------- | ------------ | ------------------- |
| `matching/`     | 37 tests     | `src/matching/*.rs` |
| `parsing/`      | 24 tests     | `src/parsing/*.rs`  |
| `api/handlers/` | 32 tests     | `tests/api/*.rs`    |
| **Total**       | **93 tests** | **93 tests**        |

---

## Success Criteria

1. ✅ All 93 Go tests have Rust equivalents
2. ✅ API contract unchanged (same endpoints, same JSON)
3. ✅ PostgreSQL schema unchanged
4. ✅ Dashboard works without modification
5. ✅ Performance equal or better than Go

---

## Rollback Plan

1. Keep Go service running on port 8081
2. Use nginx to route traffic
3. Gradual migration: 10% → 50% → 100%
4. Feature flags for new Rust endpoints
