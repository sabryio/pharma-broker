# PharmaBroker Architecture Flow Documentation

> Complete end-to-end flow analysis comparing Legacy (Monolithic Go) vs Current (Rust Core + Go Bridge)
> Last Updated: December 21, 2025

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Legacy Architecture Flow](#2-legacy-architecture-flow)
3. [Current Architecture Flow](#3-current-architecture-flow)
4. [Detailed Component Diagrams](#4-detailed-component-diagrams)
5. [Database Schema](#5-database-schema)
6. [Data Flow Comparison](#6-data-flow-comparison)
7. [Key Differences](#7-key-differences)

---

## 1. Architecture Overview

### 1.1 High-Level Comparison

```mermaid
flowchart TB
    subgraph Legacy["Legacy Architecture (Monolithic Go)"]
        direction TB
        WA1[WhatsApp] --> MGR[Manager]
        MGR --> LST[Listener]
        LST --> PRS[Parser]
        PRS --> SCR[Scorer]
        SCR --> API1[HTTP API]
        API1 --> DB1[(PostgreSQL)]
    end

    subgraph Current["Current Architecture (Microservices)"]
        direction TB
        WA2[WhatsApp] --> BRG[Go Bridge]
        BRG -->|gRPC| CORE[Rust Core]
        CORE --> API2[HTTP API]
        API2 --> DB2[(PostgreSQL)]
    end
```

### 1.2 Technology Stack

| Layer                | Legacy            | Current                    |
| -------------------- | ----------------- | -------------------------- |
| WhatsApp Integration | Go (whatsmeow)    | Go Bridge (whatsmeow)      |
| Business Logic       | Go                | Rust Core                  |
| API Framework        | Gin               | Axum                       |
| Inter-service Comm   | N/A (monolith)    | gRPC + Protobuf            |
| AI Integration       | HTTP to Gateway   | Direct Docker Model Runner |
| Database             | PostgreSQL + GORM | PostgreSQL + SQLx          |
| Real-time Updates    | SSE               | WebSocket                  |

---

## 2. Legacy Architecture Flow

### 2.1 Complete Message Flow

```mermaid
flowchart TB
    subgraph WhatsApp["WhatsApp Layer"]
        WA[WhatsApp Server]
    end

    subgraph Manager["Manager Layer (manager.go)"]
        direction TB
        CONN[Connection Handler]
        QR[QR Code Pairing]
        RECON[Reconnector]
        HSYNC[History Sync]
        GCACHE[Group Cache]
        RATE[Rate Limiter]
    end

    subgraph Listener["Listener Layer (listener.go)"]
        direction TB
        RECV[Message Receiver]
        SKIP[Skip Own Messages]
        DEDUP1[Deduplicator]
        GMON[Group Monitor Check]
        SAVE1[Save RawMessage]
        GSTAT[Update Group Stats]
        QUEUE1[Processing Queue]
    end

    subgraph Parser["Parser Layer (processor.go)"]
        direction TB
        BATCH[Batch Processor]
        TOKEN[Token-Aware Splitter]
        FTS[FTS5 Mapping Retrieval]
        AICALL[AI Gateway Call]
        CB1[Circuit Breaker]
        RETRY1[Retry Executor]
        CREATE[Create Offer/Request]
        XPOST[Cross-Post Dedup]
        MQUEUE[Match Queue]
    end

    subgraph Matcher["Matching Layer (scorer.go)"]
        direction TB
        FIND[Find Candidates]
        MEDSCORE[Medication Score 75%]
        DOSESCORE[Dosage Score 5%]
        QTYSCORE[Quantity Score 5%]
        PRICESCORE[Price Score 5%]
        RECSCORE[Recency Score 10%]
        GATE[Medication Gate 50%]
        BAND[Confidence Band]
        STORE1[Store Match]
    end

    subgraph API["API Layer (handlers/)"]
        direction TB
        OFFERS[GET /api/offers]
        REQUESTS[GET /api/requests]
        MATCHES[GET /api/matches]
        CONFIRM[POST /confirm]
        REJECT[POST /reject]
        SSE[SSE Events]
    end

    DB[(PostgreSQL)]

    %% Flow connections
    WA -->|events.Message| CONN
    CONN --> QR
    CONN --> RECON
    CONN --> HSYNC
    CONN --> GCACHE
    CONN --> RATE
    CONN -->|IncomingMessage| RECV

    RECV --> SKIP
    SKIP --> DEDUP1
    DEDUP1 --> GMON
    GMON --> SAVE1
    SAVE1 --> GSTAT
    GSTAT --> QUEUE1

    QUEUE1 --> BATCH
    BATCH --> TOKEN
    TOKEN --> FTS
    FTS --> AICALL
    AICALL --> CB1
    CB1 --> RETRY1
    RETRY1 --> CREATE
    CREATE --> XPOST
    XPOST --> MQUEUE

    MQUEUE --> FIND
    FIND --> MEDSCORE
    MEDSCORE --> GATE
    GATE --> DOSESCORE
    DOSESCORE --> QTYSCORE
    QTYSCORE --> PRICESCORE
    PRICESCORE --> RECSCORE
    RECSCORE --> BAND
    BAND --> STORE1

    STORE1 --> DB
    SAVE1 --> DB
    CREATE --> DB

    DB --> OFFERS
    DB --> REQUESTS
    DB --> MATCHES
    MATCHES --> CONFIRM
    MATCHES --> REJECT
    STORE1 -.->|broadcast| SSE
```

### 2.2 Legacy Scoring Algorithm

```mermaid
flowchart LR
    subgraph Input["Input"]
        OFF[Offer]
        REQ[Request]
    end

    subgraph Scoring["Multi-Field Scoring"]
        MED["Medication<br/>Weight: 75%"]
        DOS["Dosage<br/>Weight: 5%"]
        QTY["Quantity<br/>Weight: 5%"]
        PRC["Price<br/>Weight: 5%"]
        REC["Recency<br/>Weight: 10%"]
    end

    subgraph Gate["Medication Gate"]
        CHECK{Score ≥ 50%?}
    end

    subgraph Bands["Confidence Bands"]
        AUTO["AUTO<br/>≥ 90%"]
        SUGGEST["SUGGEST<br/>70-90%"]
        REVIEW["REVIEW<br/>50-70%"]
        NONE["NONE<br/>< 50%"]
    end

    OFF --> MED
    REQ --> MED
    MED --> CHECK
    CHECK -->|Yes| DOS
    CHECK -->|No| NONE
    DOS --> QTY
    QTY --> PRC
    PRC --> REC
    REC --> AUTO
    REC --> SUGGEST
    REC --> REVIEW
```

---

## 3. Current Architecture Flow

### 3.1 Complete Message Flow

```mermaid
flowchart TB
    subgraph WhatsApp["WhatsApp Layer"]
        WA[WhatsApp Server]
    end

    subgraph Bridge["Go Bridge (bridge/main.go)"]
        direction TB
        WACONN[WhatsApp Client]
        WAQR[QR Pairing]
        BRECON[Reconnector]
        BDEDUP[Deduplicator]
        BGCACHE[Group Cache]
        BCB[Circuit Breaker]
        BRETRY[Retry Buffer]
        BRATE[Rate Limiter]
        BHSYNC[History Sync Handler]
        W1[Worker 1]
        W2[Worker 2]
        WN[Worker N...20]
        BHEALTH[Health Server :5050]
    end

    subgraph GRPC["gRPC Layer"]
        PROTO[Protobuf Messages]
        PRPC[ProcessMessage RPC]
        HRPC[HealthCheck RPC]
        GMRPC[GetMonitoredGroups RPC]
    end

    subgraph Core["Rust Core (core/src/)"]
        direction TB
        GSVC[gRPC Server :50051]
        HAPI[HTTP API :8080]
        HWS[WebSocket /api/events]
        RAWSAVE[Save RawMessage]
        MQADD[Add to Match Queue]
        AIPARSE[AI Parser - Docker Model Runner]
        OFFREQ[Create Offer/Request]
        MATCHER[Matching Engine]
        MPROC[MatchProcessor Worker]
        LSCHED[Learning Scheduler]
    end

    subgraph Database["Database Layer"]
        DB[(PostgreSQL)]
        TRAW[raw_messages]
        TOFF[offers]
        TREQ[requests]
        TMAT[matches]
        TGRP[groups]
        TFEED[feedback]
        TAUDIT[audit_logs]
        TMAP[medication_mappings]
        TQUEUE[match_queue]
    end

    %% WhatsApp to Bridge
    WA -->|events.Message| WACONN
    WA -->|events.HistorySync| BHSYNC
    WACONN --> WAQR
    WACONN --> BRECON

    %% Bridge internal flow (sharding)
    WACONN -->|shard by group| W1
    WACONN -->|shard by group| W2
    WACONN -->|shard by group| WN
    W1 --> BDEDUP
    W2 --> BDEDUP
    WN --> BDEDUP
    BDEDUP --> BGCACHE
    BGCACHE --> BCB
    BCB -->|failure| BRETRY
    BRETRY -->|retry| BCB

    %% Bridge to Core via gRPC
    BCB -->|gRPC| PROTO
    PROTO --> PRPC
    PROTO --> HRPC
    PROTO --> GMRPC
    PRPC --> GSVC

    %% Core processing
    GSVC --> RAWSAVE
    RAWSAVE --> TRAW
    RAWSAVE --> MQADD
    MQADD --> TQUEUE

    %% Background worker
    TQUEUE --> MPROC
    MPROC --> AIPARSE
    AIPARSE --> OFFREQ
    OFFREQ --> TOFF
    OFFREQ --> TREQ
    OFFREQ --> MATCHER
    MATCHER --> TMAT
    MATCHER -.->|broadcast| HWS

    %% Learning
    LSCHED --> TFEED
    LSCHED --> MATCHER

    %% API access
    HAPI --> TOFF
    HAPI --> TREQ
    HAPI --> TMAT
    HAPI --> TGRP

    %% Group sync
    GMRPC --> TGRP
    TGRP --> BGCACHE
```

### 3.2 Go Bridge Detail Flow

```mermaid
flowchart TB
    subgraph Input["WhatsApp Events"]
        MSG[events.Message]
        HIST[events.HistorySync]
        CONN[events.Connected]
        DISC[events.Disconnected]
    end

    subgraph EventHandler["handleEvent()"]
        SWITCH{Event Type?}
    end

    subgraph MessageFlow["Message Processing"]
        ISGRP{Is Group?}
        SHARD[Shard by JID]
        WORKER[Worker Channel]
    end

    subgraph WorkerFlow["workerLoop()"]
        SKIPOWN{Skip Own?}
        CHECKDUP{Duplicate?}
        CHECKGRP{Monitored?}
        EXTRACT[Extract Content]
        FORWARD[forwardToCore]
    end

    subgraph ForwardFlow["forwardToCore()"]
        TRACE[Generate Trace ID]
        PROTO2[Create RawMessage Proto]
        CBCHECK{Circuit Open?}
        GRPCCALL[gRPC ProcessMessage]
        SUCCESS{Success?}
        RETRYBUF[Add to Retry Buffer]
        METRICS[Update Metrics]
    end

    subgraph HistorySyncFlow["handleHistorySync()"]
        COOLDOWN{Cooldown Active?}
        FILTER[Filter Old Messages]
        DEDUPHS[Deduplicate by ID]
        LIMIT[Limit 1000 msgs]
        FWDHS[Forward to Core]
    end

    MSG --> SWITCH
    HIST --> SWITCH
    CONN --> SWITCH
    DISC --> SWITCH

    SWITCH -->|Message| ISGRP
    SWITCH -->|HistorySync| COOLDOWN
    SWITCH -->|Connected| METRICS
    SWITCH -->|Disconnected| METRICS

    ISGRP -->|Yes| SHARD
    ISGRP -->|No| METRICS
    SHARD --> WORKER
    WORKER --> SKIPOWN

    SKIPOWN -->|No| CHECKDUP
    SKIPOWN -->|Yes| METRICS
    CHECKDUP -->|No| CHECKGRP
    CHECKDUP -->|Yes| METRICS
    CHECKGRP -->|Yes| EXTRACT
    CHECKGRP -->|No| METRICS
    EXTRACT --> FORWARD

    FORWARD --> TRACE
    TRACE --> PROTO2
    PROTO2 --> CBCHECK
    CBCHECK -->|No| GRPCCALL
    CBCHECK -->|Yes| RETRYBUF
    GRPCCALL --> SUCCESS
    SUCCESS -->|Yes| METRICS
    SUCCESS -->|No| RETRYBUF
    RETRYBUF -.->|retry| CBCHECK

    COOLDOWN -->|No| FILTER
    COOLDOWN -->|Yes| METRICS
    FILTER --> DEDUPHS
    DEDUPHS --> LIMIT
    LIMIT --> FWDHS
    FWDHS --> FORWARD
```

### 3.3 Rust Core Detail Flow

```mermaid
flowchart TB
    subgraph GRPCServer["gRPC Server (PharmaCoreService)"]
        PM[process_message]
        HC[health_check]
        GMG[get_monitored_groups]
    end

    subgraph ProcessMessageFlow["ProcessMessage Flow"]
        VALIDATE[Validate Input]
        SAVERAW[Save RawMessage]
        CREATEJOB[Create MatchQueueItem]
        ENQUEUE[Enqueue Job]
        RESPOND[Return ProcessResponse]
    end

    subgraph MatchProcessor["MatchProcessor Worker (1s polling)"]
        POLL[Poll Queue]
        DEQUEUE[Dequeue Batch of 10]
        GETREQ[Get Request by ID]
        GETOFFERS[Get Active Offers]
        EMBED[Compare Embeddings]
        SCORE[Score Each Pair]
        CLASSIFY[Classify Confidence]
        STOREMATCH[Store Match]
        DELETEJOB[Delete from Queue]
        BROADCAST[Broadcast via WebSocket]
    end

    subgraph AIParser["AI Parser (Docker Model Runner)"]
        PREP[Build User Prompt]
        ENHANCE[Enhance with Feedback Loop]
        GETMAP[Get Medication Mappings]
        CALLAI[Call LLM via HTTP]
        PARSERESULT[Parse JSON Response]
        CREATEENT[Create Offer/Request]
    end

    subgraph MatchingEngine["Matching Engine"]
        MEDSIM[Medication Similarity]
        DOSEMATCH[Dosage Match]
        QTYMATCH[Quantity Match]
        PRICEMATCH[Price Range Check]
        RECENCY[Recency Factor]
        FINAL[Final Score + Band]
    end

    DB[(PostgreSQL)]

    %% gRPC entry
    PM --> VALIDATE
    VALIDATE --> SAVERAW
    SAVERAW --> DB
    SAVERAW --> CREATEJOB
    CREATEJOB --> ENQUEUE
    ENQUEUE --> DB
    ENQUEUE --> RESPOND

    %% Background worker
    POLL --> DB
    DB --> DEQUEUE
    DEQUEUE --> GETREQ
    GETREQ --> DB
    GETREQ --> GETOFFERS
    GETOFFERS --> DB

    %% AI parsing (triggered inline in server.rs)
    PREP --> ENHANCE
    ENHANCE --> GETMAP
    GETMAP --> DB
    GETMAP --> CALLAI
    CALLAI --> PARSERESULT
    PARSERESULT --> CREATEENT
    CREATEENT --> DB

    %% Matching
    GETOFFERS --> EMBED
    EMBED --> MEDSIM
    MEDSIM --> DOSEMATCH
    DOSEMATCH --> QTYMATCH
    QTYMATCH --> PRICEMATCH
    PRICEMATCH --> RECENCY
    RECENCY --> FINAL
    FINAL --> STOREMATCH
    STOREMATCH --> DB
    STOREMATCH --> DELETEJOB
    DELETEJOB --> DB
    STOREMATCH --> BROADCAST
```

---

## 4. Detailed Component Diagrams

### 4.1 Bridge Resilience Components

```mermaid
flowchart TB
    subgraph Deduplicator["Deduplicator (10s TTL)"]
        DHASH["Hash: group+sender+content"]
        DCACHE["In-Memory Cache"]
        DCLEAN["Cleanup Worker"]
    end

    subgraph Reconnector["Reconnector"]
        RSTATE["State Machine"]
        RBACK["Exponential Backoff"]
        RMAX["Max Attempts: 10"]
    end

    subgraph CircuitBreaker["Circuit Breaker"]
        CBSTATE{State}
        CBCLOSED["Closed: Normal"]
        CBOPEN["Open: Reject All"]
        CBHALF["Half-Open: Test"]
        CBFAIL["Failure Threshold: 3"]
        CBTIMEOUT["Reset Timeout: 30s"]
    end

    subgraph RetryBuffer["Retry Buffer"]
        RBCAP["Capacity: 1000"]
        RBQUEUE["Message Queue"]
        RBWORKER["Retry Worker"]
        RBBACK["Exponential Backoff"]
    end

    subgraph RateLimiter["Rate Limiter"]
        RLBUCKET["Token Bucket"]
        RLRATE["Rate: 20/min"]
        RLBURST["Burst: 5"]
    end

    subgraph HistorySync["History Sync Handler"]
        HSCOOL["Cooldown: 5 min"]
        HSMAXAGE["Max Age: 24h"]
        HSLIMIT["Limit: 1000 msgs"]
    end

    subgraph GroupCache["Group Cache (5 min TTL)"]
        GCMAP["JID → Monitored Map"]
        GCSYNC["Sync from Core via gRPC"]
    end
```

### 4.2 AI Parser Pipeline (Rust Core)

```mermaid
flowchart TB
    subgraph Input["Input Message"]
        CONTENT[Message Content]
        SENDER[Sender Name]
        GROUP[Group Name]
        REPLY[Reply Context]
    end

    subgraph FeedbackLoop["LLM Feedback Loop"]
        EXAMPLES["Few-Shot Examples"]
        CORRECTIONS["Medication Corrections"]
        ENHANCE["build_enhanced_prompt()"]
    end

    subgraph Prompts["Prompt Building"]
        SYSTEM["SYSTEM_PROMPT (130 lines)"]
        USER["build_user_prompt_with_mappings()"]
        MAPPINGS["Medication Mappings from DB"]
    end

    subgraph AIClient["AI Client (Docker Model Runner)"]
        SCHEMA["JSON Schema Generation"]
        HTTP["HTTP POST to LLM"]
        PARSE["Parse Response"]
        RETRY["Retry with Backoff"]
        CB["Circuit Breaker"]
    end

    subgraph Output["Parsed Items"]
        ITEMS["Vec<ParsedItem>"]
        MED["medication"]
        QTY["quantity"]
        PRICE["price"]
        URGENT["urgency_level"]
        EXPIRY["expiry"]
    end

    CONTENT --> USER
    SENDER --> USER
    GROUP --> USER
    REPLY --> USER
    MAPPINGS --> USER

    EXAMPLES --> ENHANCE
    CORRECTIONS --> ENHANCE
    SYSTEM --> ENHANCE
    ENHANCE --> HTTP

    USER --> HTTP
    SCHEMA --> HTTP
    HTTP --> CB
    CB --> RETRY
    RETRY --> PARSE
    PARSE --> ITEMS
    ITEMS --> MED
    ITEMS --> QTY
    ITEMS --> PRICE
    ITEMS --> URGENT
    ITEMS --> EXPIRY
```

---

## 5. Database Schema

```mermaid
erDiagram
    raw_messages {
        string id PK
        string external_id
        string group_jid FK
        string group_name
        string sender_jid
        string sender_phone
        string sender_name
        text content
        timestamp timestamp
        timestamp processed_at
        text error
        string reply_to_id
        text reply_to_content
        string reply_to_sender
    }

    groups {
        string jid PK
        string name
        text description
        boolean monitored
        timestamp added_at
        timestamp last_message
        bigint message_count
    }

    offers {
        string id PK
        string raw_message_id FK
        string source_phone
        string medication
        string medication_raw
        float quantity
        string unit
        float price
        boolean urgent
        string urgency_level
        string expiry_info
        float ai_confidence
        string status
        vector content_embedding
        timestamp created_at
        timestamp updated_at
    }

    requests {
        string id PK
        string raw_message_id FK
        string source_phone
        string medication
        string medication_raw
        float quantity
        string unit
        float max_price
        boolean urgent
        string urgency_level
        string expiry_requirement
        float ai_confidence
        string status
        vector content_embedding
        timestamp created_at
        timestamp updated_at
    }

    matches {
        string id PK
        string offer_id FK
        string request_id FK
        float score
        string reasoning
        string status
        string matched_by
        timestamp created_at
        timestamp confirmed_at
        text notes
    }

    match_queue {
        string id PK
        string request_id FK
        timestamp created_at
        int attempts
        text error
    }

    feedback {
        string id PK
        string match_id FK
        string action
        string user_id
        text reason
        timestamp created_at
    }

    medication_mappings {
        string id PK
        string source_name
        string canonical_name
        float confidence
        timestamp created_at
    }

    audit_logs {
        string id PK
        string action
        string entity_type
        string entity_id
        string user_id
        jsonb details
        timestamp created_at
    }

    raw_messages ||--o{ offers : "parses to"
    raw_messages ||--o{ requests : "parses to"
    groups ||--o{ raw_messages : "contains"
    offers ||--o{ matches : "matched in"
    requests ||--o{ matches : "matched in"
    matches ||--o{ feedback : "receives"
    requests ||--o{ match_queue : "queued"
```

---

## 6. Data Flow Comparison

### 6.1 Side-by-Side Flow

```mermaid
flowchart LR
    subgraph Legacy["Legacy Flow"]
        direction TB
        L1["1. WhatsApp Event"]
        L2["2. Manager handles"]
        L3["3. Listener receives"]
        L4["4. Save RawMessage"]
        L5["5. Queue for parsing"]
        L6["6. Batch AI parse"]
        L7["7. Create Offer/Request"]
        L8["8. Queue for matching"]
        L9["9. Score matches"]
        L10["10. Store & broadcast SSE"]

        L1 --> L2 --> L3 --> L4 --> L5 --> L6 --> L7 --> L8 --> L9 --> L10
    end

    subgraph Current["Current Flow"]
        direction TB
        C1["1. WhatsApp Event"]
        C2["2. Bridge handles"]
        C3["3. Worker processes"]
        C4["4. gRPC to Core"]
        C5["5. Save RawMessage"]
        C6["6. Inline AI parse"]
        C7["7. Create Offer/Request"]
        C8["8. Enqueue for matching"]
        C9["9. MatchProcessor polls"]
        C10["10. Match & broadcast WS"]

        C1 --> C2 --> C3 --> C4 --> C5 --> C6 --> C7 --> C8 --> C9 --> C10
    end
```

### 6.2 Processing Comparison Table

| Step                 | Legacy             | Current                          |
| -------------------- | ------------------ | -------------------------------- |
| 1. Message Reception | Manager → Listener | Bridge → Worker Pool (sharded)   |
| 2. Deduplication     | In Listener        | In Bridge (Deduplicator + Cache) |
| 3. Group Check       | In Listener        | In Bridge (GroupCache + gRPC)    |
| 4. Persistence       | Direct to DB       | gRPC → Core → DB                 |
| 5. AI Parsing        | Batch in Parser    | Inline in `process_message`      |
| 6. Matching          | Batch in Scorer    | Async via MatchProcessor Worker  |
| 7. Real-time Updates | SSE                | WebSocket                        |
| 8. Resilience        | Basic reconnect    | CB + Retry Buffer + Rate Limit   |

---

## 7. Key Differences

### 7.1 Architecture Comparison

```mermaid
flowchart TB
    subgraph LegacyArch["Legacy: Monolithic"]
        direction TB
        LBIN[Single Go Binary]
        LBIN --> LWA[WhatsApp]
        LBIN --> LPARSE[Parser]
        LBIN --> LMATCH[Matcher]
        LBIN --> LAPI[API]
        LBIN --> LDB[(DB)]
    end

    subgraph CurrentArch["Current: Microservices"]
        direction TB
        CBRIDGE[Go Bridge]
        CCORE[Rust Core]
        CBRIDGE -->|gRPC| CCORE
        CBRIDGE --> CWA[WhatsApp]
        CCORE --> CPARSE[AI Parser]
        CCORE --> CMATCH[Matcher]
        CCORE --> CAPI[Axum API]
        CCORE --> CDB[(DB)]
    end
```

### 7.2 Resilience Comparison

| Feature         | Legacy              | Current                   |
| --------------- | ------------------- | ------------------------- |
| Reconnection    | Exponential backoff | Exponential backoff       |
| Circuit Breaker | AI calls only       | gRPC + AI calls           |
| Retry Buffer    | None                | 1000 message buffer       |
| Rate Limiting   | Outbound only       | Outbound + configurable   |
| Deduplication   | 10s window          | 10s window + history sync |
| Health Checks   | Single endpoint     | Bridge :5050 + Core :8080 |
| Group Caching   | 5 min TTL           | 5 min TTL + gRPC sync     |

### 7.3 Scalability

```mermaid
flowchart LR
    subgraph LegacyScale["Legacy Scaling"]
        LS1[Instance 1]
        LS2[Instance 2]
        LS1 -.-x|"❌ Conflict"| LS2
    end

    subgraph CurrentScale["Current Scaling"]
        CB1[Bridge 1]
        CC1[Core 1]
        CC2[Core 2]
        CC3[Core 3]
        CB1 --> CC1
        CB1 --> CC2
        CB1 --> CC3
    end
```

**Note:** Bridge must remain single instance (WhatsApp limitation), but Core can scale horizontally.

### 7.4 Technology Benefits

| Aspect         | Legacy (Go)     | Current (Go + Rust)           |
| -------------- | --------------- | ----------------------------- |
| Memory Safety  | GC-managed      | Rust: Compile-time guarantees |
| Concurrency    | Goroutines      | Rust: async/await + tokio     |
| Performance    | Good            | Excellent (Rust Core)         |
| Type Safety    | Good            | Excellent (Rust)              |
| Error Handling | error interface | Result<T, E> with ? operator  |
| Build Time     | Fast (~10s)     | Slower (~60s Rust)            |
| Binary Size    | Small (~30MB)   | Larger (~50MB Rust)           |

---

## Summary

### Flow Similarity

Both architectures follow the same logical flow:

1. **Ingest** → WhatsApp messages received
2. **Deduplicate** → Filter duplicate messages
3. **Persist** → Save raw messages to database
4. **Parse** → AI extracts offers/requests (with urgency + expiry)
5. **Match** → Score and find matches using embedding similarity
6. **Notify** → Real-time updates to operators

### Key Improvements in Current Architecture

1. **Separation of Concerns**: WhatsApp handling isolated in Go Bridge
2. **Resilience**: Circuit breaker + retry buffer for gRPC failures
3. **Scalability**: Rust Core can scale horizontally
4. **Performance**: Rust for CPU-intensive matching and embedding comparison
5. **AI Integration**: Direct Docker Model Runner (no TypeScript gateway)
6. **Feedback Loop**: LLM learns from operator corrections
7. **Observability**: Better health checks and WebSocket notifications
8. **Type Safety**: Rust's compile-time guarantees prevent runtime errors

### Migration Path

The current architecture maintains compatibility with legacy:

- Same database schema (extended with new fields)
- Same API endpoints (Axum mirrors Gin routes)
- Same matching algorithm (ported from Go scorer)
- Same confidence bands (AUTO/SUGGEST/REVIEW/NONE)
