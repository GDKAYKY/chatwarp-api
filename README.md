# chatwarp-api

API HTTP em Rust para o runtime Direct WA Client (sem sidecar gRPC).

## Estado atual (M0-M10)

Entregue:

- Bootstrap do crate (`src/lib.rs`) com `run()` funcional.
- Runtime Axum mínimo com:
  - `GET /`
  - `GET /docs/swagger`
  - `GET /docs/openapi.json`
  - `GET /healthz`
  - `GET /readyz`
  - `GET /metrics`
  - fallback `501` padronizado para rotas não implementadas.
- Base do protocolo WA em `src/wa/`:
  - `transport.rs` (WebSocket + framing de 3 bytes)
  - `noise.rs` (estado Noise + AES-GCM/HKDF)
  - `keys.rs` (keypair X25519)
  - `handshake.rs` + `handshake_proto.rs` (handshake sintético M2)
  - `binary_node.rs` (codec binário sintético M3)
  - `auth.rs` + `qr.rs` (estado de auth e geração de QR M4)
  - `signal/` (store/session sintéticos M5)
  - `message.rs` (modelagem e builder sintético M7)
- Event pipeline M8:
  - `src/events/dispatcher.rs`
  - `src/events/webhook.rs`
  - `src/events/websocket.rs`
  - `src/events/rabbitmq.rs`
- Manager de instâncias M6:
  - `src/instance/` (manager, handle, runner)
  - rotas `/instance/*` no runtime
- Rotas de domínio M9:
  - `src/handlers/chat.rs` (`/chat/findMessages/*`, `/chat/findChats/*`)
  - `src/handlers/group.rs` (`/group/create/*`, `/group/fetchAllGroups/*`)
  - `src/group_store.rs` (estado sintético em memória por instância)
- Hardening/Observability M10:
  - `src/observability.rs` (métricas de request)
  - middleware com `x-request-id` e logging por request
  - `DefaultBodyLimit` configurável
- Persistência M4:
  - `src/db/auth_repo.rs` (save/load de `AuthState` por instância em PostgreSQL)
  - `migrations/0001_create_auth_states.sql`
  - `migrations/0002_create_wa_runner_outbox.sql` (fila outbound para modo `wa_rs`)
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
  - `tests/message_routes_test.rs`
  - `tests/events_pipeline_test.rs`
  - `tests/chat_group_routes_test.rs`
  - `tests/observability_test.rs`

Ainda não entregue:

- integrações reais externas e rotas fora de escopo (`/call/*`, `/settings/*`, etc.).

## Requisitos

- Rust toolchain stable com `cargo`

## Execução local

```bash
export DATABASE_URL=postgres://user:pass@localhost:5432/chatwarp
cargo run
```

Modo runner `wa-rs` (opcional):

```bash
export WA_RUNNER_MODE=wa_rs
export WA_RS_BOT_COMMAND='wa-rs-bot --instance "$CHATWARP_INSTANCE_NAME"'
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
