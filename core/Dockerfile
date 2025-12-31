# Rust Core Engine Dockerfile
# Multi-stage build for smaller final image
# Build context should be project root with dockerfile: core/Dockerfile

FROM rust:1-slim-bookworm AS builder

WORKDIR /app

# Install dependencies for sqlx and protobuf
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests from core directory
COPY core/Cargo.toml core/Cargo.lock ./
COPY core/build.rs ./

# Copy proto files from project root
COPY proto ./proto

# Copy crates directory
COPY core/crates ./crates

# Create dummy src to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --bin pharma-core 2>/dev/null || true

# Copy actual source
COPY core/src ./src

# Build the real application
RUN cargo build --release --bin pharma-core
RUN cargo build --release -p pharma-analysis --bin pharma-analysis

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binaries from builder
COPY --from=builder /app/target/release/pharma-core /app/pharma-core
COPY --from=builder /app/target/release/pharma-analysis /app/pharma-analysis

# Create non-root user
RUN useradd -r -s /bin/false appuser
USER appuser

EXPOSE 8080 50051

CMD ["/app/pharma-core"]
