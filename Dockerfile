# Build stage
FROM rustlang/rust:nightly-bookworm AS builder

# Install system dependencies for build
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    cmake \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the workspace configuration and toolchain
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./

# Copy workspace members
COPY http_clients/ ./http_clients/
COPY storages/ ./storages/
COPY transports/ ./transports/
COPY waproto/ ./waproto/
COPY warp_core/ ./warp_core/
COPY src/ ./src/

# Optimized build for specific binary with all features
RUN cargo build --release --bin chatwarp-api --all-features

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    libssl3 \
    libpq5 \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 appuser

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/chatwarp-api /usr/local/bin/chatwarp-api

# Create data directory for SQLite fallback
RUN mkdir -p /app/data && chown appuser:appuser /app/data

# Environment configuration
ENV PORT=8080
ENV RUST_LOG=info
# DATABASE_URL should be set in Render/Docker environment for Supabase
# Example: postgres://postgres.your-project:password@aws-0-us-east-1.pooler.supabase.com:5432/postgres
EXPOSE 8080

USER appuser

# Healthcheck
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:${PORT}/healthz || exit 1

# Start application
CMD ["chatwarp-api"]
