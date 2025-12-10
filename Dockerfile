# =============================================================================
# Multi-stage Dockerfile for PharmaBroker (PostgreSQL)
# =============================================================================
# Prerequisites: Run `task client` or `bun run build` in internal/api/static/client
# to build the frontend first, then: docker compose up --build

# -----------------------------------------------------------------------------
# Stage 1: Build Backend (Go)
# -----------------------------------------------------------------------------
FROM golang:1.25-alpine AS backend-builder

WORKDIR /app

# Install build dependencies (no SQLite needed for PostgreSQL)
RUN apk add --no-cache gcc musl-dev

# Copy go mod files
COPY go.mod go.sum ./

# Download dependencies
RUN go mod download

# Copy source code (including pre-built frontend dist)
COPY . .

# Build Go binary with optimizations (CGO disabled for pure Go PostgreSQL driver)
RUN CGO_ENABLED=0 GOOS=linux go build \
    -ldflags="-s -w" \
    -o /pharmabroker ./cmd/app

# -----------------------------------------------------------------------------
# Stage 2: Runtime Image (Minimal)
# -----------------------------------------------------------------------------
FROM alpine:latest

# Install runtime dependencies (no SQLite libs needed)
RUN apk add --no-cache \
    ca-certificates \
    tzdata

# Create non-root user
RUN addgroup -g 1000 pharmabroker && \
    adduser -u 1000 -G pharmabroker -h /app -D pharmabroker

WORKDIR /app

# Copy binary from builder
COPY --from=backend-builder /pharmabroker ./pharmabroker

# Copy config template
COPY config.yaml ./config.yaml

# Create data directory (for WhatsApp sessions and logs)
RUN mkdir -p /app/data && chown -R pharmabroker:pharmabroker /app

# Switch to non-root user
USER pharmabroker

# Expose ports
EXPOSE 8080 5050

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:5050/health || exit 1

# Environment defaults (PostgreSQL uses DATABASE_DSN)
ENV DATABASE_DSN="postgres://postgres:password@postgres:5432/pharmabroker?sslmode=disable" \
    WHATSAPP_SESSION_DB_DSN="postgres://postgres:password@postgres:5432/pharmabroker?sslmode=disable" \
    PB_WHATSAPP_SESSION_DIR=/app/data/whatsapp \
    PB_SERVER_PORT=8080 \
    PB_SERVER_HEALTH_PORT=5050

# Volume for persistent data (WhatsApp sessions)
VOLUME ["/app/data"]

# Default command
ENTRYPOINT ["./pharmabroker", "serve"]
