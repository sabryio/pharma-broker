# Go WhatsApp Bridge Dockerfile
# Multi-stage build for smaller final image

FROM golang:1.24-bookworm AS builder

WORKDIR /app

# Install CGO dependencies for SQLite
RUN apt-get update && apt-get install -y \
    gcc \
    libc6-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy go mod files
COPY go.mod go.sum ./

# Download dependencies
RUN go mod download

# Copy source (includes proto/ with generated pb.go files)
COPY . .

# Build binary with CGO enabled (required for SQLite)
RUN CGO_ENABLED=1 go build -o pharma-bridge ./cmd/bridge

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Create data directory
RUN mkdir -p /app/data

# Copy binary and config from builder
COPY --from=builder /app/pharma-bridge /app/pharma-bridge
COPY --from=builder /app/config.yml /app/config.yml

# Create non-root user
RUN useradd -r -s /bin/false appuser && chown -R appuser:appuser /app/data
USER appuser

EXPOSE 5050

CMD ["/app/pharma-bridge"]
