FROM rust:1.88-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock build.rs ./
COPY proto ./proto
COPY src ./src
COPY tests ./tests

RUN cargo build --release --bin whatsapp-rust

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 appuser

WORKDIR /app
COPY --from=builder /app/target/release/whatsapp-rust /usr/local/bin/whatsapp-rust

ENV SERVER_PORT=8080
EXPOSE 8080
USER appuser

CMD ["whatsapp-rust"]
