# PharmaBroker Development Roadmap

> **Last Updated**: December 22, 2025  
> **Architecture**: Polyglot Microservices (Go Bridge + Rust Core)

---

## Current Architecture

```
┌─────────────────┐      ┌──────────────────────────┐      ┌──────────────────┐
│   WhatsApp      │      │       Go Bridge          │      │   Rust Core      │
│                 │─────▶│  - Deduplicator          │─────▶│   (gRPC:50051)   │
│                 │      │  - Reconnector           │      │                  │
└─────────────────┘      │  - Circuit Breaker ✅    │      │  - AI Parsing    │
                         │  - Retry Buffer ✅       │      │  - Matching      │
                         │  - Rate Limiter ✅       │      │  - REST API ✅   │
                         │  - History Sync ✅       │      │  HTTP:8080       │
                         │  - Group Cache ✅        │      │                  │
                         └──────────────────────────┘      └──────────────────┘
                                                                   │
                                                                   ▼
                                                           ┌───────────────────┐
                                                           │ Docker Model      │
                                                           │ Runner (LLM)      │
                                                           │ Qwen • Ministral  │
                                                           └───────────────────┘
```

> **Note**: The TypeScript Gateway was **removed** in favor of direct Docker Model Runner integration from Rust Core.

---

## Completed Phases ✅

### Phase 1-4: Foundation & Core (Complete)

- [x] Rust Core with SeaORM, Axum, Tonic
- [x] Go Bridge with whatsmeow
- [x] gRPC communication between Bridge → Core
- [x] RawMessage persistence
- [x] Group monitoring and stats

### Phase 5: AI Pipeline (Complete)

- [x] Direct Docker Model Runner integration (no TS Gateway)
- [x] AI parsing with structured JSON output
- [x] Offer/Request creation from parsed data
- [x] 24-module matching engine
- [x] Background MatchProcessor worker

### Phase 6: Docker Compose (Complete)

- [x] PostgreSQL 18 with pgvector
- [x] Redis 8 for caching
- [x] Core + Bridge containers
- [x] 4 LLM model bindings (Qwen, Ministral, Gemma, Embedding)

### Phase 7: REST API (Complete)

| Endpoint                        | Status |
| ------------------------------- | ------ |
| `GET /api/offers`               | ✅     |
| `GET /api/requests`             | ✅     |
| `GET /api/matches`              | ✅     |
| `POST /api/matches/:id/confirm` | ✅     |
| `POST /api/matches/:id/reject`  | ✅     |
| `GET /api/groups`               | ✅     |
| `POST /api/groups/:jid/monitor` | ✅     |
| `GET /api/stats`                | ✅     |
| `GET /api/review/queue`         | ✅     |
| `GET /api/weights`              | ✅     |
| `GET /health`                   | ✅     |
| `GET /metrics`                  | ✅     |
| WebSocket `/api/ws`             | ✅     |

### Phase 8-9: Resilience (Complete)

- [x] Token bucket rate limiter (100/min, burst 20)
- [x] History sync deduplication (5min cooldown, 24h max age)
- [x] Circuit breaker (5 failures → 30s timeout)
- [x] Retry buffer (1000 messages, exponential backoff)

### Phase 10: Database Migrations (Complete)

7 migration files applied:

1. Initial schema (groups, raw_messages, offers, requests, matches)
2. Feedback + weights
3. Review queue
4. Audit logs
5. Medication mappings
6. Match queue
7. Urgency + expiry fields

---

## Current Status 🔄

| Area                     | Status         | Notes                         |
| ------------------------ | -------------- | ----------------------------- |
| **WhatsApp Integration** | ✅ Operational | Full resilience stack         |
| **AI Parsing**           | ✅ Operational | 100% medication extraction    |
| **Matching Engine**      | ✅ Operational | 24 modules, 0.82 avg score    |
| **REST API**             | ✅ Operational | All endpoints working         |
| **Learning System**      | ✅ Operational | Gradient descent optimization |
| **Dashboard**            | ❌ Not Started | Future work                   |
| **E2E Tests**            | ❌ Not Started | Future work                   |

---

## Future Work 📋

### Priority 1: E2E Testing

- [ ] Docker-based test environment
- [ ] Full flow integration tests
- [ ] Performance benchmarks

### Priority 2: Web Dashboard

- [ ] React frontend
- [ ] Real-time WebSocket updates
- [ ] Operator management UI

### Priority 3: AI Bot System

See [AI_BOT_SYSTEM_DESIGN.md](../future/AI_BOT_SYSTEM_DESIGN.md) for:

- [ ] Natural language commands
- [ ] MCP tool integration
- [ ] Multi-platform support (WhatsApp + Telegram)

### Priority 4: Monitoring & Observability

- [ ] Enable Prometheus/Grafana stack
- [ ] Custom dashboards
- [ ] Alerting rules

---

## Quick Start

```bash
# Start infrastructure
docker compose up -d postgres redis

# Pull AI models
docker model pull ai/qwen3-vl:latest
docker model pull ai/embeddinggemma:latest

# Terminal 1: Rust Core
cd core && cargo run --release

# Terminal 2: Go Bridge
cd bridge && go run ./cmd/bridge

# Access
# REST API: http://localhost:8080
# QR Code:  http://localhost:5050/qr
```

---

## Related Documentation

- [Architecture Analysis](../ARCHITECTURE_ANALYSIS.md) - Detailed system design
- [Comprehensive Analysis](./comprehensive_analysis.md) - Project evaluation
- [App Review](./app_review.md) - Phase deliverables
- [Future Plans](../future/) - Roadmap for new features

---

_Last updated: December 22, 2025_
