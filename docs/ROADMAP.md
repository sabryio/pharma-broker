# PharmaBroker v2 Migration Roadmap

## Overview

Migration from monolithic Go to multi-service architecture:

- **Rust Core** (port 8080 HTTP, 50051 gRPC) - Business logic, matching, storage
- **Go Bridge** - WhatsApp integration only
- **TypeScript Gateway** (port 3000) - AI parsing, dashboard

---

## Completed ✅

### Phase 1: Foundation

- [x] Moved legacy code to `legacy/`
- [x] Created Rust core structure with domain entities
- [x] Created Go bridge with whatsmeow
- [x] Created TypeScript gateway with Hono

### Phase 2: gRPC Wiring

- [x] Proto definition (`proto/pharma.proto`)
- [x] Rust gRPC server with tonic
- [x] Go gRPC client connecting to Rust

### Phase 3: Hybrid Features (Go Bridge)

- [x] Reconnector (exponential backoff)
- [x] Deduplicator (in-memory cache)
- [x] Skip own messages
- [x] Circuit breaker for gRPC calls
- [x] Retry buffer for failed messages
- [x] Group cache with TTL

### Phase 4: Rust Core Business Logic

- [x] RawMessage save in ProcessMessage
- [x] Group monitoring check (is_monitored)
- [x] Group stats update (async)

---

## Remaining Work 📋

### Phase 5: AI Pipeline Integration

**Priority: High** | **Effort: Medium**

| Task                  | Service | Description                                          |
| --------------------- | ------- | ---------------------------------------------------- |
| Forward to AI Gateway | Rust    | After saving RawMessage, call TypeScript `/ai/parse` |
| Parse response        | Rust    | Handle AI structured output                          |
| Create Offer/Request  | Rust    | Save parsed items to database                        |
| Trigger matching      | Rust    | Call Scorer on new requests                          |

**Files to modify:**

- `core/src/grpc/server.rs` - Add AI gateway call
- `gateway/src/index.ts` - Ensure `/ai/parse` returns structured data

---

### Phase 6: Docker Compose Orchestration

**Priority: High** | **Effort: Low**

Create `docker-compose.yaml` with:

```yaml
services:
  postgres:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: password
      POSTGRES_DB: pharmabroker
    ports: ["5432:5432"]

  redis:
    image: redis:7
    ports: ["6379:6379"]

  core:
    build: ./core
    environment:
      DATABASE_URL: postgres://postgres:password@postgres/pharmabroker
    ports: ["8080:8080", "50051:50051"]
    depends_on: [postgres, redis]

  bridge:
    build: ./bridge
    environment:
      CORE_GRPC_ADDR: core:50051
    depends_on: [core]
    volumes: ["./bridge/data:/app/data"]

  gateway:
    build: ./gateway
    environment:
      CORE_API_URL: http://core:8080
    ports: ["3000:3000"]
    depends_on: [core]
```

---

### Phase 7: Bot Commands as REST API

**Priority: Medium** | **Effort: Medium**

Port legacy bot commands to Rust REST endpoints:

| Command                | Endpoint                         | Description        |
| ---------------------- | -------------------------------- | ------------------ |
| `/offers`              | `GET /api/offers`                | ✅ Already exists  |
| `/requests`            | `GET /api/requests`              | ✅ Already exists  |
| `/pending`             | `GET /api/matches`               | ✅ Already exists  |
| `/confirm <id>`        | `POST /api/matches/{id}/confirm` | ✅ Already exists  |
| `/reject <id>`         | `POST /api/matches/{id}/reject`  | ✅ Already exists  |
| `/status`              | `GET /api/stats`                 | ✅ Already exists  |
| `/groups`              | `GET /api/groups`                | TODO: Add endpoint |
| `/groups add <jid>`    | `POST /api/groups`               | TODO: Add endpoint |
| `/groups remove <jid>` | `DELETE /api/groups/{jid}`       | TODO: Add endpoint |

**Files to create:**

- `core/src/api/handlers/groups.rs` - Group management handlers
- `core/src/api/routes.rs` - Add group routes

---

### Phase 8: Outbound Rate Limiter ✅

**Status: Complete**

Implemented token bucket rate limiter in Go bridge to prevent WhatsApp bans.

**Implementation** (`bridge/resilience/rate_limiter.go`):

- Token bucket algorithm (20 msgs/min default, burst 5)
- `Wait(ctx)` for blocking, `Allow()` for non-blocking
- Statistics tracking (requests, allowed, waited, dropped)
- 9 unit tests passing

---

### Phase 9: History Sync Handling ✅

**Status: Complete**

Implemented history sync handler to avoid duplicate processing of historical messages.

**Implementation** (`bridge/historysync/handler.go`):

- Cooldown period (5 min default)
- Max age filtering (24 hours)
- Processed message ID cache with TTL
- Max messages per sync limit (1000)
- 9 unit tests passing

---

### Phase 10: Database Migrations

**Priority: Medium** | **Effort: Low**

Create proper migration files:

```
migrations/
  001_initial_schema.sql
  002_add_groups_table.sql
  003_add_raw_messages_table.sql
```

Use `sqlx-cli` for Rust:

```sh
cargo install sqlx-cli
sqlx migrate run
```

---

### Phase 11: End-to-End Testing

**Priority: High** | **Effort: Medium**

1. Start all services with Docker Compose
2. Send test WhatsApp message
3. Verify in database:
   - `raw_messages` has entry
   - `offers` or `requests` created
   - `matches` generated if applicable

---

### Phase 12: TypeScript Dashboard

**Priority: Low** | **Effort: High**

Create web UI in `gateway/`:

- React/Vue/Svelte frontend
- Real-time updates via WebSocket
- Display offers, requests, matches
- Group management

---

## Database Tables Required

```sql
-- Groups (for monitoring whitelist)
CREATE TABLE groups (
    jid VARCHAR(50) PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    description TEXT,
    monitored BOOLEAN DEFAULT false,
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_message TIMESTAMPTZ,
    message_count BIGINT DEFAULT 0
);

-- Raw messages (from WhatsApp)
CREATE TABLE raw_messages (
    id VARCHAR(36) PRIMARY KEY,
    external_id VARCHAR(50),
    group_jid VARCHAR(50) NOT NULL,
    group_name VARCHAR(100),
    sender_jid VARCHAR(50) NOT NULL,
    sender_phone VARCHAR(20),
    sender_name VARCHAR(100),
    content TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    processed_at TIMESTAMPTZ,
    error TEXT,
    reply_to_id VARCHAR(50),
    reply_to_content TEXT,
    reply_to_sender VARCHAR(50)
);

-- See legacy migrations for offers, requests, matches
```

---

## Quick Start Commands

```bash
# Start Rust core
cd core && cargo run

# Start Go bridge (separate terminal)
cd bridge && go run .

# Start TypeScript gateway (separate terminal)
cd gateway && bun run dev

# Or use Docker Compose (when ready)
docker-compose up
```

---

## Architecture Diagram

```
┌─────────────────┐     ┌─────────────────────────┐     ┌─────────────────┐
│   WhatsApp      │     │      Go Bridge          │     │   Rust Core     │
│                 │────▶│  - Deduplicator         │────▶│   (gRPC:50051)  │
│                 │     │  - Reconnector          │     │                 │
└─────────────────┘     │  - Circuit Breaker      │     │  - Group check  │
                        │  - Retry Buffer         │     │  - Save message │
                        │  - Rate Limiter ✅      │     │  - Call AI      │
                        │  - History Sync ✅      │     │                 │
                        │  - Group Cache          │     │  HTTP:8080      │
                        └─────────────────────────┘     └─────────────────┘
                                                               │
┌─────────────────┐     ┌─────────────────┐                    │
│   Dashboard     │────▶│   TS Gateway    │────────────────────┘
│   (Browser)     │     │   (port 3000)   │
└─────────────────┘     └─────────────────┘
                               │
                               ▼
                        ┌─────────────────┐
                        │   AI Provider   │
                        │   (LLM)         │
                        └─────────────────┘
```
