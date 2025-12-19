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

# Copy proto files
COPY proto ./proto

# Copy source
COPY . .

# Build binary with CGO enabled (required for SQLite)
RUN CGO_ENABLED=1 go build -o pharma-bridge .

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Create data directory
RUN mkdir -p /app/data

# Copy binary from builder
COPY --from=builder /app/pharma-bridge /app/pharma-bridge

# Create non-root user
RUN useradd -r -s /bin/false appuser && chown -R appuser:appuser /app/data
USER appuser

CMD ["/app/pharma-bridge"]
