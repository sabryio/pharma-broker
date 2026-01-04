# PharmaBroker Core v2 Migration Plan

## TypeScript + Effect-TS Rewrite Strategy

This document outlines a professional, step-by-step approach to rewriting the PharmaBroker core application in TypeScript with Effect-TS, while maintaining backward compatibility with the existing Rust core and Go bridge.

---

## 1. Project Assessment

### Current Architecture

| Component      | Language   | Port                      | Responsibility                              |
| -------------- | ---------- | ------------------------- | ------------------------------------------- |
| Rust Core      | Rust       | 8081 (REST), 50051 (gRPC) | AI parsing, matching engine, business logic |
| Go Bridge      | Go         | 5050 (HTTP), 50052 (gRPC) | WhatsApp integration, resilience patterns   |
| React Frontend | TypeScript | 3000                      | User interface, real-time updates           |
| PostgreSQL     | SQL        | 5432                      | Persistent storage with pgvector            |

### Rust Core Modules (24+ modules)

```
core/src/
├── ai/           # AI parsing with LLM
├── api/          # REST API (Axum)
├── grpc/         # gRPC server (Tonic)
├── matching/     # 24-module matching engine
├── worker/       # Background processors
├── repository/   # Data access (SeaORM)
├── domain/       # Domain entities
├── metrics/      # Prometheus
├── notify/       # Notifications
├── ws/           # WebSocket
└── queue/        # Message queue
```

### Go Bridge Architecture (Hexagonal)

```
bridge/
├── app/          # Core orchestration
├── domain/       # Domain models
├── ports/        # Interface definitions
├── adapters/     # Infrastructure (gRPC, WhatsApp)
├── resilience/   # Circuit breaker, rate limiter
└── deduplicator/ # Message deduplication
```

### Database Entities (18 tables)

- `offers`, `requests`, `matches` - Core business
- `raw_messages`, `groups`, `participants` - WhatsApp
- `feedback_records`, `review_queue` - Learning
- `medication_master`, `medication_alias`, `medication_mapping` - Catalog
- `audit_logs`, `match_audit_record` - Compliance
- `weight_history`, `match_queue`, `auto_approve_config` - Engine

### Migration Challenges

1. **Matching Engine Complexity**: 24 modules with ML/gradient descent
2. **Real-time Requirements**: WebSocket, gRPC streaming
3. **AI Integration**: LLM parsing, embeddings, vector search
4. **Data Consistency**: Shared PostgreSQL during migration
5. **Zero Downtime**: Must maintain service availability

---

## 2. TypeScript + Effect-TS Setup

### 2.1 Monorepo Structure

```
core-v2/
├── apps/
│   ├── server/           # Main API server (Hono + Effect)
│   ├── worker/           # Background job processor
│   └── bridge-v2/        # TypeScript bridge (future)
├── packages/
│   ├── api/              # API layer (oRPC + Effect)
│   ├── db/               # Database layer (Prisma + Effect)
│   ├── domain/           # Domain models & types
│   ├── matching/         # Matching engine (Effect)
│   ├── ai/               # AI integration (Effect)
│   ├── auth/             # Authentication (Better-Auth)
│   ├── env/              # Environment config
│   ├── config/           # Shared TypeScript config
│   └── effect-utils/     # Effect utilities & layers
├── package.json
├── turbo.json
└── tsconfig.json
```

### 2.2 TypeScript Configuration

```jsonc
// tsconfig.base.json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "lib": ["ES2022"],
    "strict": true,
    "exactOptionalPropertyTypes": true,
    "noUncheckedIndexedAccess": true,
    "noImplicitOverride": true,
    "noPropertyAccessFromIndexSignature": true,
    "forceConsistentCasingInFileNames": true,
    "verbatimModuleSyntax": true,
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
  },
}
```

### 2.3 Effect-TS Dependencies

```json
{
  "dependencies": {
    "effect": "^3.18.0",
    "@effect/platform": "^0.76.0",
    "@effect/platform-node": "^0.72.0",
    "@effect/schema": "^0.76.0",
    "@effect/sql": "^0.30.0",
    "@effect/sql-pg": "^0.20.0"
  }
}
```

---

## 3. Incremental Migration Strategy

### Phase 1: Foundation (Week 1-2)

**Goal**: Establish Effect-TS infrastructure alongside existing Rust core

```
┌─────────────────────────────────────────────────────────────┐
│                      Frontend (React)                        │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
┌─────────────────────────┐     ┌─────────────────────────┐
│   Rust Core (existing)  │     │  TypeScript v2 (new)    │
│   Port: 8081, 50051     │     │  Port: 8082             │
└─────────────────────────┘     └─────────────────────────┘
              │                               │
              └───────────────┬───────────────┘
                              ▼
                    ┌─────────────────┐
                    │   PostgreSQL    │
                    │   (shared)      │
                    └─────────────────┘
```

**Tasks**:

1. Set up Effect-TS project structure
2. Create Prisma schema matching Rust entities
3. Implement Effect layers for database access
4. Create health check and basic API endpoints
5. Set up parallel deployment in Docker

### Phase 2: Domain Layer (Week 3-4)

**Goal**: Port domain models with Effect Schema

```typescript
// packages/domain/src/offer.ts
import { Schema } from "@effect/schema";

export const OfferId = Schema.UUID.pipe(Schema.brand("OfferId"));
export type OfferId = Schema.Schema.Type<typeof OfferId>;

export const ItemStatus = Schema.Literal(
  "active",
  "matched",
  "expired",
  "cancelled"
);
export type ItemStatus = Schema.Schema.Type<typeof ItemStatus>;

export const UrgencyLevel = Schema.Literal("low", "medium", "high", "critical");
export type UrgencyLevel = Schema.Schema.Type<typeof UrgencyLevel>;

export const Offer = Schema.Struct({
  id: OfferId,
  medication: Schema.String,
  medicationRaw: Schema.NullOr(Schema.String),
  quantity: Schema.NullOr(Schema.String),
  price: Schema.NullOr(Schema.String),
  expiry: Schema.NullOr(Schema.Date),
  status: ItemStatus,
  groupJid: Schema.NullOr(Schema.String),
  senderJid: Schema.NullOr(Schema.String),
  rawMessageId: Schema.NullOr(Schema.UUID),
  aiConfidence: Schema.NullOr(Schema.Number),
  embedding: Schema.NullOr(Schema.Array(Schema.Number)),
  confirmedMatchCount: Schema.Number,
  createdAt: Schema.Date,
  updatedAt: Schema.Date,
});
export type Offer = Schema.Schema.Type<typeof Offer>;
```

### Phase 3: Repository Layer (Week 5-6)

**Goal**: Implement Effect-based repositories

```typescript
// packages/db/src/repositories/offer.ts
import { Effect, Layer, Context } from "effect";
import { SqlClient } from "@effect/sql";
import type { Offer, OfferId } from "@core-v2/domain";

export class OfferRepository extends Context.Tag("OfferRepository")<
  OfferRepository,
  {
    readonly getById: (id: OfferId) => Effect.Effect<Offer | null>;
    readonly getActive: (
      limit: number,
      offset: number
    ) => Effect.Effect<Offer[]>;
    readonly save: (offer: Offer) => Effect.Effect<Offer>;
    readonly updateStatus: (
      id: OfferId,
      status: ItemStatus
    ) => Effect.Effect<Offer>;
    readonly search: (query: string, limit: number) => Effect.Effect<Offer[]>;
    readonly findSemanticDuplicates: (
      embedding: number[],
      threshold: number
    ) => Effect.Effect<Offer[]>;
  }
>() {}

export const OfferRepositoryLive = Layer.effect(
  OfferRepository,
  Effect.gen(function* () {
    const sql = yield* SqlClient.SqlClient;

    return {
      getById: (id) =>
        sql`SELECT * FROM offers WHERE id = ${id}`.pipe(
          Effect.map((rows) => rows[0] ?? null)
        ),

      getActive: (limit, offset) =>
        sql`SELECT * FROM offers WHERE status = 'active' ORDER BY created_at DESC LIMIT ${limit} OFFSET ${offset}`,

      save: (offer) =>
        sql`INSERT INTO offers ${sql.insert(offer)} ON CONFLICT (id) DO UPDATE SET ${sql.update(offer)} RETURNING *`.pipe(
          Effect.map((rows) => rows[0])
        ),

      // ... more methods
    };
  })
);
```

### Phase 4: API Layer (Week 7-8)

**Goal**: Implement oRPC + Effect API handlers

```typescript
// packages/api/src/routers/offers.ts
import { Effect } from "effect";
import { o, protectedProcedure } from "../index";
import { OfferRepository } from "@core-v2/db";
import { Offer, OfferId } from "@core-v2/domain";
import { z } from "zod";

export const offersRouter = {
  list: protectedProcedure
    .input(
      z.object({ limit: z.number().default(50), offset: z.number().default(0) })
    )
    .handler(async ({ input, context }) => {
      const program = Effect.gen(function* () {
        const repo = yield* OfferRepository;
        return yield* repo.getActive(input.limit, input.offset);
      });

      return Effect.runPromise(program.pipe(Effect.provide(context.layers)));
    }),

  getById: protectedProcedure
    .input(z.object({ id: z.string().uuid() }))
    .handler(async ({ input, context }) => {
      const program = Effect.gen(function* () {
        const repo = yield* OfferRepository;
        const offer = yield* repo.getById(input.id as OfferId);
        if (!offer) {
          return yield* Effect.fail(new NotFoundError("Offer not found"));
        }
        return offer;
      });

      return Effect.runPromise(program.pipe(Effect.provide(context.layers)));
    }),
};
```

### Phase 5: Matching Engine (Week 9-12)

**Goal**: Port matching engine with Effect

```typescript
// packages/matching/src/engine.ts
import { Effect, Layer, Context, Ref, Stream } from "effect";
import type { Offer, Request, Match, ConfidenceBand } from "@core-v2/domain";

export interface MatchScore {
  readonly total: number;
  readonly medication: number;
  readonly quantity: number;
  readonly dosage: number;
  readonly price: number;
  readonly recency: number;
  readonly band: ConfidenceBand;
  readonly reasoning: string;
}

export class MatchingEngine extends Context.Tag("MatchingEngine")<
  MatchingEngine,
  {
    readonly findMatches: (request: Request) => Effect.Effect<Match[]>;
    readonly scoreMatch: (
      offer: Offer,
      request: Request
    ) => Effect.Effect<MatchScore>;
    readonly getWeights: () => Effect.Effect<MatchWeights>;
    readonly updateWeights: (weights: MatchWeights) => Effect.Effect<void>;
  }
>() {}

// Scorer implementation
export const createScorer = (weights: MatchWeights) => ({
  score: (offer: Offer, request: Request): Effect.Effect<MatchScore> =>
    Effect.gen(function* () {
      const medication = yield* scoreMedication(offer, request);
      const quantity = yield* scoreQuantity(offer, request);
      const dosage = yield* scoreDosage(offer, request);
      const price = yield* scorePrice(offer, request);
      const recency = yield* scoreRecency(offer);

      const total =
        medication * weights.medication +
        quantity * weights.quantity +
        dosage * weights.dosage +
        price * weights.price +
        recency * weights.recency;

      return {
        total,
        medication,
        quantity,
        dosage,
        price,
        recency,
        band: getConfidenceBand(total),
        reasoning: generateReasoning({
          medication,
          quantity,
          dosage,
          price,
          recency,
        }),
      };
    }),
});
```

### Phase 6: AI Integration (Week 13-14)

**Goal**: Port AI parsing with Effect

```typescript
// packages/ai/src/parser.ts
import { Effect, Layer, Context, Schedule } from "effect";
import { HttpClient } from "@effect/platform";

export interface ParseResult {
  readonly type: "offer" | "request" | "both" | "unknown";
  readonly medication: string | null;
  readonly quantity: string | null;
  readonly price: string | null;
  readonly expiry: Date | null;
  readonly confidence: number;
}

export class PharmaParser extends Context.Tag("PharmaParser")<
  PharmaParser,
  {
    readonly parse: (text: string) => Effect.Effect<ParseResult, ParseError>;
    readonly batchParse: (texts: string[]) => Effect.Effect<ParseResult[]>;
  }
>() {}

export const PharmaParserLive = Layer.effect(
  PharmaParser,
  Effect.gen(function* () {
    const http = yield* HttpClient.HttpClient;
    const config = yield* AiConfig;

    return {
      parse: (text) =>
        Effect.gen(function* () {
          const response = yield* http
            .post(`${config.baseUrl}/v1/chat/completions`, {
              body: JSON.stringify({
                model: config.model,
                messages: [
                  { role: "system", content: PARSER_SYSTEM_PROMPT },
                  { role: "user", content: text },
                ],
                response_format: { type: "json_object" },
              }),
            })
            .pipe(
              Effect.retry(
                Schedule.exponential("1 second").pipe(Schedule.jittered)
              ),
              Effect.timeout("30 seconds")
            );

          const json = yield* Effect.tryPromise(() => response.json());
          return yield* parseAiResponse(json);
        }),
    };
  })
);
```

### Phase 7: Bridge v2 (Week 15-16)

**Goal**: Create TypeScript bridge with Effect

```typescript
// apps/bridge-v2/src/index.ts
import { Effect, Layer, Stream, Queue } from "effect";
import { WhatsAppClient } from "./adapters/whatsapp";
import { CoreClient } from "./adapters/core";
import { CircuitBreaker, RateLimiter, Deduplicator } from "./resilience";

const BridgeProgram = Effect.gen(function* () {
  const whatsapp = yield* WhatsAppClient;
  const core = yield* CoreClient;
  const circuitBreaker = yield* CircuitBreaker;
  const rateLimiter = yield* RateLimiter;
  const dedup = yield* Deduplicator;

  yield* whatsapp.connect();

  const messageStream = whatsapp.messages().pipe(
    Stream.filter((msg) => !dedup.isDuplicate(msg.id)),
    Stream.tap((msg) => dedup.mark(msg.id)),
    Stream.mapEffect((msg) =>
      rateLimiter.acquire.pipe(
        Effect.flatMap(() => circuitBreaker.call(core.processMessage(msg)))
      )
    ),
    Stream.runDrain
  );

  yield* messageStream;
});
```

---

## 4. Typing and Interfaces

### 4.1 Branded Types for Type Safety

```typescript
// packages/domain/src/ids.ts
import { Schema } from "@effect/schema";

// Branded UUID types prevent mixing IDs
export const OfferId = Schema.UUID.pipe(Schema.brand("OfferId"));
export const RequestId = Schema.UUID.pipe(Schema.brand("RequestId"));
export const MatchId = Schema.UUID.pipe(Schema.brand("MatchId"));
export const GroupJid = Schema.String.pipe(Schema.brand("GroupJid"));

export type OfferId = Schema.Schema.Type<typeof OfferId>;
export type RequestId = Schema.Schema.Type<typeof RequestId>;
export type MatchId = Schema.Schema.Type<typeof MatchId>;
export type GroupJid = Schema.Schema.Type<typeof GroupJid>;
```

### 4.2 Effect Services Pattern

```typescript
// packages/effect-utils/src/service.ts
import { Effect, Context, Layer } from "effect";

// Define service interface
export class MyService extends Context.Tag("MyService")<
  MyService,
  {
    readonly doSomething: (input: string) => Effect.Effect<Result, MyError>;
  }
>() {}

// Implement service
export const MyServiceLive = Layer.succeed(MyService, {
  doSomething: (input) => Effect.succeed({ value: input }),
});

// Use service
const program = Effect.gen(function* () {
  const service = yield* MyService;
  return yield* service.doSomething("hello");
});
```

### 4.3 Error Types

```typescript
// packages/domain/src/errors.ts
import { Data } from "effect";

export class NotFoundError extends Data.TaggedError("NotFoundError")<{
  readonly entity: string;
  readonly id: string;
}> {}

export class ValidationError extends Data.TaggedError("ValidationError")<{
  readonly field: string;
  readonly message: string;
}> {}

export class DatabaseError extends Data.TaggedError("DatabaseError")<{
  readonly operation: string;
  readonly cause: unknown;
}> {}

export class AiParseError extends Data.TaggedError("AiParseError")<{
  readonly text: string;
  readonly cause: unknown;
}> {}

export type AppError =
  | NotFoundError
  | ValidationError
  | DatabaseError
  | AiParseError;
```

---

## 5. Error Handling and Testing

### 5.1 Effect Error Handling

```typescript
// Comprehensive error handling with Effect
const processMessage = (msg: RawMessage) =>
  Effect.gen(function* () {
    const parser = yield* PharmaParser;
    const offerRepo = yield* OfferRepository;
    const requestRepo = yield* RequestRepository;

    const parsed = yield* parser.parse(msg.content).pipe(
      Effect.catchTag("AiParseError", (e) =>
        Effect.gen(function* () {
          yield* logError("AI parse failed", e);
          return { type: "unknown" as const, confidence: 0 };
        })
      )
    );

    if (parsed.type === "offer") {
      return yield* offerRepo.save(createOffer(msg, parsed));
    } else if (parsed.type === "request") {
      return yield* requestRepo.save(createRequest(msg, parsed));
    }

    return null;
  }).pipe(
    Effect.catchAll((e) =>
      Effect.gen(function* () {
        yield* logError("Message processing failed", e);
        return null;
      })
    )
  );
```

### 5.2 Testing with Effect

```typescript
// packages/matching/src/__tests__/engine.test.ts
import { Effect, Layer } from "effect";
import { describe, it, expect } from "vitest";
import { MatchingEngine, MatchingEngineLive } from "../engine";

describe("MatchingEngine", () => {
  const TestLayer = Layer.mergeAll(
    MatchingEngineLive,
    MockOfferRepository,
    MockRequestRepository
  );

  it("should score medication match correctly", async () => {
    const program = Effect.gen(function* () {
      const engine = yield* MatchingEngine;
      const score = yield* engine.scoreMatch(mockOffer, mockRequest);
      return score;
    });

    const result = await Effect.runPromise(
      program.pipe(Effect.provide(TestLayer))
    );

    expect(result.medication).toBeGreaterThan(0.8);
    expect(result.band).toBe("auto");
  });
});
```

---

## 6. Effect Integration Patterns

### 6.1 Layer Composition

```typescript
// apps/server/src/layers.ts
import { Layer } from "effect";
import { SqlLive } from "@effect/sql-pg";
import { OfferRepositoryLive, RequestRepositoryLive } from "@core-v2/db";
import { MatchingEngineLive } from "@core-v2/matching";
import { PharmaParserLive } from "@core-v2/ai";

// Compose all layers
export const AppLive = Layer.mergeAll(
  SqlLive,
  OfferRepositoryLive,
  RequestRepositoryLive,
  MatchingEngineLive,
  PharmaParserLive
).pipe(Layer.provide(ConfigLive), Layer.provide(LoggerLive));
```

### 6.2 Resource Management

```typescript
// Automatic resource cleanup with Effect
const withDatabase = <A, E>(
  program: Effect.Effect<A, E, SqlClient.SqlClient>
) =>
  Effect.scoped(
    Effect.gen(function* () {
      const pool = yield* Effect.acquireRelease(createPool(config), (pool) =>
        Effect.promise(() => pool.end())
      );
      return yield* program.pipe(
        Effect.provideService(SqlClient.SqlClient, pool)
      );
    })
  );
```

### 6.3 Streaming with Effect

```typescript
// Real-time match processing with Effect Stream
const matchProcessor = Stream.fromQueue(matchQueue).pipe(
  Stream.mapEffect((item) =>
    Effect.gen(function* () {
      const engine = yield* MatchingEngine;
      const matches = yield* engine.findMatches(item.request);
      return { item, matches };
    })
  ),
  Stream.tap(({ item, matches }) =>
    Effect.forEach(matches, (match) => saveMatch(match))
  ),
  Stream.runDrain
);
```

---

## 7. Code Quality and Tooling

### 7.1 ESLint Configuration

```javascript
// eslint.config.js
import eslint from "@eslint/js";
import tseslint from "typescript-eslint";
import effectPlugin from "@effect/eslint-plugin";

export default tseslint.config(
  eslint.configs.recommended,
  ...tseslint.configs.strictTypeChecked,
  effectPlugin.configs.recommended,
  {
    rules: {
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_" },
      ],
      "@typescript-eslint/consistent-type-imports": "error",
      "@typescript-eslint/no-floating-promises": "error",
      "effect/no-floating-effects": "error",
    },
  }
);
```

### 7.2 Biome Configuration

```json
// biome.json
{
  "$schema": "https://biomejs.dev/schemas/1.9.0/schema.json",
  "organizeImports": { "enabled": true },
  "linter": {
    "enabled": true,
    "rules": {
      "recommended": true,
      "correctness": {
        "noUnusedImports": "error",
        "noUnusedVariables": "error"
      },
      "style": {
        "useConst": "error",
        "noNonNullAssertion": "warn"
      }
    }
  },
  "formatter": {
    "indentStyle": "space",
    "indentWidth": 2
  }
}
```

---

## 8. Backward Compatibility Strategy

### 8.1 Parallel Deployment

```yaml
# docker-compose.yaml additions
services:
  core-v2:
    build:
      context: ./core-v2
      dockerfile: apps/server/Dockerfile
    container_name: pharma-core-v2
    environment:
      DATABASE_URL: postgres://postgres:password@postgres:5432/pharmabroker
      PORT: 8082
    ports:
      - "8082:8082"
    depends_on:
      postgres:
        condition: service_healthy
    networks:
      - pharmabroker-network

  # Nginx for gradual traffic shifting
  nginx:
    image: nginx:alpine
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf
    ports:
      - "80:80"
    depends_on:
      - core
      - core-v2
```

### 8.2 Feature Flags

```typescript
// packages/config/src/features.ts
export const FeatureFlags = {
  USE_V2_MATCHING: process.env.USE_V2_MATCHING === "true",
  USE_V2_PARSING: process.env.USE_V2_PARSING === "true",
  USE_V2_API: process.env.USE_V2_API === "true",
};

// Usage in API
const getMatches = async (requestId: string) => {
  if (FeatureFlags.USE_V2_MATCHING) {
    return v2MatchingEngine.findMatches(requestId);
  }
  return rustCoreClient.getMatches(requestId);
};
```

### 8.3 Database Migration Strategy

```sql
-- Prisma schema additions (non-breaking)
-- Add new columns as nullable first
ALTER TABLE offers ADD COLUMN v2_processed BOOLEAN DEFAULT FALSE;
ALTER TABLE matches ADD COLUMN v2_score JSONB;

-- After migration complete, make required
ALTER TABLE offers ALTER COLUMN v2_processed SET NOT NULL;
```

---

## 9. Migration Timeline

| Phase         | Duration   | Deliverables                           |
| ------------- | ---------- | -------------------------------------- |
| 1. Foundation | Week 1-2   | Effect setup, Prisma schema, basic API |
| 2. Domain     | Week 3-4   | All domain types with Effect Schema    |
| 3. Repository | Week 5-6   | All repositories with Effect           |
| 4. API        | Week 7-8   | Full API parity with Rust              |
| 5. Matching   | Week 9-12  | Matching engine port                   |
| 6. AI         | Week 13-14 | AI parsing integration                 |
| 7. Bridge v2  | Week 15-16 | TypeScript bridge                      |
| 8. Testing    | Week 17-18 | Full test coverage, load testing       |
| 9. Cutover    | Week 19-20 | Gradual traffic shift, monitoring      |

---

## 10. Success Criteria

- [ ] All 18 database entities mapped to Prisma
- [ ] All repository traits implemented with Effect
- [ ] API parity with Rust core (all endpoints)
- [ ] Matching engine produces identical scores (±0.01)
- [ ] AI parsing maintains same accuracy
- [ ] WebSocket real-time updates working
- [ ] gRPC bridge communication functional
- [ ] Zero data loss during migration
- [ ] Performance within 20% of Rust core
- [ ] 80%+ test coverage

---

## 11. Bridge v2 Migration Plan

### Current Go Bridge Architecture

The Go bridge uses Hexagonal Architecture with clean separation:

```
bridge/
├── app/           # Core orchestration (Bridge struct)
├── domain/        # Message, GroupInfo, JID types
├── ports/         # Interfaces (MessageSource, MessageSink)
├── adapters/      # Implementations (gRPC, WhatsApp, QR)
├── resilience/    # Circuit breaker, rate limiter, retry
└── deduplicator/  # LRU-based message deduplication
```

### Bridge v2 Options

#### Option A: TypeScript Bridge (Recommended for Consistency)

```typescript
// apps/bridge-v2/src/index.ts
import { Effect, Layer, Stream, Queue, Schedule } from "effect";

const BridgeProgram = Effect.gen(function* () {
  const whatsapp = yield* WhatsAppClient;
  const core = yield* CoreClient;
  const circuitBreaker = yield* CircuitBreaker;
  const rateLimiter = yield* RateLimiter;
  const dedup = yield* Deduplicator;

  yield* whatsapp.connect();

  yield* whatsapp.messages().pipe(
    Stream.filter((msg) => !dedup.isDuplicate(msg.id)),
    Stream.tap((msg) => dedup.mark(msg.id)),
    Stream.mapEffect((msg) =>
      rateLimiter.acquire.pipe(
        Effect.flatMap(() => circuitBreaker.call(core.processMessage(msg))),
        Effect.retry(
          Schedule.exponential("1 second").pipe(
            Schedule.jittered,
            Schedule.upTo("30 seconds")
          )
        )
      )
    ),
    Stream.runDrain
  );
});
```

#### Option B: Keep Go Bridge (Recommended for Stability)

Keep the Go bridge as-is and update it to communicate with TypeScript core:

```go
// Update gRPC client to point to TypeScript server
type BridgeConfig struct {
    // Support both Rust and TypeScript backends
    CoreGrpcAddr   string // Rust: core:50051
    CoreV2HttpAddr string // TypeScript: core-v2:8082
    UseV2          bool   // Feature flag
}
```

### Recommended Approach: Hybrid

1. **Phase 1**: Keep Go bridge, add HTTP client for TypeScript core
2. **Phase 2**: Gradually shift traffic using feature flags
3. **Phase 3**: Optionally rewrite bridge in TypeScript if needed

### Bridge v2 TypeScript Structure

```
apps/bridge-v2/
├── src/
│   ├── index.ts              # Entry point
│   ├── adapters/
│   │   ├── whatsapp.ts       # WhatsApp client (whatsapp-web.js)
│   │   ├── core-grpc.ts      # gRPC client to Rust core
│   │   └── core-http.ts      # HTTP client to TypeScript core
│   ├── resilience/
│   │   ├── circuit-breaker.ts
│   │   ├── rate-limiter.ts
│   │   └── retry-buffer.ts
│   ├── deduplicator/
│   │   └── lru-dedup.ts
│   └── domain/
│       ├── message.ts
│       └── group.ts
├── package.json
└── tsconfig.json
```

### WhatsApp Integration Options

1. **whatsapp-web.js**: Popular Node.js library (Puppeteer-based)
2. **Baileys**: Lightweight WhatsApp Web API
3. **Keep whatsmeow**: Continue using Go bridge with whatsmeow

### Bridge v2 Resilience Patterns (Effect)

```typescript
// packages/bridge-v2/src/resilience/circuit-breaker.ts
import { Effect, Ref, Schedule } from "effect";

interface CircuitBreakerState {
  failures: number;
  lastFailure: number;
  state: "closed" | "open" | "half-open";
}

export class CircuitBreaker extends Context.Tag("CircuitBreaker")<
  CircuitBreaker,
  {
    readonly call: <A, E>(
      effect: Effect.Effect<A, E>
    ) => Effect.Effect<A, E | CircuitOpenError>;
    readonly getState: () => Effect.Effect<CircuitBreakerState>;
  }
>() {}

export const CircuitBreakerLive = (config: {
  failureThreshold: number;
  resetTimeout: number;
}) =>
  Layer.effect(
    CircuitBreaker,
    Effect.gen(function* () {
      const stateRef = yield* Ref.make<CircuitBreakerState>({
        failures: 0,
        lastFailure: 0,
        state: "closed",
      });

      return {
        call: (effect) =>
          Effect.gen(function* () {
            const state = yield* Ref.get(stateRef);

            if (state.state === "open") {
              const elapsed = Date.now() - state.lastFailure;
              if (elapsed < config.resetTimeout) {
                return yield* Effect.fail(new CircuitOpenError());
              }
              yield* Ref.update(stateRef, (s) => ({
                ...s,
                state: "half-open",
              }));
            }

            return yield* effect.pipe(
              Effect.tap(() =>
                Ref.update(stateRef, () => ({
                  failures: 0,
                  lastFailure: 0,
                  state: "closed" as const,
                }))
              ),
              Effect.tapError(() =>
                Ref.update(stateRef, (s) => {
                  const failures = s.failures + 1;
                  return {
                    failures,
                    lastFailure: Date.now(),
                    state:
                      failures >= config.failureThreshold ? "open" : s.state,
                  };
                })
              )
            );
          }),

        getState: () => Ref.get(stateRef),
      };
    })
  );
```

---

## 12. Documentation & Onboarding

### Developer Documentation

1. **Architecture Decision Records (ADRs)**
   - ADR-001: TypeScript + Effect-TS for core v2
   - ADR-002: Prisma for database layer
   - ADR-003: oRPC for API layer
   - ADR-004: Hybrid bridge strategy

2. **API Documentation**
   - OpenAPI spec auto-generated from oRPC
   - Type-safe client generation

3. **Runbooks**
   - Deployment procedures
   - Rollback procedures
   - Monitoring and alerting

### Onboarding Checklist

- [ ] Read migration plan document
- [ ] Set up local development environment
- [ ] Run both Rust and TypeScript cores locally
- [ ] Understand Effect-TS basics
- [ ] Review Prisma schema
- [ ] Complete first PR (small feature or bug fix)

### Training Resources

1. **Effect-TS**
   - Official docs: https://effect.website
   - Effect Days talks
   - Internal workshops

2. **Prisma**
   - Official docs: https://prisma.io/docs
   - Schema design patterns

3. **oRPC**
   - Official docs: https://orpc.dev
   - Type-safe API patterns

---

## 13. Risk Mitigation

| Risk                   | Mitigation                                 |
| ---------------------- | ------------------------------------------ |
| Performance regression | Benchmark against Rust, optimize hot paths |
| Data inconsistency     | Shared database, careful migration scripts |
| Feature parity gaps    | Comprehensive test suite, feature flags    |
| Team learning curve    | Training, pair programming, documentation  |
| Deployment issues      | Blue-green deployment, instant rollback    |
| AI parsing differences | A/B testing, gradual rollout               |

---

## 14. Monitoring & Observability

### Metrics to Track

- Request latency (p50, p95, p99)
- Match scoring time
- AI parsing time
- Database query time
- Error rates by endpoint
- Feature flag usage

### Dashboards

1. **Migration Progress**
   - Traffic split (v1 vs v2)
   - Error rate comparison
   - Latency comparison

2. **Business Metrics**
   - Matches per hour
   - Confirmation rate
   - Review queue depth

### Alerts

- Error rate > 1%
- Latency p99 > 500ms
- Database connection pool exhausted
- AI service unavailable
