# Rust Core Engine Dockerfile
# Multi-stage build for smaller final image

FROM rust:1.87-slim-bookworm AS builder

WORKDIR /app

# Install dependencies for sqlx and protobuf
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./

# Copy proto files
COPY ../proto ../proto

# Create dummy src to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --bin pharma-core 2>/dev/null || true

# Copy actual source
COPY src ./src

# Build the real application
RUN cargo build --release --bin pharma-core

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/pharma-core /app/pharma-core

# Create non-root user
RUN useradd -r -s /bin/false appuser
USER appuser

EXPOSE 8080 50051

CMD ["/app/pharma-core"]
