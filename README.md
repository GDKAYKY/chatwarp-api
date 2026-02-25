# chatwarp-api

API HTTP em Rust para o runtime Direct WA Client (sem sidecar gRPC).

## Estado atual (M0-M6)

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
  - `binary_node.rs` (codec binário sintético M3)
  - `auth.rs` + `qr.rs` (estado de auth e geração de QR M4)
  - `signal/` (store/session sintéticos M5)
- Manager de instâncias M6:
  - `src/instance/` (manager, handle, runner)
  - rotas `/instance/*` no runtime
- Persistência M4:
  - `src/db/auth_repo.rs` (save/load de `AuthState` por instância em PostgreSQL)
  - `migrations/0001_create_auth_states.sql`
- Testes offline:
  - `tests/app_test.rs`
  - `tests/transport_test.rs`
  - `tests/noise_test.rs`
  - `tests/handshake_test.rs`
  - `tests/binary_node_test.rs`
  - `tests/auth_state_test.rs`
  - `tests/qr_test.rs`
  - `tests/auth_repo_test.rs` (`TEST_DATABASE_URL`)
  - `tests/signal_test.rs`
  - `tests/instance_manager_test.rs`
  - `tests/instance_routes_test.rs`

Ainda não entregue:
- M7+ (message API, event pipeline e rotas de domínio restantes).

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
