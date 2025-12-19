# Project Structure - PharmaBroker v2

> **Approach**: Monorepo with old Go code preserved in `legacy/` > **Stack**: Rust core + TypeScript gateway + Go bridge

---

## Directory Layout

```
pharma-broker/
├── 📁 legacy/                    # Old Go code (reference only)
│   ├── api/
│   ├── matching/
│   ├── parsing/
│   ├── messaging/
│   ├── storage/
│   ├── domain/
│   ├── bot/
│   ├── pkg/
│   └── go.mod
│
├── 📁 core/                      # Rust Core Engine
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs               # Entry point
│   │   ├── lib.rs                # Library exports
│   │   ├── config.rs             # Configuration
│   │   ├── error.rs              # Error types
│   │   │
│   │   ├── domain/               # Entities (from legacy/domain)
│   │   │   ├── mod.rs
│   │   │   ├── types.rs          # Enums
│   │   │   ├── offer.rs
│   │   │   ├── request.rs
│   │   │   ├── match_entity.rs
│   │   │   ├── message.rs
│   │   │   └── stats.rs
│   │   │
│   │   ├── repository/           # Data access (from legacy/storage)
│   │   │   ├── mod.rs
│   │   │   ├── traits.rs         # Repository traits
│   │   │   └── postgres/
│   │   │       ├── mod.rs
│   │   │       ├── offer.rs
│   │   │       ├── request.rs
│   │   │       ├── match_repo.rs
│   │   │       └── audit.rs
│   │   │
│   │   ├── matching/             # Matching engine (from legacy/matching)
│   │   │   ├── mod.rs
│   │   │   ├── scorer.rs         # Multi-field scorer
│   │   │   ├── weights.rs        # Weight management
│   │   │   ├── learner.rs        # Adaptive learning
│   │   │   ├── scheduler.rs      # Background matching
│   │   │   └── confidence.rs     # Confidence bands
│   │   │
│   │   ├── parsing/              # NLP (from legacy/parsing)
│   │   │   ├── mod.rs
│   │   │   ├── arabic.rs
│   │   │   ├── medication.rs
│   │   │   └── normalizer.rs
│   │   │
│   │   ├── api/                  # REST API (from legacy/api)
│   │   │   ├── mod.rs
│   │   │   ├── routes.rs
│   │   │   ├── handlers/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── offers.rs
│   │   │   │   ├── requests.rs
│   │   │   │   ├── matches.rs
│   │   │   │   ├── stats.rs
│   │   │   │   └── review.rs
│   │   │   ├── middleware/
│   │   │   │   └── auth.rs
│   │   │   └── sse.rs
│   │   │
│   │   ├── grpc/                 # gRPC server (for Go bridge)
│   │   │   ├── mod.rs
│   │   │   └── server.rs
│   │   │
│   │   └── jobs/                 # Background jobs (from legacy/pkg/cronjob)
│   │       ├── mod.rs
│   │       ├── janitor.rs
│   │       └── expiry.rs
│   │
│   ├── proto/
│   │   └── pharma.proto          # gRPC definitions
│   │
│   ├── migrations/               # SQL migrations
│   │   └── 001_initial.sql
│   │
│   └── tests/
│       ├── api/
│       ├── matching/
│       └── fixtures/
│
├── 📁 bridge/                    # Go WhatsApp Bridge (~500 LOC)
│   ├── go.mod
│   ├── main.go                   # Entry point
│   ├── handler.go                # Message handler
│   ├── grpc_client.go            # Rust connection
│   └── proto/                    # Generated from core/proto
│
├── 📁 gateway/                   # TypeScript AI Gateway
│   ├── package.json
│   ├── tsconfig.json
│   ├── src/
│   │   ├── index.ts              # Entry point
│   │   ├── routes/
│   │   │   ├── ai.ts             # AI SDK routes
│   │   │   └── proxy.ts          # Proxy to Rust
│   │   ├── lib/
│   │   │   └── ai-client.ts      # Vercel AI SDK wrapper
│   │   └── types/
│   │       └── api.ts            # Shared types
│   └── test/
│
├── 📁 dashboard/                 # React Dashboard (existing)
│   ├── package.json
│   ├── src/
│   │   ├── App.tsx
│   │   ├── components/
│   │   └── hooks/
│   └── public/
│
├── 📁 deploy/                    # Deployment configs
│   ├── docker/
│   │   ├── Dockerfile.core       # Rust
│   │   ├── Dockerfile.bridge     # Go
│   │   └── Dockerfile.gateway    # TypeScript
│   └── k8s/                      # Future Kubernetes
│
├── 📁 scripts/                   # Development scripts
│   ├── dev.sh                    # Start all services
│   ├── migrate.sh                # Run migrations
│   └── test-all.sh               # Run all tests
│
├── 📁 docs/                      # Documentation
│   ├── migration_plan.md
│   ├── multi_service_architecture.md
│   └── api/
│       └── openapi.yaml
│
├── 📁 analysis/                  # Python analysis (existing)
│   └── scripts/
│
├── docker-compose.yml            # Development stack
├── docker-compose.prod.yml       # Production stack
├── .env.example
└── README.md
```

---

## Docker Compose (Development)

```yaml
# docker-compose.yml
version: "3.9"

services:
  # Rust Core Engine
  core:
    build:
      context: ./core
      dockerfile: ../deploy/docker/Dockerfile.core
    ports:
      - "8080:8080" # REST API
      - "50051:50051" # gRPC
    environment:
      - DATABASE_URL=postgres://pharma:pharma@postgres:5432/pharmabroker
      - REDIS_URL=redis://redis:6379
      - RUST_LOG=info
    depends_on:
      - postgres
      - redis
    volumes:
      - ./core:/app:ro # Hot reload in dev

  # Go WhatsApp Bridge
  bridge:
    build:
      context: ./bridge
      dockerfile: ../deploy/docker/Dockerfile.bridge
    environment:
      - CORE_GRPC_ADDR=core:50051
      - WHATSAPP_STORE_PATH=/data/whatsapp
    volumes:
      - whatsapp-data:/data/whatsapp
    depends_on:
      - core

  # TypeScript AI Gateway
  gateway:
    build:
      context: ./gateway
      dockerfile: ../deploy/docker/Dockerfile.gateway
    ports:
      - "3000:3000"
    environment:
      - CORE_API_URL=http://core:8080
      - OPENAI_API_KEY=${OPENAI_API_KEY}
    depends_on:
      - core

  # React Dashboard (dev server)
  dashboard:
    build:
      context: ./dashboard
    ports:
      - "5173:5173"
    environment:
      - VITE_API_URL=http://localhost:3000
    volumes:
      - ./dashboard:/app
      - /app/node_modules

  # PostgreSQL (same schema)
  postgres:
    image: postgres:15-alpine
    ports:
      - "5432:5432"
    environment:
      - POSTGRES_USER=pharma
      - POSTGRES_PASSWORD=pharma
      - POSTGRES_DB=pharmabroker
    volumes:
      - postgres-data:/var/lib/postgresql/data
      - ./legacy/init-db.sh:/docker-entrypoint-initdb.d/init.sh

  # Redis (events + cache)
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"

volumes:
  postgres-data:
  whatsapp-data:
```

---

## Dockerfiles

### Rust Core (Multi-stage build)

```dockerfile
# deploy/docker/Dockerfile.core
FROM rust:1.75-slim as builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/pharma-core /usr/local/bin/
CMD ["pharma-core"]
```

### Go Bridge

```dockerfile
# deploy/docker/Dockerfile.bridge
FROM golang:1.22-alpine as builder
WORKDIR /app
COPY go.mod go.sum ./
RUN go mod download
COPY . .
RUN CGO_ENABLED=0 go build -o bridge .

FROM alpine:3.19
RUN apk add --no-cache ca-certificates
COPY --from=builder /app/bridge /usr/local/bin/
CMD ["bridge"]
```

### TypeScript Gateway

```dockerfile
# deploy/docker/Dockerfile.gateway
FROM oven/bun:1 as builder
WORKDIR /app
COPY package.json bun.lockb ./
RUN bun install --frozen-lockfile
COPY . .
RUN bun run build

FROM oven/bun:1-slim
WORKDIR /app
COPY --from=builder /app/dist ./dist
COPY --from=builder /app/node_modules ./node_modules
CMD ["bun", "run", "dist/index.js"]
```

---

## Migration Script

```bash
#!/bin/bash
# scripts/move-to-legacy.sh

# Move old Go code to legacy/
mkdir -p legacy
mv api legacy/
mv matching legacy/
mv parsing legacy/
mv messaging legacy/
mv storage legacy/
mv domain legacy/
mv bot legacy/
mv pkg legacy/
mv notify legacy/
mv go.mod legacy/
mv go.sum legacy/

# Keep these at root
# - docker-compose.yml
# - config.yaml
# - analysis/
# - docs/
# - README.md

echo "✅ Go code moved to legacy/"
echo "📁 Ready for new Rust/TypeScript structure"
```

---

## Development Workflow

```bash
# 1. Start infrastructure
docker-compose up -d postgres redis

# 2. Run Rust core (dev mode)
cd core && cargo watch -x run

# 3. Run Go bridge
cd bridge && go run .

# 4. Run TypeScript gateway
cd gateway && bun run dev

# 5. Run React dashboard
cd dashboard && bun run dev

# OR: Start everything
docker-compose up
```

---

## Port Mapping

| Service    | Port  | Purpose                       |
| ---------- | ----- | ----------------------------- |
| Rust Core  | 8080  | REST API (same as current Go) |
| Rust Core  | 50051 | gRPC (for Go bridge)          |
| Gateway    | 3000  | AI proxy + extended API       |
| Dashboard  | 5173  | React dev server              |
| PostgreSQL | 5432  | Database                      |
| Redis      | 6379  | Cache + Pub/Sub               |

---

## API Compatibility

All existing endpoints remain unchanged:

```
GET  /api/offers           → core:8080 (Rust)
GET  /api/requests         → core:8080 (Rust)
GET  /api/matches          → core:8080 (Rust)
POST /api/matches/:id/confirm → core:8080 (Rust)
POST /api/matches/:id/reject  → core:8080 (Rust)
GET  /api/stats            → core:8080 (Rust)
GET  /api/sse              → core:8080 (Rust)

# New AI endpoints via Gateway
POST /api/ai/parse         → gateway:3000 (TypeScript)
POST /api/ai/suggest       → gateway:3000 (TypeScript)
```
