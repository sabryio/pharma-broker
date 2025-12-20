# PharmaBroker Migration Overview

## Architecture

```mermaid
graph TB
    subgraph "Go Services"
        WB[WhatsApp Bridge]
    end

    subgraph "Rust Core Engine"
        GRPC[gRPC Server]
        PARSE[Parsing Module]
        MATCH[Matching Engine]
        API[REST API]
        WS[WebSocket]
        NOTIFY[Notifiers]
        WORKER[Background Jobs]
    end

    subgraph "Storage"
        PG[(PostgreSQL)]
        VEC[(pgvector)]
    end

    subgraph "External"
        AI[AI Gateway]
        TG[Telegram]
        EMAIL[Email/SMTP]
        PROM[Prometheus]
        GRAF[Grafana]
    end

    WB -->|gRPC| GRPC
    GRPC --> PARSE
    PARSE -->|offers/requests| PG
    PARSE -->|embeddings| VEC
    PARSE --> MATCH
    MATCH --> PG
    MATCH --> NOTIFY
    API --> PG
    WS --> API
    NOTIFY --> TG
    NOTIFY --> EMAIL
    API -->|metrics| PROM
    PROM --> GRAF
    WORKER --> PG
```

## Module Structure

| Module               | Purpose                | Phase |
| -------------------- | ---------------------- | ----- |
| `metrics/`           | Prometheus metrics     | 1     |
| `api/handlers`       | Health, CRUD endpoints | 1     |
| `queue/`             | In-memory queue        | 2     |
| `retry/`             | Retry with backoff     | 2     |
| `ai/`                | AI gateway client      | 2     |
| `api/groups`         | Group management       | 3     |
| `ws/`                | WebSocket events       | 3     |
| `grpc/`              | Bridge communication   | 4     |
| `matching/`          | Scorer, weights        | 5     |
| `matching/embedding` | Semantic matching      | 7     |
| `parsing/`           | Batch processing       | 8     |
| `api/middleware/jwt` | JWT auth               | 9     |
| `notify/`            | Telegram, Email        | 10    |
| `worker/janitor`     | Cleanup jobs           | 11    |

## Phase Summary

| #   | Phase           | Status      | Tests           |
| --- | --------------- | ----------- | --------------- |
| 1   | Observability   | ✅          | metrics, health |
| 2   | Reliability     | ✅          | queue, retry    |
| 3   | Features        | ✅          | groups, ws      |
| 4   | Resilience      | ✅          | grpc, cache     |
| 5   | Matching        | ✅          | scorer          |
| 6   | Dashboards      | ✅          | grafana         |
| 7   | Embeddings      | ✅          | semantic        |
| 8   | Parsing         | ✅ 14 tests | batch           |
| 9   | Security        | ✅ 8 tests  | jwt             |
| 10  | Notifications   | ✅ 8 tests  | notify          |
| 11  | Background Jobs | ✅ 2 tests  | janitor         |
| 12  | E2E Testing     | 🔲          | full flow       |

## Data Flow

```mermaid
sequenceDiagram
    participant WB as WhatsApp Bridge
    participant GRPC as gRPC Server
    participant PARSE as Parser
    participant DB as PostgreSQL
    participant MATCH as Matcher
    participant WS as WebSocket

    WB->>GRPC: ProcessMessage
    GRPC->>DB: Save RawMessage
    GRPC->>PARSE: Parse with AI
    PARSE->>DB: Create Offer/Request
    PARSE->>MATCH: Find Matches
    MATCH->>DB: Save Match
    MATCH->>WS: Broadcast Event
```
