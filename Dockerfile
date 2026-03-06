# Build stage
FROM rustlang/rust:nightly-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config libssl-dev libpq-dev cmake protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./

COPY http_clients/ureq-client/Cargo.toml ./http_clients/ureq-client/Cargo.toml
COPY storages/sqlite-storage/Cargo.toml ./storages/sqlite-storage/Cargo.toml
COPY storages/postgres-storage/Cargo.toml ./storages/postgres-storage/Cargo.toml
COPY transports/tokio-transport/Cargo.toml ./transports/tokio-transport/Cargo.toml
COPY waproto/Cargo.toml waproto/build.rs ./waproto/
COPY warp_core/Cargo.toml ./warp_core/Cargo.toml
COPY warp_core/appstate/Cargo.toml ./warp_core/appstate/Cargo.toml
COPY warp_core/binary/Cargo.toml ./warp_core/binary/Cargo.toml
COPY warp_core/libsignal/Cargo.toml ./warp_core/libsignal/Cargo.toml
COPY waproto/src/whatsapp.rs ./waproto/src/whatsapp.rs

RUN mkdir -p src \
    http_clients/ureq-client/src \
    storages/sqlite-storage/src \
    storages/postgres-storage/src \
    transports/tokio-transport/src \
    waproto/src \
    warp_core/src \
    warp_core/benches \
    warp_core/appstate/src \
    warp_core/binary/src \
    warp_core/binary/benches \
    warp_core/libsignal/src \
    && echo "fn main() {}" > src/main.rs \
    && touch \
    http_clients/ureq-client/src/lib.rs \
    storages/sqlite-storage/src/lib.rs \
    storages/postgres-storage/src/lib.rs \
    transports/tokio-transport/src/lib.rs \
    waproto/src/lib.rs \
    warp_core/src/lib.rs \
    warp_core/benches/reporting_token_benchmark.rs \
    warp_core/appstate/src/lib.rs \
    warp_core/binary/src/lib.rs \
    warp_core/binary/benches/binary_benchmark.rs \
    warp_core/libsignal/src/lib.rs

# Compila todas as deps externas com stubs
RUN cargo build --release --bin chatwarp-api --all-features

# Ordem: das crates base para as que dependem delas
# waproto e warp_core são base — vão primeiro
COPY waproto/ ./waproto/
RUN find waproto/src -name "*.rs" | xargs touch \
    && cargo build --release --bin chatwarp-api --all-features

COPY warp_core/ ./warp_core/
RUN find warp_core -name "*.rs" -not -path "*/target/*" | xargs touch \
    && cargo build --release --bin chatwarp-api --all-features

# Agora as que dependem de warp_core
COPY http_clients/ ./http_clients/
COPY storages/ ./storages/
COPY transports/ ./transports/
RUN find http_clients storages transports -name "*.rs" | xargs touch \
    && cargo build --release --bin chatwarp-api --all-features

# src/ muda com mais frequência — sempre por último
COPY src/ ./src/
RUN find src -name "*.rs" | xargs touch \
    && cargo build --release --bin chatwarp-api --all-features

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
    ca-certificates curl libssl3 libpq5 sqlite3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 appuser

WORKDIR /app
COPY --from=builder /app/target/release/chatwarp-api /usr/local/bin/chatwarp-api
RUN mkdir -p /app/data && chown appuser:appuser /app/data

ENV PORT=8080
ENV RUST_LOG=info
EXPOSE 8080
USER appuser

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:${PORT}/healthz || exit 1

CMD ["chatwarp-api"]