# System Overview - PharmaBroker

**AI-Powered Pharmaceutical Trading Platform for Egyptian WhatsApp Groups**

---

## Executive Summary

PharmaBroker is an enterprise-grade polyglot microservices platform that revolutionizes pharmaceutical trading in Egypt by automating the matching of medication supply and demand through WhatsApp groups.

**Key Capabilities:**

- Real-time WhatsApp message processing
- AI-powered Arabic text parsing
- Sophisticated 24-module matching engine
- Adaptive learning from operator feedback
- Real-time dashboard with WebSocket updates

---

## High-Level Architecture

```mermaid
graph TB
    subgraph "External"
        WA[WhatsApp Groups<br/>Egyptian Pharmacists]
    end

    subgraph "Ingestion Layer"
        Bridge[Go WhatsApp Bridge<br/>Port 5050]
        Bridge_Res[Resilience Patterns<br/>• Deduplication<br/>• Rate Limiting<br/>• Circuit Breaker]
    end

    subgraph "Core Processing Layer"
        Core[Rust Core Engine<br/>Port 8081]
        AI[AI Parsing<br/>Arabic NLP]
        Match[Matching Engine<br/>24 Modules]
        Workers[Background Workers<br/>• Match Processor<br/>• Auto-Approve<br/>• Janitor]
    end

    subgraph "Data Layer"
        DB[(PostgreSQL 18<br/>+ pgvector)]
        Redis[(Redis<br/>Cache & Pub/Sub)]
    end

    subgraph "Presentation Layer"
        Frontend[React 19 Frontend<br/>Port 3000]
        WS[WebSocket Server<br/>Real-time Updates]
    end

    subgraph "Observability"
        Prom[Prometheus<br/>Metrics]
        Graf[Grafana<br/>Dashboards]
    end

    WA -->|Messages| Bridge
    Bridge --> Bridge_Res
    Bridge_Res -->|gRPC| Core
    Core --> AI
    AI --> DB
    Core --> Match
    Match --> DB
    Core --> Workers
    Workers --> DB
    Core --> WS
    WS --> Frontend
    Frontend -->|REST API| Core
    DB -.->|Cache| Redis
    Core -->|Metrics| Prom
    Prom --> Graf

    style WA fill:#e1f5ff
    style Bridge fill:#fff4e6
    style Core fill:#f3e5f5
    style DB fill:#e8f5e9
    style Frontend fill:#fce4ec
```

---

## System Components

### 1. Go WhatsApp Bridge

**Purpose:** Real-time message ingestion with resilience patterns

**Responsibilities:**

- WhatsApp protocol integration (whatsmeow)
- Message deduplication (LRU cache)
- Rate limiting (1000/min)
- Circuit breaker (3 failures → 30s timeout)
- Retry buffer (1000 messages)
- QR code authentication
- Group synchronization

**Technology:** Go 1.25+, gRPC, whatsmeow

---

### 2. Rust Core Engine

**Purpose:** AI parsing, matching, and API services

**Responsibilities:**

- AI-powered Arabic text parsing
- 24-module matching engine
- REST API (26+ endpoints)
- gRPC server (5 RPC methods)
- WebSocket server (real-time events)
- Background workers
- Business logic orchestration

**Technology:** Rust 1.75+, Axum, Tonic, SeaORM

---

### 3. PostgreSQL Database

**Purpose:** Primary data store with vector capabilities

**Responsibilities:**

- Persistent data storage (20 tables)
- Vector embeddings (pgvector)
- Full-text search (BM25)
- Referential integrity
- Audit trail storage

**Technology:** PostgreSQL 18+, pgvector extension

---

### 4. React Frontend

**Purpose:** User interface with real-time updates

**Responsibilities:**

- Dashboard visualization
- Match review interface
- Medication management
- Analytics and reporting
- Real-time WebSocket updates

**Technology:** React 19, TanStack Router, React Query

---

## Data Flow Overview

```mermaid
sequenceDiagram
    participant WA as WhatsApp
    participant B as Bridge
    participant C as Core
    participant DB as Database
    participant FE as Frontend

    WA->>B: Message received
    B->>B: Dedup + Rate limit
    B->>C: gRPC ProcessMessage
    C->>DB: Store raw_message

    Note over C: Background Processing
    C->>C: AI Parsing
    C->>DB: Store offer/request
    C->>C: Matching Engine
    C->>DB: Store match

    C->>FE: WebSocket event
    FE->>FE: Update UI

    FE->>C: Operator action
    C->>DB: Update match status
    C->>B: Notify parties (gRPC)
    B->>WA: Send WhatsApp messages
```

---

## Key Features

### 🤖 Intelligent AI Parsing

- Arabic NLP extraction from informal Egyptian dialect
- Multi-model support (Qwen, Ministral, Gemma)
- Local inference (no data leaves infrastructure)
- Feedback loop for continuous improvement

### ⚖️ Advanced Matching Engine

- 24 specialized modules working in harmony
- 5-dimension weighted scoring
- Ensemble strategies (fuzzy, embedding, full-text)
- Confidence calibration (Platt scaling)
- A/B testing framework

### 🧠 Adaptive Learning

- Gradient descent weight optimization
- Warm-start manager for new deployments
- Outlier detection for data quality
- Historical affinity learning

### 🔒 Production-Grade Resilience

- Circuit breaker pattern
- Rate limiting and throttling
- Message deduplication
- Retry mechanisms
- Comprehensive audit trail

### 📊 Real-Time Dashboard

- WebSocket-powered live updates
- Match review and confirmation
- Analytics and reporting
- Medication management
- Audit trail viewer

---

## Technology Stack

| Layer             | Technology     | Version | Purpose                 |
| ----------------- | -------------- | ------- | ----------------------- |
| **Bridge**        | Go             | 1.25+   | WhatsApp integration    |
| **Core**          | Rust           | 1.75+   | Business logic, APIs    |
| **Database**      | PostgreSQL     | 18+     | Data persistence        |
| **Cache**         | Redis          | 8+      | Caching, pub/sub        |
| **Frontend**      | React          | 19      | User interface          |
| **Communication** | gRPC           | -       | Inter-service messaging |
| **Monitoring**    | Prometheus     | -       | Metrics collection      |
| **Visualization** | Grafana        | -       | Dashboards              |
| **Orchestration** | Docker Compose | -       | Service management      |

---

## Deployment Architecture

```mermaid
graph TB
    subgraph "Production Environment"
        subgraph "Load Balancer"
            LB[Nginx/Traefik<br/>SSL Termination]
        end

        subgraph "Application Tier"
            Core1[Core Instance 1]
            Core2[Core Instance 2]
            Core3[Core Instance 3]
            Bridge[Bridge Instance]
        end

        subgraph "Data Tier"
            PG_Primary[(PostgreSQL<br/>Primary)]
            PG_Replica[(PostgreSQL<br/>Replica)]
            Redis_Cluster[(Redis Cluster)]
        end

        subgraph "Monitoring"
            Prom[Prometheus]
            Graf[Grafana]
            Loki[Loki Logs]
        end
    end

    LB --> Core1
    LB --> Core2
    LB --> Core3

    Core1 --> PG_Primary
    Core2 --> PG_Primary
    Core3 --> PG_Primary

    PG_Primary -.->|Replication| PG_Replica

    Core1 --> Redis_Cluster
    Core2 --> Redis_Cluster
    Core3 --> Redis_Cluster

    Bridge --> Core1

    Core1 --> Prom
    Core2 --> Prom
    Core3 --> Prom
    Bridge --> Prom

    Prom --> Graf
    Core1 --> Loki
    Core2 --> Loki
    Core3 --> Loki
```

---

## Performance Characteristics

| Metric                | Current    | Target       | Notes                   |
| --------------------- | ---------- | ------------ | ----------------------- |
| **API Latency (p95)** | 500ms      | <200ms       | Includes AI parsing     |
| **Vector Search**     | 50-200ms   | <20ms        | With HNSW index         |
| **Throughput**        | 50 msg/min | 100+ msg/min | Per instance            |
| **Match Accuracy**    | 75%        | 85%+         | Operator feedback       |
| **Uptime**            | 99.5%      | 99.9%        | With redundancy         |
| **Cache Hit Rate**    | 0%         | 70%+         | After Redis integration |

---

## Security Model

```mermaid
graph LR
    subgraph "Authentication"
        JWT[JWT Tokens<br/>HMAC-SHA256]
        WS_Auth[WebSocket Auth<br/>Token Validation]
    end

    subgraph "Authorization"
        RBAC[Role-Based Access<br/>Operator/Admin]
        API_Keys[API Key Management]
    end

    subgraph "Data Protection"
        TLS[TLS 1.3<br/>In Transit]
        Encrypt[Encryption at Rest<br/>PostgreSQL]
    end

    subgraph "Audit"
        Logs[Audit Logs<br/>All Actions]
        Trail[Audit Trail<br/>Immutable]
    end

    JWT --> RBAC
    WS_Auth --> RBAC
    RBAC --> API_Keys
    TLS --> Encrypt
    RBAC --> Logs
    Logs --> Trail
```

---

## Getting Started

### Prerequisites

- Docker & Docker Compose
- Go 1.25+ (for Bridge development)
- Rust 1.75+ (for Core development)
- Bun (for Frontend development)
- PostgreSQL 18+ (or use Docker)

### Quick Start

```bash
# Clone repository
git clone https://github.com/your-org/pharmabroker.git
cd pharmabroker

# Start all services
docker compose up -d

# View logs
docker compose logs -f core bridge

# Access services
# - Frontend: http://localhost:3000
# - Core API: http://localhost:8082
# - Bridge: http://localhost:5050
# - Grafana: http://localhost:3001
```

### Development Setup

```bash
# Core (Rust)
cd core
cargo run

# Bridge (Go)
cd bridge
go run .

# Frontend (React)
cd frontend
bun install
bun run dev
```

---

## Next Steps

1. **For Developers:** Review [Phase 1: Message Ingestion](../phases/01-message-ingestion.md)
2. **For Architects:** Study [Technology Stack](01-technology-stack.md)
3. **For Operations:** Check [Deployment Guide](../improvements/devops.md)

---

**Document Version:** 1.0  
**Last Updated:** February 16, 2026  
**Next Review:** March 16, 2026
