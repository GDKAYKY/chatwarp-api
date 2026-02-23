FROM rust:bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock build.rs ./
COPY proto ./proto
COPY src ./src
COPY tests ./tests

RUN cargo build --release --bin chatwarp-api

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/chatwarp-api /usr/local/bin/chatwarp-api
COPY manager/dist /app/manager/dist

EXPOSE 8080
CMD ["chatwarp-api"]
