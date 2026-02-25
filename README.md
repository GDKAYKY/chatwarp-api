# chatwarp-api

API HTTP em Rust para o runtime Direct WA Client (sem sidecar gRPC).

## Estado atual (M0-M2)

Entregue:
- Bootstrap do crate (`src/lib.rs`) com `run()` funcional.
- Runtime Axum mínimo com:
  - `GET /`
  - `GET /healthz`
  - `GET /readyz`
  - fallback `501` padronizado para rotas não implementadas.
- Base do protocolo WA em `src/wa/`:
  - `transport.rs` (WebSocket + framing de 3 bytes)
  - `noise.rs` (estado Noise + AES-GCM/HKDF)
  - `keys.rs` (keypair X25519)
  - `handshake.rs` + `handshake_proto.rs` (handshake sintético M2)
- Testes offline:
  - `tests/app_test.rs`
  - `tests/transport_test.rs`
  - `tests/noise_test.rs`
  - `tests/handshake_test.rs`

Ainda não entregue:
- M3+ (`binary_node`, auth persistida, signal e rotas de domínio).

## Requisitos

- Rust toolchain stable com `cargo`

## Execução local

```bash
cargo run
```

Servidor padrão: `http://localhost:8080`

## Qualidade

```bash
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Documentação

- `docs/PLANNING.md` - milestones e tasks
- `docs/ROUTES.md` - status de rotas
- `docs/ENV.md` - variáveis de ambiente
- `docs/PROJECT_ARCHITECTURE.md` - arquitetura alvo
